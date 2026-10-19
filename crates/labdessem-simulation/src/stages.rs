//! Simulation solve stages and workflow orchestration.

use std::collections::{BTreeMap, BTreeSet, HashMap};
use std::io::{self, Write};
use std::time::Instant;

use labdessem_core::ids::SubmarketId;
use labdessem_core::system::{OperationalLimitTarget, OperationalLimitVariable, System};
use labdessem_io::output::volume_hm3_to_flow_m3s;
use labdessem_model::{
    constraints::{ConstraintSense, LinearConstraint, LinearTerm},
    indexing::ColumnOffsets,
    objective::ObjectiveTerm,
    variables::{Variable, VariableDomain},
    Model, SolveMode,
};
use labdessem_solver::{
    model_to_lp_template, HighsSolver, LpSolution, SolverError, SolverInterface,
};

use crate::error::SimulationError;
use crate::network::{
    all_flow_constraints, compute_cmb_per_bus, compute_cmo_from_cmb, compute_line_flows,
    constant_load_shift, detect_flow_violations, extract_flow_duals, flow_terms_for_cut,
    CmbRecord, DcNetworkModel, FlowCutKey,
};
use crate::results::{
    AddedFlowCutLineResult, FlowCutMapping, FlowViolation, IterationReport,
    IterativeSimulationResult, LineFlowResult, NetworkInfeasibilityRecord,
    OperationalLimitInfeasibilityRecord,
};
use crate::workflow::{ExecutionOption, IterationStage};

const NETWORK_FLOW_SLACK_PENALTY_PER_MW: f64 = 1_000_000.0;

/// Solve a model using the HiGHS solver, returning LpSolution directly.
fn solve_model_direct(
    solver: &mut HighsSolver,
    model: &Model,
) -> Result<LpSolution, SolverError> {
    let template = model_to_lp_template(model);
    solver.load_model(&template);
    solver.solve(None).map(|view| view.to_owned())
}

/// Run simulation with the specified execution option.
pub fn run_simulation(
    system: &System,
    network_enabled: bool,
    execution_option: ExecutionOption,
) -> Result<IterativeSimulationResult, SimulationError> {
    // Create solver once and reuse across all stages
    let mut solver = HighsSolver::new().map_err(|e| {
        SimulationError::IterativeProcess(format!("Failed to create solver: {e}"))
    })?;

    match execution_option {
        ExecutionOption::Iterative => {
            run_iterative_simulation_strategy(system, network_enabled, &mut solver)
        }
        ExecutionOption::SingleMilp => {
            run_single_milp_simulation(system, network_enabled, &mut solver)
        }
    }
}

fn run_iterative_simulation_strategy(
    system: &System,
    network_enabled: bool,
    solver: &mut HighsSolver,
) -> Result<IterativeSimulationResult, SimulationError> {
    if !network_enabled {
        return run_simulation_without_network(system, solver);
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
        &mut steps,
        &mut network_infeasibilities,
        &mut operational_limit_infeasibilities,
        solver,
    )?;

    let (milp_summary, milp_violations, milp_used_slacks) = solve_stage(
        system,
        &dc_network,
        SolveMode::MixedIntegerLinearProgramming,
        &accumulated_cuts,
        None,
        IterationStage::MixedIntegerLinearProgramming,
        1,
        &mut steps,
        &mut network_infeasibilities,
        &mut operational_limit_infeasibilities,
        solver,
    )?;

    // Bifurcation: skip LP-FIXED loop if no violations or MILP was infeasible
    if milp_violations.is_empty() || milp_used_slacks {
        let all_cuts = all_flow_constraints(system);
        let cmo_solution = solve_stage(
            system,
            &dc_network,
            SolveMode::LinearProgrammingWithFixedCommitment,
            &all_cuts,
            Some(&milp_summary),
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
        let final_line_flows = compute_line_flows(system, &dc_network, &cmo_solution, &col_offsets);
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

    // Has violations and MILP was feasible — add cuts and enter LP-FIXED loop
    extend_cut_set(&mut accumulated_cuts, system, &milp_violations);

    let mut fixed_iteration = 1usize;
    let final_violations;
    loop {
        let (_, violations, used_slacks) = solve_stage(
            system,
            &dc_network,
            SolveMode::LinearProgrammingWithFixedCommitment,
            &accumulated_cuts,
            Some(&milp_summary),
            IterationStage::LinearProgrammingWithFixedCommitment,
            fixed_iteration,
            &mut steps,
            &mut network_infeasibilities,
            &mut operational_limit_infeasibilities,
            solver,
        )?;

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
    let final_line_flows = compute_line_flows(system, &dc_network, &cmo_solution, &col_offsets);
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
            IterationStage::MixedIntegerLinearProgramming,
            1,
            &mut steps,
            &mut network_infeasibilities,
            &mut operational_limit_infeasibilities,
            solver,
        )?;

        let cmo_solution = solve_stage(
            system,
            &dc_network,
            SolveMode::LinearProgrammingWithFixedCommitment,
            &accumulated_cuts,
            Some(&milp_summary),
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
        let final_line_flows = compute_line_flows(system, &dc_network, &cmo_solution, &col_offsets);
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
        IterationStage::MixedIntegerLinearProgramming,
        1,
        &mut steps,
        &mut operational_limit_infeasibilities,
        solver,
    )?;

    let cmo_solution = solve_stage_without_network(
        system,
        SolveMode::LinearProgrammingWithFixedCommitment,
        Some(&milp_summary),
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
    solver: &mut HighsSolver,
) -> Result<IterativeSimulationResult, SimulationError> {
    let mut steps = Vec::new();
    let mut operational_limit_infeasibilities = Vec::new();

    let _lp_summary = solve_stage_without_network(
        system,
        SolveMode::LinearProgramming,
        None,
        IterationStage::LinearProgramming,
        1,
        &mut steps,
        &mut operational_limit_infeasibilities,
        solver,
    )?;

    let milp_summary = solve_stage_without_network(
        system,
        SolveMode::MixedIntegerLinearProgramming,
        None,
        IterationStage::MixedIntegerLinearProgramming,
        1,
        &mut steps,
        &mut operational_limit_infeasibilities,
        solver,
    )?;

    let cmo_solution = solve_stage_without_network(
        system,
        SolveMode::LinearProgrammingWithFixedCommitment,
        Some(&milp_summary),
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

fn run_lp_cut_loop(
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

// Rationale: solve_stage needs all parameters for complete stage execution context
#[allow(clippy::too_many_arguments)]
fn solve_stage(
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
                apply_commitment_fixes(&mut relaxed_model, commitment_source, &col_offsets, horizon);
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
    let violations = detect_flow_violations(system, dc_network, &summary, &col_offsets);

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

fn solve_stage_without_network(
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
        Err(SolverError::Infeasible) | Err(SolverError::Unbounded) if !system.operational_limits.is_empty() => {
            println!(
                "{} #{:02} inviavel com restricoes de limite. Adicionando folgas...",
                stage.label(),
                iteration
            );
            io::stdout().flush().ok();

            let mut relaxed_model = Model::from_system(system, solve_mode);
            if let Some(commitment_source) = commitment_source {
                apply_commitment_fixes(&mut relaxed_model, commitment_source, &col_offsets, horizon);
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

fn apply_commitment_fixes(
    model: &mut Model,
    source: &LpSolution,
    col_offsets: &ColumnOffsets,
    horizon: usize,
) {
    // Thermal commitment
    for (idx, variable) in model.variables.thermal_commitment.iter_mut().enumerate() {
        let value = source.primal[col_offsets.thermal_commitment + idx];
        variable.fixed_value = Some(if value >= 0.5 { 1.0 } else { 0.0 });
    }

    // Thermal startup
    for (idx, variable) in model.variables.thermal_startup.iter_mut().enumerate() {
        let value = source.primal[col_offsets.thermal_startup + idx];
        variable.fixed_value = Some(if value >= 0.5 { 1.0 } else { 0.0 });
    }

    // Thermal shutdown
    for (idx, variable) in model.variables.thermal_shutdown.iter_mut().enumerate() {
        let value = source.primal[col_offsets.thermal_shutdown + idx];
        variable.fixed_value = Some(if value >= 0.5 { 1.0 } else { 0.0 });
    }

    // Hydro commitment
    for (idx, variable) in model.variables.hydro_commitment.iter_mut().enumerate() {
        let value = source.primal[col_offsets.hydro_commitment + idx];
        variable.fixed_value = Some(if value >= 0.5 { 1.0 } else { 0.0 });
    }

    // Also need to mark the horizon for later use
    let _ = horizon; // Used for offset calculation verification if needed
}

fn extend_cut_set(
    cut_set: &mut BTreeSet<FlowCutKey>,
    system: &System,
    violations: &[FlowViolation],
) -> usize {
    let mut new_cuts = 0usize;

    for violation in violations {
        if let Some(branch_idx) = system
            .branches
            .iter()
            .position(|branch| branch.id == violation.branch_id)
        {
            let inserted = cut_set.insert(FlowCutKey {
                branch_idx,
                period: violation.period - 1,
            });
            if inserted {
                new_cuts += 1;
            }
        }
    }

    new_cuts
}

fn add_flow_cuts(
    model: &mut Model,
    system: &System,
    cut_keys: &BTreeSet<FlowCutKey>,
    dc_network: &DcNetworkModel,
    use_slack_variables: bool,
) -> FlowCutMapping {
    let mut mapping = FlowCutMapping::default();
    let start_row = model.constraints.linear_constraints.len();

    for cut_key in cut_keys {
        let branch = &system.branches[cut_key.branch_idx];
        let rhs_shift = constant_load_shift(system, dc_network, cut_key.branch_idx, cut_key.period);
        let terms = flow_terms_for_cut(
            model,
            system,
            dc_network,
            cut_key.branch_idx,
            cut_key.period,
        );
        let mut upper_terms = terms.clone();
        let mut lower_terms = terms;

        if use_slack_variables {
            let upper_slack_name =
                flow_slack_name("flow_slack_upper", branch.name.as_str(), cut_key.period);
            let lower_slack_name =
                flow_slack_name("flow_slack_lower", branch.name.as_str(), cut_key.period);
            add_network_flow_slack_variable(model, &upper_slack_name);
            add_network_flow_slack_variable(model, &lower_slack_name);

            upper_terms.push(LinearTerm {
                variable: upper_slack_name,
                coefficient: -1.0,
            });
            lower_terms.push(LinearTerm {
                variable: lower_slack_name,
                coefficient: 1.0,
            });
        }

        let upper_row_idx = model.constraints.linear_constraints.len() - start_row;
        model.constraints.linear_constraints.push(LinearConstraint {
            name: format!(
                "flow_cut_upper[branch={},t={}]",
                branch.name,
                cut_key.period + 1
            ),
            terms: upper_terms,
            sense: ConstraintSense::LessOrEqual,
            rhs: branch.thermal_limit_mw + rhs_shift,
        });

        let lower_row_idx = model.constraints.linear_constraints.len() - start_row;
        model.constraints.linear_constraints.push(LinearConstraint {
            name: format!(
                "flow_cut_lower[branch={},t={}]",
                branch.name,
                cut_key.period + 1
            ),
            terms: lower_terms,
            sense: ConstraintSense::GreaterOrEqual,
            rhs: -branch.thermal_limit_mw + rhs_shift,
        });

        mapping
            .indices
            .insert((cut_key.branch_idx, cut_key.period), (upper_row_idx, lower_row_idx));
    }

    mapping
}

/// Compute CMB and CMO data from LP-CALC-CMO solution.
///
/// When network is enabled, computes:
/// - CMB per (bus, period) using PTDF and flow duals
/// - CMO per (submarket, period) as demand-weighted average of CMBs
///
/// When network is disabled, returns:
/// - Empty CMB records
/// - CMO = PiDemanda (demand balance dual)
fn compute_cmb_cmo_data(
    system: &System,
    dc_network: Option<&DcNetworkModel>,
    cmo_solution: &LpSolution,
    all_cuts: &BTreeSet<FlowCutKey>,
) -> (Vec<CmbRecord>, HashMap<(SubmarketId, usize), f64>, Option<FlowCutMapping>) {
    let horizon = system.horizon.periods;
    let periods: Vec<usize> = (0..horizon).collect();

    // When network is disabled or no dc_network or no cuts, CMO = PiDemanda
    let dc_network = match dc_network {
        Some(dc) if !all_cuts.is_empty() => dc,
        _ => {
            // No network: CMO = demand dual from model. Since we don't have easy access
            // to row offsets without flow cuts, use SolutionAccessor pattern.
            let indexing = labdessem_model::indexing::Indexing::from_system(system);
            let constraint_params = labdessem_model::indexing::ConstraintOffsetParams {
                horizon,
                fpha_enabled: system.fpha_enabled,
                num_fpha_segments: system.hydro_plants.iter().map(|p| p.fpha_segments.len()).sum(),
                future_cost_enabled: system.future_cost_enabled,
                num_future_cost_cuts: system.future_cost_cuts.len(),
                num_operational_limits: system.operational_limits.len(),
                thermal_uct_enabled: system.thermal_unit_commitment_enabled,
                hydro_uch_enabled: system.hydro_unit_commitment_enabled,
                is_commitment_mode: true,
            };
            let row_offsets = indexing.compute_constraint_offsets(&constraint_params);

            let mut cmo_values = HashMap::new();
            for (submarket_idx, submarket) in system.submarkets.iter().enumerate() {
                for period in 0..horizon {
                    let row_idx = row_offsets.demand_balance + submarket_idx * horizon + period;
                    let dual = cmo_solution.dual.get(row_idx).copied().unwrap_or(0.0);
                    let cmo = dual / system.horizon.period_duration_hours;
                    cmo_values.insert((submarket.id, period), cmo);
                }
            }
            return (Vec::new(), cmo_values, None);
        }
    };

    // With network enabled: calculate base constraint count by counting all constraints
    // BEFORE flow cuts. The flow cuts add 2 constraints per cut (upper + lower).
    // Total duals = base_constraints + 2 * num_cuts
    let num_flow_cut_constraints = all_cuts.len() * 2;
    let base_constraint_count = cmo_solution.dual.len().saturating_sub(num_flow_cut_constraints);

    // Extract demand balance duals
    let indexing = labdessem_model::indexing::Indexing::from_system(system);
    let constraint_params = labdessem_model::indexing::ConstraintOffsetParams {
        horizon,
        fpha_enabled: system.fpha_enabled,
        num_fpha_segments: system.hydro_plants.iter().map(|p| p.fpha_segments.len()).sum(),
        future_cost_enabled: system.future_cost_enabled,
        num_future_cost_cuts: system.future_cost_cuts.len(),
        num_operational_limits: system.operational_limits.len(),
        thermal_uct_enabled: system.thermal_unit_commitment_enabled,
        hydro_uch_enabled: system.hydro_unit_commitment_enabled,
        is_commitment_mode: true,
    };
    let row_offsets = indexing.compute_constraint_offsets(&constraint_params);

    let mut demand_duals = HashMap::new();
    for (submarket_idx, submarket) in system.submarkets.iter().enumerate() {
        for period in 0..horizon {
            let row_idx = row_offsets.demand_balance + submarket_idx * horizon + period;
            let dual = cmo_solution.dual.get(row_idx).copied().unwrap_or(0.0);
            demand_duals.insert((submarket.id, period), dual);
        }
    }

    // Build mapping by simulating add_flow_cuts logic
    let mut mapping = FlowCutMapping::default();
    for (idx, cut_key) in all_cuts.iter().enumerate() {
        let upper_row_idx = idx * 2;
        let lower_row_idx = idx * 2 + 1;
        mapping.indices.insert(
            (cut_key.branch_idx, cut_key.period),
            (upper_row_idx, lower_row_idx),
        );
    }

    // Extract flow duals
    let flow_duals = extract_flow_duals(cmo_solution, &mapping, base_constraint_count);

    // Compute CMB per bus
    let cmb_records = compute_cmb_per_bus(system, dc_network, &demand_duals, &flow_duals, &periods);

    // Compute CMO as demand-weighted average of CMBs
    let cmo_values = compute_cmo_from_cmb(&cmb_records, system);

    // Convert to R$/MWh by dividing by period duration
    let cmo_values: HashMap<(SubmarketId, usize), f64> = cmo_values
        .into_iter()
        .map(|(key, val)| (key, val / system.horizon.period_duration_hours))
        .collect();

    (cmb_records, cmo_values, Some(mapping))
}

fn add_network_flow_slack_variable(model: &mut Model, name: &str) {
    model.variables.network_flow_slack.push(Variable {
        name: name.to_string(),
        lower_bound: 0.0,
        upper_bound: None,
        domain: VariableDomain::Continuous,
        fixed_value: None,
    });
    model.objective.terms.push(ObjectiveTerm {
        variable: name.to_string(),
        coefficient: NETWORK_FLOW_SLACK_PENALTY_PER_MW,
    });
}

fn collect_network_infeasibilities(
    system: &System,
    cut_keys: &BTreeSet<FlowCutKey>,
    solution: &LpSolution,
    col_offsets: &ColumnOffsets,
    stage: IterationStage,
    iteration: usize,
) -> Vec<NetworkInfeasibilityRecord> {
    let mut records = Vec::new();

    // Network flow slacks are added after all standard variables.
    // They are added in pairs (upper, lower) for each cut in cut_keys order.
    let mut slack_idx = 0;
    for cut_key in cut_keys {
        let branch = &system.branches[cut_key.branch_idx];
        
        // Upper slack
        let upper_violation = solution.primal
            .get(col_offsets.network_flow_slack + slack_idx)
            .copied()
            .unwrap_or(0.0);
        slack_idx += 1;
        
        // Lower slack
        let lower_violation = solution.primal
            .get(col_offsets.network_flow_slack + slack_idx)
            .copied()
            .unwrap_or(0.0);
        slack_idx += 1;
        
        let violation_mw = upper_violation + lower_violation;

        if violation_mw > 1e-8 {
            records.push(NetworkInfeasibilityRecord {
                stage,
                iteration,
                period: cut_key.period + 1,
                branch_name: branch.name.clone(),
                violation_mw,
            });
        }
    }

    records
}

fn add_operational_limit_slacks(model: &mut Model) {
    let mut slack_specs = Vec::<(usize, String, f64)>::new();

    for (constraint_idx, constraint) in model.constraints.linear_constraints.iter().enumerate() {
        if let Some(suffix) = constraint.name.strip_prefix("operational_limit_lower") {
            slack_specs.push((
                constraint_idx,
                format!("operational_limit_slack_lower{suffix}"),
                1.0,
            ));
        } else if let Some(suffix) = constraint.name.strip_prefix("operational_limit_upper") {
            slack_specs.push((
                constraint_idx,
                format!("operational_limit_slack_upper{suffix}"),
                -1.0,
            ));
        }
    }

    for (constraint_idx, slack_name, coefficient) in slack_specs {
        model.variables.operational_limit_slack.push(Variable {
            name: slack_name.clone(),
            lower_bound: 0.0,
            upper_bound: None,
            domain: VariableDomain::Continuous,
            fixed_value: None,
        });
        model.objective.terms.push(ObjectiveTerm {
            variable: slack_name.clone(),
            coefficient: NETWORK_FLOW_SLACK_PENALTY_PER_MW,
        });
        model.constraints.linear_constraints[constraint_idx]
            .terms
            .push(LinearTerm {
                variable: slack_name,
                coefficient,
            });
    }
}

fn collect_operational_limit_infeasibilities(
    system: &System,
    solution: &LpSolution,
    col_offsets: &ColumnOffsets,
    stage: IterationStage,
    iteration: usize,
) -> Vec<OperationalLimitInfeasibilityRecord> {
    let mut aggregated =
        BTreeMap::<(usize, usize, String, OperationalLimitVariable), (f64, f64)>::new();

    // Operational limit slacks are added after network flow slacks.
    // They are added in pairs (lower, upper) for each operational limit constraint.
    // The order follows the constraint iteration order in add_operational_limit_slacks.
    let mut slack_idx = 0;
    
    for limit in &system.operational_limits {
        let plant_code = match limit.target {
            OperationalLimitTarget::ThermalPlant(id) => id.0,
            OperationalLimitTarget::HydroPlant(id) => id.0,
            OperationalLimitTarget::PumpingPlant(id) => id.0,
        };

        for period in limit.start_period..=limit.end_period {
            // Lower bound slack
            let lower_violation = solution.primal
                .get(col_offsets.operational_limit_slack + slack_idx)
                .copied()
                .unwrap_or(0.0);
            slack_idx += 1;
            
            // Upper bound slack  
            let upper_violation = solution.primal
                .get(col_offsets.operational_limit_slack + slack_idx)
                .copied()
                .unwrap_or(0.0);
            slack_idx += 1;

            if lower_violation <= 1e-8 && upper_violation <= 1e-8 {
                continue;
            }

            let lower_violation =
                operational_limit_violation_for_output(system, limit.variable, lower_violation);
            let upper_violation =
                operational_limit_violation_for_output(system, limit.variable, upper_violation);
            let entry = aggregated
                .entry((period, plant_code, limit.plant_name.clone(), limit.variable))
                .or_insert((0.0, 0.0));
            entry.0 += lower_violation;
            entry.1 += upper_violation;
        }
    }

    aggregated
        .into_iter()
        .map(
            |((period, plant_code, plant_name, variable), (lower_violation, upper_violation))| {
                OperationalLimitInfeasibilityRecord {
                    stage,
                    iteration,
                    period,
                    plant_code,
                    plant_name,
                    variable,
                    unit: operational_limit_output_unit(variable),
                    lower_violation,
                    upper_violation,
                }
            },
        )
        .collect()
}

fn operational_limit_violation_for_output(
    system: &System,
    variable: OperationalLimitVariable,
    violation: f64,
) -> f64 {
    match variable {
        OperationalLimitVariable::Spillage
        | OperationalLimitVariable::Defluence
        | OperationalLimitVariable::Turbining
        | OperationalLimitVariable::Diversion
        | OperationalLimitVariable::Pumping => {
            volume_hm3_to_flow_m3s(violation, system.horizon.period_duration_hours)
        }
        OperationalLimitVariable::Generation | OperationalLimitVariable::Volume => violation,
    }
}

fn operational_limit_output_unit(variable: OperationalLimitVariable) -> &'static str {
    match variable {
        OperationalLimitVariable::Generation => "MW",
        OperationalLimitVariable::Volume => "hm3",
        OperationalLimitVariable::Spillage
        | OperationalLimitVariable::Defluence
        | OperationalLimitVariable::Turbining
        | OperationalLimitVariable::Diversion
        | OperationalLimitVariable::Pumping => "m3/s",
    }
}

pub fn summarize_added_flow_cut_lines(
    system: &System,
    cut_keys: &BTreeSet<FlowCutKey>,
    final_line_flows: &[LineFlowResult],
) -> Vec<AddedFlowCutLineResult> {
    let mut periods_by_branch = BTreeMap::<usize, Vec<usize>>::new();
    for cut_key in cut_keys {
        periods_by_branch
            .entry(cut_key.branch_idx)
            .or_default()
            .push(cut_key.period + 1);
    }

    let mut output = Vec::with_capacity(periods_by_branch.len());
    for (branch_idx, active_periods) in periods_by_branch {
        let branch = &system.branches[branch_idx];
        let selected_flow = final_line_flows
            .iter()
            .filter(|line_flow| {
                line_flow.branch_id == branch.id && active_periods.contains(&line_flow.period)
            })
            .max_by(|left, right| left.flow_mw.abs().total_cmp(&right.flow_mw.abs()));

        let flow_mw = selected_flow
            .map_or(0.0, |line_flow| line_flow.flow_mw);

        output.push(AddedFlowCutLineResult {
            branch_id: branch.id,
            branch_name: branch.name.clone(),
            from_bus_id: branch.from_bus_id,
            to_bus_id: branch.to_bus_id,
            flow_mw,
            limit_mw: branch.thermal_limit_mw,
        });
    }

    output
}

fn flow_slack_name(prefix: &str, branch_name: &str, period: usize) -> String {
    format!("{prefix}[branch={branch_name},t={}]", period + 1)
}

fn model_dimensions(model: &Model) -> (usize, usize) {
    let variables = &model.variables;
    let variable_count = variables.thermal_generation.len()
        + variables.hydro_generation.len()
        + variables.hydro_turbining.len()
        + variables.hydro_spillage.len()
        + variables.hydro_diversion.len()
        + variables.pumping.len()
        + variables.hydro_volume.len()
        + variables.future_cost.len()
        + variables.renewable_generation.len()
        + variables.interchange.len()
        + variables.thermal_commitment.len()
        + variables.thermal_startup.len()
        + variables.thermal_shutdown.len()
        + variables.hydro_commitment.len()
        + variables.network_flow_slack.len()
        + variables.operational_limit_slack.len();

    let constraint_count = model.constraints.linear_constraints.len();

    (variable_count, constraint_count)
}
