//! Solver interaction and stage execution functions.

use std::collections::BTreeSet;
use std::io::{self, Write};
use std::time::Instant;

use labdessem_core::system::System;
use labdessem_model::{Model, SolveMode};
use labdessem_network::{DcNetworkModel, FlowCutKey, detect_flow_violations};
use labdessem_solver::{
    HighsSolver, LpSolution, SolverError, SolverInterface, model_to_lp_template,
};

use crate::error::SimulationError;
use crate::results::{
    FlowViolation, IterationReport, NetworkInfeasibilityRecord, OperationalLimitInfeasibilityRecord,
};
use crate::workflow::IterationStage;

use super::common::model_dimensions;
use super::helpers::{
    add_flow_cuts, add_operational_limit_slacks, collect_network_infeasibilities,
    collect_operational_limit_infeasibilities, extend_cut_set,
};

/// Solve a model using the HiGHS solver, returning LpSolution directly.
pub(crate) fn solve_model_direct(
    solver: &mut HighsSolver,
    model: &Model,
) -> Result<LpSolution, SolverError> {
    let template = model_to_lp_template(model);
    solver.load_model(&template);
    solver.solve(None).map(|view| view.to_owned())
}

/// Apply commitment fixes from a previous MILP solution.
pub(crate) fn apply_commitment_fixes(
    model: &mut Model,
    source: &LpSolution,
    col_offsets: &labdessem_model::indexing::ColumnOffsets,
    horizon: usize,
) {
    for (idx, variable) in model.variables.thermal_commitment.iter_mut().enumerate() {
        let value = source.primal[col_offsets.thermal_commitment + idx];
        variable.fixed_value = Some(if value >= 0.5 { 1.0 } else { 0.0 });
    }

    for (idx, variable) in model.variables.thermal_startup.iter_mut().enumerate() {
        let value = source.primal[col_offsets.thermal_startup + idx];
        variable.fixed_value = Some(if value >= 0.5 { 1.0 } else { 0.0 });
    }

    for (idx, variable) in model.variables.thermal_shutdown.iter_mut().enumerate() {
        let value = source.primal[col_offsets.thermal_shutdown + idx];
        variable.fixed_value = Some(if value >= 0.5 { 1.0 } else { 0.0 });
    }

    for (idx, variable) in model.variables.hydro_commitment.iter_mut().enumerate() {
        let value = source.primal[col_offsets.hydro_commitment + idx];
        variable.fixed_value = Some(if value >= 0.5 { 1.0 } else { 0.0 });
    }

    let _ = horizon;
}

/// Run LP cut loop until no violations or stalled.
pub(crate) fn run_lp_cut_loop(
    system: &System,
    dc_network: &DcNetworkModel,
    accumulated_cuts: &mut BTreeSet<FlowCutKey>,
    steps: &mut Vec<IterationReport>,
    network_infeasibilities: &mut Vec<NetworkInfeasibilityRecord>,
    operational_limit_infeasibilities: &mut Vec<OperationalLimitInfeasibilityRecord>,
    solver: &mut HighsSolver,
) -> Result<(), SimulationError> {
    let mut iteration = 1usize;
    loop {
        let (_summary, violations, used_slacks) = solve_stage(
            system,
            dc_network,
            SolveMode::LinearProgramming,
            accumulated_cuts,
            None,
            IterationStage::LinearProgramming,
            iteration,
            steps,
            network_infeasibilities,
            operational_limit_infeasibilities,
            solver,
        )?;

        if violations.is_empty() {
            return Ok(());
        }

        let new_cuts = extend_cut_set(accumulated_cuts, system, &violations);
        if new_cuts == 0 {
            if used_slacks {
                return Ok(());
            }

            return Err(SimulationError::IterativeProcess(
                "LP flow-cut loop stalled because the same network violations persisted without generating new cuts".into(),
            ));
        }

        iteration += 1;
    }
}

#[allow(clippy::too_many_arguments)]
pub(crate) fn solve_stage(
    system: &System,
    dc_network: &DcNetworkModel,
    solve_mode: SolveMode,
    accumulated_cuts: &BTreeSet<FlowCutKey>,
    commitment_source: Option<&LpSolution>,
    stage: IterationStage,
    iteration: usize,
    steps: &mut Vec<IterationReport>,
    network_infeasibilities: &mut Vec<NetworkInfeasibilityRecord>,
    operational_limit_infeasibilities: &mut Vec<OperationalLimitInfeasibilityRecord>,
    solver: &mut HighsSolver,
) -> Result<(LpSolution, Vec<FlowViolation>, bool), SimulationError> {
    println!("Resolvendo {} #{:02}...", stage.label(), iteration);
    io::stdout().flush().ok();

    let indexing = labdessem_model::indexing::Indexing::from_system(system);
    let col_offsets = indexing.compute_column_offsets(system.horizon.periods);
    let horizon = system.horizon.periods;

    let mut model = Model::from_system(system, solve_mode);
    if let Some(commitment_source) = commitment_source {
        apply_commitment_fixes(&mut model, commitment_source, &col_offsets, horizon);
    }
    add_flow_cuts(&mut model, system, accumulated_cuts, dc_network, false);
    let (variable_count, constraint_count) = model_dimensions(&model);

    let started_at = Instant::now();
    let mut used_slacks = false;
    let summary = match solve_model_direct(solver, &model) {
        Ok(summary) => summary,
        Err(SolverError::Infeasible) | Err(SolverError::Unbounded)
            if !accumulated_cuts.is_empty() || !system.operational_limits.is_empty() =>
        {
            used_slacks = true;
            println!(
                "{} #{:02} inviavel com restricoes de rede/limite. Adicionando folgas...",
                stage.label(),
                iteration
            );
            io::stdout().flush().ok();

            let mut relaxed_model = Model::from_system(system, solve_mode);
            if let Some(commitment_source) = commitment_source {
                apply_commitment_fixes(
                    &mut relaxed_model,
                    commitment_source,
                    &col_offsets,
                    horizon,
                );
            }
            add_flow_cuts(
                &mut relaxed_model,
                system,
                accumulated_cuts,
                dc_network,
                true,
            );
            add_operational_limit_slacks(&mut relaxed_model);

            let relaxed_summary = solve_model_direct(solver, &relaxed_model)?;
            network_infeasibilities.extend(collect_network_infeasibilities(
                system,
                accumulated_cuts,
                &relaxed_summary,
                &col_offsets,
                stage,
                iteration,
            ));
            operational_limit_infeasibilities.extend(collect_operational_limit_infeasibilities(
                system,
                &relaxed_summary,
                &col_offsets,
                stage,
                iteration,
            ));
            relaxed_summary
        }
        Err(error) => return Err(error.into()),
    };
    let solve_time = started_at.elapsed();
    let violations = detect_flow_violations(system, dc_network, &summary.primal, &col_offsets);

    println!(
        "{} #{:02} concluido | objective = {:.4} | flow violations = {} | solve time = {:.3} s",
        stage.label(),
        iteration,
        summary.objective,
        violations.len(),
        solve_time.as_secs_f64()
    );
    io::stdout().flush().ok();

    steps.push(IterationReport {
        stage,
        iteration,
        variable_count,
        constraint_count,
        objective_value: summary.objective,
        solution_status: if used_slacks {
            "OPTIMAL_WITH_SLACK".into()
        } else {
            "OPTIMAL".into()
        },
        solve_time,
        violation_count: violations.len(),
    });

    Ok((summary, violations, used_slacks))
}

pub(crate) fn solve_stage_without_network(
    system: &System,
    solve_mode: SolveMode,
    commitment_source: Option<&LpSolution>,
    stage: IterationStage,
    iteration: usize,
    steps: &mut Vec<IterationReport>,
    operational_limit_infeasibilities: &mut Vec<OperationalLimitInfeasibilityRecord>,
    solver: &mut HighsSolver,
) -> Result<LpSolution, SimulationError> {
    println!("Resolvendo {} #{:02}...", stage.label(), iteration);
    io::stdout().flush().ok();

    let indexing = labdessem_model::indexing::Indexing::from_system(system);
    let col_offsets = indexing.compute_column_offsets(system.horizon.periods);
    let horizon = system.horizon.periods;

    let mut model = Model::from_system(system, solve_mode);
    if let Some(commitment_source) = commitment_source {
        apply_commitment_fixes(&mut model, commitment_source, &col_offsets, horizon);
    }
    let (variable_count, constraint_count) = model_dimensions(&model);

    let started_at = Instant::now();
    let summary = match solve_model_direct(solver, &model) {
        Ok(summary) => summary,
        Err(SolverError::Infeasible) | Err(SolverError::Unbounded)
            if !system.operational_limits.is_empty() =>
        {
            println!(
                "{} #{:02} inviavel com restricoes de limite. Adicionando folgas...",
                stage.label(),
                iteration
            );
            io::stdout().flush().ok();

            let mut relaxed_model = Model::from_system(system, solve_mode);
            if let Some(commitment_source) = commitment_source {
                apply_commitment_fixes(
                    &mut relaxed_model,
                    commitment_source,
                    &col_offsets,
                    horizon,
                );
            }
            add_operational_limit_slacks(&mut relaxed_model);

            let relaxed_summary = solve_model_direct(solver, &relaxed_model)?;
            operational_limit_infeasibilities.extend(collect_operational_limit_infeasibilities(
                system,
                &relaxed_summary,
                &col_offsets,
                stage,
                iteration,
            ));
            relaxed_summary
        }
        Err(error) => return Err(error.into()),
    };
    let solve_time = started_at.elapsed();

    println!(
        "{} #{:02} finalizado | objective = {:.4} | flow violations = 0 | solve time = {:.3} s",
        stage.label(),
        iteration,
        summary.objective,
        solve_time.as_secs_f64()
    );
    io::stdout().flush().ok();

    steps.push(IterationReport {
        stage,
        iteration,
        variable_count,
        constraint_count,
        objective_value: summary.objective,
        solution_status: if operational_limit_infeasibilities
            .iter()
            .any(|record| record.stage == stage && record.iteration == iteration)
        {
            "OPTIMAL_WITH_SLACK".into()
        } else {
            "OPTIMAL".into()
        },
        solve_time,
        violation_count: 0,
    });

    Ok(summary)
}
