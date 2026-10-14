//! Operational limit and future cost constraints.

use labdessem_core::system::{OperationalLimitTarget, OperationalLimitVariable, System};

use crate::indexing::Indexing;
use crate::variables::Variables;

use super::{ConstraintSense, LinearConstraint, LinearTerm, display_period, term};

/// Build future cost (Benders cut) constraints.
pub fn build_future_cost_constraints(
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

/// Build operational limit constraints.
pub fn build_operational_limit_constraints(
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
