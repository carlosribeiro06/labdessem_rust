//! Output data transfer objects for simulation results.

use std::collections::HashMap;

use labdessem_core::{ids::SubmarketId, system::OperationalLimitVariable};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum IterationStageLabel {
    Lp,
    Milp,
    LpFixed,
    LpCalcCmo,
}

impl IterationStageLabel {
    pub fn label(&self) -> &'static str {
        match self {
            Self::Lp => "LP",
            Self::Milp => "MILP",
            Self::LpFixed => "LP-FIXED",
            Self::LpCalcCmo => "LP-CALC-CMO",
        }
    }
}

#[derive(Debug, Clone)]
pub struct IterationReportDto {
    pub stage: IterationStageLabel,
    pub iteration: usize,
    pub variable_count: usize,
    pub constraint_count: usize,
    pub objective_value: f64,
    pub solution_status: String,
    pub solve_time_seconds: f64,
    pub violation_count: usize,
}

#[derive(Debug, Clone)]
pub struct NetworkInfeasibilityDto {
    pub stage: IterationStageLabel,
    pub iteration: usize,
    pub period: usize,
    pub branch_name: String,
    pub violation_mw: f64,
}

#[derive(Debug, Clone)]
pub struct OperationalLimitInfeasibilityDto {
    pub stage: IterationStageLabel,
    pub iteration: usize,
    pub period: usize,
    pub plant_code: usize,
    pub plant_name: String,
    pub variable: OperationalLimitVariable,
    pub unit: &'static str,
    pub lower_violation: f64,
    pub upper_violation: f64,
}

#[derive(Debug, Clone)]
pub struct LineFlowResultDto {
    pub branch_name: String,
    pub from_bus_name: String,
    pub to_bus_name: String,
    pub original_period: usize,
    pub flow_mw: f64,
    pub limit_mw: f64,
    pub violated: bool,
}

#[derive(Debug, Clone)]
pub struct AddedFlowCutLineDto {
    pub branch_id: usize,
    pub branch_name: String,
    pub from_bus_id: usize,
    pub to_bus_id: usize,
    pub flow_mw: f64,
    pub limit_mw: f64,
}

#[derive(Debug, Clone)]
pub struct CmbRecordDto {
    pub original_period: usize,
    pub bus_id: usize,
    pub bus_name: String,
    pub submarket_name: String,
    pub cmb: f64,
}

#[derive(Debug, Clone, Default)]
pub struct OutputData {
    /// Iteration reports from each solve step
    pub steps: Vec<IterationReportDto>,

    /// Final line flows (network enabled only)
    pub final_line_flows: Vec<LineFlowResultDto>,

    /// Added flow cut lines across all iterations
    pub added_flow_cut_lines: Vec<AddedFlowCutLineDto>,

    /// Nodal marginal costs (network enabled only)
    pub cmb_records: Vec<CmbRecordDto>,

    /// Network infeasibility records
    pub network_infeasibilities: Vec<NetworkInfeasibilityDto>,

    /// Operational limit infeasibility records
    pub operational_limit_infeasibilities: Vec<OperationalLimitInfeasibilityDto>,

    /// CMO values for network cases (submarket, period -> cmo)
    pub cmo_values: Option<HashMap<(SubmarketId, usize), f64>>,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn iteration_stage_label_returns_correct_string() {
        assert_eq!(IterationStageLabel::Lp.label(), "LP");
        assert_eq!(IterationStageLabel::Milp.label(), "MILP");
        assert_eq!(IterationStageLabel::LpFixed.label(), "LP-FIXED");
        assert_eq!(IterationStageLabel::LpCalcCmo.label(), "LP-CALC-CMO");
    }

    #[test]
    fn output_data_default_is_empty() {
        let data = OutputData::default();
        assert!(data.steps.is_empty());
        assert!(data.final_line_flows.is_empty());
        assert!(data.cmo_values.is_none());
    }
}
