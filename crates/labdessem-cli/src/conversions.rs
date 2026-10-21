//! Conversion functions from simulation result types to I/O DTOs.

use labdessem_io::output::{
    AddedFlowCutLineDto, CmbRecordDto, IterationReportDto, IterationStageLabel, LineFlowResultDto,
    NetworkInfeasibilityDto, OperationalLimitInfeasibilityDto, OutputData,
};
use labdessem_network::CmbRecord;
use labdessem_simulation::{
    AddedFlowCutLineResult, IterationReport, IterationStage, IterativeSimulationResult,
    LineFlowResult, NetworkInfeasibilityRecord, OperationalLimitInfeasibilityRecord,
};

/// Convert simulation results to output DTOs.
pub fn convert_to_output_data(result: &IterativeSimulationResult) -> OutputData {
    OutputData {
        steps: result.steps.iter().map(convert_iteration_report).collect(),
        cmo_solution: Some(result.cmo_solution.clone()),
        cmo_values: result.cmo_values.clone(),
        final_line_flows: result
            .final_line_flows
            .iter()
            .map(convert_line_flow)
            .collect(),
        added_flow_cut_lines: result
            .added_flow_cut_lines
            .iter()
            .map(convert_added_flow_cut)
            .collect(),
        network_infeasibilities: result
            .network_infeasibilities
            .iter()
            .map(convert_network_infeasibility)
            .collect(),
        operational_limit_infeasibilities: result
            .operational_limit_infeasibilities
            .iter()
            .map(convert_operational_limit_infeasibility)
            .collect(),
        cmb_records: result.cmb_records.iter().map(convert_cmb_record).collect(),
    }
}

fn convert_iteration_stage(stage: IterationStage) -> IterationStageLabel {
    match stage {
        IterationStage::LinearProgramming => IterationStageLabel::Lp,
        IterationStage::MixedIntegerLinearProgramming => IterationStageLabel::Milp,
        IterationStage::LinearProgrammingWithFixedCommitment => IterationStageLabel::LpFixed,
        IterationStage::LinearProgrammingCalculateCmo => IterationStageLabel::LpCalcCmo,
    }
}

fn convert_iteration_report(report: &IterationReport) -> IterationReportDto {
    IterationReportDto {
        stage: convert_iteration_stage(report.stage),
        iteration: report.iteration,
        variable_count: report.variable_count,
        constraint_count: report.constraint_count,
        objective_value: report.objective_value,
        solution_status: report.solution_status.clone(),
        solve_time_seconds: report.solve_time.as_secs_f64(),
        violation_count: report.violation_count,
    }
}

fn convert_line_flow(flow: &LineFlowResult) -> LineFlowResultDto {
    LineFlowResultDto {
        branch_id: flow.branch_id.0,
        branch_name: flow.branch_name.clone(),
        from_bus_name: flow.from_bus_name.clone(),
        to_bus_name: flow.to_bus_name.clone(),
        period: flow.period,
        flow_mw: flow.flow_mw,
        limit_mw: flow.limit_mw,
    }
}

fn convert_added_flow_cut(cut: &AddedFlowCutLineResult) -> AddedFlowCutLineDto {
    AddedFlowCutLineDto {
        branch_id: cut.branch_id.0,
        branch_name: cut.branch_name.clone(),
        from_bus_id: cut.from_bus_id.0,
        to_bus_id: cut.to_bus_id.0,
        flow_mw: cut.flow_mw,
        limit_mw: cut.limit_mw,
    }
}

fn convert_network_infeasibility(record: &NetworkInfeasibilityRecord) -> NetworkInfeasibilityDto {
    NetworkInfeasibilityDto {
        stage: convert_iteration_stage(record.stage),
        iteration: record.iteration,
        period: record.period,
        branch_name: record.branch_name.clone(),
        violation_mw: record.violation_mw,
    }
}

fn convert_operational_limit_infeasibility(
    record: &OperationalLimitInfeasibilityRecord,
) -> OperationalLimitInfeasibilityDto {
    OperationalLimitInfeasibilityDto {
        stage: convert_iteration_stage(record.stage),
        iteration: record.iteration,
        period: record.period,
        plant_code: record.plant_code,
        plant_name: record.plant_name.clone(),
        variable: record.variable,
        unit: record.unit,
        lower_violation: record.lower_violation,
        upper_violation: record.upper_violation,
    }
}

fn convert_cmb_record(record: &CmbRecord) -> CmbRecordDto {
    CmbRecordDto {
        bus_id: record.bus_id.0,
        period: record.period,
        cmb: record.cmb,
        submarket_id: record.submarket_id.0,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn convert_iteration_stage_maps_all_variants() {
        assert_eq!(
            convert_iteration_stage(IterationStage::LinearProgramming),
            IterationStageLabel::Lp
        );
        assert_eq!(
            convert_iteration_stage(IterationStage::MixedIntegerLinearProgramming),
            IterationStageLabel::Milp
        );
        assert_eq!(
            convert_iteration_stage(IterationStage::LinearProgrammingWithFixedCommitment),
            IterationStageLabel::LpFixed
        );
        assert_eq!(
            convert_iteration_stage(IterationStage::LinearProgrammingCalculateCmo),
            IterationStageLabel::LpCalcCmo
        );
    }
}
