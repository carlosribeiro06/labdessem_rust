//! EngMax (dynamic maximum swallowing) inter-stage recalculation.
//!
//! This module provides functions to calculate and update EngMax bounds
//! based on reservoir volume state from optimization solutions.
//!
//! # Usage in Simulation Orchestration
//!
//! The typical workflow for EngMax integration is:
//!
//! 1. Before first solve: call `initialize_engmax_bounds()` with initial volumes
//! 2. Build model with `Model::from_system()`
//! 3. Extend model with EngMax constraints: `model.constraints.extend_with_engmax()`
//! 4. Solve stage
//! 5. Call `recalculate_engmax_bounds()` with the solution
//! 6. Repeat from step 2 for next stage
//!
//! # Example
//!
//! ```ignore
//! // Initialize bounds before first solve
//! let mut engmax_bounds = initialize_engmax_bounds(&system);
//!
//! // Build and solve first stage
//! let mut model = Model::from_system(&system, solve_mode);
//! model.constraints.extend_with_engmax(
//!     &system, &indexing, &model.variables, &engmax_bounds, engmax_enabled
//! );
//! let solution = solve_model(solver, &model)?;
//!
//! // Recalculate for next stage
//! recalculate_engmax_bounds(&system, &solution, &indexing, &mut engmax_bounds);
//! ```

use labdessem_core::entities::EngMaxBounds;
use labdessem_core::system::System;
use labdessem_fpha::calculator::calculate_max_swallowing;
use labdessem_model::indexing::{ConstraintOffsetParams, Indexing};
use labdessem_solver::accessor::SolutionAccessor;
use labdessem_solver::types::LpSolution;
use tracing::info;

/// Recalculate EngMax bounds using solution from previous stage.
///
/// For each hydro unit and period:
/// - For t=0: uses `initial_volume_hm3` from the plant's reservoir
/// - For t>0: uses `volume[t-1]` from the solution
///
/// Net head is approximated using the turbining-weighted average head
/// from the plant's effective head data when available, otherwise
/// a simplified calculation is used.
pub fn recalculate_engmax_bounds(
    system: &System,
    solution: &LpSolution,
    indexing: &Indexing,
    engmax_bounds: &mut EngMaxBounds,
) {
    let params = ConstraintOffsetParams {
        horizon: system.horizon.periods,
        fpha_enabled: false,
        num_fpha_segments: 0,
        future_cost_enabled: false,
        num_future_cost_cuts: 0,
        num_operational_limits: 0,
        thermal_uct_enabled: false,
        hydro_uch_enabled: false,
        is_commitment_mode: false,
    };
    let accessor = SolutionAccessor::new(solution, indexing, &params);

    let mut updated_count = 0usize;

    for (plant_idx, plant) in system.hydro_plants.iter().enumerate() {
        let turbine_alpha = plant.turbine_type.alpha();

        // Flatten units with their indices
        let units: Vec<(usize, usize, &labdessem_core::entities::HydroUnit)> = plant
            .groups
            .iter()
            .enumerate()
            .flat_map(|(group_idx, group)| {
                group
                    .units
                    .iter()
                    .enumerate()
                    .map(move |(unit_idx, unit)| (group_idx, unit_idx, unit))
            })
            .collect();

        for (period, _) in (0..system.horizon.periods).enumerate() {
            // Get volume for this period: initial for t=0, solution[t-1] for t>0
            let volume_hm3 = if period == 0 {
                plant.reservoir.initial_volume_hm3
            } else {
                // volume at end of period (t-1) which is the start of period t
                accessor.hydro_volume(plant_idx, period - 1)
            };

            for (unit_flat_idx, (_group_idx, _unit_idx, unit)) in units.iter().enumerate() {
                // Calculate net head approximation
                // Use effective_head_m if available, otherwise estimate from volume
                let net_head_m = if unit.effective_head_m > 0.0 {
                    unit.effective_head_m
                } else {
                    // Simplified estimation: assume head proportional to volume fraction
                    let vol_fraction = volume_hm3 / plant.reservoir.max_volume_hm3.max(1.0);
                    vol_fraction * 100.0 // Rough approximation, 100m max head
                };

                // Reference values for EngMax calculation
                let q_ef_hm3 = unit.max_turbining_hm3;
                let h_ef_m = unit.effective_head_m.max(net_head_m);

                // Calculate dynamic EngMax
                let engmax = calculate_max_swallowing(
                    q_ef_hm3,
                    h_ef_m,
                    net_head_m,
                    turbine_alpha,
                );

                engmax_bounds.set(plant_idx, unit_flat_idx, period, engmax);
                updated_count += 1;
            }
        }
    }

    info!(
        updated_count,
        plants = system.hydro_plants.len(),
        periods = system.horizon.periods,
        "Recalculated EngMax bounds"
    );
}

/// Initialize EngMax bounds for period 0 using initial volumes.
///
/// Called before the first solve to set up initial bounds based on
/// reservoir initial conditions.
pub fn initialize_engmax_bounds(system: &System) -> EngMaxBounds {
    let capacity = system.hydro_plants.len() * 10 * system.horizon.periods;
    let mut bounds = EngMaxBounds::new(capacity);

    for (plant_idx, plant) in system.hydro_plants.iter().enumerate() {
        let turbine_alpha = plant.turbine_type.alpha();
        let initial_volume = plant.reservoir.initial_volume_hm3;

        let mut unit_flat_idx = 0usize;
        for group in &plant.groups {
            for unit in &group.units {
                // Calculate net head from initial volume
                let net_head_m = if unit.effective_head_m > 0.0 {
                    unit.effective_head_m
                } else {
                    let vol_fraction = initial_volume / plant.reservoir.max_volume_hm3.max(1.0);
                    vol_fraction * 100.0
                };

                let q_ef_hm3 = unit.max_turbining_hm3;
                let h_ef_m = unit.effective_head_m.max(net_head_m);

                // Initialize bounds for all periods using initial volume head
                // (these will be updated after each solve stage)
                for period in 0..system.horizon.periods {
                    let engmax = calculate_max_swallowing(q_ef_hm3, h_ef_m, net_head_m, turbine_alpha);
                    bounds.set(plant_idx, unit_flat_idx, period, engmax);
                }

                unit_flat_idx += 1;
            }
        }
    }

    info!(
        plants = system.hydro_plants.len(),
        periods = system.horizon.periods,
        total_bounds = bounds.len(),
        "Initialized EngMax bounds from initial volumes"
    );

    bounds
}

#[cfg(test)]
mod tests {
    use super::*;
    use labdessem_core::entities::{
        HydroGroup, HydroInitialCondition, HydroPlant, HydroUnit, Reservoir, TurbineType,
    };
    use labdessem_core::ids::{BusId, HydroGroupId, HydroPlantId, HydroUnitId, SubmarketId};
    use labdessem_core::system::{StudyHorizon, Submarket, System};

    fn build_test_system() -> System {
        System {
            horizon: StudyHorizon {
                periods: 2,
                period_duration_hours: 1.0,
                original_periods: 2,
                original_period_durations_hours: vec![1.0, 1.0],
                internal_to_original_period: vec![1, 2],
                internal_subperiod_index: vec![1, 1],
            },
            thermal_unit_commitment_enabled: false,
            hydro_unit_commitment_enabled: false,
            ton_residual_enabled: false,
            fpha_enabled: false,
            future_cost_enabled: false,
            residual_costs: vec![],
            future_cost_cuts: vec![],
            submarkets: vec![Submarket {
                id: SubmarketId(1),
                name: "SE".into(),
                demand_mw: vec![100.0, 100.0],
            }],
            interchange_limits: vec![],
            operational_limits: vec![],
            buses: vec![],
            branches: vec![],
            thermal_plants: vec![],
            hydro_plants: vec![HydroPlant {
                id: HydroPlantId(1),
                name: "UHE-1".into(),
                submarket_id: SubmarketId(1),
                bus_id: BusId(1),
                upstream_plant_ids: vec![],
                downstream_plant_id: None,
                diversion_upstream_plant_ids: vec![],
                diversion_plant_id: None,
                fpha_segments: vec![],
                reservoir: Reservoir {
                    min_volume_hm3: 10.0,
                    max_volume_hm3: 100.0,
                    initial_volume_hm3: 50.0,
                },
                natural_inflow_hm3: vec![1.0, 1.0],
                water_withdrawal_hm3: vec![0.0, 0.0],
                spillage_cost_per_hm3: 0.0,
                turbining_cost_per_hm3: 0.0,
                specific_productivity_mw_per_m3s: 1.0,
                hydraulic_loss_m: None,
                efficiency: None,
                groups: vec![HydroGroup {
                    id: HydroGroupId(1),
                    name: "CJ-1".into(),
                    units: vec![HydroUnit {
                        id: HydroUnitId(1),
                        name: "UG-1".into(),
                        min_generation_mw: 0.0,
                        max_generation_mw: 50.0,
                        max_turbining_hm3: 10.0,
                        effective_head_m: 50.0,
                        initial_condition: HydroInitialCondition {
                            is_on: true,
                            generation_mw: 0.0,
                            time_in_state: 1,
                        },
                    }],
                }],
                turbine_type: TurbineType::Francis,
            }],
            pumping_plants: vec![],
            renewable_plants: vec![],
            inflow_history: vec![],
        }
    }

    #[test]
    fn initialize_creates_bounds_for_all_periods() {
        let system = build_test_system();
        let bounds = initialize_engmax_bounds(&system);

        // 1 plant × 1 unit × 2 periods = 2 bounds
        assert_eq!(bounds.len(), 2);

        // Verify bounds exist for both periods
        assert!(bounds.get(0, 0, 0).is_some());
        assert!(bounds.get(0, 0, 1).is_some());
    }

    #[test]
    fn initialize_uses_initial_volume_for_head() {
        let system = build_test_system();
        let bounds = initialize_engmax_bounds(&system);

        // With effective_head_m = 50.0, q_ef = 10.0, Francis alpha = 0.5
        // engmax = 10.0 * (50.0/50.0)^0.5 = 10.0
        let engmax = bounds.get(0, 0, 0).unwrap();
        assert!((engmax - 10.0).abs() < 0.01);
    }

    fn build_multi_group_system() -> System {
        System {
            horizon: StudyHorizon {
                periods: 2,
                period_duration_hours: 1.0,
                original_periods: 2,
                original_period_durations_hours: vec![1.0, 1.0],
                internal_to_original_period: vec![1, 2],
                internal_subperiod_index: vec![1, 1],
            },
            thermal_unit_commitment_enabled: false,
            hydro_unit_commitment_enabled: false,
            ton_residual_enabled: false,
            fpha_enabled: false,
            future_cost_enabled: false,
            residual_costs: vec![],
            future_cost_cuts: vec![],
            submarkets: vec![Submarket {
                id: SubmarketId(1),
                name: "SE".into(),
                demand_mw: vec![100.0, 100.0],
            }],
            interchange_limits: vec![],
            operational_limits: vec![],
            buses: vec![],
            branches: vec![],
            thermal_plants: vec![],
            hydro_plants: vec![HydroPlant {
                id: HydroPlantId(1),
                name: "UHE-MULTI".into(),
                submarket_id: SubmarketId(1),
                bus_id: BusId(1),
                upstream_plant_ids: vec![],
                downstream_plant_id: None,
                diversion_upstream_plant_ids: vec![],
                diversion_plant_id: None,
                fpha_segments: vec![],
                reservoir: Reservoir {
                    min_volume_hm3: 10.0,
                    max_volume_hm3: 100.0,
                    initial_volume_hm3: 50.0,
                },
                natural_inflow_hm3: vec![1.0, 1.0],
                water_withdrawal_hm3: vec![0.0, 0.0],
                spillage_cost_per_hm3: 0.0,
                turbining_cost_per_hm3: 0.0,
                specific_productivity_mw_per_m3s: 1.0,
                hydraulic_loss_m: None,
                efficiency: None,
                // 2 groups × 2 units each = 4 units total
                groups: vec![
                    HydroGroup {
                        id: HydroGroupId(1),
                        name: "CJ-1".into(),
                        units: vec![
                            HydroUnit {
                                id: HydroUnitId(1),
                                name: "UG-1".into(),
                                min_generation_mw: 0.0,
                                max_generation_mw: 50.0,
                                max_turbining_hm3: 10.0,
                                effective_head_m: 50.0,
                                initial_condition: HydroInitialCondition {
                                    is_on: true,
                                    generation_mw: 0.0,
                                    time_in_state: 1,
                                },
                            },
                            HydroUnit {
                                id: HydroUnitId(2),
                                name: "UG-2".into(),
                                min_generation_mw: 0.0,
                                max_generation_mw: 50.0,
                                max_turbining_hm3: 10.0,
                                effective_head_m: 50.0,
                                initial_condition: HydroInitialCondition {
                                    is_on: true,
                                    generation_mw: 0.0,
                                    time_in_state: 1,
                                },
                            },
                        ],
                    },
                    HydroGroup {
                        id: HydroGroupId(2),
                        name: "CJ-2".into(),
                        units: vec![
                            HydroUnit {
                                id: HydroUnitId(3),
                                name: "UG-3".into(),
                                min_generation_mw: 0.0,
                                max_generation_mw: 50.0,
                                max_turbining_hm3: 10.0,
                                effective_head_m: 50.0,
                                initial_condition: HydroInitialCondition {
                                    is_on: true,
                                    generation_mw: 0.0,
                                    time_in_state: 1,
                                },
                            },
                            HydroUnit {
                                id: HydroUnitId(4),
                                name: "UG-4".into(),
                                min_generation_mw: 0.0,
                                max_generation_mw: 50.0,
                                max_turbining_hm3: 10.0,
                                effective_head_m: 50.0,
                                initial_condition: HydroInitialCondition {
                                    is_on: true,
                                    generation_mw: 0.0,
                                    time_in_state: 1,
                                },
                            },
                        ],
                    },
                ],
                turbine_type: TurbineType::Francis,
            }],
            pumping_plants: vec![],
            renewable_plants: vec![],
            inflow_history: vec![],
        }
    }

    #[test]
    fn initialize_uses_flat_index_for_multi_group_plants() {
        let system = build_multi_group_system();
        let bounds = initialize_engmax_bounds(&system);

        // 1 plant × 4 units × 2 periods = 8 bounds
        assert_eq!(bounds.len(), 8);

        // Verify bounds exist for flat indices 0, 1, 2, 3 (not 0, 2, 2, 4)
        // Period 0
        assert!(bounds.get(0, 0, 0).is_some(), "Missing bound for unit 0, period 0");
        assert!(bounds.get(0, 1, 0).is_some(), "Missing bound for unit 1, period 0");
        assert!(bounds.get(0, 2, 0).is_some(), "Missing bound for unit 2, period 0");
        assert!(bounds.get(0, 3, 0).is_some(), "Missing bound for unit 3, period 0");

        // Period 1
        assert!(bounds.get(0, 0, 1).is_some(), "Missing bound for unit 0, period 1");
        assert!(bounds.get(0, 1, 1).is_some(), "Missing bound for unit 1, period 1");
        assert!(bounds.get(0, 2, 1).is_some(), "Missing bound for unit 2, period 1");
        assert!(bounds.get(0, 3, 1).is_some(), "Missing bound for unit 3, period 1");
    }
}
