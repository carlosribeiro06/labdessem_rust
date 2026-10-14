//! Demand balance and interchange constraints.

use labdessem_core::system::System;

use crate::indexing::Indexing;
use crate::variables::Variables;

use super::{ConstraintSense, LinearConstraint, LinearTerm, display_period, term};

/// Build demand balance constraints for all submarkets.
pub fn build_demand_balance_constraints(
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

/// Build interchange limit constraints.
pub fn build_interchange_limit_constraints(
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
