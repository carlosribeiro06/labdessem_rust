//! Thermal unit constraints.
//!
//! Includes generation limits, commitment transitions, ramps, and min up/down times.

// Rationale: expect() calls are for validated trajectory configurations
#![allow(clippy::expect_used)]

use labdessem_core::system::System;
use labdessem_uc::thermal::{
    ThermalUnitExclusiveInfo, ThermalUnitMinUpDownInfo, ThermalUnitTrajectoryInfo,
    ThermalUnitTransitionInfo, build_initial_state_fixing_constraints,
    build_min_down_time_constraints, build_min_up_time_constraints,
    build_startup_shutdown_exclusive_constraints, build_thermal_transition_constraints,
    build_trajectory_bounds_constraints,
};
use labdessem_uc::types::UcVariable;

use crate::indexing::Indexing;
use crate::variables::Variables;

use super::{LinearConstraint, uc_constraint_to_linear};

/// Build thermal ramp and trajectory constraints.
pub fn build_thermal_ramp_constraints(
    system: &System,
    indexing: &Indexing,
    variables: &Variables,
) -> Vec<LinearConstraint> {
    let horizon = system.horizon.periods;
    let mut constraints = Vec::new();

    let commitment_vars = variables_to_uc_vars(&variables.thermal_commitment);
    let startup_vars = variables_to_uc_vars(&variables.thermal_startup);
    let shutdown_vars = variables_to_uc_vars(&variables.thermal_shutdown);

    if commitment_vars.is_empty() {
        return constraints;
    }

    // Build transition constraints
    let transition_infos = build_transition_infos(system, indexing);
    let uc_transition = build_thermal_transition_constraints(
        &transition_infos,
        &commitment_vars,
        &startup_vars,
        &shutdown_vars,
        horizon,
    );
    constraints.extend(uc_transition.into_iter().map(uc_constraint_to_linear));

    // Build exclusive constraints
    let exclusive_infos = build_exclusive_infos(system, indexing);
    let uc_exclusive = build_startup_shutdown_exclusive_constraints(
        &exclusive_infos,
        &startup_vars,
        &shutdown_vars,
        horizon,
    );
    constraints.extend(uc_exclusive.into_iter().map(uc_constraint_to_linear));

    // Build trajectory bounds constraints
    let gen_var_names: Vec<_> = variables
        .thermal_generation
        .iter()
        .map(|v| v.name.clone())
        .collect();
    let trajectory_data = build_trajectory_data(system, indexing);
    let trajectory_infos = build_trajectory_infos(system, indexing, &trajectory_data);
    let uc_trajectory = build_trajectory_bounds_constraints(
        &trajectory_infos,
        &gen_var_names,
        &commitment_vars,
        &startup_vars,
        &shutdown_vars,
        horizon,
    );
    constraints.extend(uc_trajectory.into_iter().map(uc_constraint_to_linear));

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

    let commitment_vars = variables_to_uc_vars(&variables.thermal_commitment);
    if commitment_vars.is_empty() {
        return constraints;
    }

    // Build initial state fixing constraints
    let min_up_down_infos = build_min_up_down_infos(system, indexing);
    let uc_initial =
        build_initial_state_fixing_constraints(&min_up_down_infos, &commitment_vars, horizon);
    constraints.extend(uc_initial.into_iter().map(uc_constraint_to_linear));

    // Build min up time constraints
    let uc_min_up = build_min_up_time_constraints(&min_up_down_infos, &commitment_vars, horizon);
    constraints.extend(uc_min_up.into_iter().map(uc_constraint_to_linear));

    // Build min down time constraints
    let uc_min_down =
        build_min_down_time_constraints(&min_up_down_infos, &commitment_vars, horizon);
    constraints.extend(uc_min_down.into_iter().map(uc_constraint_to_linear));

    constraints
}

fn variables_to_uc_vars(vars: &[crate::variables::Variable]) -> Vec<UcVariable> {
    vars.iter()
        .map(|v| UcVariable {
            name: v.name.clone(),
            lower_bound: v.lower_bound,
            upper_bound: v.upper_bound.unwrap_or(1.0),
            domain: labdessem_uc::CommitmentDomain::Binary,
        })
        .collect()
}

fn build_transition_infos<'a>(
    system: &'a System,
    indexing: &Indexing,
) -> Vec<ThermalUnitTransitionInfo<'a>> {
    indexing
        .thermal_unit_entries
        .iter()
        .map(|entry| {
            let plant = &system.thermal_plants[entry.plant_idx];
            let unit = &plant.units[entry.unit_idx];
            ThermalUnitTransitionInfo {
                plant_name: &plant.name,
                unit_name: &unit.name,
                initial_is_on: unit.initial_condition.is_on,
            }
        })
        .collect()
}

fn build_exclusive_infos<'a>(
    system: &'a System,
    indexing: &Indexing,
) -> Vec<ThermalUnitExclusiveInfo<'a>> {
    indexing
        .thermal_unit_entries
        .iter()
        .map(|entry| {
            let plant = &system.thermal_plants[entry.plant_idx];
            let unit = &plant.units[entry.unit_idx];
            ThermalUnitExclusiveInfo {
                plant_name: &plant.name,
                unit_name: &unit.name,
            }
        })
        .collect()
}

struct TrajectoryData {
    initial_startup_remaining: Option<Vec<f64>>,
    initial_shutdown_remaining: Option<Vec<f64>>,
}

fn build_trajectory_data(system: &System, indexing: &Indexing) -> Vec<TrajectoryData> {
    indexing
        .thermal_unit_entries
        .iter()
        .map(|entry| {
            let plant = &system.thermal_plants[entry.plant_idx];
            let unit = &plant.units[entry.unit_idx];

            let initial_startup_remaining = if unit.initial_condition.is_ramping_up {
                Some(
                    unit.initial_startup_remaining_trajectory()
                        .expect("validated thermal startup trajectory"),
                )
            } else {
                None
            };
            let initial_shutdown_remaining = if unit.initial_condition.is_ramping_down {
                Some(
                    unit.initial_shutdown_remaining_trajectory()
                        .expect("validated thermal shutdown trajectory"),
                )
            } else {
                None
            };

            TrajectoryData {
                initial_startup_remaining,
                initial_shutdown_remaining,
            }
        })
        .collect()
}

fn build_trajectory_infos<'a>(
    system: &'a System,
    indexing: &Indexing,
    trajectory_data: &'a [TrajectoryData],
) -> Vec<ThermalUnitTrajectoryInfo<'a>> {
    indexing
        .thermal_unit_entries
        .iter()
        .enumerate()
        .map(|(i, entry)| {
            let plant = &system.thermal_plants[entry.plant_idx];
            let unit = &plant.units[entry.unit_idx];
            let data = &trajectory_data[i];

            ThermalUnitTrajectoryInfo {
                plant_name: &plant.name,
                unit_name: &unit.name,
                min_generation_mw: unit.min_generation_mw,
                max_generation_mw: unit.max_generation_mw,
                startup_trajectory_mw: &unit.startup_trajectory_mw,
                shutdown_trajectory_mw: &unit.shutdown_trajectory_mw,
                initial_startup_remaining: data.initial_startup_remaining.as_deref(),
                initial_shutdown_remaining: data.initial_shutdown_remaining.as_deref(),
            }
        })
        .collect()
}

fn build_min_up_down_infos<'a>(
    system: &'a System,
    indexing: &Indexing,
) -> Vec<ThermalUnitMinUpDownInfo<'a>> {
    indexing
        .thermal_unit_entries
        .iter()
        .map(|entry| {
            let plant = &system.thermal_plants[entry.plant_idx];
            let unit = &plant.units[entry.unit_idx];

            ThermalUnitMinUpDownInfo {
                plant_name: &plant.name,
                unit_name: &unit.name,
                min_up_time: unit.min_up_time,
                min_down_time: unit.min_down_time,
                initial_is_on: unit.initial_condition.is_on,
                initial_time_in_state: unit.initial_condition.time_in_state,
            }
        })
        .collect()
}
