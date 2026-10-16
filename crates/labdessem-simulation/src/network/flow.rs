//! Flow detection and violation tracking.

// Rationale: expect() calls are for validated entities from validated System
#![allow(clippy::expect_used)]

use std::collections::BTreeSet;

use labdessem_core::system::System;
use labdessem_model::{constraints::LinearTerm, Model};
use labdessem_solver::SolveSummary;

use super::ptdf::{dot, round_to_digits};
use super::{DcNetworkModel, FlowCutKey};
use crate::results::{FlowViolation, LineFlowResult};

/// Generate all possible flow cut keys for a system.
pub fn all_flow_constraints(system: &System) -> BTreeSet<FlowCutKey> {
    (0..system.branches.len())
        .flat_map(|branch_idx| {
            (0..system.horizon.periods).map(move |period| FlowCutKey { branch_idx, period })
        })
        .collect()
}

/// Build the linear terms for a flow cut constraint.
pub fn flow_terms_for_cut(
    model: &Model,
    system: &System,
    dc_network: &DcNetworkModel,
    branch_idx: usize,
    period: usize,
) -> Vec<LinearTerm> {
    let horizon = system.horizon.periods;
    let mut terms = Vec::new();

    for (entry_idx, entry) in model.indexing.thermal_unit_entries.iter().enumerate() {
        let plant = &system.thermal_plants[entry.plant_idx];
        let Some(bus_pos) = dc_network.non_slack_bus_positions.get(&plant.bus_id) else {
            continue;
        };
        let coefficient = dc_network.ptdf_rows[branch_idx][*bus_pos];
        if coefficient.abs() <= 1e-12 {
            continue;
        }

        let variable = &model.variables.thermal_generation[entry_idx * horizon + period];
        terms.push(LinearTerm {
            variable: variable.name.clone(),
            coefficient,
        });
    }

    for (entry_idx, entry) in model.indexing.hydro_unit_entries.iter().enumerate() {
        let plant = &system.hydro_plants[entry.plant_idx];
        let Some(bus_pos) = dc_network.non_slack_bus_positions.get(&plant.bus_id) else {
            continue;
        };
        let coefficient = dc_network.ptdf_rows[branch_idx][*bus_pos];
        if coefficient.abs() <= 1e-12 {
            continue;
        }

        let variable = &model.variables.hydro_generation[entry_idx * horizon + period];
        terms.push(LinearTerm {
            variable: variable.name.clone(),
            coefficient,
        });
    }

    for (entry_idx, entry) in model.indexing.renewable_plant_entries.iter().enumerate() {
        let plant = &system.renewable_plants[entry.plant_idx];
        let Some(bus_pos) = dc_network.non_slack_bus_positions.get(&plant.bus_id) else {
            continue;
        };
        let coefficient = dc_network.ptdf_rows[branch_idx][*bus_pos];
        if coefficient.abs() <= 1e-12 {
            continue;
        }

        let variable = &model.variables.renewable_generation[entry_idx * horizon + period];
        terms.push(LinearTerm {
            variable: variable.name.clone(),
            coefficient,
        });
    }

    let flow_conversion = system.horizon.period_duration_hours * 0.0036;
    for (entry_idx, entry) in model.indexing.pumping_plant_entries.iter().enumerate() {
        let plant = &system.pumping_plants[entry.plant_idx];
        let Some(bus_pos) = dc_network.non_slack_bus_positions.get(&plant.bus_id) else {
            continue;
        };
        let coefficient = -dc_network.ptdf_rows[branch_idx][*bus_pos]
            * plant.specific_consumption_mw_per_m3s
            / flow_conversion;
        if coefficient.abs() <= 1e-12 {
            continue;
        }

        let variable = &model.variables.pumping[entry_idx * horizon + period];
        terms.push(LinearTerm {
            variable: variable.name.clone(),
            coefficient,
        });
    }

    terms
}

/// Compute the constant load shift for a branch and period.
pub fn constant_load_shift(
    system: &System,
    dc_network: &DcNetworkModel,
    branch_idx: usize,
    period: usize,
) -> f64 {
    dc_network
        .non_slack_bus_ids
        .iter()
        .enumerate()
        .map(|(bus_pos, bus_id)| {
            let bus = system
                .buses
                .iter()
                .find(|candidate| candidate.id == *bus_id)
                .expect("bus should exist for non-slack PTDF position");
            dc_network.ptdf_rows[branch_idx][bus_pos] * bus.demand_mw[period]
        })
        .sum()
}

/// Detect flow violations in the solved model.
pub fn detect_flow_violations(
    system: &System,
    dc_network: &DcNetworkModel,
    summary: &SolveSummary,
) -> Vec<FlowViolation> {
    let line_flows = compute_line_flows(system, dc_network, summary);
    let mut violations = Vec::new();

    for line_flow in line_flows {
        let rounded_flow = round_to_digits(line_flow.flow_mw, 4);
        if rounded_flow.abs() > line_flow.limit_mw {
            violations.push(FlowViolation {
                branch_id: line_flow.branch_id,
                branch_name: line_flow.branch_name,
                period: line_flow.period,
                flow_mw: line_flow.flow_mw,
                limit_mw: line_flow.limit_mw,
            });
        }
    }

    violations
}

/// Compute line flows for all branches and periods.
pub fn compute_line_flows(
    system: &System,
    dc_network: &DcNetworkModel,
    summary: &SolveSummary,
) -> Vec<LineFlowResult> {
    let injections = injections_by_period(system, dc_network, summary);
    let mut line_flows = Vec::with_capacity(system.horizon.periods * system.branches.len());

    for (period, period_injections) in injections.iter().enumerate() {
        for (branch_idx, branch) in system.branches.iter().enumerate() {
            let flow = dot(&dc_network.ptdf_rows[branch_idx], period_injections);
            line_flows.push(LineFlowResult {
                branch_id: branch.id,
                branch_name: branch.name.clone(),
                from_bus_name: bus_name(system, branch.from_bus_id),
                to_bus_name: bus_name(system, branch.to_bus_id),
                period: period + 1,
                flow_mw: flow,
                limit_mw: branch.thermal_limit_mw,
            });
        }
    }

    line_flows
}

fn bus_name(system: &System, bus_id: labdessem_core::ids::BusId) -> String {
    system
        .buses
        .iter()
        .find(|bus| bus.id == bus_id).map_or_else(|| format!("BUS-{}", bus_id.0), |bus| bus.name.clone())
}

fn injections_by_period(
    system: &System,
    dc_network: &DcNetworkModel,
    summary: &SolveSummary,
) -> Vec<Vec<f64>> {
    let horizon = system.horizon.periods;
    let bus_count = dc_network.non_slack_bus_ids.len();
    let mut injections = vec![vec![0.0; bus_count]; horizon];

    for bus_id in &dc_network.non_slack_bus_ids {
        let Some(bus_pos) = dc_network.non_slack_bus_positions.get(bus_id) else {
            continue;
        };

        for plant in &system.thermal_plants {
            if plant.bus_id != *bus_id {
                continue;
            }

            for unit in &plant.units {
                for (period, injection) in injections.iter_mut().enumerate() {
                    let variable = labdessem_model::names::thermal_generation_name(
                        &plant.name,
                        &unit.name,
                        period,
                    );
                    if let Some(value) = summary.variable_values.get(&variable) {
                        injection[*bus_pos] += value;
                    }
                }
            }
        }

        for plant in &system.hydro_plants {
            if plant.bus_id != *bus_id {
                continue;
            }

            for group in &plant.groups {
                for unit in &group.units {
                    for (period, injection) in injections.iter_mut().enumerate() {
                        let variable = labdessem_model::names::hydro_generation_name(
                            &plant.name,
                            &group.name,
                            &unit.name,
                            period,
                        );
                        if let Some(value) = summary.variable_values.get(&variable) {
                            injection[*bus_pos] += value;
                        }
                    }
                }
            }
        }

        for plant in &system.renewable_plants {
            if plant.bus_id != *bus_id {
                continue;
            }

            for (period, injection) in injections.iter_mut().enumerate() {
                let variable =
                    labdessem_model::names::renewable_generation_name(&plant.name, period);
                if let Some(value) = summary.variable_values.get(&variable) {
                    injection[*bus_pos] += value;
                }
            }
        }

        let flow_conversion = system.horizon.period_duration_hours * 0.0036;
        for plant in &system.pumping_plants {
            if plant.bus_id != *bus_id {
                continue;
            }

            for (period, injection) in injections.iter_mut().enumerate() {
                let variable = labdessem_model::names::pumping_name(&plant.name, period);
                if let Some(value) = summary.variable_values.get(&variable) {
                    let consumption_mw = value * plant.specific_consumption_mw_per_m3s / flow_conversion;
                    injection[*bus_pos] -= consumption_mw;
                }
            }
        }

        let bus = system
            .buses
            .iter()
            .find(|bus| bus.id == *bus_id)
            .expect("bus should exist for non-slack PTDF position");
        for (period, injection) in injections.iter_mut().enumerate() {
            injection[*bus_pos] -= bus.demand_mw[period];
        }
    }

    injections
}
