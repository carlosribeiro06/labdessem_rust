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

use labdessem_core::entities::{EngMaxBounds, HydroPlant};
use labdessem_core::system::System;
use labdessem_fpha::calculator::calculate_max_swallowing;
use labdessem_model::indexing::{ConstraintOffsetParams, Indexing};
use labdessem_solver::accessor::SolutionAccessor;
use labdessem_solver::types::LpSolution;
use tracing::info;

use crate::error::SimulationError;

/// Linear interpolation in volume-level table.
/// Clamps to table bounds (no extrapolation).
fn interpolate_forebay(table: &[(f64, f64)], volume_hm3: f64) -> f64 {
    if table.is_empty() {
        return 0.0;
    }
    if table.len() == 1 {
        return table[0].1; // Single point (run-of-river)
    }

    let v_min = table[0].0;
    let v_max = table[table.len() - 1].0;
    let v = volume_hm3.clamp(v_min, v_max);

    // Find bracketing points using binary search
    let upper = table.partition_point(|(vol, _)| *vol <= v);
    let hi = upper.min(table.len() - 1).max(1);
    let lo = hi - 1;

    let (v_lo, h_lo) = table[lo];
    let (v_hi, h_hi) = table[hi];

    let span = v_hi - v_lo;
    if span <= 0.0 {
        h_lo
    } else {
        let t = (v - v_lo) / span;
        h_lo + t * (h_hi - h_lo)
    }
}

/// Evaluate tailrace polynomial: a0 + a1*q + a2*q² + a3*q³ + a4*q⁴
fn evaluate_tailrace(coefficients: &[f64; 5], outflow_m3s: f64) -> f64 {
    let q = outflow_m3s;
    coefficients[0]
        + coefficients[1] * q
        + coefficients[2] * q * q
        + coefficients[3] * q * q * q
        + coefficients[4] * q * q * q * q
}

/// Calculate net head from volume and outflow using plant geometry.
///
/// Returns `fallback_head_m` if geometry data is not available.
fn calculate_net_head(
    plant: &HydroPlant,
    fallback_head_m: f64,
    volume_hm3: f64,
    outflow_m3s: f64,
) -> f64 {
    let (Some(vlt), Some(tc)) = (&plant.volume_level_table, &plant.tailrace_coefficients) else {
        return fallback_head_m;
    };

    let forebay_level = interpolate_forebay(vlt, volume_hm3);
    let tailrace_level = evaluate_tailrace(tc, outflow_m3s);
    let gross_head = forebay_level - tailrace_level;
    let hydraulic_loss = plant.hydraulic_loss_m.unwrap_or(0.0);

    (gross_head - hydraulic_loss).max(0.0)
}

/// Plant-level EngMax reference values (DESSEM eq. 9.34) over all registered units.
struct PlantReference {
    q_ef_hm3: f64,
    h_ef_m: f64,
    alpha: f64,
}

impl PlantReference {
    fn from_plant(plant: &HydroPlant) -> Self {
        let (q_ef_hm3, head_sum_m, unit_count) = plant
            .groups
            .iter()
            .flat_map(|group| &group.units)
            .fold((0.0, 0.0, 0.0), |(q, h, n), unit| {
                (
                    q + unit.max_turbining_hm3,
                    h + unit.effective_head_m,
                    n + 1.0,
                )
            });
        let h_ef_m = if unit_count > 0.0 {
            head_sum_m / unit_count
        } else {
            0.0
        };

        Self {
            q_ef_hm3,
            h_ef_m,
            alpha: plant.turbine_type.alpha(),
        }
    }
}

/// Plant outflow in m3/s with every unit turbining at its maximum.
fn plant_max_outflow_m3s(plant: &HydroPlant, period_duration_hours: f64) -> f64 {
    plant
        .groups
        .iter()
        .flat_map(|group| &group.units)
        .map(|unit| unit.max_turbining_hm3 * 1e6 / (period_duration_hours * 3600.0))
        .sum()
}

/// Raw plant EngMax in hm3 per period for a forebay volume and tailrace flow.
fn plant_bound_hm3(
    plant: &HydroPlant,
    reference: &PlantReference,
    head_volume_hm3: f64,
    tailrace_flow_m3s: f64,
) -> f64 {
    let net_head_m =
        calculate_net_head(plant, reference.h_ef_m, head_volume_hm3, tailrace_flow_m3s);
    calculate_max_swallowing(
        reference.q_ef_hm3,
        reference.h_ef_m,
        net_head_m,
        reference.alpha,
    )
}

/// Average volume (hm3) of `plant_idx` over `period` in the solved model:
/// the mean of the volume at the start (slot `period`) and at the end
/// (slot `period + 1`) of the period (ADR-006).
fn period_average_volume_hm3(
    accessor: &SolutionAccessor<'_>,
    plant_idx: usize,
    period: usize,
) -> f64 {
    0.5 * (accessor.hydro_volume(plant_idx, period) + accessor.hydro_volume(plant_idx, period + 1))
}

/// Names of the groups holding a unit with `effective_head_m <= 0.0`, in input order,
/// when the plant has such units and also units with `effective_head_m > 0.0`.
fn partial_effective_head_groups(plant: &HydroPlant) -> Vec<String> {
    let units = || plant.groups.iter().flat_map(|group| &group.units);
    if !(units().any(|unit| unit.effective_head_m > 0.0)
        && units().any(|unit| unit.effective_head_m <= 0.0))
    {
        return Vec::new();
    }

    plant
        .groups
        .iter()
        .filter(|group| group.units.iter().any(|unit| unit.effective_head_m <= 0.0))
        .map(|group| group.name.clone())
        .collect()
}

/// Recalculate plant EngMax bounds using the solution from the previous stage.
///
/// The bound for period `t` uses the previous solve's average volume over `t`,
/// `(V[t] + V[t + 1]) / 2` in the accessor slot layout (slot 0 = initial volume,
/// slot `t + 1` = end of period `t`). The first solve's bounds come from
/// [`initialize_engmax_bounds`] (initial volume).
///
/// When the plant has VHA and tailrace data, the net head is the VHA forebay
/// level at that volume, minus the tailrace polynomial level at the plant's
/// maximum outflow, minus the hydraulic loss in metres. Otherwise the net head
/// falls back to the plant `h_ef`. The bound is the raw EngMax, not capped by `Q_ef`.
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
        let reference = PlantReference::from_plant(plant);
        let tailrace_flow_m3s = plant_max_outflow_m3s(plant, system.horizon.period_duration_hours);

        for period in 0..system.horizon.periods {
            let volume_hm3 = period_average_volume_hm3(&accessor, plant_idx, period);
            let bound = plant_bound_hm3(plant, &reference, volume_hm3, tailrace_flow_m3s);
            engmax_bounds.set(plant_idx, period, bound);
            updated_count += 1;
        }
    }

    info!(
        updated_count,
        plants = system.hydro_plants.len(),
        periods = system.horizon.periods,
        "Recalculated EngMax bounds"
    );
}

/// Initialize EngMax bounds before the first solve.
///
/// Fills every period of every plant from the initial volume.
///
/// # Errors
///
/// Returns [`SimulationError::PartialEffectiveHead`] when a plant has `Hef.m <= 0`
/// on some of its groups but not on all of them. A plant with `Hef.m <= 0` on every
/// group is accepted and gets `EngMax = Q_ef` for every period.
pub fn initialize_engmax_bounds(system: &System) -> Result<EngMaxBounds, SimulationError> {
    for plant in &system.hydro_plants {
        let groups = partial_effective_head_groups(plant);
        if !groups.is_empty() {
            return Err(SimulationError::PartialEffectiveHead {
                plant: plant.name.clone(),
                groups,
            });
        }
    }

    let periods = system.horizon.periods;
    let mut bounds = EngMaxBounds::new(system.hydro_plants.len() * periods);

    for (plant_idx, plant) in system.hydro_plants.iter().enumerate() {
        let reference = PlantReference::from_plant(plant);
        let bound = plant_bound_hm3(
            plant,
            &reference,
            plant.reservoir.initial_volume_hm3,
            plant_max_outflow_m3s(plant, system.horizon.period_duration_hours),
        );
        for period in 0..periods {
            bounds.set(plant_idx, period, bound);
        }
    }

    info!(
        plants = system.hydro_plants.len(),
        periods,
        total_bounds = bounds.len(),
        "Initialized EngMax bounds from initial volumes"
    );

    Ok(bounds)
}

#[cfg(test)]
// tests: a panic is the failure report
#[allow(clippy::unwrap_used, clippy::expect_used)]
mod tests {
    use super::{
        PlantReference, calculate_net_head, initialize_engmax_bounds, plant_bound_hm3,
        recalculate_engmax_bounds,
    };
    use crate::error::SimulationError;
    use labdessem_core::entities::{
        EngMaxBounds, HydroGroup, HydroInitialCondition, HydroPlant, HydroUnit, Reservoir,
        TurbineType,
    };
    use labdessem_core::ids::{BusId, HydroGroupId, HydroPlantId, HydroUnitId, SubmarketId};
    use labdessem_core::system::{StudyHorizon, Submarket, System};
    use labdessem_model::indexing::Indexing;
    use labdessem_solver::types::LpSolution;

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
                volume_level_table: None,
                tailrace_coefficients: None,
            }],
            pumping_plants: vec![],
            renewable_plants: vec![],
            inflow_history: vec![],
        }
    }

    #[test]
    fn initialize_creates_bounds_for_all_periods() {
        let system = build_test_system();
        let bounds = initialize_engmax_bounds(&system).expect("initialize bounds");

        // 1 plant × 2 periods = 2 bounds
        assert_eq!(bounds.len(), 2);

        // Verify bounds exist for both periods
        assert!(bounds.get(0, 0).is_some());
        assert!(bounds.get(0, 1).is_some());
    }

    #[test]
    fn initialize_uses_initial_volume_for_head() {
        let system = build_test_system();
        let bounds = initialize_engmax_bounds(&system).expect("initialize bounds");

        // Single unit, no geometry: bound = Q_ef = 10.0
        let engmax = bounds.get(0, 0).unwrap();
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
                volume_level_table: None,
                tailrace_coefficients: None,
            }],
            pumping_plants: vec![],
            renewable_plants: vec![],
            inflow_history: vec![],
        }
    }

    fn set_effective_head(system: &mut System, group_idx: usize, head_m: f64) {
        for unit in &mut system.hydro_plants[0].groups[group_idx].units {
            unit.effective_head_m = head_m;
        }
    }

    /// One Francis plant with VHA and tailrace geometry: `Q_ef` = 0.72 hm3 (two units of
    /// 0.36 hm3 = 100 m3/s each over 1 h), plant max outflow 200 m3/s.
    fn geometry_plant_system(h_ef_m: f64, loss_m: Option<f64>) -> System {
        let mut system = build_test_system();
        let plant = &mut system.hydro_plants[0];
        plant.name = "UHE-GEO".into();
        plant.turbine_type = TurbineType::Francis;
        plant.reservoir = Reservoir {
            min_volume_hm3: 100.0,
            max_volume_hm3: 800.0,
            initial_volume_hm3: 250.0,
        };
        plant.volume_level_table = Some(vec![(100.0, 905.0), (400.0, 910.0), (800.0, 913.0)]);
        plant.tailrace_coefficients = Some([885.3, 0.0, 1.97e-5, -2.55e-8, 0.0]);
        plant.hydraulic_loss_m = loss_m;
        let unit = &mut plant.groups[0].units[0];
        unit.max_turbining_hm3 = 0.36;
        unit.effective_head_m = h_ef_m;
        let second = HydroUnit {
            id: HydroUnitId(2),
            name: "UG-2".into(),
            ..unit.clone()
        };
        plant.groups[0].units.push(second);
        system
    }

    fn assert_rel(actual: f64, expected: f64) {
        assert!(
            (actual - expected).abs() <= 1e-9 * expected.abs().max(1.0),
            "expected {expected}, got {actual}"
        );
    }

    /// Solution whose every volume slot is `volume_hm3`, every unit turbines
    /// `turbining_hm3` and every spillage is 0.0.
    fn uniform_solution(system: &System, volume_hm3: f64, turbining_hm3: f64) -> LpSolution {
        let horizon = system.horizon.periods;
        let indexing = Indexing::from_system(system);
        let offsets = indexing.compute_column_offsets(horizon);
        let mut primal = vec![0.0; offsets.total];
        for plant_idx in 0..indexing.hydro_plant_entries.len() {
            let start = offsets.hydro_volume + plant_idx * (horizon + 1);
            primal[start..=start + horizon].fill(volume_hm3);
        }
        let start = offsets.hydro_turbining;
        primal[start..start + indexing.hydro_units * horizon].fill(turbining_hm3);
        LpSolution {
            objective: 0.0,
            primal,
            dual: vec![],
            reduced_costs: vec![0.0; offsets.total],
            iterations: 0,
            solve_time_seconds: 0.0,
        }
    }

    /// D2(b) (ADR-006): the head volume of a period is the mean of its start and end volumes.
    fn d2_head_volume_hm3(start_hm3: f64, end_hm3: f64) -> f64 {
        f64::midpoint(start_hm3, end_hm3)
    }

    #[test]
    fn plant_reference_aggregates_all_registered_units() {
        let mut system = build_multi_group_system();
        set_effective_head(&mut system, 0, 40.0);
        let plant = &mut system.hydro_plants[0];
        plant.groups[1].units.truncate(1);
        plant.groups[1].units[0].max_turbining_hm3 = 20.0;
        plant.groups[1].units[0].effective_head_m = 70.0;

        let reference = PlantReference::from_plant(&system.hydro_plants[0]);

        // Sum of every unit: 10 + 10 + 20
        assert!((reference.q_ef_hm3 - 40.0).abs() < 1e-12);
        // Unit-count weighted: (40 + 40 + 70) / 3, not the group mean (40 + 70) / 2 = 55
        assert!((reference.h_ef_m - 50.0).abs() < 1e-12);
        assert!((reference.alpha - 0.5).abs() < 1e-12);
    }

    #[test]
    fn initialize_stores_one_bound_per_plant_and_period() {
        let system = build_multi_group_system();
        let bounds = initialize_engmax_bounds(&system).expect("initialize bounds");

        assert_eq!(
            bounds.len(),
            system.hydro_plants.len() * system.horizon.periods
        );
    }

    #[test]
    fn initialize_gives_plant_without_units_a_zero_bound() {
        let mut system = build_multi_group_system();
        system.hydro_plants[0].groups.clear();
        let bounds = initialize_engmax_bounds(&system).expect("initialize bounds");

        assert_eq!(bounds.len(), system.horizon.periods);
        for period in 0..system.horizon.periods {
            assert!(bounds.get(0, period).unwrap().abs() < 1e-12);
        }
    }

    #[test]
    fn initialize_rejects_partial_effective_head() {
        let mut system = build_multi_group_system();
        set_effective_head(&mut system, 1, 0.0);

        let error = initialize_engmax_bounds(&system).expect_err("partial Hef.m must be rejected");

        assert!(matches!(
            &error,
            SimulationError::PartialEffectiveHead { plant, groups }
                if plant == "UHE-MULTI" && *groups == ["CJ-2"]
        ));
        assert_eq!(
            error.to_string(),
            "EngMax: plant UHE-MULTI has Hef.m <= 0 on group(s) CJ-2 but not on every group; \
             set Hef.m for every group of the plant"
        );
    }

    #[test]
    fn initialize_keeps_plant_without_effective_head_inert() {
        let mut system = build_multi_group_system();
        set_effective_head(&mut system, 0, 0.0);
        set_effective_head(&mut system, 1, 0.0);

        let bounds = initialize_engmax_bounds(&system).expect("inert plant is accepted");

        // Q_ef = 4 units × 10.0
        for period in 0..system.horizon.periods {
            assert!((bounds.get(0, period).unwrap() - 40.0).abs() < 1e-12);
        }
    }

    #[test]
    fn calculate_net_head_follows_plant_geometry() {
        let system = geometry_plant_system(30.0, None);
        let plant = &system.hydro_plants[0];

        // Tailrace at 200 m3/s: 885.3 + 1.97e-5 * 200^2 - 2.55e-8 * 200^3
        //   = 885.3 + 0.788 - 0.204 = 885.884 m.
        // Forebay table: (100 hm3, 905 m), (400, 910), (800, 913); net head = forebay - 885.884.
        let cases = [
            // segment 1, t = (250 - 100) / 300 = 0.5: 905 + 0.5 * 5 = 907.5
            (250.0, 21.616),
            // segment 2, t = (600 - 400) / 400 = 0.5: 910 + 0.5 * 3 = 911.5
            (600.0, 25.616),
            // breakpoint: 910
            (400.0, 24.116),
            // clamped to 100 hm3: 905
            (50.0, 19.116),
            // clamped to 800 hm3: 913
            (900.0, 27.116),
        ];
        for (volume_hm3, expected_m) in cases {
            assert_rel(
                calculate_net_head(plant, 30.0, volume_hm3, 200.0),
                expected_m,
            );
        }

        // The loss is subtracted in metres: 21.616 - 0.095 = 21.521
        let lossy = geometry_plant_system(30.0, Some(0.095));
        assert_rel(
            calculate_net_head(&lossy.hydro_plants[0], 30.0, 250.0, 200.0),
            21.521,
        );
    }

    #[test]
    fn calculate_net_head_floors_non_positive_gross_head() {
        let mut system = geometry_plant_system(30.0, None);
        // Tailrace at 200 m3/s: 920.0 + 0.788 - 0.204 = 920.584 m above the 907.5 m forebay at
        // 250 hm3, so the gross head is 907.5 - 920.584 = -13.084 m.
        system.hydro_plants[0].tailrace_coefficients = Some([920.0, 0.0, 1.97e-5, -2.55e-8, 0.0]);
        let plant = &system.hydro_plants[0];
        let reference = PlantReference::from_plant(plant);

        assert_rel(calculate_net_head(plant, 30.0, 250.0, 200.0), 0.0);
        assert_rel(plant_bound_hm3(plant, &reference, 250.0, 200.0), 0.0);
    }

    #[test]
    fn calculate_net_head_falls_back_without_full_geometry() {
        let full = geometry_plant_system(30.0, None).hydro_plants.remove(0);
        let plants = [
            HydroPlant {
                volume_level_table: None,
                ..full.clone()
            },
            HydroPlant {
                tailrace_coefficients: None,
                ..full.clone()
            },
            HydroPlant {
                volume_level_table: None,
                tailrace_coefficients: None,
                ..full
            },
        ];

        for plant in &plants {
            assert_rel(calculate_net_head(plant, 37.5, 250.0, 200.0), 37.5);

            // Net head falls back to h_ef = 30 m: 0.72 * (30 / 30)^0.5 = Q_ef
            let reference = PlantReference::from_plant(plant);
            assert_rel(plant_bound_hm3(plant, &reference, 250.0, 200.0), 0.72);
        }
    }

    #[test]
    fn plant_bound_is_below_q_ef_when_head_is_below_h_ef() {
        let system = geometry_plant_system(30.0, None);
        let plant = &system.hydro_plants[0];
        let reference = PlantReference::from_plant(plant);

        // Net head at V = 250 hm3, Q = 200 m3/s is 21.616 m < h_ef = 30 m:
        // 0.72 * (21.616 / 30)^0.5 = 0.72 * 0.848842... = 0.611166491228
        let bound = plant_bound_hm3(plant, &reference, 250.0, 200.0);

        assert_rel(bound, 0.611_166_491_228);
        assert!(bound < 0.72);
    }

    #[test]
    fn stored_bound_exceeds_q_ef_above_h_ef() {
        let system = geometry_plant_system(20.0, None);
        let periods = system.horizon.periods;
        // Net head 21.616 m > h_ef = 20 m and the bound is not capped by Q_ef = 0.72 hm3:
        // 0.72 * (21.616 / 20)^0.5 = 0.72 * 1.039615... = 0.748523025698
        let expected = 0.748_523_025_698;

        let initialized = initialize_engmax_bounds(&system).expect("initialize bounds");

        let indexing = Indexing::from_system(&system);
        let solution = uniform_solution(&system, 250.0, 0.36);
        let mut recalculated = EngMaxBounds::new(periods);
        recalculate_engmax_bounds(&system, &solution, &indexing, &mut recalculated);

        for bounds in [&initialized, &recalculated] {
            for period in 0..periods {
                let bound = bounds
                    .get(0, period)
                    .expect("bound stored for every period");
                assert!(bound > 0.72);
                assert_rel(bound, expected);
            }
        }
    }

    #[test]
    fn recalculate_uses_period_average_volume() {
        let system = geometry_plant_system(30.0, None);
        let periods = system.horizon.periods;
        let indexing = Indexing::from_system(&system);
        let mut solution = uniform_solution(&system, 0.0, 0.36);
        // Plant 0 volume slots: slot 0 = initial volume, slot t + 1 = end of period t.
        let slots = [250.0, 600.0, 400.0];
        let volume_start = indexing.compute_column_offsets(periods).hydro_volume;
        solution.primal[volume_start..volume_start + slots.len()].copy_from_slice(&slots);

        let mut bounds = EngMaxBounds::new(periods);
        recalculate_engmax_bounds(&system, &solution, &indexing, &mut bounds);

        // D2 head volumes: period 0 = (250 + 600) / 2 = 425, period 1 = (600 + 400) / 2 = 500.
        // Tailrace at 200 m3/s is 885.884 m, Q_ef = 0.72 hm3, h_ef = 30 m.
        // Period 0: forebay = 910 + (425 - 400) / 400 * 3 = 910.1875, net head = 24.3035,
        //   bound = 0.72 * (24.3035 / 30)^0.5 = 0.648046664986.
        // Period 1: forebay = 910 + (500 - 400) / 400 * 3 = 910.75, net head = 24.866,
        //   bound = 0.72 * (24.866 / 30)^0.5 = 0.655503226537.
        // Rejected rules (each gives a different pair):
        //   today's rule (250, 250):   0.611166 for both periods;
        //   start of period (250, 600): 0.611166 and 0.665315;
        //   end of period (600, 400):   0.665315 and 0.645542.
        let plant = &system.hydro_plants[0];
        let reference = PlantReference::from_plant(plant);
        let head_volumes = [
            d2_head_volume_hm3(slots[0], slots[1]),
            d2_head_volume_hm3(slots[1], slots[2]),
        ];
        assert_rel(head_volumes[0], 425.0);
        assert_rel(head_volumes[1], 500.0);

        let expected = [0.648_046_664_986, 0.655_503_226_537];
        for period in 0..periods {
            let bound = bounds
                .get(0, period)
                .expect("bound stored for every period");
            assert_rel(bound, expected[period]);
            assert_rel(
                bound,
                plant_bound_hm3(plant, &reference, head_volumes[period], 200.0),
            );
        }
    }
}
