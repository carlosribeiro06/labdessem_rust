//! Simulation orchestration and strategy execution.

use std::collections::BTreeSet;

use labdessem_core::entities::EngMaxBounds;
use labdessem_core::system::System;
use labdessem_model::SolveMode;
use labdessem_network::{DcNetworkModel, FlowCutKey, all_flow_constraints, compute_line_flows};
use labdessem_solver::HighsSolver;

use crate::engmax::{initialize_engmax_bounds, recalculate_engmax_bounds};
use crate::error::SimulationError;
use crate::results::IterativeSimulationResult;
use crate::workflow::{ExecutionOption, IterationStage};

use super::helpers::{compute_cmb_cmo_data, extend_cut_set, summarize_added_flow_cut_lines};
use super::solve::{run_lp_cut_loop, solve_stage, solve_stage_without_network};

/// Run simulation with the specified execution option.
pub fn run_simulation(
    system: &System,
    network_enabled: bool,
    execution_option: ExecutionOption,
) -> Result<IterativeSimulationResult, SimulationError> {
    run_simulation_with_engmax(system, network_enabled, execution_option, false)
}

/// Run simulation with the specified execution option and optional EngMax.
pub fn run_simulation_with_engmax(
    system: &System,
    network_enabled: bool,
    execution_option: ExecutionOption,
    engmax_enabled: bool,
) -> Result<IterativeSimulationResult, SimulationError> {
    let mut solver = HighsSolver::new()
        .map_err(|e| SimulationError::IterativeProcess(format!("Failed to create solver: {e}")))?;

    // Initialize EngMax bounds if enabled
    let engmax_bounds = if engmax_enabled {
        Some(initialize_engmax_bounds(system))
    } else {
        None
    };

    match execution_option {
        ExecutionOption::Iterative => {
            run_iterative_simulation_strategy(
                system,
                network_enabled,
                engmax_bounds,
                engmax_enabled,
                &mut solver,
            )
        }
        ExecutionOption::SingleMilp => {
            run_single_milp_simulation(
                system,
                network_enabled,
                engmax_bounds,
                engmax_enabled,
                &mut solver,
            )
        }
    }
}

fn run_iterative_simulation_strategy(
    system: &System,
    network_enabled: bool,
    mut engmax_bounds: Option<EngMaxBounds>,
    engmax_enabled: bool,
    solver: &mut HighsSolver,
) -> Result<IterativeSimulationResult, SimulationError> {
    if !network_enabled {
        return run_simulation_without_network(system, engmax_bounds, engmax_enabled, solver);
    }

    let dc_network = DcNetworkModel::from_system(system)?;
    let mut accumulated_cuts = BTreeSet::<FlowCutKey>::new();
    let mut steps = Vec::new();
    let mut network_infeasibilities = Vec::new();
    let mut operational_limit_infeasibilities = Vec::new();

    run_lp_cut_loop(
        system,
        &dc_network,
        &mut accumulated_cuts,
        engmax_bounds.as_ref(),
        engmax_enabled,
        &mut steps,
        &mut network_infeasibilities,
        &mut operational_limit_infeasibilities,
        solver,
    )?;

    // Recalculate EngMax bounds after LP if enabled
    // Note: LP solution is not preserved in cut loop, so we skip recalculation here
    // The MILP will use initial bounds

    let (milp_summary, milp_violations, milp_used_slacks) = solve_stage(
        system,
        &dc_network,
        SolveMode::MixedIntegerLinearProgramming,
        &accumulated_cuts,
        None,
        engmax_bounds.as_ref(),
        engmax_enabled,
        IterationStage::MixedIntegerLinearProgramming,
        1,
        &mut steps,
        &mut network_infeasibilities,
        &mut operational_limit_infeasibilities,
        solver,
    )?;

    // Recalculate EngMax bounds after MILP if enabled
    if let Some(ref mut bounds) = engmax_bounds {
        let indexing = labdessem_model::indexing::Indexing::from_system(system);
        recalculate_engmax_bounds(system, &milp_summary, &indexing, bounds);
    }

    if milp_violations.is_empty() || milp_used_slacks {
        let all_cuts = all_flow_constraints(system);
        let cmo_solution = solve_stage(
            system,
            &dc_network,
            SolveMode::LinearProgrammingWithFixedCommitment,
            &all_cuts,
            Some(&milp_summary),
            engmax_bounds.as_ref(),
            engmax_enabled,
            IterationStage::LinearProgrammingCalculateCmo,
            1,
            &mut steps,
            &mut network_infeasibilities,
            &mut operational_limit_infeasibilities,
            solver,
        )?
        .0;

        let indexing = labdessem_model::indexing::Indexing::from_system(system);
        let col_offsets = indexing.compute_column_offsets(system.horizon.periods);
        let final_line_flows =
            compute_line_flows(system, &dc_network, &cmo_solution.primal, &col_offsets);
        let added_flow_cut_lines =
            summarize_added_flow_cut_lines(system, &accumulated_cuts, &final_line_flows);

        let (cmb_records, cmo_values, flow_cut_mapping) =
            compute_cmb_cmo_data(system, Some(&dc_network), &cmo_solution, &all_cuts);

        return Ok(IterativeSimulationResult {
            steps,
            cmo_solution,
            final_violations: milp_violations,
            accumulated_flow_cuts: accumulated_cuts.len(),
            final_line_flows,
            added_flow_cut_lines,
            network_infeasibilities,
            operational_limit_infeasibilities,
            cmb_records,
            cmo_values,
            flow_cut_mapping,
        });
    }

    extend_cut_set(&mut accumulated_cuts, system, &milp_violations);

    let mut fixed_iteration = 1usize;
    let final_violations;
    loop {
        let (lp_fixed_solution, violations, used_slacks) = solve_stage(
            system,
            &dc_network,
            SolveMode::LinearProgrammingWithFixedCommitment,
            &accumulated_cuts,
            Some(&milp_summary),
            engmax_bounds.as_ref(),
            engmax_enabled,
            IterationStage::LinearProgrammingWithFixedCommitment,
            fixed_iteration,
            &mut steps,
            &mut network_infeasibilities,
            &mut operational_limit_infeasibilities,
            solver,
        )?;

        // Recalculate EngMax bounds after LP-FIXED if enabled
        if let Some(ref mut bounds) = engmax_bounds {
            let indexing = labdessem_model::indexing::Indexing::from_system(system);
            recalculate_engmax_bounds(system, &lp_fixed_solution, &indexing, bounds);
        }

        if violations.is_empty() {
            final_violations = violations;
            break;
        }

        let new_cuts = extend_cut_set(&mut accumulated_cuts, system, &violations);
        if new_cuts == 0 {
            if used_slacks {
                final_violations = violations;
                break;
            }

            return Err(SimulationError::IterativeProcess(
                "LP with fixed commitment still has flow violations, but no new flow cut could be added".into(),
            ));
        }

        fixed_iteration += 1;
    }

    let all_cuts = all_flow_constraints(system);
    let cmo_solution = solve_stage(
        system,
        &dc_network,
        SolveMode::LinearProgrammingWithFixedCommitment,
        &all_cuts,
        Some(&milp_summary),
        engmax_bounds.as_ref(),
        engmax_enabled,
        IterationStage::LinearProgrammingCalculateCmo,
        1,
        &mut steps,
        &mut network_infeasibilities,
        &mut operational_limit_infeasibilities,
        solver,
    )?
    .0;

    let indexing = labdessem_model::indexing::Indexing::from_system(system);
    let col_offsets = indexing.compute_column_offsets(system.horizon.periods);
    let final_line_flows =
        compute_line_flows(system, &dc_network, &cmo_solution.primal, &col_offsets);
    let added_flow_cut_lines =
        summarize_added_flow_cut_lines(system, &accumulated_cuts, &final_line_flows);

    let (cmb_records, cmo_values, flow_cut_mapping) =
        compute_cmb_cmo_data(system, Some(&dc_network), &cmo_solution, &all_cuts);

    Ok(IterativeSimulationResult {
        steps,
        cmo_solution,
        final_violations,
        accumulated_flow_cuts: accumulated_cuts.len(),
        final_line_flows,
        added_flow_cut_lines,
        network_infeasibilities,
        operational_limit_infeasibilities,
        cmb_records,
        cmo_values,
        flow_cut_mapping,
    })
}

fn run_single_milp_simulation(
    system: &System,
    network_enabled: bool,
    mut engmax_bounds: Option<EngMaxBounds>,
    engmax_enabled: bool,
    solver: &mut HighsSolver,
) -> Result<IterativeSimulationResult, SimulationError> {
    let mut steps = Vec::new();
    let mut network_infeasibilities = Vec::new();
    let mut operational_limit_infeasibilities = Vec::new();

    if network_enabled {
        let dc_network = DcNetworkModel::from_system(system)?;
        let accumulated_cuts = all_flow_constraints(system);
        let (milp_summary, final_violations, _) = solve_stage(
            system,
            &dc_network,
            SolveMode::MixedIntegerLinearProgramming,
            &accumulated_cuts,
            None,
            engmax_bounds.as_ref(),
            engmax_enabled,
            IterationStage::MixedIntegerLinearProgramming,
            1,
            &mut steps,
            &mut network_infeasibilities,
            &mut operational_limit_infeasibilities,
            solver,
        )?;

        // Recalculate EngMax bounds after MILP if enabled
        if let Some(ref mut bounds) = engmax_bounds {
            let indexing = labdessem_model::indexing::Indexing::from_system(system);
            recalculate_engmax_bounds(system, &milp_summary, &indexing, bounds);
        }

        let cmo_solution = solve_stage(
            system,
            &dc_network,
            SolveMode::LinearProgrammingWithFixedCommitment,
            &accumulated_cuts,
            Some(&milp_summary),
            engmax_bounds.as_ref(),
            engmax_enabled,
            IterationStage::LinearProgrammingCalculateCmo,
            1,
            &mut steps,
            &mut network_infeasibilities,
            &mut operational_limit_infeasibilities,
            solver,
        )?
        .0;

        let indexing = labdessem_model::indexing::Indexing::from_system(system);
        let col_offsets = indexing.compute_column_offsets(system.horizon.periods);
        let final_line_flows =
            compute_line_flows(system, &dc_network, &cmo_solution.primal, &col_offsets);
        let added_flow_cut_lines =
            summarize_added_flow_cut_lines(system, &accumulated_cuts, &final_line_flows);

        let (cmb_records, cmo_values, flow_cut_mapping) =
            compute_cmb_cmo_data(system, Some(&dc_network), &cmo_solution, &accumulated_cuts);

        return Ok(IterativeSimulationResult {
            steps,
            cmo_solution,
            final_violations,
            accumulated_flow_cuts: accumulated_cuts.len(),
            final_line_flows,
            added_flow_cut_lines,
            network_infeasibilities,
            operational_limit_infeasibilities,
            cmb_records,
            cmo_values,
            flow_cut_mapping,
        });
    }

    let milp_summary = solve_stage_without_network(
        system,
        SolveMode::MixedIntegerLinearProgramming,
        None,
        engmax_bounds.as_ref(),
        engmax_enabled,
        IterationStage::MixedIntegerLinearProgramming,
        1,
        &mut steps,
        &mut operational_limit_infeasibilities,
        solver,
    )?;

    // Recalculate EngMax bounds after MILP if enabled
    if let Some(ref mut bounds) = engmax_bounds {
        let indexing = labdessem_model::indexing::Indexing::from_system(system);
        recalculate_engmax_bounds(system, &milp_summary, &indexing, bounds);
    }

    let cmo_solution = solve_stage_without_network(
        system,
        SolveMode::LinearProgrammingWithFixedCommitment,
        Some(&milp_summary),
        engmax_bounds.as_ref(),
        engmax_enabled,
        IterationStage::LinearProgrammingCalculateCmo,
        1,
        &mut steps,
        &mut operational_limit_infeasibilities,
        solver,
    )?;

    let empty_cuts = BTreeSet::new();
    let (cmb_records, cmo_values, flow_cut_mapping) =
        compute_cmb_cmo_data(system, None, &cmo_solution, &empty_cuts);

    Ok(IterativeSimulationResult {
        steps,
        cmo_solution,
        final_violations: Vec::new(),
        accumulated_flow_cuts: 0,
        final_line_flows: Vec::new(),
        added_flow_cut_lines: Vec::new(),
        network_infeasibilities: Vec::new(),
        operational_limit_infeasibilities,
        cmb_records,
        cmo_values,
        flow_cut_mapping,
    })
}

fn run_simulation_without_network(
    system: &System,
    mut engmax_bounds: Option<EngMaxBounds>,
    engmax_enabled: bool,
    solver: &mut HighsSolver,
) -> Result<IterativeSimulationResult, SimulationError> {
    let mut steps = Vec::new();
    let mut operational_limit_infeasibilities = Vec::new();

    let lp_summary = solve_stage_without_network(
        system,
        SolveMode::LinearProgramming,
        None,
        engmax_bounds.as_ref(),
        engmax_enabled,
        IterationStage::LinearProgramming,
        1,
        &mut steps,
        &mut operational_limit_infeasibilities,
        solver,
    )?;

    // Recalculate EngMax bounds after LP if enabled
    if let Some(ref mut bounds) = engmax_bounds {
        let indexing = labdessem_model::indexing::Indexing::from_system(system);
        recalculate_engmax_bounds(system, &lp_summary, &indexing, bounds);
    }

    let milp_summary = solve_stage_without_network(
        system,
        SolveMode::MixedIntegerLinearProgramming,
        None,
        engmax_bounds.as_ref(),
        engmax_enabled,
        IterationStage::MixedIntegerLinearProgramming,
        1,
        &mut steps,
        &mut operational_limit_infeasibilities,
        solver,
    )?;

    // Recalculate EngMax bounds after MILP if enabled
    if let Some(ref mut bounds) = engmax_bounds {
        let indexing = labdessem_model::indexing::Indexing::from_system(system);
        recalculate_engmax_bounds(system, &milp_summary, &indexing, bounds);
    }

    let cmo_solution = solve_stage_without_network(
        system,
        SolveMode::LinearProgrammingWithFixedCommitment,
        Some(&milp_summary),
        engmax_bounds.as_ref(),
        engmax_enabled,
        IterationStage::LinearProgrammingCalculateCmo,
        1,
        &mut steps,
        &mut operational_limit_infeasibilities,
        solver,
    )?;

    let empty_cuts = BTreeSet::new();
    let (cmb_records, cmo_values, flow_cut_mapping) =
        compute_cmb_cmo_data(system, None, &cmo_solution, &empty_cuts);

    Ok(IterativeSimulationResult {
        steps,
        cmo_solution,
        final_violations: Vec::new(),
        accumulated_flow_cuts: 0,
        final_line_flows: Vec::new(),
        added_flow_cut_lines: Vec::new(),
        network_infeasibilities: Vec::new(),
        operational_limit_infeasibilities,
        cmb_records,
        cmo_values,
        flow_cut_mapping,
    })
}
