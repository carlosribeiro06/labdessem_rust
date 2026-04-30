pub mod constraints;
pub mod indexing;
pub mod objective;
pub mod variables;

use labdessem_core::system::System;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SolveMode {
    LinearProgramming,
    MixedIntegerLinearProgramming,
    LinearProgrammingWithFixedCommitment,
}

#[derive(Debug)]
pub struct Model {
    pub solve_mode: SolveMode,
    pub indexing: indexing::Indexing,
    pub variables: variables::Variables,
    pub constraints: constraints::ConstraintSet,
    pub objective: objective::Objective,
}

impl Model {
    pub fn from_system(system: &System, solve_mode: SolveMode) -> Self {
        let indexing = indexing::Indexing::from_system(system);
        let variables = variables::Variables::for_system(system, &indexing, solve_mode);
        let constraints =
            constraints::ConstraintSet::for_system(system, &indexing, &variables, solve_mode);
        let objective = objective::Objective::for_system(system, &indexing, &variables, solve_mode);

        Self {
            solve_mode,
            indexing,
            variables,
            constraints,
            objective,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use labdessem_core::{
        hydro::{
            HydroFphaSegment, HydroGroup, HydroInitialCondition, HydroPlant, HydroUnit, Reservoir,
        },
        ids::{
            BusId, HydroGroupId, HydroPlantId, HydroUnitId, PumpingPlantId, RenewablePlantId,
            SubmarketId, ThermalPlantId, ThermalUnitId,
        },
        renewable::RenewablePlant,
        system::{
            Bus, OperationalLimit, OperationalLimitTarget, OperationalLimitVariable, PumpingPlant,
            StudyHorizon, Submarket, System,
        },
        thermal::{ThermalInitialCondition, ThermalPlant, ThermalUnit},
    };

    fn fpha_segments() -> Vec<HydroFphaSegment> {
        vec![HydroFphaSegment {
            segment: 1,
            correction_factor: 1.0,
            rhs: 0.0,
            volume_coefficient: 0.0,
            turbining_coefficient: 2.5,
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
            residual_costs: vec![],
            submarkets: vec![
                Submarket {
                    id: SubmarketId(1),
                    name: "SE".into(),
                    demand_mw: vec![100.0, 105.0],
                    deficit_cost_per_mwh: 1_000.0,
                },
                Submarket {
                    id: SubmarketId(2),
                    name: "S".into(),
                    demand_mw: vec![60.0, 62.0],
                    deficit_cost_per_mwh: 1_000.0,
                },
            ],
            interchange_limits: vec![
                labdessem_core::system::InterchangeLimit {
                    from_submarket_id: SubmarketId(1),
                    to_submarket_id: SubmarketId(2),
                    max_flow_mw: 80.0,
                    penalty_cost_per_mwh: 2.0,
                },
                labdessem_core::system::InterchangeLimit {
                    from_submarket_id: SubmarketId(2),
                    to_submarket_id: SubmarketId(1),
                    max_flow_mw: 65.0,
                    penalty_cost_per_mwh: 3.0,
                },
            ],
            operational_limits: vec![],
            buses: vec![
                Bus {
                    id: BusId(1),
                    name: "BUS-1".into(),
                    submarket_id: SubmarketId(1),
                    angle_reference: true,
                    demand_mw: vec![40.0, 42.0],
                },
                Bus {
                    id: BusId(2),
                    name: "BUS-2".into(),
                    submarket_id: SubmarketId(2),
                    angle_reference: false,
                    demand_mw: vec![60.0, 63.0],
                },
            ],
            branches: vec![],
            thermal_plants: vec![ThermalPlant {
                id: ThermalPlantId(1),
                name: "UTE-1".into(),
                submarket_id: SubmarketId(1),
                bus_id: BusId(1),
                units: vec![ThermalUnit {
                    id: ThermalUnitId(1),
                    name: "GT-1".into(),
                    min_generation_mw: 20.0,
                    max_generation_mw: 100.0,
                    startup_trajectory_mw: vec![20.0, 40.0],
                    shutdown_trajectory_mw: vec![40.0, 20.0],
                    min_up_time: 1,
                    min_down_time: 1,
                    startup_cost: 10.0,
                    shutdown_cost: 5.0,
                    variable_cost_per_mwh: 100.0,
                    initial_condition: ThermalInitialCondition {
                        is_on: true,
                        generation_mw: 20.0,
                        time_in_state: 1,
                        time_in_ramp: 1,
                        is_ramping_up: false,
                        is_ramping_down: false,
                    },
                }],
            }],
            hydro_plants: vec![HydroPlant {
                id: HydroPlantId(1),
                name: "UHE-1".into(),
                submarket_id: SubmarketId(2),
                bus_id: BusId(2),
                upstream_plant_ids: vec![],
                downstream_plant_id: None,
                diversion_upstream_plant_ids: vec![],
                diversion_plant_id: None,
                fpha_segments: fpha_segments(),
                reservoir: Reservoir {
                    min_volume_hm3: 1.0,
                    max_volume_hm3: 10.0,
                    initial_volume_hm3: 5.0,
                },
                natural_inflow_hm3: vec![1.0, 1.0],
                water_withdrawal_hm3: vec![0.0, 0.0],
                spillage_cost_per_hm3: 0.0,
                turbining_cost_per_hm3: 0.0,
                groups: vec![HydroGroup {
                    id: HydroGroupId(1),
                    name: "CJ-1".into(),
                    units: vec![HydroUnit {
                        id: HydroUnitId(1),
                        name: "UG-1".into(),
                        min_generation_mw: 5.0,
                        max_generation_mw: 50.0,
                        max_turbining_hm3: 20.0,
                        initial_condition: HydroInitialCondition {
                            is_on: true,
                            generation_mw: 5.0,
                            time_in_state: 1,
                        },
                    }],
                }],
            }],
            pumping_plants: Vec::new(),
            renewable_plants: vec![
                RenewablePlant {
                    id: RenewablePlantId(1),
                    name: "REN-1".into(),
                    submarket_id: SubmarketId(1),
                    bus_id: BusId(1),
                    available_generation_mw: vec![10.0, 10.0],
                },
                RenewablePlant {
                    id: RenewablePlantId(2),
                    name: "REN-2".into(),
                    submarket_id: SubmarketId(2),
                    bus_id: BusId(2),
                    available_generation_mw: vec![8.0, 7.0],
                },
            ],
        }
    }

    fn build_system_with_multiple_upstreams() -> System {
        System {
            horizon: StudyHorizon {
                periods: 1,
                period_duration_hours: 1.0,
                original_periods: 1,
                original_period_durations_hours: vec![1.0],
                internal_to_original_period: vec![1],
                internal_subperiod_index: vec![1],
            },
            thermal_unit_commitment_enabled: true,
            hydro_unit_commitment_enabled: true,
            ton_residual_enabled: false,
            residual_costs: vec![],
            submarkets: vec![Submarket {
                id: SubmarketId(1),
                name: "SE".into(),
                demand_mw: vec![0.0],
                deficit_cost_per_mwh: 1_000.0,
            }],
            interchange_limits: vec![],
            operational_limits: vec![],
            buses: vec![Bus {
                id: BusId(1),
                name: "BUS-1".into(),
                submarket_id: SubmarketId(1),
                angle_reference: true,
                demand_mw: vec![0.0],
            }],
            branches: vec![],
            thermal_plants: vec![],
            hydro_plants: vec![
                HydroPlant {
                    id: HydroPlantId(1),
                    name: "UHE-A".into(),
                    submarket_id: SubmarketId(1),
                    bus_id: BusId(1),
                    upstream_plant_ids: vec![],
                    downstream_plant_id: Some(HydroPlantId(3)),
                    diversion_upstream_plant_ids: vec![],
                    diversion_plant_id: None,
                    fpha_segments: fpha_segments(),
                    reservoir: Reservoir {
                        min_volume_hm3: 0.0,
                        max_volume_hm3: 10.0,
                        initial_volume_hm3: 1.0,
                    },
                    natural_inflow_hm3: vec![0.0],
                    water_withdrawal_hm3: vec![0.0],
                    spillage_cost_per_hm3: 0.0,
                    turbining_cost_per_hm3: 0.0,
                    groups: vec![HydroGroup {
                        id: HydroGroupId(1),
                        name: "CJ-A".into(),
                        units: vec![HydroUnit {
                            id: HydroUnitId(1),
                            name: "UG-A".into(),
                            min_generation_mw: 1.0,
                            max_generation_mw: 10.0,
                            max_turbining_hm3: 5.0,
                            initial_condition: HydroInitialCondition {
                                is_on: true,
                                generation_mw: 1.0,
                                time_in_state: 1,
                            },
                        }],
                    }],
                },
                HydroPlant {
                    id: HydroPlantId(2),
                    name: "UHE-B".into(),
                    submarket_id: SubmarketId(1),
                    bus_id: BusId(1),
                    upstream_plant_ids: vec![],
                    downstream_plant_id: Some(HydroPlantId(3)),
                    diversion_upstream_plant_ids: vec![],
                    diversion_plant_id: None,
                    fpha_segments: fpha_segments(),
                    reservoir: Reservoir {
                        min_volume_hm3: 0.0,
                        max_volume_hm3: 10.0,
                        initial_volume_hm3: 1.0,
                    },
                    natural_inflow_hm3: vec![0.0],
                    water_withdrawal_hm3: vec![0.0],
                    spillage_cost_per_hm3: 0.0,
                    turbining_cost_per_hm3: 0.0,
                    groups: vec![HydroGroup {
                        id: HydroGroupId(2),
                        name: "CJ-B".into(),
                        units: vec![HydroUnit {
                            id: HydroUnitId(2),
                            name: "UG-B".into(),
                            min_generation_mw: 1.0,
                            max_generation_mw: 10.0,
                            max_turbining_hm3: 5.0,
                            initial_condition: HydroInitialCondition {
                                is_on: true,
                                generation_mw: 1.0,
                                time_in_state: 1,
                            },
                        }],
                    }],
                },
                HydroPlant {
                    id: HydroPlantId(3),
                    name: "UHE-C".into(),
                    submarket_id: SubmarketId(1),
                    bus_id: BusId(1),
                    upstream_plant_ids: vec![HydroPlantId(1), HydroPlantId(2)],
                    downstream_plant_id: None,
                    diversion_upstream_plant_ids: vec![],
                    diversion_plant_id: None,
                    fpha_segments: fpha_segments(),
                    reservoir: Reservoir {
                        min_volume_hm3: 0.0,
                        max_volume_hm3: 20.0,
                        initial_volume_hm3: 5.0,
                    },
                    natural_inflow_hm3: vec![3.0],
                    water_withdrawal_hm3: vec![0.0],
                    spillage_cost_per_hm3: 0.0,
                    turbining_cost_per_hm3: 0.0,
                    groups: vec![HydroGroup {
                        id: HydroGroupId(3),
                        name: "CJ-C".into(),
                        units: vec![HydroUnit {
                            id: HydroUnitId(3),
                            name: "UG-C".into(),
                            min_generation_mw: 1.0,
                            max_generation_mw: 10.0,
                            max_turbining_hm3: 5.0,
                            initial_condition: HydroInitialCondition {
                                is_on: true,
                                generation_mw: 1.0,
                                time_in_state: 1,
                            },
                        }],
                    }],
                },
            ],
            pumping_plants: Vec::new(),
            renewable_plants: vec![],
        }
    }

    #[test]
    fn builds_model_skeleton() {
        let system = build_system();
        let model = Model::from_system(&system, SolveMode::MixedIntegerLinearProgramming);

        assert_eq!(model.indexing.thermal_units, 1);
        assert_eq!(model.indexing.hydro_units, 1);
        assert_eq!(model.variables.hydro_turbining.len(), 2);
        assert_eq!(model.variables.hydro_spillage.len(), 2);
        assert_eq!(model.variables.hydro_volume.len(), 3);
        assert_eq!(model.variables.deficit.len(), 4);
        assert_eq!(model.variables.interchange.len(), 4);
        assert_eq!(model.constraints.demand_balance().len(), 4);
        assert_eq!(model.constraints.hydro_balance().len(), 2);
        assert_eq!(model.constraints.interchange_limits().len(), 4);
        assert_eq!(model.constraints.hydro_turbining_limits().len(), 4);
        assert_eq!(model.constraints.hydro_spillage_nonnegativity().len(), 2);
        assert_eq!(model.constraints.hydro_fpha().len(), 2);
        assert!(
            model
                .constraints
                .names()
                .contains(&"linearized_network_flow")
        );
        assert!(model.constraints.names().contains(&"unit_commitment"));
    }

    #[test]
    fn builds_demand_balance_by_submarket_and_period() {
        let system = build_system();
        let model = Model::from_system(&system, SolveMode::LinearProgramming);

        let balances = model.constraints.demand_balance();
        assert_eq!(balances.len(), 4);

        let first = &balances[0];
        assert_eq!(first.name, "demand_balance[submarket=SE,t=1]");
        assert_eq!(first.rhs, 100.0);

        let term_names: Vec<_> = first
            .terms
            .iter()
            .map(|term| term.variable.as_str())
            .collect();
        assert!(term_names.contains(&"thermal_generation[p=UTE-1,u=GT-1,t=1]"));
        assert!(term_names.contains(&"renewable_generation[p=REN-1,t=1]"));
        assert!(term_names.contains(&"interchange[from=S,to=SE,t=1]"));
        assert!(term_names.contains(&"interchange[from=SE,to=S,t=1]"));
        assert!(term_names.contains(&"deficit[submarket=SE,t=1]"));

        let incoming = first
            .terms
            .iter()
            .find(|term| term.variable == "interchange[from=S,to=SE,t=1]")
            .expect("incoming interchange term should exist");
        assert_eq!(incoming.coefficient, 1.0);

        let outgoing = first
            .terms
            .iter()
            .find(|term| term.variable == "interchange[from=SE,to=S,t=1]")
            .expect("outgoing interchange term should exist");
        assert_eq!(outgoing.coefficient, -1.0);
    }

    #[test]
    fn builds_hydro_turbining_and_spillage_variables() {
        let system = build_system();
        let model = Model::from_system(&system, SolveMode::LinearProgramming);

        assert_eq!(
            model.variables.hydro_turbining[0].name,
            "hydro_turbining[p=UHE-1,g=CJ-1,u=UG-1,t=1]"
        );
        assert_eq!(model.variables.hydro_turbining[0].lower_bound, 0.0);
        assert_eq!(model.variables.hydro_turbining[0].upper_bound, Some(20.0));

        assert_eq!(
            model.variables.hydro_spillage[0].name,
            "hydro_spillage[p=UHE-1,t=1]"
        );
        assert_eq!(model.variables.hydro_spillage[0].lower_bound, 0.0);
        assert_eq!(model.variables.hydro_spillage[0].upper_bound, None);

        assert_eq!(
            model.variables.hydro_volume[0].name,
            "hydro_volume[p=UHE-1,t=0]"
        );
        assert_eq!(model.variables.hydro_volume[0].lower_bound, 1.0);
        assert_eq!(model.variables.hydro_volume[0].upper_bound, Some(10.0));
        assert_eq!(model.variables.hydro_volume[0].fixed_value, Some(5.0));
    }

    #[test]
    fn adds_pumping_to_demand_and_hydro_balances() {
        let mut system = build_system_with_multiple_upstreams();
        system.pumping_plants = vec![PumpingPlant {
            id: PumpingPlantId(1),
            name: "USIE-1".into(),
            submarket_id: SubmarketId(1),
            bus_id: BusId(1),
            downstream_hydro_id: HydroPlantId(3),
            upstream_hydro_id: HydroPlantId(1),
            min_pumping_hm3: 0.0,
            max_pumping_hm3: 3.6,
            specific_consumption_mw_per_m3s: 0.5,
        }];

        let model = Model::from_system(&system, SolveMode::LinearProgramming);
        assert_eq!(model.variables.pumping.len(), 1);
        assert_eq!(model.variables.pumping[0].name, "pumping[p=USIE-1,t=1]");

        let demand = model.constraints.demand_balance()[0];
        let pumping_in_demand = demand
            .terms
            .iter()
            .find(|term| term.variable == "pumping[p=USIE-1,t=1]")
            .expect("pumping should enter demand balance");
        assert!((pumping_in_demand.coefficient + 0.5 / 0.0036).abs() < 1e-8);

        let hydro_balances = model.constraints.hydro_balance();
        let upstream_balance = hydro_balances
            .iter()
            .find(|constraint| constraint.name == "hydro_balance[p=UHE-A,t=1]")
            .expect("upstream hydro balance should exist");
        assert!(
            upstream_balance
                .terms
                .iter()
                .any(|term| term.variable == "pumping[p=USIE-1,t=1]" && term.coefficient == -1.0)
        );

        let downstream_balance = hydro_balances
            .iter()
            .find(|constraint| constraint.name == "hydro_balance[p=UHE-C,t=1]")
            .expect("downstream hydro balance should exist");
        assert!(
            downstream_balance
                .terms
                .iter()
                .any(|term| term.variable == "pumping[p=USIE-1,t=1]" && term.coefficient == 1.0)
        );
    }

    #[test]
    fn builds_interchange_limit_constraints_with_directional_values() {
        let system = build_system();
        let model = Model::from_system(&system, SolveMode::LinearProgramming);

        let limits = model.constraints.interchange_limits();
        assert_eq!(limits.len(), 4);

        let se_to_s = limits
            .iter()
            .find(|constraint| constraint.name == "interchange_limit[from=SE,to=S,t=1]")
            .expect("SE to S interchange limit should exist");
        assert_eq!(se_to_s.rhs, 80.0);

        let s_to_se = limits
            .iter()
            .find(|constraint| constraint.name == "interchange_limit[from=S,to=SE,t=1]")
            .expect("S to SE interchange limit should exist");
        assert_eq!(s_to_se.rhs, 65.0);
    }

    #[test]
    fn does_not_fix_hydro_volume_in_first_period() {
        let system = build_system();
        let model = Model::from_system(&system, SolveMode::LinearProgramming);
        assert!(
            !model
                .constraints
                .names()
                .iter()
                .any(|name| name.starts_with("initial_hydro_volume["))
        );
    }

    #[test]
    fn builds_hydro_balance_for_first_period_with_boundary_condition() {
        let system = build_system();
        let model = Model::from_system(&system, SolveMode::LinearProgramming);

        let first = model
            .constraints
            .hydro_balance()
            .into_iter()
            .find(|constraint| constraint.name == "hydro_balance[p=UHE-1,t=1]")
            .expect("first-period hydro balance should exist");

        assert_eq!(first.sense, constraints::ConstraintSense::Equal);
        assert_eq!(first.rhs, 1.0);
        assert!(first
            .terms
            .iter()
            .any(|term| term.variable == "hydro_volume[p=UHE-1,t=1]" && term.coefficient == 1.0));
        assert!(
            first.terms.iter().any(
                |term| term.variable == "hydro_volume[p=UHE-1,t=0]" && term.coefficient == -1.0
            )
        );
        assert!(first.terms.iter().any(|term| {
            term.variable == "hydro_turbining[p=UHE-1,g=CJ-1,u=UG-1,t=1]" && term.coefficient == 1.0
        }));
        assert!(
            first
                .terms
                .iter()
                .any(|term| term.variable == "hydro_spillage[p=UHE-1,t=1]"
                    && term.coefficient == 1.0)
        );
        assert!(
            !first.terms.iter().any(
                |term| term.variable == "hydro_volume[p=UHE-1,t=2]" && term.coefficient == -1.0
            )
        );
    }

    #[test]
    fn builds_hydro_balance_for_later_periods_with_previous_volume() {
        let system = build_system();
        let model = Model::from_system(&system, SolveMode::LinearProgramming);

        let second = model
            .constraints
            .hydro_balance()
            .into_iter()
            .find(|constraint| constraint.name == "hydro_balance[p=UHE-1,t=2]")
            .expect("later-period hydro balance should exist");

        assert_eq!(second.sense, constraints::ConstraintSense::Equal);
        assert_eq!(second.rhs, 1.0);
        assert!(second
            .terms
            .iter()
            .any(|term| term.variable == "hydro_volume[p=UHE-1,t=2]" && term.coefficient == 1.0));
        assert!(
            second.terms.iter().any(
                |term| term.variable == "hydro_volume[p=UHE-1,t=1]" && term.coefficient == -1.0
            )
        );
    }

    #[test]
    fn builds_hydro_turbining_spillage_and_fpha_constraints() {
        let system = build_system();
        let model = Model::from_system(&system, SolveMode::LinearProgramming);

        let turbining_upper = model
            .constraints
            .hydro_turbining_limits()
            .into_iter()
            .find(|constraint| {
                constraint.name == "hydro_turbining_upper[p=UHE-1,g=CJ-1,u=UG-1,t=1]"
            })
            .expect("hydro turbining upper limit should exist");
        assert_eq!(turbining_upper.rhs, 20.0);

        let spillage_nonnegative = model
            .constraints
            .hydro_spillage_nonnegativity()
            .into_iter()
            .find(|constraint| constraint.name == "hydro_spillage_nonnegative[p=UHE-1,t=1]")
            .expect("hydro spillage nonnegativity should exist");
        assert_eq!(
            spillage_nonnegative.sense,
            constraints::ConstraintSense::GreaterOrEqual
        );
        assert_eq!(spillage_nonnegative.rhs, 0.0);

        let fpha = model
            .constraints
            .hydro_fpha()
            .into_iter()
            .find(|constraint| constraint.name == "hydro_fpha[p=UHE-1,seg=1,t=1]")
            .expect("hydro FPHA constraint should exist");
        assert_eq!(fpha.sense, constraints::ConstraintSense::LessOrEqual);
        assert!(fpha.terms.iter().any(|term| term.variable
            == "hydro_generation[p=UHE-1,g=CJ-1,u=UG-1,t=1]"
            && term.coefficient == 1.0));
        assert!(fpha.terms.iter().any(|term| term.variable
            == "hydro_turbining[p=UHE-1,g=CJ-1,u=UG-1,t=1]"
            && (term.coefficient + (2.5 / 0.0036)).abs() < 1e-9));
    }

    #[test]
    fn sums_all_upstream_plants_in_hydro_balance() {
        let system = build_system_with_multiple_upstreams();
        let model = Model::from_system(&system, SolveMode::LinearProgramming);

        let balance = model
            .constraints
            .hydro_balance()
            .into_iter()
            .find(|constraint| constraint.name == "hydro_balance[p=UHE-C,t=1]")
            .expect("hydro balance with multiple upstreams should exist");

        assert!(balance.terms.iter().any(|term| term.variable
            == "hydro_turbining[p=UHE-A,g=CJ-A,u=UG-A,t=1]"
            && term.coefficient == -1.0));
        assert!(
            balance
                .terms
                .iter()
                .any(|term| term.variable == "hydro_spillage[p=UHE-A,t=1]"
                    && term.coefficient == -1.0)
        );
        assert!(balance.terms.iter().any(|term| term.variable
            == "hydro_turbining[p=UHE-B,g=CJ-B,u=UG-B,t=1]"
            && term.coefficient == -1.0));
        assert!(
            balance
                .terms
                .iter()
                .any(|term| term.variable == "hydro_spillage[p=UHE-B,t=1]"
                    && term.coefficient == -1.0)
        );
    }

    #[test]
    fn adds_diversion_flows_to_hydro_balance() {
        let mut system = build_system_with_multiple_upstreams();
        system.hydro_plants[0].diversion_plant_id = Some(HydroPlantId(3));
        system.hydro_plants[2].diversion_upstream_plant_ids = vec![HydroPlantId(1)];

        let model = Model::from_system(&system, SolveMode::LinearProgramming);

        let diversion_destination_balance = model
            .constraints
            .hydro_balance()
            .into_iter()
            .find(|constraint| constraint.name == "hydro_balance[p=UHE-C,t=1]")
            .expect("hydro balance with diversion upstream should exist");

        assert!(diversion_destination_balance.terms.iter().any(|term| {
            term.variable == "hydro_diversion[p=UHE-A,t=1]" && term.coefficient == -1.0
        }));

        let diversion_origin_balance = model
            .constraints
            .hydro_balance()
            .into_iter()
            .find(|constraint| constraint.name == "hydro_balance[p=UHE-A,t=1]")
            .expect("hydro balance with diversion outflow should exist");

        assert!(diversion_origin_balance.terms.iter().any(|term| {
            term.variable == "hydro_diversion[p=UHE-A,t=1]" && term.coefficient == 1.0
        }));
    }

    #[test]
    fn builds_channeling_constraints_for_commitment_modes() {
        let system = build_system();
        let model = Model::from_system(&system, SolveMode::MixedIntegerLinearProgramming);

        let channeling = model.constraints.channeling();
        assert_eq!(channeling.len(), 4);

        let hydro_lower = channeling
            .iter()
            .find(|constraint| {
                constraint.name == "channeling_hydro_lower[p=UHE-1,g=CJ-1,u=UG-1,t=2]"
            })
            .expect("hydro lower channeling should exist");
        assert_eq!(
            hydro_lower.sense,
            constraints::ConstraintSense::GreaterOrEqual
        );
        assert!(hydro_lower.terms.iter().any(|term| term.variable
            == "hydro_generation[p=UHE-1,g=CJ-1,u=UG-1,t=2]"
            && term.coefficient == 1.0));
        assert!(hydro_lower.terms.iter().any(|term| term.variable
            == "hydro_on[p=UHE-1,g=CJ-1,u=UG-1,t=2]"
            && term.coefficient == -5.0));
    }

    #[test]
    fn does_not_build_channeling_constraints_in_pure_lp_mode() {
        let system = build_system();
        let model = Model::from_system(&system, SolveMode::LinearProgramming);

        assert!(model.constraints.channeling().is_empty());
        assert!(model.constraints.thermal_min_up_down().is_empty());
        assert!(model.constraints.thermal_ramps().is_empty());
    }

    #[test]
    fn builds_volume_operational_limits_on_useful_volume() {
        let mut system = build_system();
        system.operational_limits = vec![OperationalLimit {
            target: OperationalLimitTarget::HydroPlant(HydroPlantId(1)),
            plant_name: "UHE-1".into(),
            variable: OperationalLimitVariable::Volume,
            start_period: 1,
            end_period: 1,
            lower_bound: Some(2.0),
            upper_bound: Some(4.0),
        }];

        let model = Model::from_system(&system, SolveMode::LinearProgramming);
        let limits = model.constraints.operational_limits();

        let lower = limits
            .iter()
            .find(|constraint| constraint.name == "operational_limit_lower[p=UHE-1,var=VOL,t=1]")
            .expect("lower volume operational limit should exist");
        assert_eq!(lower.sense, constraints::ConstraintSense::GreaterOrEqual);
        assert_eq!(lower.rhs, 3.0);

        let upper = limits
            .iter()
            .find(|constraint| constraint.name == "operational_limit_upper[p=UHE-1,var=VOL,t=1]")
            .expect("upper volume operational limit should exist");
        assert_eq!(upper.sense, constraints::ConstraintSense::LessOrEqual);
        assert_eq!(upper.rhs, 5.0);
    }

    #[test]
    fn converts_flow_operational_limits_from_m3s_to_hm3() {
        let mut system = build_system();
        system.operational_limits = vec![OperationalLimit {
            target: OperationalLimitTarget::HydroPlant(HydroPlantId(1)),
            plant_name: "UHE-1".into(),
            variable: OperationalLimitVariable::Turbining,
            start_period: 1,
            end_period: 1,
            lower_bound: Some(100.0),
            upper_bound: Some(200.0),
        }];

        let model = Model::from_system(&system, SolveMode::LinearProgramming);
        let limits = model.constraints.operational_limits();

        let lower = limits
            .iter()
            .find(|constraint| constraint.name == "operational_limit_lower[p=UHE-1,var=TURB,t=1]")
            .expect("lower turbining operational limit should exist");
        assert!((lower.rhs - 0.36).abs() < 1e-10);

        let upper = limits
            .iter()
            .find(|constraint| constraint.name == "operational_limit_upper[p=UHE-1,var=TURB,t=1]")
            .expect("upper turbining operational limit should exist");
        assert!((upper.rhs - 0.72).abs() < 1e-10);
    }

    #[test]
    fn builds_pumping_operational_limits() {
        let mut system = build_system();
        system.pumping_plants = vec![PumpingPlant {
            id: PumpingPlantId(1),
            name: "USIE-1".into(),
            submarket_id: SubmarketId(1),
            bus_id: BusId(1),
            downstream_hydro_id: HydroPlantId(1),
            upstream_hydro_id: HydroPlantId(1),
            min_pumping_hm3: 0.0,
            max_pumping_hm3: 3.6,
            specific_consumption_mw_per_m3s: 0.5,
        }];
        system.pumping_plants[0].upstream_hydro_id = HydroPlantId(999);
        system.pumping_plants[0].downstream_hydro_id = HydroPlantId(1);
        system.hydro_plants.push(HydroPlant {
            id: HydroPlantId(999),
            name: "UHE-UP".into(),
            submarket_id: SubmarketId(1),
            bus_id: BusId(1),
            upstream_plant_ids: vec![],
            downstream_plant_id: None,
            diversion_upstream_plant_ids: vec![],
            diversion_plant_id: None,
            fpha_segments: fpha_segments(),
            reservoir: Reservoir {
                min_volume_hm3: 0.0,
                max_volume_hm3: 10.0,
                initial_volume_hm3: 1.0,
            },
            natural_inflow_hm3: vec![0.0, 0.0],
            water_withdrawal_hm3: vec![0.0, 0.0],
            spillage_cost_per_hm3: 0.0,
            turbining_cost_per_hm3: 0.0,
            groups: vec![HydroGroup {
                id: HydroGroupId(1),
                name: "CJ-1".into(),
                units: vec![HydroUnit {
                    id: HydroUnitId(1),
                    name: "UG-1".into(),
                    min_generation_mw: 0.0,
                    max_generation_mw: 1.0,
                    max_turbining_hm3: 1.0,
                    initial_condition: HydroInitialCondition {
                        is_on: false,
                        generation_mw: 0.0,
                        time_in_state: 1,
                    },
                }],
            }],
        });
        system.operational_limits = vec![OperationalLimit {
            target: OperationalLimitTarget::PumpingPlant(PumpingPlantId(1)),
            plant_name: "USIE-1".into(),
            variable: OperationalLimitVariable::Pumping,
            start_period: 1,
            end_period: 1,
            lower_bound: None,
            upper_bound: Some(100.0),
        }];

        let model = Model::from_system(&system, SolveMode::LinearProgramming);
        let limit = model
            .constraints
            .operational_limits()
            .into_iter()
            .find(|constraint| constraint.name == "operational_limit_upper[p=USIE-1,var=QBOM,t=1]")
            .expect("pumping operational limit should exist");
        assert!((limit.rhs - 0.36).abs() < 1e-10);
        assert!(
            limit
                .terms
                .iter()
                .any(|term| term.variable == "pumping[p=USIE-1,t=1]" && term.coefficient == 1.0)
        );
    }

    #[test]
    fn builds_thermal_min_up_down_constraints_with_remaining_initial_on_time() {
        let mut system = build_system();
        system.horizon.periods = 4;
        system.submarkets[0].demand_mw = vec![100.0, 105.0, 110.0, 115.0];
        system.submarkets[1].demand_mw = vec![60.0, 62.0, 64.0, 66.0];
        system.buses[0].demand_mw = vec![40.0, 42.0, 44.0, 46.0];
        system.buses[1].demand_mw = vec![60.0, 63.0, 66.0, 69.0];
        system.renewable_plants[0].available_generation_mw = vec![10.0, 10.0, 10.0, 10.0];
        system.renewable_plants[1].available_generation_mw = vec![8.0, 7.0, 6.0, 5.0];
        system.hydro_plants[0].natural_inflow_hm3 = vec![1.0, 1.0, 1.0, 1.0];
        system.hydro_plants[0].water_withdrawal_hm3 = vec![0.0, 0.0, 0.0, 0.0];
        system.thermal_plants[0].units[0].min_up_time = 3;
        system.thermal_plants[0].units[0].min_down_time = 2;
        system.thermal_plants[0].units[0].initial_condition.is_on = true;
        system.thermal_plants[0].units[0]
            .initial_condition
            .time_in_state = 1;

        let model = Model::from_system(&system, SolveMode::MixedIntegerLinearProgramming);
        let thermal_limits = model.constraints.thermal_min_up_down();

        assert!(thermal_limits.iter().any(|constraint| constraint.name
            == "thermal_initial_on_fix[p=UTE-1,u=GT-1,t=1]"
            && constraint.rhs == 1.0));
        assert!(thermal_limits.iter().any(|constraint| constraint.name
            == "thermal_initial_on_fix[p=UTE-1,u=GT-1,t=2]"
            && constraint.rhs == 1.0));

        let first_ton = thermal_limits
            .iter()
            .find(|constraint| constraint.name == "thermal_min_up[p=UTE-1,u=GT-1,t=1]")
            .expect("first-period thermal minimum up constraint should exist");
        assert_eq!(
            first_ton.sense,
            constraints::ConstraintSense::GreaterOrEqual
        );
        assert_eq!(first_ton.rhs, -3.0);
        assert!(
            first_ton
                .terms
                .iter()
                .any(|term| term.variable == "thermal_on[p=UTE-1,u=GT-1,t=1]"
                    && term.coefficient == -3.0)
        );
        assert!(first_ton.terms.iter().any(|term| term.variable
            == "thermal_on[p=UTE-1,u=GT-1,t=2]"
            && term.coefficient == 1.0));
        assert!(first_ton.terms.iter().any(|term| term.variable
            == "thermal_on[p=UTE-1,u=GT-1,t=3]"
            && term.coefficient == 1.0));
    }

    #[test]
    fn builds_thermal_ramp_constraints_for_commitment_modes() {
        let mut system = build_system();
        system.horizon.periods = 3;
        system.submarkets[0].demand_mw = vec![100.0, 105.0, 110.0];
        system.submarkets[1].demand_mw = vec![60.0, 62.0, 64.0];
        system.buses[0].demand_mw = vec![40.0, 42.0, 44.0];
        system.buses[1].demand_mw = vec![60.0, 63.0, 66.0];
        system.renewable_plants[0].available_generation_mw = vec![10.0, 10.0, 10.0];
        system.renewable_plants[1].available_generation_mw = vec![8.0, 7.0, 6.0];
        system.hydro_plants[0].natural_inflow_hm3 = vec![1.0, 1.0, 1.0];
        system.hydro_plants[0].water_withdrawal_hm3 = vec![0.0, 0.0, 0.0];
        system.thermal_plants[0].units[0].startup_trajectory_mw = vec![20.0, 40.0];
        system.thermal_plants[0].units[0].shutdown_trajectory_mw = vec![40.0, 20.0];

        let model = Model::from_system(&system, SolveMode::MixedIntegerLinearProgramming);
        let ramps = model.constraints.thermal_ramps();

        let first_transition = ramps
            .iter()
            .find(|constraint| constraint.name == "thermal_transition[p=UTE-1,u=GT-1,t=1]")
            .expect("first thermal transition constraint should exist");
        assert_eq!(first_transition.sense, constraints::ConstraintSense::Equal);
        assert_eq!(first_transition.rhs, 1.0);
        assert!(first_transition.terms.iter().any(|term| term.variable
            == "thermal_on[p=UTE-1,u=GT-1,t=1]"
            && term.coefficient == 1.0));
        assert!(first_transition.terms.iter().any(|term| term.variable
            == "thermal_startup[p=UTE-1,u=GT-1,t=1]"
            && term.coefficient == -1.0));
        assert!(first_transition.terms.iter().any(|term| term.variable
            == "thermal_shutdown[p=UTE-1,u=GT-1,t=1]"
            && term.coefficient == 1.0));

        let first_lower = ramps
            .iter()
            .find(|constraint| constraint.name == "thermal_ramp_lower[p=UTE-1,u=GT-1,t=1]")
            .expect("first thermal lower ramp constraint should exist");
        assert_eq!(
            first_lower.sense,
            constraints::ConstraintSense::GreaterOrEqual
        );
        assert_eq!(first_lower.rhs, 0.0);
        assert!(first_lower.terms.iter().any(|term| term.variable
            == "thermal_generation[p=UTE-1,u=GT-1,t=1]"
            && term.coefficient == 1.0));
        assert!(
            first_lower
                .terms
                .iter()
                .any(|term| term.variable == "thermal_on[p=UTE-1,u=GT-1,t=1]"
                    && term.coefficient == -20.0)
        );
        assert!(first_lower.terms.iter().any(|term| term.variable
            == "thermal_startup[p=UTE-1,u=GT-1,t=1]"
            && term.coefficient == 20.0));
        assert!(first_lower.terms.iter().any(|term| term.variable
            == "thermal_startup[p=UTE-1,u=GT-1,t=1]"
            && term.coefficient == -20.0));
    }

    #[test]
    fn builds_objective_with_operating_costs_in_lp_mode() {
        let system = build_system();
        let model = Model::from_system(&system, SolveMode::LinearProgramming);

        assert_eq!(model.objective.sense, objective::ObjectiveSense::Minimize);

        let thermal_cost = model
            .objective
            .terms
            .iter()
            .find(|term| term.variable == "thermal_generation[p=UTE-1,u=GT-1,t=1]")
            .expect("thermal generation cost should exist");
        assert_eq!(thermal_cost.coefficient, 100.0);

        let deficit_cost = model
            .objective
            .terms
            .iter()
            .find(|term| term.variable == "deficit[submarket=SE,t=1]")
            .expect("deficit cost should exist");
        assert_eq!(deficit_cost.coefficient, 1_000.0);

        let spillage_cost = model
            .objective
            .terms
            .iter()
            .find(|term| term.variable == "hydro_spillage[p=UHE-1,t=1]")
            .expect("spillage cost should exist");
        assert_eq!(spillage_cost.coefficient, 0.0);

        let interchange_cost = model
            .objective
            .terms
            .iter()
            .find(|term| term.variable == "interchange[from=SE,to=S,t=1]")
            .expect("interchange penalty should exist");
        assert_eq!(interchange_cost.coefficient, 2.0);

        assert!(
            !model
                .objective
                .terms
                .iter()
                .any(|term| term.variable.starts_with("thermal_startup["))
        );
    }

    #[test]
    fn builds_objective_with_startup_and_shutdown_costs_in_commitment_modes() {
        let system = build_system();
        let model = Model::from_system(&system, SolveMode::MixedIntegerLinearProgramming);

        let thermal_startup = model
            .objective
            .terms
            .iter()
            .find(|term| term.variable == "thermal_startup[p=UTE-1,u=GT-1,t=1]")
            .expect("thermal startup cost should exist");
        assert_eq!(thermal_startup.coefficient, 10.0);

        let thermal_shutdown = model
            .objective
            .terms
            .iter()
            .find(|term| term.variable == "thermal_shutdown[p=UTE-1,u=GT-1,t=1]")
            .expect("thermal shutdown cost should exist");
        assert_eq!(thermal_shutdown.coefficient, 5.0);

    }
}
