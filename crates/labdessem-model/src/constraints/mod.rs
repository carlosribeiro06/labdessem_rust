mod hydro;
mod thermal;

use crate::{
    SolveMode,
    indexing::Indexing,
    variables::{Variable, Variables},
};
use labdessem_core::system::{OperationalLimitTarget, OperationalLimitVariable, System};

pub use hydro::{
    build_hydro_balance_constraints, build_hydro_commitment_channeling_constraints,
    build_hydro_fpha_constraints, build_hydro_generation_turbining_coupling_constraints,
    build_hydro_productivity_constraints, build_hydro_spillage_nonnegativity_constraints,
    build_hydro_turbining_limit_constraints,
};
pub use thermal::{build_thermal_min_up_down_constraints, build_thermal_ramp_constraints};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ConstraintSense {
    Equal,
    LessOrEqual,
    GreaterOrEqual,
}

#[derive(Debug, Clone, PartialEq)]
pub struct LinearTerm {
    pub variable: String,
    pub coefficient: f64,
}

#[derive(Debug, Clone, PartialEq)]
pub struct LinearConstraint {
    pub name: String,
    pub terms: Vec<LinearTerm>,
    pub sense: ConstraintSense,
    pub rhs: f64,
}

#[derive(Debug, Clone, PartialEq)]
pub struct ConstraintSet {
    pub linear_constraints: Vec<LinearConstraint>,
}

impl ConstraintSet {
    pub fn for_system(
        system: &System,
        indexing: &Indexing,
        variables: &Variables,
        solve_mode: SolveMode,
    ) -> Self {
        let mut linear_constraints = build_demand_balance_constraints(system, indexing, variables);
        linear_constraints.extend(build_hydro_balance_constraints(system, indexing, variables));
        linear_constraints.extend(build_interchange_limit_constraints(
            system, indexing, variables,
        ));
        linear_constraints.extend(build_hydro_turbining_limit_constraints(
            system, indexing, variables,
        ));
        linear_constraints.extend(build_hydro_spillage_nonnegativity_constraints(
            indexing, variables, system,
        ));
        if system.fpha_enabled {
            linear_constraints.extend(build_hydro_fpha_constraints(system, indexing, variables));
            linear_constraints.extend(build_hydro_generation_turbining_coupling_constraints(
                system, indexing, variables,
            ));
        } else {
            linear_constraints.extend(build_hydro_productivity_constraints(
                system, indexing, variables,
            ));
        }
        if system.future_cost_enabled {
            linear_constraints.extend(build_future_cost_constraints(system, indexing, variables));
        }
        linear_constraints.extend(build_operational_limit_constraints(
            system, indexing, variables,
        ));

        if matches!(
            solve_mode,
            SolveMode::MixedIntegerLinearProgramming
                | SolveMode::LinearProgrammingWithFixedCommitment
        ) {
            if system.thermal_unit_commitment_enabled {
                linear_constraints.extend(build_thermal_min_up_down_constraints(
                    system, indexing, variables,
                ));
                linear_constraints
                    .extend(build_thermal_ramp_constraints(system, indexing, variables));
            }
            if system.hydro_unit_commitment_enabled {
                linear_constraints.extend(build_hydro_commitment_channeling_constraints(
                    system, indexing, variables,
                ));
            }
        }

        linear_constraints.push(LinearConstraint {
            name: "linearized_network_flow".into(),
            terms: Vec::new(),
            sense: ConstraintSense::Equal,
            rhs: 0.0,
        });

        if matches!(
            solve_mode,
            SolveMode::MixedIntegerLinearProgramming
                | SolveMode::LinearProgrammingWithFixedCommitment
        ) && (system.thermal_unit_commitment_enabled || system.hydro_unit_commitment_enabled)
        {
            linear_constraints.push(LinearConstraint {
                name: "unit_commitment".into(),
                terms: Vec::new(),
                sense: ConstraintSense::Equal,
                rhs: 0.0,
            });
        }

        Self { linear_constraints }
    }

    pub fn names(&self) -> Vec<&str> {
        self.linear_constraints
            .iter()
            .map(|constraint| constraint.name.as_str())
            .collect()
    }

    pub fn demand_balance(&self) -> Vec<&LinearConstraint> {
        self.linear_constraints
            .iter()
            .filter(|constraint| constraint.name.starts_with("demand_balance["))
            .collect()
    }

    pub fn channeling(&self) -> Vec<&LinearConstraint> {
        self.linear_constraints
            .iter()
            .filter(|constraint| constraint.name.starts_with("channeling_"))
            .collect()
    }

    pub fn thermal_min_up_down(&self) -> Vec<&LinearConstraint> {
        self.linear_constraints
            .iter()
            .filter(|constraint| {
                constraint.name.starts_with("thermal_min_up[")
                    || constraint.name.starts_with("thermal_min_down[")
                    || constraint.name.starts_with("thermal_initial_")
            })
            .collect()
    }

    pub fn thermal_ramps(&self) -> Vec<&LinearConstraint> {
        self.linear_constraints
            .iter()
            .filter(|constraint| {
                constraint.name.starts_with("thermal_transition[")
                    || constraint
                        .name
                        .starts_with("thermal_startup_shutdown_exclusive[")
                    || constraint.name.starts_with("thermal_ramp_")
            })
            .collect()
    }

    pub fn hydro_balance(&self) -> Vec<&LinearConstraint> {
        self.linear_constraints
            .iter()
            .filter(|constraint| constraint.name.starts_with("hydro_balance["))
            .collect()
    }

    pub fn interchange_limits(&self) -> Vec<&LinearConstraint> {
        self.linear_constraints
            .iter()
            .filter(|constraint| constraint.name.starts_with("interchange_limit["))
            .collect()
    }

    pub fn hydro_turbining_limits(&self) -> Vec<&LinearConstraint> {
        self.linear_constraints
            .iter()
            .filter(|constraint| constraint.name.starts_with("hydro_turbining_"))
            .collect()
    }

    pub fn hydro_spillage_nonnegativity(&self) -> Vec<&LinearConstraint> {
        self.linear_constraints
            .iter()
            .filter(|constraint| constraint.name.starts_with("hydro_spillage_nonnegative["))
            .collect()
    }

    pub fn hydro_fpha(&self) -> Vec<&LinearConstraint> {
        self.linear_constraints
            .iter()
            .filter(|constraint| constraint.name.starts_with("hydro_fpha["))
            .collect()
    }

    pub fn hydro_generation_turbining_coupling(&self) -> Vec<&LinearConstraint> {
        self.linear_constraints
            .iter()
            .filter(|constraint| {
                constraint
                    .name
                    .starts_with("hydro_generation_turbining_coupling[")
            })
            .collect()
    }

    pub fn hydro_productivity(&self) -> Vec<&LinearConstraint> {
        self.linear_constraints
            .iter()
            .filter(|constraint| constraint.name.starts_with("hydro_productivity["))
            .collect()
    }

    pub fn future_cost(&self) -> Vec<&LinearConstraint> {
        self.linear_constraints
            .iter()
            .filter(|constraint| constraint.name.starts_with("future_cost_cut["))
            .collect()
    }

    pub fn operational_limits(&self) -> Vec<&LinearConstraint> {
        self.linear_constraints
            .iter()
            .filter(|constraint| constraint.name.starts_with("operational_limit_"))
            .collect()
    }
}

fn build_demand_balance_constraints(
    system: &System,
    indexing: &Indexing,
    variables: &Variables,
) -> Vec<LinearConstraint> {
    let horizon = system.horizon.periods;

    (0..system.submarkets.len())
        .flat_map(|submarket_idx| {
            let submarket = &system.submarkets[submarket_idx];

            (0..horizon).map(move |period| LinearConstraint {
                name: format!(
                    "demand_balance[submarket={},t={}]",
                    submarket.name,
                    display_period(period)
                ),
                terms: demand_balance_terms(system, indexing, variables, submarket_idx, period),
                sense: ConstraintSense::Equal,
                rhs: submarket.demand_mw[period],
            })
        })
        .collect()
}

fn build_interchange_limit_constraints(
    system: &System,
    indexing: &Indexing,
    variables: &Variables,
) -> Vec<LinearConstraint> {
    indexing
        .interchange_entries
        .iter()
        .enumerate()
        .filter_map(|(entry_idx, entry)| {
            let from = &system.submarkets[entry.from_submarket_idx];
            let to = &system.submarkets[entry.to_submarket_idx];
            let limit = system.interchange_limits.iter().find(|limit| {
                limit.from_submarket_id == from.id && limit.to_submarket_id == to.id
            })?;

            Some(LinearConstraint {
                name: format!(
                    "interchange_limit[from={},to={},t={}]",
                    from.name,
                    to.name,
                    display_period(entry.period)
                ),
                terms: vec![term(&variables.interchange[entry_idx], 1.0)],
                sense: ConstraintSense::LessOrEqual,
                rhs: limit.max_flow_mw,
            })
        })
        .collect()
}

fn build_future_cost_constraints(
    system: &System,
    indexing: &Indexing,
    variables: &Variables,
) -> Vec<LinearConstraint> {
    let Some(future_cost) = variables.future_cost.first() else {
        return Vec::new();
    };
    let horizon = system.horizon.periods;

    system
        .future_cost_cuts
        .iter()
        .filter(|cut| cut.is_active)
        .map(|cut| {
            let mut terms = vec![term(future_cost, 1.0)];
            let mut rhs = cut.intercept;

            for (plant_entry_idx, entry) in indexing.hydro_plant_entries.iter().enumerate() {
                let plant = &system.hydro_plants[entry.plant_idx];
                let coefficient = cut.coefficients[plant_entry_idx];
                if coefficient.abs() <= 1e-12 {
                    continue;
                }

                let final_volume =
                    &variables.hydro_volume[plant_entry_idx * (horizon + 1) + horizon];
                terms.push(term(final_volume, -coefficient));
                rhs -= coefficient * plant.reservoir.min_volume_hm3;
            }

            LinearConstraint {
                name: format!("future_cost_cut[stage={},cut={}]", cut.stage_id, cut.cut_id),
                terms,
                sense: ConstraintSense::GreaterOrEqual,
                rhs,
            }
        })
        .collect()
}

fn build_operational_limit_constraints(
    system: &System,
    indexing: &Indexing,
    variables: &Variables,
) -> Vec<LinearConstraint> {
    let horizon = system.horizon.periods;
    let mut constraints = Vec::new();

    for limit in &system.operational_limits {
        for period in limit.start_period - 1..=limit.end_period - 1 {
            let terms =
                operational_limit_terms(system, indexing, variables, limit, period, horizon);

            if let Some(lower_bound) = limit.lower_bound {
                let rhs = operational_limit_rhs(system, limit, lower_bound);
                constraints.push(LinearConstraint {
                    name: format!(
                        "operational_limit_lower[p={},var={},t={}]",
                        limit.plant_name,
                        operational_limit_variable_label(limit.variable),
                        display_period(period)
                    ),
                    terms: terms.clone(),
                    sense: ConstraintSense::GreaterOrEqual,
                    rhs,
                });
            }

            if let Some(upper_bound) = limit.upper_bound {
                let rhs = operational_limit_rhs(system, limit, upper_bound);
                constraints.push(LinearConstraint {
                    name: format!(
                        "operational_limit_upper[p={},var={},t={}]",
                        limit.plant_name,
                        operational_limit_variable_label(limit.variable),
                        display_period(period)
                    ),
                    terms: terms.clone(),
                    sense: ConstraintSense::LessOrEqual,
                    rhs,
                });
            }
        }
    }

    constraints
}

fn operational_limit_rhs(
    system: &System,
    limit: &labdessem_core::system::OperationalLimit,
    informed_bound: f64,
) -> f64 {
    match limit.variable {
        OperationalLimitVariable::Volume => {
            let OperationalLimitTarget::HydroPlant(plant_id) = limit.target else {
                return informed_bound;
            };
            let plant = system
                .hydro_plants
                .iter()
                .find(|plant| plant.id == plant_id)
                .expect("validated hydro plant should exist");
            informed_bound + plant.reservoir.min_volume_hm3
        }
        OperationalLimitVariable::Spillage
        | OperationalLimitVariable::Defluence
        | OperationalLimitVariable::Turbining
        | OperationalLimitVariable::Diversion
        | OperationalLimitVariable::Pumping => {
            flow_m3s_to_hm3(informed_bound, system.horizon.period_duration_hours)
        }
        OperationalLimitVariable::Generation => informed_bound,
    }
}

fn operational_limit_variable_label(variable: OperationalLimitVariable) -> &'static str {
    match variable {
        OperationalLimitVariable::Generation => "GER",
        OperationalLimitVariable::Spillage => "VERT",
        OperationalLimitVariable::Volume => "VOL",
        OperationalLimitVariable::Defluence => "DEFLU",
        OperationalLimitVariable::Turbining => "TURB",
        OperationalLimitVariable::Diversion => "QDES",
        OperationalLimitVariable::Pumping => "QBOM",
    }
}

fn operational_limit_terms(
    system: &System,
    indexing: &Indexing,
    variables: &Variables,
    limit: &labdessem_core::system::OperationalLimit,
    period: usize,
    horizon: usize,
) -> Vec<LinearTerm> {
    match (limit.target, limit.variable) {
        (OperationalLimitTarget::ThermalPlant(plant_id), OperationalLimitVariable::Generation) => {
            indexing
                .thermal_unit_entries
                .iter()
                .enumerate()
                .filter(|(_, entry)| system.thermal_plants[entry.plant_idx].id == plant_id)
                .map(|(entry_idx, _)| {
                    term(
                        &variables.thermal_generation[entry_idx * horizon + period],
                        1.0,
                    )
                })
                .collect()
        }
        (OperationalLimitTarget::HydroPlant(plant_id), OperationalLimitVariable::Generation) => {
            indexing
                .hydro_unit_entries
                .iter()
                .enumerate()
                .filter(|(_, entry)| system.hydro_plants[entry.plant_idx].id == plant_id)
                .map(|(entry_idx, _)| {
                    term(
                        &variables.hydro_generation[entry_idx * horizon + period],
                        1.0,
                    )
                })
                .collect()
        }
        (OperationalLimitTarget::HydroPlant(plant_id), OperationalLimitVariable::Spillage) => {
            let plant_entry_idx = indexing
                .hydro_plant_entries
                .iter()
                .position(|entry| system.hydro_plants[entry.plant_idx].id == plant_id)
                .expect("validated hydro plant should exist");
            vec![term(
                &variables.hydro_spillage[plant_entry_idx * horizon + period],
                1.0,
            )]
        }
        (OperationalLimitTarget::HydroPlant(plant_id), OperationalLimitVariable::Volume) => {
            let plant_entry_idx = indexing
                .hydro_plant_entries
                .iter()
                .position(|entry| system.hydro_plants[entry.plant_idx].id == plant_id)
                .expect("validated hydro plant should exist");
            vec![term(
                &variables.hydro_volume[plant_entry_idx * (horizon + 1) + period + 1],
                1.0,
            )]
        }
        (OperationalLimitTarget::HydroPlant(plant_id), OperationalLimitVariable::Turbining) => {
            indexing
                .hydro_unit_entries
                .iter()
                .enumerate()
                .filter(|(_, entry)| system.hydro_plants[entry.plant_idx].id == plant_id)
                .map(|(entry_idx, _)| {
                    term(
                        &variables.hydro_turbining[entry_idx * horizon + period],
                        1.0,
                    )
                })
                .collect()
        }
        (OperationalLimitTarget::HydroPlant(plant_id), OperationalLimitVariable::Defluence) => {
            let mut terms: Vec<LinearTerm> = indexing
                .hydro_unit_entries
                .iter()
                .enumerate()
                .filter(|(_, entry)| system.hydro_plants[entry.plant_idx].id == plant_id)
                .map(|(entry_idx, _)| {
                    term(
                        &variables.hydro_turbining[entry_idx * horizon + period],
                        1.0,
                    )
                })
                .collect();
            let plant_entry_idx = indexing
                .hydro_plant_entries
                .iter()
                .position(|entry| system.hydro_plants[entry.plant_idx].id == plant_id)
                .expect("validated hydro plant should exist");
            terms.push(term(
                &variables.hydro_spillage[plant_entry_idx * horizon + period],
                1.0,
            ));
            terms
        }
        (OperationalLimitTarget::HydroPlant(plant_id), OperationalLimitVariable::Diversion) => {
            let plant_entry_idx = indexing
                .hydro_plant_entries
                .iter()
                .position(|entry| system.hydro_plants[entry.plant_idx].id == plant_id)
                .expect("validated hydro plant should exist");
            vec![term(
                &variables.hydro_diversion[plant_entry_idx * horizon + period],
                1.0,
            )]
        }
        (OperationalLimitTarget::PumpingPlant(plant_id), OperationalLimitVariable::Pumping) => {
            let plant_entry_idx = indexing
                .pumping_plant_entries
                .iter()
                .position(|entry| system.pumping_plants[entry.plant_idx].id == plant_id)
                .expect("validated pumping plant should exist");
            vec![term(
                &variables.pumping[plant_entry_idx * horizon + period],
                1.0,
            )]
        }
        _ => unreachable!("validated operational limits should be compatible with target"),
    }
}

fn flow_m3s_to_hm3(flow_m3s: f64, period_duration_hours: f64) -> f64 {
    flow_m3s * period_duration_hours * 0.0036
}

fn demand_balance_terms(
    system: &System,
    indexing: &Indexing,
    variables: &Variables,
    submarket_idx: usize,
    period: usize,
) -> Vec<LinearTerm> {
    let horizon = system.horizon.periods;
    let mut terms = Vec::new();

    for (entry_idx, entry) in indexing.thermal_unit_entries.iter().enumerate() {
        if entry.submarket_idx == submarket_idx {
            let variable = &variables.thermal_generation[entry_idx * horizon + period];
            terms.push(term(variable, 1.0));
        }
    }

    for (entry_idx, entry) in indexing.hydro_unit_entries.iter().enumerate() {
        if entry.submarket_idx == submarket_idx {
            let variable = &variables.hydro_generation[entry_idx * horizon + period];
            terms.push(term(variable, 1.0));
        }
    }

    for (entry_idx, entry) in indexing.renewable_plant_entries.iter().enumerate() {
        if entry.submarket_idx == submarket_idx {
            let variable = &variables.renewable_generation[entry_idx * horizon + period];
            terms.push(term(variable, 1.0));
        }
    }

    for (entry_idx, entry) in indexing.interchange_entries.iter().enumerate() {
        if entry.period != period {
            continue;
        }

        let variable = &variables.interchange[entry_idx];
        if entry.to_submarket_idx == submarket_idx {
            terms.push(term(variable, 1.0));
        }
        if entry.from_submarket_idx == submarket_idx {
            terms.push(term(variable, -1.0));
        }
    }

    let flow_conversion = system.horizon.period_duration_hours * 0.0036;
    for (entry_idx, entry) in indexing.pumping_plant_entries.iter().enumerate() {
        if entry.submarket_idx == submarket_idx {
            let plant = &system.pumping_plants[entry.plant_idx];
            let variable = &variables.pumping[entry_idx * horizon + period];
            terms.push(term(
                variable,
                -(plant.specific_consumption_mw_per_m3s / flow_conversion),
            ));
        }
    }

    let deficit = &variables.deficit[submarket_idx * horizon + period];
    terms.push(term(deficit, 1.0));

    terms
}

pub(crate) fn term(variable: &Variable, coefficient: f64) -> LinearTerm {
    LinearTerm {
        variable: variable.name.clone(),
        coefficient,
    }
}

pub(crate) fn display_period(period: usize) -> usize {
    period + 1
}
