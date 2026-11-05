//! Result types for simulation outputs.

use std::collections::HashMap;
use std::time::Duration;

use labdessem_core::entities::EngMaxBounds;
use labdessem_core::{ids::SubmarketId, system::OperationalLimitVariable};
use labdessem_solver::LpSolution;

use crate::IterationStage;

// Re-export network types for backward compatibility
pub use labdessem_network::{
    AddedFlowCutLineResult, CmbRecord, FlowCutMapping, FlowViolation, LineFlowResult,
};

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
    pub cmo_solution: LpSolution,
    pub final_violations: Vec<FlowViolation>,
    pub accumulated_flow_cuts: usize,
    pub final_line_flows: Vec<LineFlowResult>,
    pub added_flow_cut_lines: Vec<AddedFlowCutLineResult>,
    pub network_infeasibilities: Vec<NetworkInfeasibilityRecord>,
    pub operational_limit_infeasibilities: Vec<OperationalLimitInfeasibilityRecord>,
    /// CMB records per (bus, period) — populated when network_enabled=true
    pub cmb_records: Vec<CmbRecord>,
    /// CMO values per (submarket, period) — always populated
    pub cmo_values: HashMap<(SubmarketId, usize), f64>,
    /// Flow cut mapping from LP-CALC-CMO stage — None when network disabled
    pub flow_cut_mapping: Option<FlowCutMapping>,
    /// EngMax outcome of the run (ADR-011); `None` when EngMax is disabled
    pub engmax: Option<EngMaxOutcome>,
}

/// EngMax at the end of a run (ADR-011).
#[derive(Debug, Clone)]
pub struct EngMaxOutcome {
    /// Bounds the final LP-CALC-CMO model was built from; the hydro CSV reports these.
    pub used: EngMaxBounds,
    /// EngMax recomputed from the final LP-CALC-CMO solution.
    pub recomputed: EngMaxBounds,
    /// Consistency of the final turbining with `recomputed` (R12).
    pub diagnostic: EngMaxDiagnostic,
}

/// Consistency of the final LP-CALC-CMO turbining with EngMax recomputed from that same
/// solution (ADR-011, R12), over every plant with units and every model period, in m3/s.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct EngMaxDiagnostic {
    /// Plant-periods whose turbining exceeds the recomputed bound by more than 0.01 m3/s,
    /// including those whose bound is zero.
    pub exceed_count: usize,
    /// The plant-periods of `exceed_count` whose recomputed bound is zero.
    pub zero_bound_exceed_count: usize,
    /// Largest excess over the counted plant-periods (m3/s); 0.0 when none is counted.
    pub max_excess_m3s: f64,
    /// Largest excess divided by its bound (a fraction), over the counted plant-periods with a
    /// positive bound; 0.0 when there is none.
    pub max_relative_excess: f64,
    /// Plant name and 1-based model period of `max_excess_m3s`, the first in plant-then-period
    /// order on a tie; `None` when nothing is counted.
    pub worst: Option<(String, usize)>,
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
