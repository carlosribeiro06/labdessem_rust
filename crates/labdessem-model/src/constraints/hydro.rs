//! Hydro plant constraints.
//!
//! Includes hydraulic balance, turbining limits, spillage, FPHA cuts,
//! productivity coupling, and commitment channeling.

use labdessem_core::system::System;

use crate::indexing::Indexing;
use crate::variables::Variables;

use super::{ConstraintSense, LinearConstraint, display_period, term, uc_constraint_to_linear};

/// Build hydraulic balance constraints for all hydro plants.
pub fn build_hydro_balance_constraints(
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

/// Build turbining limit constraints for hydro units.
pub fn build_hydro_turbining_limit_constraints(
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

/// Build spillage non-negativity constraints.
pub fn build_hydro_spillage_nonnegativity_constraints(
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

/// Build FPHA (piecewise linear) generation constraints.
pub fn build_hydro_fpha_constraints(
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

/// Build generation-turbining coupling constraints for FPHA mode.
pub fn build_hydro_generation_turbining_coupling_constraints(
    system: &System,
    indexing: &Indexing,
    variables: &Variables,
) -> Vec<LinearConstraint> {
    let horizon = system.horizon.periods;
    let flow_conversion = system.horizon.period_duration_hours * 0.0036;
    let mut constraints = Vec::new();

    for (entry_idx, entry) in indexing.hydro_unit_entries.iter().enumerate() {
        let plant = &system.hydro_plants[entry.plant_idx];
        let group = &plant.groups[entry.group_idx];
        let unit = &group.units[entry.unit_idx];
        let max_productivity = plant
            .fpha_segments
            .iter()
            .map(|segment| {
                segment.correction_factor * segment.turbining_coefficient / flow_conversion
            })
            .filter(|value| value.is_finite() && *value > 0.0)
            .fold(0.0, f64::max);

        if max_productivity <= 0.0 {
            continue;
        }

        for period in 0..horizon {
            let generation = &variables.hydro_generation[entry_idx * horizon + period];
            let turbining = &variables.hydro_turbining[entry_idx * horizon + period];
            constraints.push(LinearConstraint {
                name: format!(
                    "hydro_generation_turbining_coupling[p={},g={},u={},t={}]",
                    plant.name,
                    group.name,
                    unit.name,
                    display_period(period)
                ),
                terms: vec![term(generation, 1.0), term(turbining, -max_productivity)],
                sense: ConstraintSense::LessOrEqual,
                rhs: 0.0,
            });
        }
    }

    constraints
}

/// Build productivity-based generation constraints (non-FPHA mode).
pub fn build_hydro_productivity_constraints(
    system: &System,
    indexing: &Indexing,
    variables: &Variables,
) -> Vec<LinearConstraint> {
    let horizon = system.horizon.periods;
    let flow_conversion = system.horizon.period_duration_hours * 0.0036;

    indexing
        .hydro_plant_entries
        .iter()
        .flat_map(|entry| {
            let plant = &system.hydro_plants[entry.plant_idx];
            let productivity_per_hm3 = plant.specific_productivity_mw_per_m3s / flow_conversion;

            (0..horizon).map(move |period| {
                let mut terms = Vec::new();

                for (unit_entry_idx, unit_entry) in indexing.hydro_unit_entries.iter().enumerate() {
                    if unit_entry.plant_idx == entry.plant_idx {
                        terms.push(term(
                            &variables.hydro_generation[unit_entry_idx * horizon + period],
                            1.0,
                        ));
                        terms.push(term(
                            &variables.hydro_turbining[unit_entry_idx * horizon + period],
                            -productivity_per_hm3,
                        ));
                    }
                }

                LinearConstraint {
                    name: format!(
                        "hydro_productivity[p={},t={}]",
                        plant.name,
                        display_period(period)
                    ),
                    terms,
                    sense: ConstraintSense::Equal,
                    rhs: 0.0,
                }
            })
        })
        .collect()
}

/// Build commitment channeling constraints for hydro units.
pub fn build_hydro_commitment_channeling_constraints(
    system: &System,
    indexing: &Indexing,
    variables: &Variables,
) -> Vec<LinearConstraint> {
    let horizon = system.horizon.periods;

    if variables.hydro_commitment.is_empty() {
        return Vec::new();
    }

    let gen_var_names: Vec<_> = variables
        .hydro_generation
        .iter()
        .map(|v| v.name.clone())
        .collect();

    let commitment_vars: Vec<_> = variables
        .hydro_commitment
        .iter()
        .map(|v| labdessem_uc::UcVariable {
            name: v.name.clone(),
            lower_bound: v.lower_bound,
            upper_bound: v.upper_bound.unwrap_or(1.0),
            domain: labdessem_uc::CommitmentDomain::Binary,
        })
        .collect();

    let channeling_infos: Vec<_> = indexing
        .hydro_unit_entries
        .iter()
        .map(|entry| {
            let plant = &system.hydro_plants[entry.plant_idx];
            let group = &plant.groups[entry.group_idx];
            let unit = &group.units[entry.unit_idx];
            labdessem_uc::hydro::HydroUnitChannelingInfo {
                plant_name: &plant.name,
                group_name: &group.name,
                unit_name: &unit.name,
                min_generation_mw: unit.min_generation_mw,
                max_generation_mw: unit.max_generation_mw,
            }
        })
        .collect();

    labdessem_uc::hydro::build_hydro_channeling_constraints(
        &channeling_infos,
        &gen_var_names,
        &commitment_vars,
        horizon,
    )
    .into_iter()
    .map(uc_constraint_to_linear)
    .collect()
}

/// Build EngMax (dynamic maximum swallowing) constraints for hydro units.
///
/// These constraints coexist with the static turbining limit constraints.
/// Only active when `StudyConfig.engmax == 1`.
pub fn build_engmax_constraints(
    system: &System,
    indexing: &Indexing,
    variables: &Variables,
    engmax_bounds: &labdessem_core::entities::EngMaxBounds,
    engmax_enabled: bool,
) -> Vec<LinearConstraint> {
    if !engmax_enabled {
        return Vec::new();
    }

    let horizon = system.horizon.periods;
    let mut constraints = Vec::new();

    let mut current_plant_idx = usize::MAX;
    let mut unit_flat_idx = 0usize;

    for (entry_idx, entry) in indexing.hydro_unit_entries.iter().enumerate() {
        // Reset flat index when plant changes (entries are ordered by plant, group, unit)
        if entry.plant_idx != current_plant_idx {
            current_plant_idx = entry.plant_idx;
            unit_flat_idx = 0;
        }

        let plant = &system.hydro_plants[entry.plant_idx];
        let group = &plant.groups[entry.group_idx];
        let unit = &group.units[entry.unit_idx];

        for period in 0..horizon {
            if let Some(engmax_bound) = engmax_bounds.get(entry.plant_idx, unit_flat_idx, period) {
                let turbining = &variables.hydro_turbining[entry_idx * horizon + period];

                constraints.push(LinearConstraint {
                    name: format!(
                        "engmax[p={},g={},u={},t={}]",
                        plant.name,
                        group.name,
                        unit.name,
                        display_period(period)
                    ),
                    terms: vec![term(turbining, 1.0)],
                    sense: ConstraintSense::LessOrEqual,
                    rhs: engmax_bound,
                });
            }
        }

        unit_flat_idx += 1;
    }

    constraints
}
