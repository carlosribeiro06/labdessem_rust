//! Simulation solve stages and workflow orchestration.

use std::collections::{BTreeMap, BTreeSet};
use std::io::{self, Write};
use std::time::Instant;

use labdessem_core::system::{OperationalLimitTarget, OperationalLimitVariable, System};
use labdessem_io::output::volume_hm3_to_flow_m3s;
use labdessem_model::{
    constraints::{ConstraintSense, LinearConstraint, LinearTerm},
    objective::ObjectiveTerm,
    variables::{Variable, VariableDomain},
    Model, SolveMode,
};
use labdessem_solver::{solve_model, SolverError, SolveSummary};

use crate::error::SimulationError;
use crate::network::{
    all_flow_constraints, compute_line_flows, constant_load_shift, detect_flow_violations,
    flow_terms_for_cut, DcNetworkModel, FlowCutKey,
};
use crate::results::{
    AddedFlowCutLineResult, FlowViolation, IterationReport, IterativeSimulationResult,
    LineFlowResult, NetworkInfeasibilityRecord, OperationalLimitInfeasibilityRecord,
};
use crate::workflow::{ExecutionOption, IterationStage};

const NETWORK_FLOW_SLACK_PENALTY_PER_MW: f64 = 1_000_000.0;

/// Run simulation with the specified execution option.
pub fn run_simulation(
    system: &System,
    network_enabled: bool,
    execution_option: ExecutionOption,
) -> Result<IterativeSimulationResult, SimulationError> {
    match execution_option {
        ExecutionOption::Iterative => run_iterative_simulation_strategy(system, network_enabled),
        ExecutionOption::SingleMilp => run_single_milp_simulation(system, network_enabled),
    }
}

fn run_iterative_simulation_strategy(
    system: &System,
    network_enabled: bool,
) -> Result<IterativeSimulationResult, SimulationError> {
    if !network_enabled {
        return run_simulation_without_network(system);
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
    )?;

    // Bifurcation: skip LP-FIXED loop if no violations or MILP was infeasible
    if milp_violations.is_empty() || milp_used_slacks {
        let all_cuts = all_flow_constraints(system);
        let cmo_summary = solve_stage(
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
        )?
        .0;
        let final_line_flows = compute_line_flows(system, &dc_network, &cmo_summary);
        let added_flow_cut_lines =
            summarize_added_flow_cut_lines(system, &accumulated_cuts, &final_line_flows);

        return Ok(IterativeSimulationResult {
            steps,
            cmo_summary,
            final_violations: milp_violations,
            accumulated_flow_cuts: accumulated_cuts.len(),
            final_line_flows,
            added_flow_cut_lines,
            network_infeasibilities,
            operational_limit_infeasibilities,
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
    let cmo_summary = solve_stage(
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
    )?
    .0;

    let final_line_flows = compute_line_flows(system, &dc_network, &cmo_summary);
    let added_flow_cut_lines =
        summarize_added_flow_cut_lines(system, &accumulated_cuts, &final_line_flows);

    Ok(IterativeSimulationResult {
        steps,
        cmo_summary,
        final_violations,
        accumulated_flow_cuts: accumulated_cuts.len(),
        final_line_flows,
        added_flow_cut_lines,
        network_infeasibilities,
        operational_limit_infeasibilities,
    })
}

fn run_single_milp_simulation(
    system: &System,
    network_enabled: bool,
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
        )?;

        let cmo_summary = solve_stage(
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
        )?
        .0;

        let final_line_flows = compute_line_flows(system, &dc_network, &cmo_summary);
        let added_flow_cut_lines =
            summarize_added_flow_cut_lines(system, &accumulated_cuts, &final_line_flows);

        return Ok(IterativeSimulationResult {
            steps,
            cmo_summary,
            final_violations,
            accumulated_flow_cuts: accumulated_cuts.len(),
            final_line_flows,
            added_flow_cut_lines,
            network_infeasibilities,
            operational_limit_infeasibilities,
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
    )?;

    let cmo_summary = solve_stage_without_network(
        system,
        SolveMode::LinearProgrammingWithFixedCommitment,
        Some(&milp_summary),
        IterationStage::LinearProgrammingCalculateCmo,
        1,
        &mut steps,
        &mut operational_limit_infeasibilities,
    )?;

    Ok(IterativeSimulationResult {
        steps,
        cmo_summary,
        final_violations: Vec::new(),
        accumulated_flow_cuts: 0,
        final_line_flows: Vec::new(),
        added_flow_cut_lines: Vec::new(),
        network_infeasibilities: Vec::new(),
        operational_limit_infeasibilities,
    })
}

fn run_simulation_without_network(
    system: &System,
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
    )?;

    let milp_summary = solve_stage_without_network(
        system,
        SolveMode::MixedIntegerLinearProgramming,
        None,
        IterationStage::MixedIntegerLinearProgramming,
        1,
        &mut steps,
        &mut operational_limit_infeasibilities,
    )?;

    let cmo_summary = solve_stage_without_network(
        system,
        SolveMode::LinearProgrammingWithFixedCommitment,
        Some(&milp_summary),
        IterationStage::LinearProgrammingCalculateCmo,
        1,
        &mut steps,
        &mut operational_limit_infeasibilities,
    )?;

    Ok(IterativeSimulationResult {
        steps,
        cmo_summary,
        final_violations: Vec::new(),
        accumulated_flow_cuts: 0,
        final_line_flows: Vec::new(),
        added_flow_cut_lines: Vec::new(),
        network_infeasibilities: Vec::new(),
        operational_limit_infeasibilities,
    })
}

fn run_lp_cut_loop(
    system: &System,
    dc_network: &DcNetworkModel,
    accumulated_cuts: &mut BTreeSet<FlowCutKey>,
    steps: &mut Vec<IterationReport>,
    network_infeasibilities: &mut Vec<NetworkInfeasibilityRecord>,
    operational_limit_infeasibilities: &mut Vec<OperationalLimitInfeasibilityRecord>,
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
    commitment_source: Option<&SolveSummary>,
    stage: IterationStage,
    iteration: usize,
    steps: &mut Vec<IterationReport>,
    network_infeasibilities: &mut Vec<NetworkInfeasibilityRecord>,
    operational_limit_infeasibilities: &mut Vec<OperationalLimitInfeasibilityRecord>,
) -> Result<(SolveSummary, Vec<FlowViolation>, bool), SimulationError> {
    println!("Resolvendo {} #{:02}...", stage.label(), iteration);
    io::stdout().flush().ok();

    let mut model = Model::from_system(system, solve_mode);
    if let Some(commitment_source) = commitment_source {
        apply_commitment_fixes(&mut model, commitment_source);
    }
    add_flow_cuts(&mut model, system, accumulated_cuts, dc_network, false);
    let (variable_count, constraint_count) = model_dimensions(&model);

    let started_at = Instant::now();
    let mut used_slacks = false;
    let summary = match solve_model(&model) {
        Ok(summary) => summary,
        Err(SolverError::InfeasibleOrUnbounded(_))
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
                apply_commitment_fixes(&mut relaxed_model, commitment_source);
            }
            add_flow_cuts(
                &mut relaxed_model,
                system,
                accumulated_cuts,
                dc_network,
                true,
            );
            add_operational_limit_slacks(&mut relaxed_model);

            let relaxed_summary = solve_model(&relaxed_model)?;
            network_infeasibilities.extend(collect_network_infeasibilities(
                system,
                accumulated_cuts,
                &relaxed_summary,
                stage,
                iteration,
            ));
            operational_limit_infeasibilities.extend(collect_operational_limit_infeasibilities(
                system,
                &relaxed_summary,
                stage,
                iteration,
            ));
            relaxed_summary
        }
        Err(error) => return Err(error.into()),
    };
    let solve_time = started_at.elapsed();
    let violations = detect_flow_violations(system, dc_network, &summary);

    println!(
        "{} #{:02} concluido | objective = {:.4} | flow violations = {} | solve time = {:.3} s",
        stage.label(),
        iteration,
        summary.objective_value,
        violations.len(),
        solve_time.as_secs_f64()
    );
    io::stdout().flush().ok();

    steps.push(IterationReport {
        stage,
        iteration,
        variable_count,
        constraint_count,
        objective_value: summary.objective_value,
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
    commitment_source: Option<&SolveSummary>,
    stage: IterationStage,
    iteration: usize,
    steps: &mut Vec<IterationReport>,
    operational_limit_infeasibilities: &mut Vec<OperationalLimitInfeasibilityRecord>,
) -> Result<SolveSummary, SimulationError> {
    println!("Resolvendo {} #{:02}...", stage.label(), iteration);
    io::stdout().flush().ok();

    let mut model = Model::from_system(system, solve_mode);
    if let Some(commitment_source) = commitment_source {
        apply_commitment_fixes(&mut model, commitment_source);
    }
    let (variable_count, constraint_count) = model_dimensions(&model);

    let started_at = Instant::now();
    let summary = match solve_model(&model) {
        Ok(summary) => summary,
        Err(SolverError::InfeasibleOrUnbounded(_)) if !system.operational_limits.is_empty() => {
            println!(
                "{} #{:02} inviavel com restricoes de limite. Adicionando folgas...",
                stage.label(),
                iteration
            );
            io::stdout().flush().ok();

            let mut relaxed_model = Model::from_system(system, solve_mode);
            if let Some(commitment_source) = commitment_source {
                apply_commitment_fixes(&mut relaxed_model, commitment_source);
            }
            add_operational_limit_slacks(&mut relaxed_model);

            let relaxed_summary = solve_model(&relaxed_model)?;
            operational_limit_infeasibilities.extend(collect_operational_limit_infeasibilities(
                system,
                &relaxed_summary,
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
        summary.objective_value,
        solve_time.as_secs_f64()
    );
    io::stdout().flush().ok();

    steps.push(IterationReport {
        stage,
        iteration,
        variable_count,
        constraint_count,
        objective_value: summary.objective_value,
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

fn apply_commitment_fixes(model: &mut Model, source: &SolveSummary) {
    let variable_vectors = [
        &mut model.variables.thermal_commitment,
        &mut model.variables.thermal_startup,
        &mut model.variables.thermal_shutdown,
        &mut model.variables.hydro_commitment,
    ];

    for variables in variable_vectors {
        for variable in variables {
            let value = source
                .variable_values
                .get(&variable.name)
                .copied()
                .unwrap_or(0.0);
            variable.fixed_value = Some(match variable.domain {
                VariableDomain::Binary => {
                    if value >= 0.5 {
                        1.0
                    } else {
                        0.0
                    }
                }
                VariableDomain::Continuous => value,
            });
        }
    }
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
) {
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
    }
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
    summary: &SolveSummary,
    stage: IterationStage,
    iteration: usize,
) -> Vec<NetworkInfeasibilityRecord> {
    let mut records = Vec::new();

    for cut_key in cut_keys {
        let branch = &system.branches[cut_key.branch_idx];
        let upper_violation = summary
            .variable_values
            .get(&flow_slack_name(
                "flow_slack_upper",
                branch.name.as_str(),
                cut_key.period,
            ))
            .copied()
            .unwrap_or(0.0);
        let lower_violation = summary
            .variable_values
            .get(&flow_slack_name(
                "flow_slack_lower",
                branch.name.as_str(),
                cut_key.period,
            ))
            .copied()
            .unwrap_or(0.0);
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
    summary: &SolveSummary,
    stage: IterationStage,
    iteration: usize,
) -> Vec<OperationalLimitInfeasibilityRecord> {
    let mut aggregated =
        BTreeMap::<(usize, usize, String, OperationalLimitVariable), (f64, f64)>::new();

    for limit in &system.operational_limits {
        let plant_code = match limit.target {
            OperationalLimitTarget::ThermalPlant(id) => id.0,
            OperationalLimitTarget::HydroPlant(id) => id.0,
            OperationalLimitTarget::PumpingPlant(id) => id.0,
        };

        for period in limit.start_period..=limit.end_period {
            let lower_violation = summary
                .variable_values
                .get(&operational_limit_slack_name(
                    true,
                    limit.plant_name.as_str(),
                    limit.variable,
                    period,
                ))
                .copied()
                .unwrap_or(0.0);
            let upper_violation = summary
                .variable_values
                .get(&operational_limit_slack_name(
                    false,
                    limit.plant_name.as_str(),
                    limit.variable,
                    period,
                ))
                .copied()
                .unwrap_or(0.0);

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

fn operational_limit_slack_name(
    is_lower: bool,
    plant_name: &str,
    variable: OperationalLimitVariable,
    period: usize,
) -> String {
    let bound = if is_lower { "lower" } else { "upper" };
    format!(
        "operational_limit_slack_{bound}[p={plant_name},var={},t={period}]",
        operational_limit_variable_label(variable)
    )
}

fn operational_limit_variable_label(variable: OperationalLimitVariable) -> &'static str {
    match variable {
        OperationalLimitVariable::Generation => "GHID",
        OperationalLimitVariable::Volume => "VOL",
        OperationalLimitVariable::Spillage => "VERT",
        OperationalLimitVariable::Defluence => "DEFL",
        OperationalLimitVariable::Turbining => "TURB",
        OperationalLimitVariable::Diversion => "DIV",
        OperationalLimitVariable::Pumping => "QBOM",
    }
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
        + variables.deficit.len()
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
