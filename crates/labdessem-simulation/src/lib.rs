mod error;
mod network;
mod output;
mod results;
mod stages;
mod workflow;

pub use error::SimulationError;
pub use results::{
    AddedFlowCutLineResult, FlowViolation, IterationReport, IterativeSimulationResult,
    LineFlowResult, NetworkInfeasibilityRecord, OperationalLimitInfeasibilityRecord,
};
pub use workflow::{ExecutionOption, IterationStage};

use labdessem_core::system::System;

/// Run iterative simulation with optional network constraints.
pub fn run_iterative_simulation(
    system: &System,
    network_enabled: bool,
) -> Result<IterativeSimulationResult, SimulationError> {
    stages::run_simulation(system, network_enabled, ExecutionOption::Iterative)
}

/// Run simulation with specified execution option.
pub fn run_simulation(
    system: &System,
    network_enabled: bool,
    execution_option: ExecutionOption,
) -> Result<IterativeSimulationResult, SimulationError> {
    stages::run_simulation(system, network_enabled, execution_option)
}

pub fn write_results_csvs(
    system: &labdessem_core::system::System,
    result: &IterativeSimulationResult,
    output_dir: impl AsRef<std::path::Path>,
    network_enabled: bool,
) -> Result<(), SimulationError> {
    output::write_results_csvs(system, result, output_dir, network_enabled)
}

#[cfg(test)]
mod tests {
    use super::run_iterative_simulation;
    use labdessem_core::{
        entities::{
            HydroFphaSegment, HydroGroup, HydroInitialCondition, HydroPlant, HydroUnit, Reservoir,
            ThermalInitialCondition, ThermalPlant, ThermalUnit,
        },
        ids::{
            BranchId, BusId, HydroGroupId, HydroPlantId, HydroUnitId, SubmarketId, ThermalPlantId,
            ThermalUnitId,
        },
        system::{Branch, Bus, StudyHorizon, Submarket, System},
    };

    fn fpha_segments() -> Vec<HydroFphaSegment> {
        vec![HydroFphaSegment {
            segment: 1,
            correction_factor: 1.0,
            rhs: 0.0,
            volume_coefficient: 0.0,
            turbining_coefficient: 1.0,
            lateral_flow_coefficient: 0.0,
        }]
    }

    fn build_system() -> System {
        System {
            horizon: StudyHorizon {
                periods: 2,
                period_duration_hours: 1.0,
                original_periods: 2,
                original_period_durations_hours: vec![1.0, 1.0],
                internal_to_original_period: vec![1, 2],
                internal_subperiod_index: vec![1, 1],
            },
            thermal_unit_commitment_enabled: true,
            hydro_unit_commitment_enabled: true,
            ton_residual_enabled: false,
            fpha_enabled: true,
            future_cost_enabled: false,
            residual_costs: vec![],
            future_cost_cuts: vec![],
            submarkets: vec![
                Submarket {
                    id: SubmarketId(1),
                    name: "SE".into(),
                    demand_mw: vec![100.0, 100.0],
                    deficit_cost_per_mwh: 1_000.0,
                },
                Submarket {
                    id: SubmarketId(2),
                    name: "S".into(),
                    demand_mw: vec![50.0, 50.0],
                    deficit_cost_per_mwh: 1_000.0,
                },
            ],
            interchange_limits: vec![],
            operational_limits: vec![],
            buses: vec![
                Bus {
                    id: BusId(1),
                    name: "BUS-1".into(),
                    submarket_id: SubmarketId(1),
                    angle_reference: true,
                    demand_mw: vec![0.0, 0.0],
                },
                Bus {
                    id: BusId(2),
                    name: "BUS-2".into(),
                    submarket_id: SubmarketId(2),
                    angle_reference: false,
                    demand_mw: vec![150.0, 150.0],
                },
            ],
            branches: vec![Branch {
                id: BranchId(1),
                name: "L1".into(),
                from_bus_id: BusId(1),
                to_bus_id: BusId(2),
                reactance_pu: 0.1,
                thermal_limit_mw: 200.0,
            }],
            thermal_plants: vec![ThermalPlant {
                id: ThermalPlantId(1),
                name: "UTE1".into(),
                submarket_id: SubmarketId(2),
                bus_id: BusId(2),
                units: vec![ThermalUnit {
                    id: ThermalUnitId(1),
                    name: "UTE1-1".into(),
                    min_generation_mw: 0.0,
                    max_generation_mw: 200.0,
                    startup_trajectory_mw: vec![50.0],
                    shutdown_trajectory_mw: vec![50.0],
                    min_up_time: 1,
                    min_down_time: 1,
                    startup_cost: 1.0,
                    shutdown_cost: 1.0,
                    variable_cost_per_mwh: 100.0,
                    initial_condition: ThermalInitialCondition {
                        is_on: false,
                        generation_mw: 0.0,
                        time_in_state: 1,
                        time_in_ramp: 0,
                        is_ramping_up: false,
                        is_ramping_down: false,
                    },
                }],
            }],
            hydro_plants: vec![HydroPlant {
                id: HydroPlantId(1),
                name: "UHE1".into(),
                submarket_id: SubmarketId(1),
                bus_id: BusId(1),
                upstream_plant_ids: vec![],
                downstream_plant_id: None,
                diversion_upstream_plant_ids: vec![],
                diversion_plant_id: None,
                fpha_segments: fpha_segments(),
                reservoir: Reservoir {
                    min_volume_hm3: 0.0,
                    max_volume_hm3: 500.0,
                    initial_volume_hm3: 200.0,
                },
                natural_inflow_hm3: vec![100.0, 100.0],
                water_withdrawal_hm3: vec![0.0, 0.0],
                spillage_cost_per_hm3: 0.1,
                turbining_cost_per_hm3: 0.0,
                specific_productivity_mw_per_m3s: 1.0,
                groups: vec![HydroGroup {
                    id: HydroGroupId(1),
                    name: "CJ1".into(),
                    units: vec![HydroUnit {
                        id: HydroUnitId(1),
                        name: "UG1".into(),
                        min_generation_mw: 0.0,
                        max_generation_mw: 200.0,
                        max_turbining_hm3: 500.0,
                        initial_condition: HydroInitialCondition {
                            is_on: true,
                            generation_mw: 0.0,
                            time_in_state: 1,
                        },
                    }],
                }],
            }],
            pumping_plants: Vec::new(),
            renewable_plants: vec![],
        }
    }

    #[test]
    fn runs_iterative_process_and_reports_steps() {
        let result = run_iterative_simulation(&build_system(), true)
            .expect("iterative simulation should solve the simple case");

        assert!(!result.steps.is_empty());
        assert_eq!(
            result
                .steps
                .iter()
                .filter(|step| step.stage == super::IterationStage::MixedIntegerLinearProgramming)
                .count(),
            1
        );
    }
}
