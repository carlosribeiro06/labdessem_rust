//! Output data transfer objects for simulation results.

use std::collections::HashMap;

use labdessem_core::{ids::SubmarketId, system::OperationalLimitVariable};
use labdessem_solver::LpSolution;

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
    pub branch_id: usize,
    pub branch_name: String,
    pub from_bus_name: String,
    pub to_bus_name: String,
    pub period: usize,
    pub flow_mw: f64,
    pub limit_mw: f64,
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
    pub bus_id: usize,
    pub period: usize,
    pub cmb: f64,
    pub submarket_id: usize,
}

#[derive(Debug, Clone, Default)]
pub struct OutputData {
    pub steps: Vec<IterationReportDto>,
    pub cmo_solution: Option<LpSolution>,
    pub cmo_values: HashMap<(SubmarketId, usize), f64>,
    pub final_line_flows: Vec<LineFlowResultDto>,
    pub added_flow_cut_lines: Vec<AddedFlowCutLineDto>,
    pub network_infeasibilities: Vec<NetworkInfeasibilityDto>,
    pub operational_limit_infeasibilities: Vec<OperationalLimitInfeasibilityDto>,
    pub cmb_records: Vec<CmbRecordDto>,
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
        assert!(data.cmo_solution.is_none());
        assert!(data.cmo_values.is_empty());
    }
}
