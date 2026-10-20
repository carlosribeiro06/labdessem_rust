//! Network result types.

use std::collections::HashMap;

use labdessem_core::ids::{BranchId, BusId};

/// Maps flow cut keys to their constraint row indices in the LP model.
///
/// Each entry maps `(branch_idx, period)` to `(upper_row_idx, lower_row_idx)`
/// representing the indices of the upper and lower flow limit constraints.
#[derive(Debug, Clone, Default)]
pub struct FlowCutMapping {
    pub indices: HashMap<(usize, usize), (usize, usize)>,
}

/// Represents a flow constraint violation on a branch.
#[derive(Debug, Clone, PartialEq)]
pub struct FlowViolation {
    pub branch_id: BranchId,
    pub branch_name: String,
    pub period: usize,
    pub flow_mw: f64,
    pub limit_mw: f64,
}

/// Line flow result for network output.
#[derive(Debug, Clone, PartialEq)]
pub struct LineFlowResult {
    pub branch_id: BranchId,
    pub branch_name: String,
    pub from_bus_name: String,
    pub to_bus_name: String,
    pub period: usize,
    pub flow_mw: f64,
    pub limit_mw: f64,
}

/// Record of a flow cut line added during iterations.
#[derive(Debug, Clone, PartialEq)]
pub struct AddedFlowCutLineResult {
    pub branch_id: BranchId,
    pub branch_name: String,
    pub from_bus_id: BusId,
    pub to_bus_id: BusId,
    pub flow_mw: f64,
    pub limit_mw: f64,
}
