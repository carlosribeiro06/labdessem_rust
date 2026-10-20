use crate::{SolveMode, indexing::Indexing};
use labdessem_core::system::System;
use labdessem_uc::types::CommitmentDomain;
use labdessem_uc::{
    hydro::{HydroUnitInfo, build_hydro_commitment_variables},
    thermal::{
        ThermalUnitInfo, build_thermal_commitment_variables, build_thermal_shutdown_variables,
        build_thermal_startup_variables,
    },
};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum VariableDomain {
    Continuous,
    Binary,
}

#[derive(Debug, Clone, PartialEq)]
pub struct Variable {
    pub name: String,
    pub lower_bound: f64,
    pub upper_bound: Option<f64>,
    pub domain: VariableDomain,
    pub fixed_value: Option<f64>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct Variables {
    pub thermal_generation: Vec<Variable>,
    pub hydro_generation: Vec<Variable>,
    pub hydro_turbining: Vec<Variable>,
    pub hydro_spillage: Vec<Variable>,
    pub hydro_diversion: Vec<Variable>,
    pub pumping: Vec<Variable>,
    pub hydro_volume: Vec<Variable>,
    pub future_cost: Vec<Variable>,
    pub renewable_generation: Vec<Variable>,
    pub interchange: Vec<Variable>,
    pub thermal_commitment: Vec<Variable>,
    pub thermal_startup: Vec<Variable>,
    pub thermal_shutdown: Vec<Variable>,
    pub hydro_commitment: Vec<Variable>,
    pub network_flow_slack: Vec<Variable>,
    pub operational_limit_slack: Vec<Variable>,
}

impl Variables {
    pub fn for_system(system: &System, indexing: &Indexing, solve_mode: SolveMode) -> Self {
        let horizon = system.horizon.periods;

        let thermal_generation = indexing
            .thermal_unit_entries
            .iter()
            .flat_map(|entry| {
                let plant = &system.thermal_plants[entry.plant_idx];
                let unit = &plant.units[entry.unit_idx];
                (0..horizon).map(move |period| Variable {
                    name: format!(
                        "thermal_generation[p={},u={},t={}]",
                        plant.name,
                        unit.name,
                        display_period(period)
                    ),
                    lower_bound: 0.0,
                    upper_bound: Some(unit.max_generation_mw),
                    domain: VariableDomain::Continuous,
                    fixed_value: None,
                })
            })
            .collect();

        let hydro_generation = indexing
            .hydro_unit_entries
            .iter()
            .flat_map(|entry| {
                let plant = &system.hydro_plants[entry.plant_idx];
                let group = &plant.groups[entry.group_idx];
                let unit = &group.units[entry.unit_idx];
                (0..horizon).map(move |period| Variable {
                    name: format!(
                        "hydro_generation[p={},g={},u={},t={}]",
                        plant.name,
                        group.name,
                        unit.name,
                        display_period(period)
                    ),
                    lower_bound: 0.0,
                    upper_bound: Some(unit.max_generation_mw),
                    domain: VariableDomain::Continuous,
                    fixed_value: None,
                })
            })
            .collect();

        let hydro_turbining = indexing
            .hydro_unit_entries
            .iter()
            .flat_map(|entry| {
                let plant = &system.hydro_plants[entry.plant_idx];
                let group = &plant.groups[entry.group_idx];
                let unit = &group.units[entry.unit_idx];
                (0..horizon).map(move |period| Variable {
                    name: format!(
                        "hydro_turbining[p={},g={},u={},t={}]",
                        plant.name,
                        group.name,
                        unit.name,
                        display_period(period)
                    ),
                    lower_bound: 0.0,
                    upper_bound: Some(unit.max_turbining_hm3),
                    domain: VariableDomain::Continuous,
                    fixed_value: None,
                })
            })
            .collect();

        let hydro_spillage = indexing
            .hydro_plant_entries
            .iter()
            .flat_map(|entry| {
                let plant = &system.hydro_plants[entry.plant_idx];
                (0..horizon).map(move |period| Variable {
                    name: format!(
                        "hydro_spillage[p={},t={}]",
                        plant.name,
                        display_period(period)
                    ),
                    lower_bound: 0.0,
                    upper_bound: None,
                    domain: VariableDomain::Continuous,
                    fixed_value: None,
                })
            })
            .collect();

        let hydro_diversion = indexing
            .hydro_plant_entries
            .iter()
            .flat_map(|entry| {
                let plant = &system.hydro_plants[entry.plant_idx];
                (0..horizon).map(move |period| Variable {
                    name: format!(
                        "hydro_diversion[p={},t={}]",
                        plant.name,
                        display_period(period)
                    ),
                    lower_bound: 0.0,
                    upper_bound: if plant.diversion_plant_id.is_some() {
                        None
                    } else {
                        Some(0.0)
                    },
                    domain: VariableDomain::Continuous,
                    fixed_value: None,
                })
            })
            .collect();

        let hydro_volume = indexing
            .hydro_plant_entries
            .iter()
            .flat_map(|entry| {
                let plant = &system.hydro_plants[entry.plant_idx];
                (0..=horizon).map(move |period| Variable {
                    name: format!("hydro_volume[p={},t={}]", plant.name, period),
                    lower_bound: plant.reservoir.min_volume_hm3,
                    upper_bound: Some(plant.reservoir.max_volume_hm3),
                    domain: VariableDomain::Continuous,
                    fixed_value: if period == 0 {
                        Some(plant.reservoir.initial_volume_hm3)
                    } else {
                        None
                    },
                })
            })
            .collect();

        let pumping = indexing
            .pumping_plant_entries
            .iter()
            .flat_map(|entry| {
                let plant = &system.pumping_plants[entry.plant_idx];
                (0..horizon).map(move |period| Variable {
                    name: format!("pumping[p={},t={}]", plant.name, display_period(period)),
                    lower_bound: plant.min_pumping_hm3,
                    upper_bound: Some(plant.max_pumping_hm3),
                    domain: VariableDomain::Continuous,
                    fixed_value: None,
                })
            })
            .collect();

        let future_cost = if system.future_cost_enabled {
            vec![Variable {
                name: "future_cost".into(),
                lower_bound: 0.0,
                upper_bound: None,
                domain: VariableDomain::Continuous,
                fixed_value: None,
            }]
        } else {
            Vec::new()
        };

        let renewable_generation = indexing
            .renewable_plant_entries
            .iter()
            .flat_map(|entry| {
                let plant = &system.renewable_plants[entry.plant_idx];
                (0..horizon).map(move |period| Variable {
                    name: format!(
                        "renewable_generation[p={},t={}]",
                        plant.name,
                        display_period(period)
                    ),
                    lower_bound: 0.0,
                    upper_bound: Some(plant.available_generation_mw[period]),
                    domain: VariableDomain::Continuous,
                    fixed_value: None,
                })
            })
            .collect();

        let interchange = indexing
            .interchange_entries
            .iter()
            .map(|entry| {
                let from = &system.submarkets[entry.from_submarket_idx];
                let to = &system.submarkets[entry.to_submarket_idx];

                Variable {
                    name: format!(
                        "interchange[from={},to={},t={}]",
                        from.name,
                        to.name,
                        display_period(entry.period)
                    ),
                    lower_bound: 0.0,
                    upper_bound: system
                        .interchange_limits
                        .iter()
                        .find(|limit| {
                            limit.from_submarket_id == from.id && limit.to_submarket_id == to.id
                        })
                        .map(|limit| limit.max_flow_mw),
                    domain: VariableDomain::Continuous,
                    fixed_value: None,
                }
            })
            .collect();

        let thermal_commitment = build_thermal_commitment(system, indexing, solve_mode, horizon);
        let thermal_startup = build_thermal_startup(system, indexing, solve_mode, horizon);
        let thermal_shutdown = build_thermal_shutdown(system, indexing, solve_mode, horizon);
        let hydro_commitment = build_hydro_commitment(system, indexing, solve_mode, horizon);

        Self {
            thermal_generation,
            hydro_generation,
            hydro_turbining,
            hydro_spillage,
            hydro_diversion,
            pumping,
            hydro_volume,
            future_cost,
            renewable_generation,
            interchange,
            thermal_commitment,
            thermal_startup,
            thermal_shutdown,
            hydro_commitment,
            network_flow_slack: Vec::new(),
            operational_limit_slack: Vec::new(),
        }
    }
}

fn uc_domain_from_solve_mode(solve_mode: SolveMode) -> CommitmentDomain {
    match solve_mode {
        SolveMode::LinearProgramming => CommitmentDomain::Inactive,
        SolveMode::MixedIntegerLinearProgramming => CommitmentDomain::Binary,
        SolveMode::LinearProgrammingWithFixedCommitment => CommitmentDomain::Fixed(0),
    }
}

fn uc_variable_to_variable(uc_var: labdessem_uc::types::UcVariable) -> Variable {
    let (domain, fixed_value) = match uc_var.domain {
        CommitmentDomain::Binary => (VariableDomain::Binary, None),
        CommitmentDomain::Fixed(v) => (VariableDomain::Continuous, Some(f64::from(v))),
        CommitmentDomain::Inactive => (VariableDomain::Continuous, None),
    };
    Variable {
        name: uc_var.name,
        lower_bound: uc_var.lower_bound,
        upper_bound: Some(uc_var.upper_bound),
        domain,
        fixed_value,
    }
}

fn build_thermal_commitment(
    system: &System,
    indexing: &Indexing,
    solve_mode: SolveMode,
    horizon: usize,
) -> Vec<Variable> {
    if !system.thermal_unit_commitment_enabled {
        return Vec::new();
    }
    let domain = uc_domain_from_solve_mode(solve_mode);
    if !domain.is_active() {
        return Vec::new();
    }
    let units: Vec<_> = indexing
        .thermal_unit_entries
        .iter()
        .map(|entry| {
            let plant = &system.thermal_plants[entry.plant_idx];
            let unit = &plant.units[entry.unit_idx];
            ThermalUnitInfo {
                plant_name: &plant.name,
                unit_name: &unit.name,
            }
        })
        .collect();
    build_thermal_commitment_variables(&units, horizon, domain)
        .into_iter()
        .map(uc_variable_to_variable)
        .collect()
}

fn build_thermal_startup(
    system: &System,
    indexing: &Indexing,
    solve_mode: SolveMode,
    horizon: usize,
) -> Vec<Variable> {
    if !system.thermal_unit_commitment_enabled {
        return Vec::new();
    }
    let domain = uc_domain_from_solve_mode(solve_mode);
    if !domain.is_active() {
        return Vec::new();
    }
    let units: Vec<_> = indexing
        .thermal_unit_entries
        .iter()
        .map(|entry| {
            let plant = &system.thermal_plants[entry.plant_idx];
            let unit = &plant.units[entry.unit_idx];
            ThermalUnitInfo {
                plant_name: &plant.name,
                unit_name: &unit.name,
            }
        })
        .collect();
    build_thermal_startup_variables(&units, horizon, domain)
        .into_iter()
        .map(uc_variable_to_variable)
        .collect()
}

fn build_thermal_shutdown(
    system: &System,
    indexing: &Indexing,
    solve_mode: SolveMode,
    horizon: usize,
) -> Vec<Variable> {
    if !system.thermal_unit_commitment_enabled {
        return Vec::new();
    }
    let domain = uc_domain_from_solve_mode(solve_mode);
    if !domain.is_active() {
        return Vec::new();
    }
    let units: Vec<_> = indexing
        .thermal_unit_entries
        .iter()
        .map(|entry| {
            let plant = &system.thermal_plants[entry.plant_idx];
            let unit = &plant.units[entry.unit_idx];
            ThermalUnitInfo {
                plant_name: &plant.name,
                unit_name: &unit.name,
            }
        })
        .collect();
    build_thermal_shutdown_variables(&units, horizon, domain)
        .into_iter()
        .map(uc_variable_to_variable)
        .collect()
}

fn build_hydro_commitment(
    system: &System,
    indexing: &Indexing,
    solve_mode: SolveMode,
    horizon: usize,
) -> Vec<Variable> {
    if !system.hydro_unit_commitment_enabled {
        return Vec::new();
    }
    let domain = uc_domain_from_solve_mode(solve_mode);
    if !domain.is_active() {
        return Vec::new();
    }
    let units: Vec<_> = indexing
        .hydro_unit_entries
        .iter()
        .map(|entry| {
            let plant = &system.hydro_plants[entry.plant_idx];
            let group = &plant.groups[entry.group_idx];
            let unit = &group.units[entry.unit_idx];
            HydroUnitInfo {
                plant_name: &plant.name,
                group_name: &group.name,
                unit_name: &unit.name,
            }
        })
        .collect();
    build_hydro_commitment_variables(&units, horizon, domain)
        .into_iter()
        .map(uc_variable_to_variable)
        .collect()
}

fn display_period(period: usize) -> usize {
    period + 1
}
