use crate::{
    SolveMode,
    indexing::Indexing,
    variables::{Variable, Variables},
};
use labdessem_core::system::System;

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
        linear_constraints.extend(build_hydro_productivity_constraints(
            system, indexing, variables,
        ));

        if matches!(
            solve_mode,
            SolveMode::MixedIntegerLinearProgramming
                | SolveMode::LinearProgrammingWithFixedCommitment
        ) {
            linear_constraints.extend(build_thermal_commitment_channeling_constraints(
                system, indexing, variables,
            ));
            linear_constraints.extend(build_hydro_commitment_channeling_constraints(
                system, indexing, variables,
            ));
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
        ) {
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

    pub fn hydro_productivity(&self) -> Vec<&LinearConstraint> {
        self.linear_constraints
            .iter()
            .filter(|constraint| constraint.name.starts_with("hydro_productivity["))
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

fn build_hydro_balance_constraints(
    system: &System,
    indexing: &Indexing,
    variables: &Variables,
) -> Vec<LinearConstraint> {
    let horizon = system.horizon.periods;

    indexing
        .hydro_plant_entries
        .iter()
        .enumerate()
        .flat_map(|(plant_entry_idx, plant_entry)| {
            let plant = &system.hydro_plants[plant_entry.plant_idx];

            (0..horizon).map(move |period| {
                let mut terms = Vec::new();

                let volume_offset = plant_entry_idx * (horizon + 1);
                let previous_volume = &variables.hydro_volume[volume_offset + period];
                let current_volume = &variables.hydro_volume[volume_offset + period + 1];
                terms.push(term(current_volume, 1.0));
                terms.push(term(previous_volume, -1.0));

                for (unit_entry_idx, unit_entry) in indexing.hydro_unit_entries.iter().enumerate() {
                    if unit_entry.plant_idx == plant_entry.plant_idx {
                        let turbining =
                            &variables.hydro_turbining[unit_entry_idx * horizon + period];
                        terms.push(term(turbining, 1.0));
                    }
                }

                let spillage = &variables.hydro_spillage[plant_entry_idx * horizon + period];
                terms.push(term(spillage, 1.0));

                for upstream_id in &plant.upstream_plant_ids {
                    if let Some(upstream_entry_idx) = indexing
                        .hydro_plant_entries
                        .iter()
                        .position(|entry| system.hydro_plants[entry.plant_idx].id == *upstream_id)
                    {
                        for (unit_entry_idx, unit_entry) in
                            indexing.hydro_unit_entries.iter().enumerate()
                        {
                            if unit_entry.plant_idx
                                == indexing.hydro_plant_entries[upstream_entry_idx].plant_idx
                            {
                                let upstream_turbining =
                                    &variables.hydro_turbining[unit_entry_idx * horizon + period];
                                terms.push(term(upstream_turbining, -1.0));
                            }
                        }

                        let upstream_spillage =
                            &variables.hydro_spillage[upstream_entry_idx * horizon + period];
                        terms.push(term(upstream_spillage, -1.0));
                    }
                }

                LinearConstraint {
                    name: format!("hydro_balance[p={},t={}]", plant.name, display_period(period)),
                    terms,
                    sense: ConstraintSense::Equal,
                    rhs: plant.natural_inflow_hm3[period],
                }
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

fn build_hydro_turbining_limit_constraints(
    system: &System,
    indexing: &Indexing,
    variables: &Variables,
) -> Vec<LinearConstraint> {
    let horizon = system.horizon.periods;
    let mut constraints = Vec::new();

    for (entry_idx, entry) in indexing.hydro_unit_entries.iter().enumerate() {
        let plant = &system.hydro_plants[entry.plant_idx];
        let group = &plant.groups[entry.group_idx];
        let unit = &group.units[entry.unit_idx];

        for period in 0..horizon {
            let turbining = &variables.hydro_turbining[entry_idx * horizon + period];

            constraints.push(LinearConstraint {
                name: format!(
                    "hydro_turbining_upper[p={},g={},u={},t={}]",
                    plant.name,
                    group.name,
                    unit.name,
                    display_period(period)
                ),
                terms: vec![term(turbining, 1.0)],
                sense: ConstraintSense::LessOrEqual,
                rhs: unit.max_turbining_hm3,
            });

            constraints.push(LinearConstraint {
                name: format!(
                    "hydro_turbining_lower[p={},g={},u={},t={}]",
                    plant.name,
                    group.name,
                    unit.name,
                    display_period(period)
                ),
                terms: vec![term(turbining, 1.0)],
                sense: ConstraintSense::GreaterOrEqual,
                rhs: 0.0,
            });
        }
    }

    constraints
}

fn build_hydro_spillage_nonnegativity_constraints(
    indexing: &Indexing,
    variables: &Variables,
    system: &System,
) -> Vec<LinearConstraint> {
    let horizon = system.horizon.periods;

    indexing
        .hydro_plant_entries
        .iter()
        .enumerate()
        .flat_map(|(entry_idx, entry)| {
            let plant = &system.hydro_plants[entry.plant_idx];
            (0..horizon).map(move |period| LinearConstraint {
                name: format!(
                    "hydro_spillage_nonnegative[p={},t={}]",
                    plant.name,
                    display_period(period)
                ),
                terms: vec![term(
                    &variables.hydro_spillage[entry_idx * horizon + period],
                    1.0,
                )],
                sense: ConstraintSense::GreaterOrEqual,
                rhs: 0.0,
            })
        })
        .collect()
}

fn build_hydro_productivity_constraints(
    system: &System,
    indexing: &Indexing,
    variables: &Variables,
) -> Vec<LinearConstraint> {
    let horizon = system.horizon.periods;

    indexing
        .hydro_unit_entries
        .iter()
        .enumerate()
        .flat_map(|(entry_idx, entry)| {
            let plant = &system.hydro_plants[entry.plant_idx];
            let group = &plant.groups[entry.group_idx];
            let unit = &group.units[entry.unit_idx];

            (0..horizon).map(move |period| LinearConstraint {
                name: format!(
                    "hydro_productivity[p={},g={},u={},t={}]",
                    plant.name,
                    group.name,
                    unit.name,
                    display_period(period)
                ),
                terms: vec![
                    term(
                        &variables.hydro_generation[entry_idx * horizon + period],
                        1.0,
                    ),
                    term(
                        &variables.hydro_turbining[entry_idx * horizon + period],
                        -unit.productivity_mw_per_hm3,
                    ),
                ],
                sense: ConstraintSense::Equal,
                rhs: 0.0,
            })
        })
        .collect()
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

    for (entry_idx, entry) in indexing.wind_plant_entries.iter().enumerate() {
        if entry.submarket_idx == submarket_idx {
            let variable = &variables.wind_generation[entry_idx * horizon + period];
            terms.push(term(variable, 1.0));
        }
    }

    for (entry_idx, entry) in indexing.solar_plant_entries.iter().enumerate() {
        if entry.submarket_idx == submarket_idx {
            let variable = &variables.solar_generation[entry_idx * horizon + period];
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

    let deficit = &variables.deficit[submarket_idx * horizon + period];
    terms.push(term(deficit, 1.0));

    terms
}

fn term(variable: &Variable, coefficient: f64) -> LinearTerm {
    LinearTerm {
        variable: variable.name.clone(),
        coefficient,
    }
}

fn build_thermal_commitment_channeling_constraints(
    system: &System,
    indexing: &Indexing,
    variables: &Variables,
) -> Vec<LinearConstraint> {
    let horizon = system.horizon.periods;
    let mut constraints = Vec::new();

    for (entry_idx, entry) in indexing.thermal_unit_entries.iter().enumerate() {
        let plant = &system.thermal_plants[entry.plant_idx];
        let unit = &plant.units[entry.unit_idx];

        for period in 0..horizon {
            let generation = &variables.thermal_generation[entry_idx * horizon + period];
            let on = &variables.thermal_commitment[entry_idx * horizon + period];

            constraints.push(LinearConstraint {
                name: format!(
                    "channeling_thermal_upper[p={},u={},t={}]",
                    plant.name,
                    unit.name,
                    display_period(period)
                ),
                terms: vec![term(generation, 1.0), term(on, -unit.max_generation_mw)],
                sense: ConstraintSense::LessOrEqual,
                rhs: 0.0,
            });

            constraints.push(LinearConstraint {
                name: format!(
                    "channeling_thermal_lower[p={},u={},t={}]",
                    plant.name,
                    unit.name,
                    display_period(period)
                ),
                terms: vec![term(generation, 1.0), term(on, -unit.min_generation_mw)],
                sense: ConstraintSense::GreaterOrEqual,
                rhs: 0.0,
            });
        }
    }

    constraints
}

fn build_hydro_commitment_channeling_constraints(
    system: &System,
    indexing: &Indexing,
    variables: &Variables,
) -> Vec<LinearConstraint> {
    let horizon = system.horizon.periods;
    let mut constraints = Vec::new();

    for (entry_idx, entry) in indexing.hydro_unit_entries.iter().enumerate() {
        let plant = &system.hydro_plants[entry.plant_idx];
        let group = &plant.groups[entry.group_idx];
        let unit = &group.units[entry.unit_idx];

        for period in 0..horizon {
            let generation = &variables.hydro_generation[entry_idx * horizon + period];
            let on = &variables.hydro_commitment[entry_idx * horizon + period];

            constraints.push(LinearConstraint {
                name: format!(
                    "channeling_hydro_upper[p={},g={},u={},t={}]",
                    plant.name,
                    group.name,
                    unit.name,
                    display_period(period)
                ),
                terms: vec![term(generation, 1.0), term(on, -unit.max_generation_mw)],
                sense: ConstraintSense::LessOrEqual,
                rhs: 0.0,
            });

            constraints.push(LinearConstraint {
                name: format!(
                    "channeling_hydro_lower[p={},g={},u={},t={}]",
                    plant.name,
                    group.name,
                    unit.name,
                    display_period(period)
                ),
                terms: vec![term(generation, 1.0), term(on, -unit.min_generation_mw)],
                sense: ConstraintSense::GreaterOrEqual,
                rhs: 0.0,
            });
        }
    }

    constraints
}

fn display_period(period: usize) -> usize {
    period + 1
}
