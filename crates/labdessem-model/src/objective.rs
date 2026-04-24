use crate::{
    SolveMode,
    indexing::Indexing,
    variables::{Variable, Variables},
};
use labdessem_core::system::System;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ObjectiveSense {
    Minimize,
}

#[derive(Debug, Clone, PartialEq)]
pub struct ObjectiveTerm {
    pub variable: String,
    pub coefficient: f64,
}

#[derive(Debug, Clone, PartialEq)]
pub struct Objective {
    pub sense: ObjectiveSense,
    pub terms: Vec<ObjectiveTerm>,
}

impl Objective {
    pub fn for_system(
        system: &System,
        indexing: &Indexing,
        variables: &Variables,
        solve_mode: SolveMode,
    ) -> Self {
        let mut terms = Vec::new();
        let period_duration_hours = system.horizon.period_duration_hours;
        let horizon = system.horizon.periods;

        for (entry_idx, entry) in indexing.thermal_unit_entries.iter().enumerate() {
            let plant = &system.thermal_plants[entry.plant_idx];
            let unit = &plant.units[entry.unit_idx];

            for period in 0..horizon {
                let generation = &variables.thermal_generation[entry_idx * horizon + period];
                terms.push(term(
                    generation,
                    unit.variable_cost_per_mwh * period_duration_hours,
                ));
            }
        }

        for submarket_idx in 0..system.submarkets.len() {
            let submarket = &system.submarkets[submarket_idx];
            for period in 0..horizon {
                let deficit = &variables.deficit[submarket_idx * horizon + period];
                terms.push(term(
                    deficit,
                    submarket.deficit_cost_per_mwh * period_duration_hours,
                ));
            }
        }

        for (entry_idx, entry) in indexing.hydro_plant_entries.iter().enumerate() {
            let plant = &system.hydro_plants[entry.plant_idx];

            for period in 0..horizon {
                let spillage = &variables.hydro_spillage[entry_idx * horizon + period];
                terms.push(term(spillage, plant.spillage_cost_per_hm3));
            }
        }

        for (entry_idx, entry) in indexing.hydro_unit_entries.iter().enumerate() {
            let plant = &system.hydro_plants[entry.plant_idx];

            for period in 0..horizon {
                let turbining = &variables.hydro_turbining[entry_idx * horizon + period];
                terms.push(term(turbining, plant.turbining_cost_per_hm3));
            }
        }

        for (entry_idx, interchange) in indexing.interchange_entries.iter().enumerate() {
            let from = system.submarkets[interchange.from_submarket_idx].id;
            let to = system.submarkets[interchange.to_submarket_idx].id;

            if let Some(limit) = system
                .interchange_limits
                .iter()
                .find(|limit| limit.from_submarket_id == from && limit.to_submarket_id == to)
            {
                terms.push(term(
                    &variables.interchange[entry_idx],
                    limit.penalty_cost_per_mwh * period_duration_hours,
                ));
            }
        }

        if matches!(
            solve_mode,
            SolveMode::MixedIntegerLinearProgramming
                | SolveMode::LinearProgrammingWithFixedCommitment
        ) {
            if system.thermal_unit_commitment_enabled {
                for (entry_idx, entry) in indexing.thermal_unit_entries.iter().enumerate() {
                    let plant = &system.thermal_plants[entry.plant_idx];
                    let unit = &plant.units[entry.unit_idx];

                    for period in 0..horizon {
                        let startup = &variables.thermal_startup[entry_idx * horizon + period];
                        let shutdown = &variables.thermal_shutdown[entry_idx * horizon + period];
                        terms.push(term(startup, unit.startup_cost));
                        terms.push(term(shutdown, unit.shutdown_cost));
                        if system.ton_residual_enabled {
                            let cmo = system.residual_costs.iter().find_map(|residual_cost| {
                                (residual_cost.submarket_id == plant.submarket_id)
                                    .then_some(residual_cost.cmo_per_mwh)
                            });
                            if let Some(cmo_per_mwh) = cmo {
                                let residual_generation_mw = thermal_post_horizon_generation(unit);
                                let residual_cost_per_mwh =
                                    (unit.variable_cost_per_mwh - cmo_per_mwh).max(0.0);
                                terms.push(term(
                                    startup,
                                    residual_generation_mw
                                        * residual_cost_per_mwh
                                        * period_duration_hours,
                                ));
                            }
                        }
                    }
                }
            }

            if system.hydro_unit_commitment_enabled {
                for (entry_idx, entry) in indexing.hydro_unit_entries.iter().enumerate() {
                    let plant = &system.hydro_plants[entry.plant_idx];
                    let group = &plant.groups[entry.group_idx];
                    let unit = &group.units[entry.unit_idx];

                    for period in 0..horizon {
                        let startup = &variables.hydro_startup[entry_idx * horizon + period];
                        let shutdown = &variables.hydro_shutdown[entry_idx * horizon + period];
                        terms.push(term(startup, unit.startup_cost));
                        terms.push(term(shutdown, unit.shutdown_cost));
                    }
                }
            }
        }

        Self {
            sense: ObjectiveSense::Minimize,
            terms,
        }
    }
}

fn thermal_post_horizon_generation(unit: &labdessem_core::thermal::ThermalUnit) -> f64 {
    let startup_sum: f64 = unit.startup_trajectory_mw.iter().sum();
    let steady_periods = unit
        .min_up_time
        .saturating_sub(unit.startup_trajectory_mw.len());
    let steady_sum = steady_periods as f64 * unit.min_generation_mw;
    let shutdown_sum: f64 = unit.shutdown_trajectory_mw.iter().sum();

    startup_sum + steady_sum + shutdown_sum
}

fn term(variable: &Variable, coefficient: f64) -> ObjectiveTerm {
    ObjectiveTerm {
        variable: variable.name.clone(),
        coefficient,
    }
}
