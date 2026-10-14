//! Thermal unit constraints.
//!
//! Includes generation limits, commitment transitions, ramps, and min up/down times.

use labdessem_core::system::System;

use crate::indexing::Indexing;
use crate::variables::Variables;

use super::{ConstraintSense, LinearConstraint, LinearTerm, display_period, term};

/// Build thermal ramp and trajectory constraints.
pub fn build_thermal_ramp_constraints(
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

/// Build thermal minimum up/down time constraints.
pub fn build_thermal_min_up_down_constraints(
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
