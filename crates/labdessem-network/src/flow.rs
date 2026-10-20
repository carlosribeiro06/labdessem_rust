//! Flow computation and violation detection.

use std::collections::BTreeSet;

use labdessem_core::{ColumnOffsets, system::System};

use crate::model::{DcNetworkModel, FlowCutKey};
use crate::ptdf::{dot, round_to_digits};
use crate::types::{FlowViolation, LineFlowResult};

/// Generate all possible flow cut keys for a system.
pub fn all_flow_constraints(system: &System) -> BTreeSet<FlowCutKey> {
    (0..system.branches.len())
        .flat_map(|branch_idx| {
            (0..system.horizon.periods).map(move |period| FlowCutKey { branch_idx, period })
        })
        .collect()
}

/// Compute the constant load shift for a branch and period.
#[allow(clippy::expect_used)]
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

/// Detect flow violations from primal solution values.
pub fn detect_flow_violations(
    system: &System,
    dc_network: &DcNetworkModel,
    primal_values: &[f64],
    col_offsets: &ColumnOffsets,
) -> Vec<FlowViolation> {
    let line_flows = compute_line_flows(system, dc_network, primal_values, col_offsets);
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
    primal_values: &[f64],
    col_offsets: &ColumnOffsets,
) -> Vec<LineFlowResult> {
    let injections = injections_by_period(system, dc_network, primal_values, col_offsets);
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
        .find(|bus| bus.id == bus_id)
        .map_or_else(|| format!("BUS-{}", bus_id.0), |bus| bus.name.clone())
}

#[allow(clippy::expect_used)]
fn injections_by_period(
    system: &System,
    dc_network: &DcNetworkModel,
    primal_values: &[f64],
    col_offsets: &ColumnOffsets,
) -> Vec<Vec<f64>> {
    let horizon = system.horizon.periods;
    let bus_count = dc_network.non_slack_bus_ids.len();
    let mut injections = vec![vec![0.0; bus_count]; horizon];

    // Thermal generation
    let mut thermal_unit_idx = 0;
    for plant in &system.thermal_plants {
        let bus_pos = dc_network.non_slack_bus_positions.get(&plant.bus_id);
        for _unit in &plant.units {
            if let Some(&bus_pos) = bus_pos {
                for period in 0..horizon {
                    let idx = col_offsets.thermal_generation + thermal_unit_idx * horizon + period;
                    injections[period][bus_pos] += primal_values[idx];
                }
            }
            thermal_unit_idx += 1;
        }
    }

    // Hydro generation
    let mut hydro_unit_idx = 0;
    for plant in &system.hydro_plants {
        let bus_pos = dc_network.non_slack_bus_positions.get(&plant.bus_id);
        for group in &plant.groups {
            for _unit in &group.units {
                if let Some(&bus_pos) = bus_pos {
                    for period in 0..horizon {
                        let idx = col_offsets.hydro_generation + hydro_unit_idx * horizon + period;
                        injections[period][bus_pos] += primal_values[idx];
                    }
                }
                hydro_unit_idx += 1;
            }
        }
    }

    // Renewable generation
    for (plant_idx, plant) in system.renewable_plants.iter().enumerate() {
        let Some(&bus_pos) = dc_network.non_slack_bus_positions.get(&plant.bus_id) else {
            continue;
        };
        for period in 0..horizon {
            let idx = col_offsets.renewable_generation + plant_idx * horizon + period;
            injections[period][bus_pos] += primal_values[idx];
        }
    }

    // Pumping (negative injection - consumption)
    let flow_conversion = system.horizon.period_duration_hours * 0.0036;
    for (plant_idx, plant) in system.pumping_plants.iter().enumerate() {
        let Some(&bus_pos) = dc_network.non_slack_bus_positions.get(&plant.bus_id) else {
            continue;
        };
        for period in 0..horizon {
            let idx = col_offsets.pumping + plant_idx * horizon + period;
            let pumping_hm3 = primal_values[idx];
            let consumption_mw =
                pumping_hm3 * plant.specific_consumption_mw_per_m3s / flow_conversion;
            injections[period][bus_pos] -= consumption_mw;
        }
    }

    // Demand (negative injection)
    for bus_id in &dc_network.non_slack_bus_ids {
        let Some(&bus_pos) = dc_network.non_slack_bus_positions.get(bus_id) else {
            continue;
        };
        let bus = system
            .buses
            .iter()
            .find(|bus| bus.id == *bus_id)
            .expect("bus should exist for non-slack PTDF position");
        for period in 0..horizon {
            injections[period][bus_pos] -= bus.demand_mw[period];
        }
    }

    injections
}
