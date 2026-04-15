use crate::{
    SolveMode,
    indexing::Indexing,
    variables::{Variable, Variables},
};
use labdessem_core::system::{OperationalLimitTarget, OperationalLimitVariable, System};

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
        linear_constraints.extend(build_hydro_fpha_constraints(system, indexing, variables));
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
                linear_constraints.extend(build_hydro_min_up_down_constraints(
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

    pub fn hydro_min_up_down(&self) -> Vec<&LinearConstraint> {
        self.linear_constraints
            .iter()
            .filter(|constraint| {
                constraint.name.starts_with("hydro_min_up[")
                    || constraint.name.starts_with("hydro_min_down[")
                    || constraint.name.starts_with("hydro_initial_")
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

                let diversion = &variables.hydro_diversion[plant_entry_idx * horizon + period];
                terms.push(term(diversion, 1.0));

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

                for upstream_id in &plant.diversion_upstream_plant_ids {
                    if let Some(upstream_entry_idx) = indexing
                        .hydro_plant_entries
                        .iter()
                        .position(|entry| system.hydro_plants[entry.plant_idx].id == *upstream_id)
                    {
                        let upstream_diversion =
                            &variables.hydro_diversion[upstream_entry_idx * horizon + period];
                        terms.push(term(upstream_diversion, -1.0));
                    }
                }

                for (pumping_entry_idx, pumping_entry) in
                    indexing.pumping_plant_entries.iter().enumerate()
                {
                    let pumping_plant = &system.pumping_plants[pumping_entry.plant_idx];
                    let pumping_variable = &variables.pumping[pumping_entry_idx * horizon + period];

                    if pumping_plant.downstream_hydro_id == plant.id {
                        terms.push(term(pumping_variable, 1.0));
                    }
                    if pumping_plant.upstream_hydro_id == plant.id {
                        terms.push(term(pumping_variable, -1.0));
                    }
                }

                LinearConstraint {
                    name: format!(
                        "hydro_balance[p={},t={}]",
                        plant.name,
                        display_period(period)
                    ),
                    terms,
                    sense: ConstraintSense::Equal,
                    rhs: plant.natural_inflow_hm3[period] - plant.water_withdrawal_hm3[period],
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

fn build_hydro_fpha_constraints(
    system: &System,
    indexing: &Indexing,
    variables: &Variables,
) -> Vec<LinearConstraint> {
    let horizon = system.horizon.periods;
    let flow_conversion = system.horizon.period_duration_hours * 0.0036;

    indexing
        .hydro_plant_entries
        .iter()
        .enumerate()
        .flat_map(|(plant_entry_idx, entry)| {
            let plant = &system.hydro_plants[entry.plant_idx];

            (0..horizon).flat_map(move |period| {
                plant.fpha_segments.iter().map(move |segment| {
                    let mut terms = Vec::new();

                    for (unit_entry_idx, unit_entry) in
                        indexing.hydro_unit_entries.iter().enumerate()
                    {
                        if unit_entry.plant_idx == entry.plant_idx {
                            terms.push(term(
                                &variables.hydro_generation[unit_entry_idx * horizon + period],
                                1.0,
                            ));
                            terms.push(term(
                                &variables.hydro_turbining[unit_entry_idx * horizon + period],
                                -(segment.correction_factor * segment.turbining_coefficient
                                    / flow_conversion),
                            ));
                        }
                    }

                    terms.push(term(
                        &variables.hydro_volume[plant_entry_idx * (horizon + 1) + period + 1],
                        -(segment.correction_factor * segment.volume_coefficient),
                    ));
                    terms.push(term(
                        &variables.hydro_spillage[plant_entry_idx * horizon + period],
                        -(segment.correction_factor * segment.lateral_flow_coefficient
                            / flow_conversion),
                    ));

                    LinearConstraint {
                        name: format!(
                            "hydro_fpha[p={},seg={},t={}]",
                            plant.name,
                            segment.segment,
                            display_period(period)
                        ),
                        terms,
                        sense: ConstraintSense::LessOrEqual,
                        rhs: segment.correction_factor * segment.rhs,
                    }
                })
            })
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
                constraints.push(LinearConstraint {
                    name: format!(
                        "operational_limit_lower[p={},var={},t={}]",
                        limit.plant_name,
                        operational_limit_variable_label(limit.variable),
                        display_period(period)
                    ),
                    terms: terms.clone(),
                    sense: ConstraintSense::GreaterOrEqual,
                    rhs: lower_bound,
                });
            }

            if let Some(upper_bound) = limit.upper_bound {
                constraints.push(LinearConstraint {
                    name: format!(
                        "operational_limit_upper[p={},var={},t={}]",
                        limit.plant_name,
                        operational_limit_variable_label(limit.variable),
                        display_period(period)
                    ),
                    terms: terms.clone(),
                    sense: ConstraintSense::LessOrEqual,
                    rhs: upper_bound,
                });
            }
        }
    }

    constraints
}

fn operational_limit_variable_label(variable: OperationalLimitVariable) -> &'static str {
    match variable {
        OperationalLimitVariable::Generation => "GER",
        OperationalLimitVariable::Spillage => "VERT",
        OperationalLimitVariable::Volume => "VOL",
        OperationalLimitVariable::Defluence => "DEFLU",
        OperationalLimitVariable::Turbining => "TURB",
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
        _ => unreachable!("validated operational limits should be compatible with target"),
    }
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

fn term(variable: &Variable, coefficient: f64) -> LinearTerm {
    LinearTerm {
        variable: variable.name.clone(),
        coefficient,
    }
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

fn build_thermal_ramp_constraints(
    system: &System,
    indexing: &Indexing,
    variables: &Variables,
) -> Vec<LinearConstraint> {
    let horizon = system.horizon.periods;
    let mut constraints = Vec::new();

    for (entry_idx, entry) in indexing.thermal_unit_entries.iter().enumerate() {
        let plant = &system.thermal_plants[entry.plant_idx];
        let unit = &plant.units[entry.unit_idx];
        let initial_status = if unit.initial_condition.is_on {
            1.0
        } else {
            0.0
        };
        let initial_startup_remaining = if unit.initial_condition.is_ramping_up {
            unit.initial_startup_remaining_trajectory()
                .expect("validated thermal startup trajectory should be consistent")
        } else {
            Vec::new()
        };
        let initial_shutdown_remaining = if unit.initial_condition.is_ramping_down {
            unit.initial_shutdown_remaining_trajectory()
                .expect("validated thermal shutdown trajectory should be consistent")
        } else {
            Vec::new()
        };

        for period in 0..horizon {
            let on = &variables.thermal_commitment[entry_idx * horizon + period];
            let startup = &variables.thermal_startup[entry_idx * horizon + period];
            let shutdown = &variables.thermal_shutdown[entry_idx * horizon + period];

            let mut transition_terms =
                vec![term(on, 1.0), term(startup, -1.0), term(shutdown, 1.0)];
            let transition_rhs = if period == 0 {
                initial_status
            } else {
                let previous_on = &variables.thermal_commitment[entry_idx * horizon + period - 1];
                transition_terms.push(term(previous_on, -1.0));
                0.0
            };

            constraints.push(LinearConstraint {
                name: format!(
                    "thermal_transition[p={},u={},t={}]",
                    plant.name,
                    unit.name,
                    display_period(period)
                ),
                terms: transition_terms,
                sense: ConstraintSense::Equal,
                rhs: transition_rhs,
            });

            constraints.push(LinearConstraint {
                name: format!(
                    "thermal_startup_shutdown_exclusive[p={},u={},t={}]",
                    plant.name,
                    unit.name,
                    display_period(period)
                ),
                terms: vec![term(startup, 1.0), term(shutdown, 1.0)],
                sense: ConstraintSense::LessOrEqual,
                rhs: 1.0,
            });

            let mut startup_sum_terms = Vec::new();
            let mut startup_weighted_terms = Vec::new();
            for (k, startup_level) in unit.startup_trajectory_mw.iter().enumerate() {
                if period >= k {
                    let startup_var =
                        &variables.thermal_startup[entry_idx * horizon + (period - k)];
                    startup_sum_terms.push(term(startup_var, 1.0));
                    startup_weighted_terms.push(term(startup_var, *startup_level));
                }
            }

            let mut shutdown_sum_terms = Vec::new();
            let mut shutdown_weighted_terms = Vec::new();
            let shutdown_len = unit.shutdown_trajectory_mw.len();
            for k in 1..=shutdown_len {
                if period + k < horizon {
                    let shutdown_var =
                        &variables.thermal_shutdown[entry_idx * horizon + (period + k)];
                    shutdown_sum_terms.push(term(shutdown_var, 1.0));
                    shutdown_weighted_terms.push(term(
                        shutdown_var,
                        unit.shutdown_trajectory_mw[shutdown_len - k],
                    ));
                }
            }

            let initial_startup_active = if period < initial_startup_remaining.len() {
                1.0
            } else {
                0.0
            };
            let initial_startup_generation = initial_startup_remaining
                .get(period)
                .copied()
                .unwrap_or(0.0);
            let initial_shutdown_active = if period < initial_shutdown_remaining.len() {
                1.0
            } else {
                0.0
            };
            let initial_shutdown_generation = initial_shutdown_remaining
                .get(period)
                .copied()
                .unwrap_or(0.0);

            let generation = &variables.thermal_generation[entry_idx * horizon + period];
            let mut lower_terms = vec![term(generation, 1.0), term(on, -unit.min_generation_mw)];
            let mut upper_terms = vec![term(generation, 1.0), term(on, -unit.max_generation_mw)];

            for startup_term in &startup_sum_terms {
                lower_terms.push(LinearTerm {
                    variable: startup_term.variable.clone(),
                    coefficient: unit.min_generation_mw,
                });
                upper_terms.push(LinearTerm {
                    variable: startup_term.variable.clone(),
                    coefficient: unit.max_generation_mw,
                });
            }
            for shutdown_term in &shutdown_sum_terms {
                lower_terms.push(LinearTerm {
                    variable: shutdown_term.variable.clone(),
                    coefficient: unit.min_generation_mw,
                });
                upper_terms.push(LinearTerm {
                    variable: shutdown_term.variable.clone(),
                    coefficient: unit.max_generation_mw,
                });
            }
            for startup_term in &startup_weighted_terms {
                lower_terms.push(LinearTerm {
                    variable: startup_term.variable.clone(),
                    coefficient: -startup_term.coefficient,
                });
                upper_terms.push(LinearTerm {
                    variable: startup_term.variable.clone(),
                    coefficient: -startup_term.coefficient,
                });
            }
            for shutdown_term in &shutdown_weighted_terms {
                lower_terms.push(LinearTerm {
                    variable: shutdown_term.variable.clone(),
                    coefficient: -shutdown_term.coefficient,
                });
                upper_terms.push(LinearTerm {
                    variable: shutdown_term.variable.clone(),
                    coefficient: -shutdown_term.coefficient,
                });
            }

            let lower_rhs = -unit.min_generation_mw
                * (initial_startup_active + initial_shutdown_active)
                + initial_startup_generation
                + initial_shutdown_generation;
            let upper_rhs = -unit.max_generation_mw
                * (initial_startup_active + initial_shutdown_active)
                + initial_startup_generation
                + initial_shutdown_generation;

            constraints.push(LinearConstraint {
                name: format!(
                    "thermal_ramp_lower[p={},u={},t={}]",
                    plant.name,
                    unit.name,
                    display_period(period)
                ),
                terms: lower_terms,
                sense: ConstraintSense::GreaterOrEqual,
                rhs: lower_rhs,
            });

            constraints.push(LinearConstraint {
                name: format!(
                    "thermal_ramp_upper[p={},u={},t={}]",
                    plant.name,
                    unit.name,
                    display_period(period)
                ),
                terms: upper_terms,
                sense: ConstraintSense::LessOrEqual,
                rhs: upper_rhs,
            });
        }
    }

    constraints
}

fn build_thermal_min_up_down_constraints(
    system: &System,
    indexing: &Indexing,
    variables: &Variables,
) -> Vec<LinearConstraint> {
    let horizon = system.horizon.periods;
    let mut constraints = Vec::new();

    for (entry_idx, entry) in indexing.thermal_unit_entries.iter().enumerate() {
        let plant = &system.thermal_plants[entry.plant_idx];
        let unit = &plant.units[entry.unit_idx];
        let initial_on = unit.initial_condition.is_on;
        let initial_status = if initial_on { 1.0 } else { 0.0 };

        let remaining_on = if initial_on {
            unit.min_up_time
                .saturating_sub(unit.initial_condition.time_in_state)
        } else {
            0
        };
        let remaining_off = if !initial_on {
            unit.min_down_time
                .saturating_sub(unit.initial_condition.time_in_state)
        } else {
            0
        };

        for period in 0..remaining_on.min(horizon) {
            let on = &variables.thermal_commitment[entry_idx * horizon + period];
            constraints.push(LinearConstraint {
                name: format!(
                    "thermal_initial_on_fix[p={},u={},t={}]",
                    plant.name,
                    unit.name,
                    display_period(period)
                ),
                terms: vec![term(on, 1.0)],
                sense: ConstraintSense::Equal,
                rhs: 1.0,
            });
        }

        for period in 0..remaining_off.min(horizon) {
            let on = &variables.thermal_commitment[entry_idx * horizon + period];
            constraints.push(LinearConstraint {
                name: format!(
                    "thermal_initial_off_fix[p={},u={},t={}]",
                    plant.name,
                    unit.name,
                    display_period(period)
                ),
                terms: vec![term(on, 1.0)],
                sense: ConstraintSense::Equal,
                rhs: 0.0,
            });
        }

        for period in 0..horizon {
            let min_up_window = unit.min_up_time.min(horizon - period);
            let mut min_up_terms = Vec::new();
            for ts in period..(period + min_up_window) {
                let on = &variables.thermal_commitment[entry_idx * horizon + ts];
                min_up_terms.push(term(on, 1.0));
            }
            let current_on = &variables.thermal_commitment[entry_idx * horizon + period];
            min_up_terms.push(term(current_on, -(min_up_window as f64)));

            if period == 0 {
                constraints.push(LinearConstraint {
                    name: format!(
                        "thermal_min_up[p={},u={},t={}]",
                        plant.name,
                        unit.name,
                        display_period(period)
                    ),
                    terms: min_up_terms,
                    sense: ConstraintSense::GreaterOrEqual,
                    rhs: -(min_up_window as f64) * initial_status,
                });
            } else {
                let previous_on = &variables.thermal_commitment[entry_idx * horizon + period - 1];
                min_up_terms.push(term(previous_on, min_up_window as f64));
                constraints.push(LinearConstraint {
                    name: format!(
                        "thermal_min_up[p={},u={},t={}]",
                        plant.name,
                        unit.name,
                        display_period(period)
                    ),
                    terms: min_up_terms,
                    sense: ConstraintSense::GreaterOrEqual,
                    rhs: 0.0,
                });
            }

            let min_down_window = unit.min_down_time.min(horizon - period);
            let mut min_down_terms = Vec::new();
            for ts in period..(period + min_down_window) {
                let on = &variables.thermal_commitment[entry_idx * horizon + ts];
                min_down_terms.push(term(on, 1.0));
            }
            let current_on = &variables.thermal_commitment[entry_idx * horizon + period];
            min_down_terms.push(term(current_on, -(min_down_window as f64)));

            if period == 0 {
                constraints.push(LinearConstraint {
                    name: format!(
                        "thermal_min_down[p={},u={},t={}]",
                        plant.name,
                        unit.name,
                        display_period(period)
                    ),
                    terms: min_down_terms,
                    sense: ConstraintSense::LessOrEqual,
                    rhs: (min_down_window as f64) * (1.0 - initial_status),
                });
            } else {
                let previous_on = &variables.thermal_commitment[entry_idx * horizon + period - 1];
                min_down_terms.push(term(previous_on, min_down_window as f64));
                constraints.push(LinearConstraint {
                    name: format!(
                        "thermal_min_down[p={},u={},t={}]",
                        plant.name,
                        unit.name,
                        display_period(period)
                    ),
                    terms: min_down_terms,
                    sense: ConstraintSense::LessOrEqual,
                    rhs: min_down_window as f64,
                });
            }
        }
    }

    constraints
}

fn build_hydro_min_up_down_constraints(
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
        let initial_on = unit.initial_condition.is_on;
        let initial_status = if initial_on { 1.0 } else { 0.0 };

        let remaining_on = if initial_on {
            unit.min_up_time
                .saturating_sub(unit.initial_condition.time_in_state)
        } else {
            0
        };
        let remaining_off = if !initial_on {
            unit.min_down_time
                .saturating_sub(unit.initial_condition.time_in_state)
        } else {
            0
        };

        for period in 0..remaining_on.min(horizon) {
            let on = &variables.hydro_commitment[entry_idx * horizon + period];
            constraints.push(LinearConstraint {
                name: format!(
                    "hydro_initial_on_fix[p={},g={},u={},t={}]",
                    plant.name,
                    group.name,
                    unit.name,
                    display_period(period)
                ),
                terms: vec![term(on, 1.0)],
                sense: ConstraintSense::Equal,
                rhs: 1.0,
            });
        }

        for period in 0..remaining_off.min(horizon) {
            let on = &variables.hydro_commitment[entry_idx * horizon + period];
            constraints.push(LinearConstraint {
                name: format!(
                    "hydro_initial_off_fix[p={},g={},u={},t={}]",
                    plant.name,
                    group.name,
                    unit.name,
                    display_period(period)
                ),
                terms: vec![term(on, 1.0)],
                sense: ConstraintSense::Equal,
                rhs: 0.0,
            });
        }

        for period in 0..horizon {
            let min_up_window = unit.min_up_time.min(horizon - period);
            let mut min_up_terms = Vec::new();
            for ts in period..(period + min_up_window) {
                let on = &variables.hydro_commitment[entry_idx * horizon + ts];
                min_up_terms.push(term(on, 1.0));
            }
            let current_on = &variables.hydro_commitment[entry_idx * horizon + period];
            min_up_terms.push(term(current_on, -(min_up_window as f64)));

            if period == 0 {
                constraints.push(LinearConstraint {
                    name: format!(
                        "hydro_min_up[p={},g={},u={},t={}]",
                        plant.name,
                        group.name,
                        unit.name,
                        display_period(period)
                    ),
                    terms: min_up_terms,
                    sense: ConstraintSense::GreaterOrEqual,
                    rhs: -(min_up_window as f64) * initial_status,
                });
            } else {
                let previous_on = &variables.hydro_commitment[entry_idx * horizon + period - 1];
                min_up_terms.push(term(previous_on, min_up_window as f64));
                constraints.push(LinearConstraint {
                    name: format!(
                        "hydro_min_up[p={},g={},u={},t={}]",
                        plant.name,
                        group.name,
                        unit.name,
                        display_period(period)
                    ),
                    terms: min_up_terms,
                    sense: ConstraintSense::GreaterOrEqual,
                    rhs: 0.0,
                });
            }

            let min_down_window = unit.min_down_time.min(horizon - period);
            let mut min_down_terms = Vec::new();
            for ts in period..(period + min_down_window) {
                let on = &variables.hydro_commitment[entry_idx * horizon + ts];
                min_down_terms.push(term(on, 1.0));
            }
            let current_on = &variables.hydro_commitment[entry_idx * horizon + period];
            min_down_terms.push(term(current_on, -(min_down_window as f64)));

            if period == 0 {
                constraints.push(LinearConstraint {
                    name: format!(
                        "hydro_min_down[p={},g={},u={},t={}]",
                        plant.name,
                        group.name,
                        unit.name,
                        display_period(period)
                    ),
                    terms: min_down_terms,
                    sense: ConstraintSense::LessOrEqual,
                    rhs: (min_down_window as f64) * (1.0 - initial_status),
                });
            } else {
                let previous_on = &variables.hydro_commitment[entry_idx * horizon + period - 1];
                min_down_terms.push(term(previous_on, min_down_window as f64));
                constraints.push(LinearConstraint {
                    name: format!(
                        "hydro_min_down[p={},g={},u={},t={}]",
                        plant.name,
                        group.name,
                        unit.name,
                        display_period(period)
                    ),
                    terms: min_down_terms,
                    sense: ConstraintSense::LessOrEqual,
                    rhs: min_down_window as f64,
                });
            }
        }
    }

    constraints
}

fn display_period(period: usize) -> usize {
    period + 1
}
