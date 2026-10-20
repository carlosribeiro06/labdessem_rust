//! Flow violation detection.

use labdessem_core::{ColumnOffsets, system::System};

use crate::linalg::round_to_digits;
use crate::topology::model::DcNetworkModel;
use crate::types::FlowViolation;

use super::computation::compute_line_flows;

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
