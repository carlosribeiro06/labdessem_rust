//! Result types for simulation outputs.

use std::time::Duration;

use labdessem_core::{
    ids::{BranchId, BusId},
    system::OperationalLimitVariable,
};
use labdessem_solver::SolveSummary;

use crate::IterationStage;

/// Represents a flow constraint violation on a branch.
#[derive(Debug, Clone, PartialEq)]
pub struct FlowViolation {
    pub branch_id: BranchId,
    pub branch_name: String,
    pub period: usize,
    pub flow_mw: f64,
    pub limit_mw: f64,
}

/// Report for a single iteration step in the simulation.
#[derive(Debug, Clone)]
pub struct IterationReport {
    pub stage: IterationStage,
    pub iteration: usize,
    pub variable_count: usize,
    pub constraint_count: usize,
    pub objective_value: f64,
    pub solution_status: String,
    pub solve_time: Duration,
    pub violation_count: usize,
}

/// Complete result from an iterative simulation run.
#[derive(Debug, Clone)]
pub struct IterativeSimulationResult {
    pub steps: Vec<IterationReport>,
    pub final_summary: SolveSummary,
    pub cmo_summary: SolveSummary,
    pub final_violations: Vec<FlowViolation>,
    pub accumulated_flow_cuts: usize,
    pub final_line_flows: Vec<LineFlowResult>,
    pub added_flow_cut_lines: Vec<AddedFlowCutLineResult>,
    pub network_infeasibilities: Vec<NetworkInfeasibilityRecord>,
    pub operational_limit_infeasibilities: Vec<OperationalLimitInfeasibilityRecord>,
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

/// Record of a network infeasibility.
#[derive(Debug, Clone, PartialEq)]
pub struct NetworkInfeasibilityRecord {
    pub stage: IterationStage,
    pub iteration: usize,
    pub period: usize,
    pub branch_name: String,
    pub violation_mw: f64,
}

/// Record of an operational limit infeasibility.
#[derive(Debug, Clone, PartialEq)]
pub struct OperationalLimitInfeasibilityRecord {
    pub stage: IterationStage,
    pub iteration: usize,
    pub period: usize,
    pub plant_code: usize,
    pub plant_name: String,
    pub variable: OperationalLimitVariable,
    pub unit: &'static str,
    pub lower_violation: f64,
    pub upper_violation: f64,
}
