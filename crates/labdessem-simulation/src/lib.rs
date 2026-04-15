use std::{
    collections::{BTreeMap, BTreeSet, HashMap},
    error::Error,
    fmt, fs,
    io::{self, Write},
    path::Path,
    time::{Duration, Instant},
};

use labdessem_core::{
    ids::{BranchId, BusId},
    system::{OperationalLimitTarget, OperationalLimitVariable, System},
};
use labdessem_model::{
    Model, SolveMode,
    constraints::{ConstraintSense, LinearConstraint, LinearTerm},
    objective::ObjectiveTerm,
    variables::{Variable, VariableDomain},
};
use labdessem_solver::{SolveSummary, SolverError, solve_model};

const NETWORK_FLOW_SLACK_PENALTY_PER_MW: f64 = 1_000_000.0;

#[derive(Debug)]
pub enum SimulationError {
    Solver(SolverError),
    InvalidNetwork(String),
    IterativeProcess(String),
}

impl fmt::Display for SimulationError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Solver(error) => write!(f, "{error}"),
            Self::InvalidNetwork(message) | Self::IterativeProcess(message) => {
                write!(f, "{message}")
            }
        }
    }
}

impl Error for SimulationError {
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        match self {
            Self::Solver(error) => Some(error),
            Self::InvalidNetwork(_) | Self::IterativeProcess(_) => None,
        }
    }
}

impl From<SolverError> for SimulationError {
    fn from(value: SolverError) -> Self {
        Self::Solver(value)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum IterationStage {
    LinearProgramming,
    MixedIntegerLinearProgramming,
    LinearProgrammingWithFixedCommitment,
}

impl IterationStage {
    pub fn label(&self) -> &'static str {
        match self {
            Self::LinearProgramming => "LP",
            Self::MixedIntegerLinearProgramming => "MILP",
            Self::LinearProgrammingWithFixedCommitment => "LP-FIXED",
        }
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct FlowViolation {
    pub branch_id: BranchId,
    pub branch_name: String,
    pub period: usize,
    pub flow_mw: f64,
    pub limit_mw: f64,
}

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

#[derive(Debug, Clone)]
pub struct IterativeSimulationResult {
    pub steps: Vec<IterationReport>,
    pub final_summary: SolveSummary,
    pub final_violations: Vec<FlowViolation>,
    pub accumulated_flow_cuts: usize,
    pub final_line_flows: Vec<LineFlowResult>,
    pub added_flow_cut_lines: Vec<AddedFlowCutLineResult>,
    pub network_infeasibilities: Vec<NetworkInfeasibilityRecord>,
    pub operational_limit_infeasibilities: Vec<OperationalLimitInfeasibilityRecord>,
}

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

#[derive(Debug, Clone, PartialEq)]
pub struct AddedFlowCutLineResult {
    pub branch_id: BranchId,
    pub branch_name: String,
    pub from_bus_id: BusId,
    pub to_bus_id: BusId,
    pub flow_mw: f64,
    pub limit_mw: f64,
}

#[derive(Debug, Clone, PartialEq)]
pub struct NetworkInfeasibilityRecord {
    pub stage: IterationStage,
    pub iteration: usize,
    pub period: usize,
    pub branch_name: String,
    pub violation_mw: f64,
}

#[derive(Debug, Clone, PartialEq)]
pub struct OperationalLimitInfeasibilityRecord {
    pub stage: IterationStage,
    pub iteration: usize,
    pub period: usize,
    pub plant_code: usize,
    pub plant_name: String,
    pub lower_violation_mw: f64,
    pub upper_violation_mw: f64,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
struct FlowCutKey {
    branch_idx: usize,
    period: usize,
}

#[derive(Debug, Clone)]
struct DcNetworkModel {
    non_slack_bus_ids: Vec<BusId>,
    non_slack_bus_positions: HashMap<BusId, usize>,
    ptdf_rows: Vec<Vec<f64>>,
}

pub fn run_iterative_simulation(
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
    let milp_new_cuts = extend_cut_set(&mut accumulated_cuts, system, &milp_violations);
    if milp_used_slacks && milp_new_cuts == 0 {
        let final_line_flows = compute_line_flows(system, &dc_network, &milp_summary);
        let added_flow_cut_lines =
            summarize_added_flow_cut_lines(system, &accumulated_cuts, &final_line_flows);

        return Ok(IterativeSimulationResult {
            steps,
            final_summary: milp_summary,
            final_violations: milp_violations,
            accumulated_flow_cuts: accumulated_cuts.len(),
            final_line_flows,
            added_flow_cut_lines,
            network_infeasibilities,
            operational_limit_infeasibilities,
        });
    }

    let mut fixed_iteration = 1usize;
    let final_summary;
    let final_violations;
    loop {
        let (summary, violations, used_slacks) = solve_stage(
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
            final_summary = summary;
            final_violations = violations;
            break;
        }

        let new_cuts = extend_cut_set(&mut accumulated_cuts, system, &violations);
        if new_cuts == 0 {
            if used_slacks {
                final_summary = summary;
                final_violations = violations;
                break;
            }

            return Err(SimulationError::IterativeProcess(
                "LP with fixed commitment still has flow violations, but no new flow cut could be added".into(),
            ));
        }

        fixed_iteration += 1;
    }

    let final_line_flows = compute_line_flows(system, &dc_network, &final_summary);
    let added_flow_cut_lines =
        summarize_added_flow_cut_lines(system, &accumulated_cuts, &final_line_flows);

    Ok(IterativeSimulationResult {
        steps,
        final_summary,
        final_violations,
        accumulated_flow_cuts: accumulated_cuts.len(),
        final_line_flows,
        added_flow_cut_lines,
        network_infeasibilities,
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

    let final_summary = solve_stage_without_network(
        system,
        SolveMode::LinearProgrammingWithFixedCommitment,
        Some(&milp_summary),
        IterationStage::LinearProgrammingWithFixedCommitment,
        1,
        &mut steps,
        &mut operational_limit_infeasibilities,
    )?;

    Ok(IterativeSimulationResult {
        steps,
        final_summary,
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
    let violations = detect_flow_violations(system, dc_network, &summary)?;

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
        &mut model.variables.hydro_startup,
        &mut model.variables.hydro_shutdown,
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
    let mut aggregated = BTreeMap::<(usize, usize, String), (f64, f64)>::new();

    for limit in &system.operational_limits {
        let plant_code = match limit.target {
            OperationalLimitTarget::ThermalPlant(id) => id.0,
            OperationalLimitTarget::HydroPlant(id) => id.0,
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

            let entry = aggregated
                .entry((period, plant_code, limit.plant_name.clone()))
                .or_insert((0.0, 0.0));
            entry.0 += lower_violation;
            entry.1 += upper_violation;
        }
    }

    aggregated
        .into_iter()
        .map(
            |((period, plant_code, plant_name), (lower_violation_mw, upper_violation_mw))| {
                OperationalLimitInfeasibilityRecord {
                    stage,
                    iteration,
                    period,
                    plant_code,
                    plant_name,
                    lower_violation_mw,
                    upper_violation_mw,
                }
            },
        )
        .collect()
}

fn flow_terms_for_cut(
    model: &Model,
    system: &System,
    dc_network: &DcNetworkModel,
    branch_idx: usize,
    period: usize,
) -> Vec<LinearTerm> {
    let horizon = system.horizon.periods;
    let mut terms = Vec::new();

    for (entry_idx, entry) in model.indexing.thermal_unit_entries.iter().enumerate() {
        let plant = &system.thermal_plants[entry.plant_idx];
        let Some(bus_pos) = dc_network.non_slack_bus_positions.get(&plant.bus_id) else {
            continue;
        };
        let coefficient = dc_network.ptdf_rows[branch_idx][*bus_pos];
        if coefficient.abs() <= 1e-12 {
            continue;
        }

        let variable = &model.variables.thermal_generation[entry_idx * horizon + period];
        terms.push(LinearTerm {
            variable: variable.name.clone(),
            coefficient,
        });
    }

    for (entry_idx, entry) in model.indexing.hydro_unit_entries.iter().enumerate() {
        let plant = &system.hydro_plants[entry.plant_idx];
        let Some(bus_pos) = dc_network.non_slack_bus_positions.get(&plant.bus_id) else {
            continue;
        };
        let coefficient = dc_network.ptdf_rows[branch_idx][*bus_pos];
        if coefficient.abs() <= 1e-12 {
            continue;
        }

        let variable = &model.variables.hydro_generation[entry_idx * horizon + period];
        terms.push(LinearTerm {
            variable: variable.name.clone(),
            coefficient,
        });
    }

    for (entry_idx, entry) in model.indexing.wind_plant_entries.iter().enumerate() {
        let plant = &system.wind_plants[entry.plant_idx];
        let Some(bus_pos) = dc_network.non_slack_bus_positions.get(&plant.bus_id) else {
            continue;
        };
        let coefficient = dc_network.ptdf_rows[branch_idx][*bus_pos];
        if coefficient.abs() <= 1e-12 {
            continue;
        }

        let variable = &model.variables.wind_generation[entry_idx * horizon + period];
        terms.push(LinearTerm {
            variable: variable.name.clone(),
            coefficient,
        });
    }

    for (entry_idx, entry) in model.indexing.solar_plant_entries.iter().enumerate() {
        let plant = &system.solar_plants[entry.plant_idx];
        let Some(bus_pos) = dc_network.non_slack_bus_positions.get(&plant.bus_id) else {
            continue;
        };
        let coefficient = dc_network.ptdf_rows[branch_idx][*bus_pos];
        if coefficient.abs() <= 1e-12 {
            continue;
        }

        let variable = &model.variables.solar_generation[entry_idx * horizon + period];
        terms.push(LinearTerm {
            variable: variable.name.clone(),
            coefficient,
        });
    }

    terms
}

fn constant_load_shift(
    system: &System,
    dc_network: &DcNetworkModel,
    branch_idx: usize,
    period: usize,
) -> f64 {
    dc_network
        .non_slack_bus_ids
        .iter()
        .enumerate()
        .map(|(bus_pos, bus_id)| {
            let bus = system
                .buses
                .iter()
                .find(|candidate| candidate.id == *bus_id)
                .expect("bus should exist for non-slack PTDF position");
            dc_network.ptdf_rows[branch_idx][bus_pos] * bus.demand_mw[period]
        })
        .sum()
}

fn detect_flow_violations(
    system: &System,
    dc_network: &DcNetworkModel,
    summary: &SolveSummary,
) -> Result<Vec<FlowViolation>, SimulationError> {
    let line_flows = compute_line_flows(system, dc_network, summary);
    let mut violations = Vec::new();

    for line_flow in line_flows {
        let rounded_flow = round_to_digits(line_flow.flow_mw, 4);
        if rounded_flow.abs() > line_flow.limit_mw {
            violations.push(FlowViolation {
                branch_id: line_flow.branch_id,
                branch_name: line_flow.branch_name,
                period: line_flow.period,
                flow_mw: line_flow.flow_mw,
                limit_mw: line_flow.limit_mw,
            });
        }
    }

    Ok(violations)
}

fn compute_line_flows(
    system: &System,
    dc_network: &DcNetworkModel,
    summary: &SolveSummary,
) -> Vec<LineFlowResult> {
    let injections = injections_by_period(system, dc_network, summary);
    let mut line_flows = Vec::with_capacity(system.horizon.periods * system.branches.len());

    for period in 0..system.horizon.periods {
        for (branch_idx, branch) in system.branches.iter().enumerate() {
            let flow = dot(&dc_network.ptdf_rows[branch_idx], &injections[period]);
            line_flows.push(LineFlowResult {
                branch_id: branch.id,
                branch_name: branch.name.clone(),
                from_bus_name: bus_name(system, branch.from_bus_id),
                to_bus_name: bus_name(system, branch.to_bus_id),
                period: period + 1,
                flow_mw: flow,
                limit_mw: branch.thermal_limit_mw,
            });
        }
    }

    line_flows
}

fn summarize_added_flow_cut_lines(
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
            .map(|line_flow| line_flow.flow_mw)
            .unwrap_or(0.0);

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

fn injections_by_period(
    system: &System,
    dc_network: &DcNetworkModel,
    summary: &SolveSummary,
) -> Vec<Vec<f64>> {
    let horizon = system.horizon.periods;
    let mut injections = vec![vec![0.0; dc_network.non_slack_bus_ids.len()]; horizon];

    for period in 0..horizon {
        for (bus_pos, bus_id) in dc_network.non_slack_bus_ids.iter().enumerate() {
            let bus = system
                .buses
                .iter()
                .find(|candidate| candidate.id == *bus_id)
                .expect("bus should exist for non-slack PTDF position");
            injections[period][bus_pos] -= bus.demand_mw[period];
        }
    }

    for plant in &system.thermal_plants {
        let Some(bus_pos) = dc_network.non_slack_bus_positions.get(&plant.bus_id) else {
            continue;
        };

        for unit in &plant.units {
            for period in 0..horizon {
                let variable_name =
                    thermal_generation_name(plant.name.as_str(), unit.name.as_str(), period);
                injections[period][*bus_pos] += summary
                    .variable_values
                    .get(&variable_name)
                    .copied()
                    .unwrap_or(0.0);
            }
        }
    }

    for plant in &system.hydro_plants {
        let Some(bus_pos) = dc_network.non_slack_bus_positions.get(&plant.bus_id) else {
            continue;
        };

        for group in &plant.groups {
            for unit in &group.units {
                for period in 0..horizon {
                    let variable_name = hydro_generation_name(
                        plant.name.as_str(),
                        group.name.as_str(),
                        unit.name.as_str(),
                        period,
                    );
                    injections[period][*bus_pos] += summary
                        .variable_values
                        .get(&variable_name)
                        .copied()
                        .unwrap_or(0.0);
                }
            }
        }
    }

    for plant in &system.wind_plants {
        let Some(bus_pos) = dc_network.non_slack_bus_positions.get(&plant.bus_id) else {
            continue;
        };

        for period in 0..horizon {
            let variable_name =
                renewable_generation_name("wind_generation", plant.name.as_str(), period);
            injections[period][*bus_pos] += summary
                .variable_values
                .get(&variable_name)
                .copied()
                .unwrap_or(0.0);
        }
    }

    for plant in &system.solar_plants {
        let Some(bus_pos) = dc_network.non_slack_bus_positions.get(&plant.bus_id) else {
            continue;
        };

        for period in 0..horizon {
            let variable_name =
                renewable_generation_name("solar_generation", plant.name.as_str(), period);
            injections[period][*bus_pos] += summary
                .variable_values
                .get(&variable_name)
                .copied()
                .unwrap_or(0.0);
        }
    }

    injections
}

impl DcNetworkModel {
    fn from_system(system: &System) -> Result<Self, SimulationError> {
        let _slack_bus = system
            .buses
            .iter()
            .find(|bus| bus.angle_reference)
            .ok_or_else(|| {
                SimulationError::InvalidNetwork(
                    "system does not define a reference bus for DC flow".into(),
                )
            })?;

        let non_slack_bus_ids = system
            .buses
            .iter()
            .filter(|bus| !bus.angle_reference)
            .map(|bus| bus.id)
            .collect::<Vec<_>>();
        let non_slack_bus_positions = non_slack_bus_ids
            .iter()
            .enumerate()
            .map(|(idx, bus_id)| (*bus_id, idx))
            .collect::<HashMap<_, _>>();

        if non_slack_bus_ids.is_empty() || system.branches.is_empty() {
            return Ok(Self {
                non_slack_bus_ids,
                non_slack_bus_positions,
                ptdf_rows: vec![Vec::new(); system.branches.len()],
            });
        }

        let reduced_b = build_reduced_susceptance(system, &non_slack_bus_positions);
        let branch_susceptance_diag = build_branch_susceptance_diag(system);
        let reduced_incidence = build_reduced_incidence(system, &non_slack_bus_positions);
        let inverse_reduced_b = invert_matrix(&reduced_b)?;
        let branch_times_incidence =
            multiply_matrices(&branch_susceptance_diag, &reduced_incidence)?;
        let ptdf_rows = multiply_matrices(&branch_times_incidence, &inverse_reduced_b)?;

        Ok(Self {
            non_slack_bus_ids,
            non_slack_bus_positions,
            ptdf_rows,
        })
    }
}

fn build_reduced_susceptance(
    system: &System,
    non_slack_bus_positions: &HashMap<BusId, usize>,
) -> Vec<Vec<f64>> {
    let size = non_slack_bus_positions.len();
    let mut matrix = vec![vec![0.0; size]; size];

    for branch in &system.branches {
        let susceptance = 1.0 / branch.reactance_pu;
        let from_pos = non_slack_bus_positions.get(&branch.from_bus_id).copied();
        let to_pos = non_slack_bus_positions.get(&branch.to_bus_id).copied();

        match (from_pos, to_pos) {
            (Some(i), Some(j)) => {
                matrix[i][i] += susceptance;
                matrix[j][j] += susceptance;
                matrix[i][j] -= susceptance;
                matrix[j][i] -= susceptance;
            }
            (Some(i), None) => {
                matrix[i][i] += susceptance;
            }
            (None, Some(j)) => {
                matrix[j][j] += susceptance;
            }
            (None, None) => {}
        }
    }

    matrix
}

fn build_branch_susceptance_diag(system: &System) -> Vec<Vec<f64>> {
    let size = system.branches.len();
    let mut matrix = vec![vec![0.0; size]; size];

    for (branch_idx, branch) in system.branches.iter().enumerate() {
        matrix[branch_idx][branch_idx] = 1.0 / branch.reactance_pu;
    }

    matrix
}

fn build_reduced_incidence(
    system: &System,
    non_slack_bus_positions: &HashMap<BusId, usize>,
) -> Vec<Vec<f64>> {
    let row_count = system.branches.len();
    let column_count = non_slack_bus_positions.len();
    let mut matrix = vec![vec![0.0; column_count]; row_count];

    for (branch_idx, branch) in system.branches.iter().enumerate() {
        if let Some(from_pos) = non_slack_bus_positions.get(&branch.from_bus_id) {
            matrix[branch_idx][*from_pos] = 1.0;
        }
        if let Some(to_pos) = non_slack_bus_positions.get(&branch.to_bus_id) {
            matrix[branch_idx][*to_pos] = -1.0;
        }
    }

    matrix
}

fn invert_matrix(matrix: &[Vec<f64>]) -> Result<Vec<Vec<f64>>, SimulationError> {
    let n = matrix.len();
    if n == 0 {
        return Ok(Vec::new());
    }

    if matrix.iter().any(|row| row.len() != n) {
        return Err(SimulationError::InvalidNetwork(
            "matrix inversion requires a square matrix".into(),
        ));
    }

    let mut inverse = vec![vec![0.0; n]; n];
    for column in 0..n {
        let mut rhs = vec![0.0; n];
        rhs[column] = 1.0;
        let solution = solve_linear_system(matrix, &rhs)?;
        for row in 0..n {
            inverse[row][column] = solution[row];
        }
    }

    Ok(inverse)
}

fn multiply_matrices(
    left: &[Vec<f64>],
    right: &[Vec<f64>],
) -> Result<Vec<Vec<f64>>, SimulationError> {
    if left.is_empty() || right.is_empty() {
        return Ok(Vec::new());
    }

    let left_columns = left[0].len();
    let right_rows = right.len();
    let right_columns = right[0].len();

    if left.iter().any(|row| row.len() != left_columns)
        || right.iter().any(|row| row.len() != right_columns)
    {
        return Err(SimulationError::InvalidNetwork(
            "matrix multiplication requires rectangular matrices".into(),
        ));
    }

    if left_columns != right_rows {
        return Err(SimulationError::InvalidNetwork(format!(
            "matrix multiplication dimension mismatch: left is {}x{}, right is {}x{}",
            left.len(),
            left_columns,
            right_rows,
            right_columns
        )));
    }

    let mut product = vec![vec![0.0; right_columns]; left.len()];
    for (row_idx, left_row) in left.iter().enumerate() {
        for column_idx in 0..right_columns {
            let mut value = 0.0;
            for k in 0..left_columns {
                value += left_row[k] * right[k][column_idx];
            }
            product[row_idx][column_idx] = value;
        }
    }

    Ok(product)
}

fn solve_linear_system(matrix: &[Vec<f64>], rhs: &[f64]) -> Result<Vec<f64>, SimulationError> {
    let n = matrix.len();
    if n == 0 {
        return Ok(Vec::new());
    }
    if rhs.len() != n {
        return Err(SimulationError::InvalidNetwork(
            "linear system dimensions are inconsistent".into(),
        ));
    }

    let mut augmented = vec![vec![0.0; n + 1]; n];
    for row in 0..n {
        for col in 0..n {
            augmented[row][col] = matrix[row][col];
        }
        augmented[row][n] = rhs[row];
    }

    for pivot in 0..n {
        let mut best_row = pivot;
        let mut best_value = augmented[pivot][pivot].abs();
        for row in (pivot + 1)..n {
            let candidate = augmented[row][pivot].abs();
            if candidate > best_value {
                best_row = row;
                best_value = candidate;
            }
        }

        if best_value <= 1e-12 {
            return Err(SimulationError::InvalidNetwork(
                "reduced DC susceptance matrix is singular".into(),
            ));
        }

        if best_row != pivot {
            augmented.swap(best_row, pivot);
        }

        let pivot_value = augmented[pivot][pivot];
        for col in pivot..=n {
            augmented[pivot][col] /= pivot_value;
        }

        for row in 0..n {
            if row == pivot {
                continue;
            }

            let factor = augmented[row][pivot];
            if factor.abs() <= 1e-12 {
                continue;
            }

            for col in pivot..=n {
                augmented[row][col] -= factor * augmented[pivot][col];
            }
        }
    }

    Ok((0..n).map(|row| augmented[row][n]).collect())
}

fn dot(left: &[f64], right: &[f64]) -> f64 {
    left.iter().zip(right.iter()).map(|(a, b)| a * b).sum()
}

fn round_to_digits(value: f64, digits: u32) -> f64 {
    let factor = 10_f64.powi(digits as i32);
    (value * factor).round() / factor
}

fn model_dimensions(model: &Model) -> (usize, usize) {
    let variables = &model.variables;
    let variable_count = variables.thermal_generation.len()
        + variables.hydro_generation.len()
        + variables.hydro_turbining.len()
        + variables.hydro_spillage.len()
        + variables.hydro_diversion.len()
        + variables.hydro_volume.len()
        + variables.deficit.len()
        + variables.wind_generation.len()
        + variables.solar_generation.len()
        + variables.interchange.len()
        + variables.thermal_commitment.len()
        + variables.thermal_startup.len()
        + variables.thermal_shutdown.len()
        + variables.hydro_commitment.len()
        + variables.hydro_startup.len()
        + variables.hydro_shutdown.len()
        + variables.network_flow_slack.len()
        + variables.operational_limit_slack.len();

    (variable_count, model.constraints.linear_constraints.len())
}

pub fn write_results_csvs(
    system: &System,
    result: &IterativeSimulationResult,
    output_dir: impl AsRef<Path>,
    network_enabled: bool,
) -> Result<(), SimulationError> {
    let output_dir = output_dir.as_ref();
    fs::create_dir_all(output_dir).map_err(|error| {
        SimulationError::IterativeProcess(format!(
            "failed to create output directory {}: {error}",
            output_dir.display()
        ))
    })?;

    write_hydro_csv(system, &result.final_summary, output_dir)?;
    write_thermal_csv(system, &result.final_summary, output_dir)?;
    write_process_iterations_csv(&result.steps, output_dir)?;
    write_operational_limits_csv(system, output_dir)?;
    write_operational_limit_infeasibility_csv(
        &result.operational_limit_infeasibilities,
        output_dir,
    )?;
    if network_enabled {
        write_network_csv(&result.final_line_flows, output_dir)?;
        write_added_flow_cut_lines_csv(&result.added_flow_cut_lines, output_dir)?;
        write_network_infeasibility_csv(&result.network_infeasibilities, output_dir)?;
    }
    write_renewable_csv(system, &result.final_summary, output_dir, true)?;
    write_renewable_csv(system, &result.final_summary, output_dir, false)?;

    Ok(())
}

fn write_hydro_csv(
    system: &System,
    summary: &SolveSummary,
    output_dir: &Path,
) -> Result<(), SimulationError> {
    let mut csv = String::from(
        "Usina;Conjunto;Unidade;Periodo;VolumeHM3;GeracaoMW;TurbinamentoHM3;TurbinamentoM3s;VertimentoHM3;VertimentoM3s;DesvioHM3\n",
    );
    let period_duration_hours = system.horizon.period_duration_hours;

    for plant in &system.hydro_plants {
        for period in 0..system.horizon.periods {
            let volume = summary
                .variable_values
                .get(&hydro_volume_name(plant.name.as_str(), period + 1))
                .copied()
                .unwrap_or(0.0);
            let spillage = summary
                .variable_values
                .get(&hydro_spillage_name(plant.name.as_str(), period))
                .copied()
                .unwrap_or(0.0);
            let diversion = summary
                .variable_values
                .get(&hydro_diversion_name(plant.name.as_str(), period))
                .copied()
                .unwrap_or(0.0);
            let spillage_m3s = volume_hm3_to_flow_m3s(spillage, period_duration_hours);
            let mut total_generation = 0.0;
            let mut total_turbining = 0.0;

            for group in &plant.groups {
                for unit in &group.units {
                    let generation = summary
                        .variable_values
                        .get(&hydro_generation_name(
                            plant.name.as_str(),
                            group.name.as_str(),
                            unit.name.as_str(),
                            period,
                        ))
                        .copied()
                        .unwrap_or(0.0);
                    let turbining = summary
                        .variable_values
                        .get(&hydro_turbining_name(
                            plant.name.as_str(),
                            group.name.as_str(),
                            unit.name.as_str(),
                            period,
                        ))
                        .copied()
                        .unwrap_or(0.0);
                    let turbining_m3s = volume_hm3_to_flow_m3s(turbining, period_duration_hours);
                    total_generation += generation;
                    total_turbining += turbining;

                    csv.push_str(&format!(
                        "{};{};{};{};{:.6};{:.6};{:.6};{:.6};{:.6};{:.6};{:.6}\n",
                        plant.name,
                        group.id.0,
                        unit.id.0,
                        period + 1,
                        volume,
                        generation,
                        turbining,
                        turbining_m3s,
                        spillage,
                        spillage_m3s,
                        diversion
                    ));
                }
            }

            let total_turbining_m3s =
                volume_hm3_to_flow_m3s(total_turbining, period_duration_hours);
            csv.push_str(&format!(
                "{};99;99;{};{:.6};{:.6};{:.6};{:.6};{:.6};{:.6};{:.6}\n",
                plant.name,
                period + 1,
                volume,
                total_generation,
                total_turbining,
                total_turbining_m3s,
                spillage,
                spillage_m3s,
                diversion
            ));
        }
    }

    write_csv_file(output_dir.join("resultado_hidreletricas.csv"), csv)
}

fn write_process_iterations_csv(
    steps: &[IterationReport],
    output_dir: &Path,
) -> Result<(), SimulationError> {
    let mut csv = String::from(
        "Etapa;Iteracao;NumeroRestricoes;NumeroVariaveis;ValorFuncaoObjetivo;StatusSolucao;TempoResolucaoSeg\n",
    );

    for step in steps {
        csv.push_str(&format!(
            "{};{};{};{};{:.6};{};{:.6}\n",
            step.stage.label(),
            step.iteration,
            step.constraint_count,
            step.variable_count,
            step.objective_value,
            step.solution_status,
            step.solve_time.as_secs_f64()
        ));
    }

    write_csv_file(output_dir.join("resultado_processo_iter.csv"), csv)
}

fn write_thermal_csv(
    system: &System,
    summary: &SolveSummary,
    output_dir: &Path,
) -> Result<(), SimulationError> {
    let residual_enabled = system.ton_residual_enabled;
    let mut csv = if residual_enabled {
        String::from(
            "Usina;Unidade;Periodo;GeracaoMW;StatusOn;TempoPermanenciaOn;TempoPermanenciaOff;TempoResidualHoras;CustoResidual\n",
        )
    } else {
        String::from(
            "Usina;Unidade;Periodo;GeracaoMW;StatusOn;TempoPermanenciaOn;TempoPermanenciaOff\n",
        )
    };

    for plant in &system.thermal_plants {
        let mut unit_rows = Vec::new();
        let mut aggregated_generation = vec![0.0; system.horizon.periods];
        let mut aggregated_status = vec![false; system.horizon.periods];
        let mut aggregated_residual_hours = vec![0.0; system.horizon.periods];
        let mut aggregated_residual_cost = vec![0.0; system.horizon.periods];
        let residual_cost_per_mwh = if residual_enabled {
            system.residual_costs.iter().find_map(|residual_cost| {
                (residual_cost.submarket_id == plant.submarket_id)
                    .then_some(residual_cost.cmo_per_mwh)
            })
        } else {
            None
        };

        for unit in &plant.units {
            let statuses = (0..system.horizon.periods)
                .map(|period| {
                    let commitment_name =
                        thermal_commitment_name(plant.name.as_str(), unit.name.as_str(), period);
                    summary
                        .variable_values
                        .get(&commitment_name)
                        .copied()
                        .map(|value| value >= 0.5)
                        .unwrap_or_else(|| {
                            let generation = summary
                                .variable_values
                                .get(&thermal_generation_name(
                                    plant.name.as_str(),
                                    unit.name.as_str(),
                                    period,
                                ))
                                .copied()
                                .unwrap_or(0.0);
                            generation.abs() > 1e-8
                        })
                })
                .collect::<Vec<_>>();
            let (times_on, times_off) = thermal_residence_times(
                unit.initial_condition.is_on,
                unit.initial_condition.time_in_state,
                &statuses,
            );

            for period in 0..system.horizon.periods {
                let generation = summary
                    .variable_values
                    .get(&thermal_generation_name(
                        plant.name.as_str(),
                        unit.name.as_str(),
                        period,
                    ))
                    .copied()
                    .unwrap_or(0.0);
                let (residual_hours, residual_cost) =
                    if let Some(cmo_per_mwh) = residual_cost_per_mwh {
                        thermal_residual_output_values(system, unit, period, cmo_per_mwh)
                    } else {
                        (0.0, 0.0)
                    };
                aggregated_generation[period] += generation;
                aggregated_status[period] |= statuses[period];
                aggregated_residual_hours[period] += residual_hours;
                aggregated_residual_cost[period] += residual_cost;

                unit_rows.push((
                    period,
                    unit.id.0,
                    generation,
                    if statuses[period] { 1 } else { 0 },
                    times_on[period],
                    times_off[period],
                    residual_hours,
                    residual_cost,
                ));
            }
        }

        let initial_is_on = plant.units.iter().any(|unit| unit.initial_condition.is_on);
        let initial_time_in_state = plant
            .units
            .iter()
            .map(|unit| unit.initial_condition.time_in_state)
            .max()
            .unwrap_or(0);
        let (agg_times_on, agg_times_off) =
            thermal_residence_times(initial_is_on, initial_time_in_state, &aggregated_status);

        for period in 0..system.horizon.periods {
            for (
                row_period,
                unit_id,
                generation,
                status_on,
                time_on,
                time_off,
                residual_hours,
                residual_cost,
            ) in &unit_rows
            {
                if *row_period == period {
                    if residual_enabled {
                        csv.push_str(&format!(
                            "{};{};{};{:.6};{};{};{};{:.6};{:.6}\n",
                            plant.name,
                            unit_id,
                            period + 1,
                            generation,
                            status_on,
                            time_on,
                            time_off,
                            residual_hours,
                            residual_cost
                        ));
                    } else {
                        csv.push_str(&format!(
                            "{};{};{};{:.6};{};{};{}\n",
                            plant.name,
                            unit_id,
                            period + 1,
                            generation,
                            status_on,
                            time_on,
                            time_off
                        ));
                    }
                }
            }

            if residual_enabled {
                csv.push_str(&format!(
                    "{};99;{};{:.6};{};{};{};{:.6};{:.6}\n",
                    plant.name,
                    period + 1,
                    aggregated_generation[period],
                    if aggregated_status[period] { 1 } else { 0 },
                    agg_times_on[period],
                    agg_times_off[period],
                    aggregated_residual_hours[period],
                    aggregated_residual_cost[period]
                ));
            } else {
                csv.push_str(&format!(
                    "{};99;{};{:.6};{};{};{}\n",
                    plant.name,
                    period + 1,
                    aggregated_generation[period],
                    if aggregated_status[period] { 1 } else { 0 },
                    agg_times_on[period],
                    agg_times_off[period]
                ));
            }
        }
    }

    write_csv_file(output_dir.join("resultado_termicas.csv"), csv)
}

fn thermal_residual_output_values(
    system: &System,
    unit: &labdessem_core::thermal::ThermalUnit,
    startup_period: usize,
    cmo_per_mwh: f64,
) -> (f64, f64) {
    let residual_profile = thermal_post_horizon_profile(unit);
    let periods_inside_horizon = system.horizon.periods.saturating_sub(startup_period);
    let residual_steps = residual_profile
        .len()
        .saturating_sub(periods_inside_horizon);

    if residual_steps == 0 {
        return (0.0, 0.0);
    }

    let residual_mw_sum: f64 = residual_profile[periods_inside_horizon..].iter().sum();
    let residual_hours = residual_steps as f64 * system.horizon.period_duration_hours;
    let residual_cost = residual_mw_sum
        * (unit.variable_cost_per_mwh - cmo_per_mwh).max(0.0)
        * system.horizon.period_duration_hours;

    (residual_hours, residual_cost)
}

fn thermal_post_horizon_profile(unit: &labdessem_core::thermal::ThermalUnit) -> Vec<f64> {
    let mut profile = unit.startup_trajectory_mw.clone();
    let steady_periods = unit
        .min_up_time
        .saturating_sub(unit.startup_trajectory_mw.len());
    profile.extend(std::iter::repeat_n(unit.min_generation_mw, steady_periods));
    profile.extend(unit.shutdown_trajectory_mw.iter().copied());
    profile
}

fn write_network_csv(
    line_flows: &[LineFlowResult],
    output_dir: &Path,
) -> Result<(), SimulationError> {
    let mut csv = String::from("Linha;BarraDe;BarraPara;Periodo;FluxoMW;LimiteMW;Violacao\n");

    for line_flow in line_flows {
        csv.push_str(&format!(
            "{};{};{};{};{:.6};{:.6};{}\n",
            line_flow.branch_name,
            line_flow.from_bus_name,
            line_flow.to_bus_name,
            line_flow.period,
            line_flow.flow_mw,
            line_flow.limit_mw,
            if line_flow.flow_mw.abs() > line_flow.limit_mw {
                1
            } else {
                0
            }
        ));
    }

    write_csv_file(output_dir.join("resultado_rede.csv"), csv)
}

fn write_added_flow_cut_lines_csv(
    added_lines: &[AddedFlowCutLineResult],
    output_dir: &Path,
) -> Result<(), SimulationError> {
    let mut csv = String::from("NomeLinha;CodigoLinha;BarraDe;BarraPara;FluxoMW;CapacidadeMW\n");

    for line in added_lines {
        csv.push_str(&format!(
            "{};{};{};{};{:.6};{:.6}\n",
            line.branch_name,
            line.branch_id.0,
            line.from_bus_id.0,
            line.to_bus_id.0,
            line.flow_mw,
            line.limit_mw
        ));
    }

    write_csv_file(output_dir.join("resultado_linhas_adicionadas.csv"), csv)
}

fn write_network_infeasibility_csv(
    infeasibilities: &[NetworkInfeasibilityRecord],
    output_dir: &Path,
) -> Result<(), SimulationError> {
    let mut csv = String::from("Etapa;Iteracao;Periodo;Linha;ViolacaoMW\n");

    for record in infeasibilities {
        csv.push_str(&format!(
            "{};{};{};{};{:.6}\n",
            record.stage.label(),
            record.iteration,
            record.period,
            record.branch_name,
            record.violation_mw
        ));
    }

    write_csv_file(output_dir.join("resultado_inviabilidade_rede.csv"), csv)
}

fn write_operational_limit_infeasibility_csv(
    infeasibilities: &[OperationalLimitInfeasibilityRecord],
    output_dir: &Path,
) -> Result<(), SimulationError> {
    let mut csv =
        String::from("Etapa;Iteracao;Periodo;CodigoUsina;NomeUsina;ViolacaoLinf;ViolacaoLsup\n");

    for record in infeasibilities {
        csv.push_str(&format!(
            "{};{};{};{};{};{:.6};{:.6}\n",
            record.stage.label(),
            record.iteration,
            record.period,
            record.plant_code,
            record.plant_name,
            record.lower_violation_mw,
            record.upper_violation_mw
        ));
    }

    write_csv_file(output_dir.join("resultado_inviabilidade_lim.csv"), csv)
}

fn write_operational_limits_csv(system: &System, output_dir: &Path) -> Result<(), SimulationError> {
    let mut csv = String::from("TipoUsina;CodigoUsina;NomeUsina;Variavel;Periodo;Linf;Lsup\n");

    for limit in &system.operational_limits {
        let (plant_type, plant_code) = match limit.target {
            OperationalLimitTarget::ThermalPlant(id) => ("TERMICA", id.0),
            OperationalLimitTarget::HydroPlant(id) => ("HIDRAULICA", id.0),
        };

        for period in limit.start_period..=limit.end_period {
            let lower_bound = limit
                .lower_bound
                .map(|value| format!("{value:.6}"))
                .unwrap_or_default();
            let upper_bound = limit
                .upper_bound
                .map(|value| format!("{value:.6}"))
                .unwrap_or_default();

            csv.push_str(&format!(
                "{};{};{};{};{};{};{}\n",
                plant_type,
                plant_code,
                limit.plant_name,
                operational_limit_variable_label(limit.variable),
                period,
                lower_bound,
                upper_bound
            ));
        }
    }

    write_csv_file(output_dir.join("resultado_rest_lim.csv"), csv)
}

fn write_renewable_csv(
    system: &System,
    summary: &SolveSummary,
    output_dir: &Path,
    is_wind: bool,
) -> Result<(), SimulationError> {
    let mut csv = String::from("Usina;Periodo;GeracaoMW\n");

    if is_wind {
        for plant in &system.wind_plants {
            for period in 0..system.horizon.periods {
                let generation = summary
                    .variable_values
                    .get(&renewable_generation_name(
                        "wind_generation",
                        plant.name.as_str(),
                        period,
                    ))
                    .copied()
                    .unwrap_or(0.0);
                csv.push_str(&format!(
                    "{};{};{:.6}\n",
                    plant.name,
                    period + 1,
                    generation
                ));
            }
        }

        write_csv_file(output_dir.join("resultado_eolicas.csv"), csv)
    } else {
        for plant in &system.solar_plants {
            for period in 0..system.horizon.periods {
                let generation = summary
                    .variable_values
                    .get(&renewable_generation_name(
                        "solar_generation",
                        plant.name.as_str(),
                        period,
                    ))
                    .copied()
                    .unwrap_or(0.0);
                csv.push_str(&format!(
                    "{};{};{:.6}\n",
                    plant.name,
                    period + 1,
                    generation
                ));
            }
        }

        write_csv_file(output_dir.join("resultado_solares.csv"), csv)
    }
}

fn write_csv_file(path: impl AsRef<Path>, contents: String) -> Result<(), SimulationError> {
    let path = path.as_ref();
    fs::write(path, contents).map_err(|error| {
        SimulationError::IterativeProcess(format!(
            "failed to write output file {}: {error}",
            path.display()
        ))
    })
}

fn thermal_residence_times(
    initial_is_on: bool,
    initial_time_in_state: usize,
    statuses: &[bool],
) -> (Vec<usize>, Vec<usize>) {
    let mut times_on = Vec::with_capacity(statuses.len());
    let mut times_off = Vec::with_capacity(statuses.len());
    let mut previous_status = initial_is_on;
    let mut current_residence = initial_time_in_state;

    for &status in statuses {
        if status == previous_status {
            current_residence += 1;
        } else {
            current_residence = 1;
            previous_status = status;
        }

        if status {
            times_on.push(current_residence);
            times_off.push(0);
        } else {
            times_on.push(0);
            times_off.push(current_residence);
        }
    }

    (times_on, times_off)
}

fn thermal_generation_name(plant_name: &str, unit_name: &str, period: usize) -> String {
    format!(
        "thermal_generation[p={},u={},t={}]",
        plant_name,
        unit_name,
        period + 1
    )
}

fn thermal_commitment_name(plant_name: &str, unit_name: &str, period: usize) -> String {
    format!(
        "thermal_on[p={},u={},t={}]",
        plant_name,
        unit_name,
        period + 1
    )
}

fn hydro_generation_name(
    plant_name: &str,
    group_name: &str,
    unit_name: &str,
    period: usize,
) -> String {
    format!(
        "hydro_generation[p={},g={},u={},t={}]",
        plant_name,
        group_name,
        unit_name,
        period + 1
    )
}

fn hydro_turbining_name(
    plant_name: &str,
    group_name: &str,
    unit_name: &str,
    period: usize,
) -> String {
    format!(
        "hydro_turbining[p={},g={},u={},t={}]",
        plant_name,
        group_name,
        unit_name,
        period + 1
    )
}

fn hydro_spillage_name(plant_name: &str, period: usize) -> String {
    format!("hydro_spillage[p={},t={}]", plant_name, period + 1)
}

fn hydro_diversion_name(plant_name: &str, period: usize) -> String {
    format!("hydro_diversion[p={},t={}]", plant_name, period + 1)
}

fn volume_hm3_to_flow_m3s(volume_hm3: f64, period_duration_hours: f64) -> f64 {
    volume_hm3 / (period_duration_hours * 0.0036)
}

fn hydro_volume_name(plant_name: &str, period: usize) -> String {
    format!("hydro_volume[p={},t={}]", plant_name, period)
}

fn renewable_generation_name(prefix: &str, plant_name: &str, period: usize) -> String {
    format!("{prefix}[p={},t={}]", plant_name, period + 1)
}

fn flow_slack_name(prefix: &str, branch_name: &str, period: usize) -> String {
    format!("{prefix}[branch={},t={}]", branch_name, period + 1)
}

fn operational_limit_variable_label(variable: OperationalLimitVariable) -> &'static str {
    match variable {
        OperationalLimitVariable::Generation => "GER",
        OperationalLimitVariable::Spillage => "VERT",
        OperationalLimitVariable::Volume => "VOL",
        OperationalLimitVariable::Defluence => "DEFLU",
        OperationalLimitVariable::Turbining => "TURB",
    }
}

fn operational_limit_slack_name(
    is_lower: bool,
    plant_name: &str,
    variable: OperationalLimitVariable,
    period: usize,
) -> String {
    let prefix = if is_lower {
        "operational_limit_slack_lower"
    } else {
        "operational_limit_slack_upper"
    };
    format!(
        "{prefix}[p={},var={},t={}]",
        plant_name,
        operational_limit_variable_label(variable),
        period
    )
}

fn bus_name(system: &System, bus_id: BusId) -> String {
    system
        .buses
        .iter()
        .find(|bus| bus.id == bus_id)
        .map(|bus| bus.name.clone())
        .unwrap_or_else(|| format!("BUS-{}", bus_id.0))
}

#[cfg(test)]
mod tests {
    use super::run_iterative_simulation;
    use labdessem_core::{
        hydro::{
            HydroFphaSegment, HydroGroup, HydroInitialCondition, HydroPlant, HydroUnit, Reservoir,
        },
        ids::{
            BranchId, BusId, HydroGroupId, HydroPlantId, HydroUnitId, SubmarketId, ThermalPlantId,
            ThermalUnitId,
        },
        system::{Branch, Bus, StudyHorizon, Submarket, System},
        thermal::{ThermalInitialCondition, ThermalPlant, ThermalUnit},
    };

    fn fpha_segments() -> Vec<HydroFphaSegment> {
        vec![HydroFphaSegment {
            segment: 1,
            correction_factor: 1.0,
            rhs: 0.0,
            volume_coefficient: 0.0,
            turbining_coefficient: 1.0,
            lateral_flow_coefficient: 0.0,
        }]
    }

    fn build_system() -> System {
        System {
            horizon: StudyHorizon {
                periods: 2,
                period_duration_hours: 1.0,
            },
            thermal_unit_commitment_enabled: true,
            hydro_unit_commitment_enabled: true,
            ton_residual_enabled: false,
            residual_costs: vec![],
            submarkets: vec![
                Submarket {
                    id: SubmarketId(1),
                    name: "SE".into(),
                    demand_mw: vec![100.0, 100.0],
                    deficit_cost_per_mwh: 1_000.0,
                },
                Submarket {
                    id: SubmarketId(2),
                    name: "S".into(),
                    demand_mw: vec![50.0, 50.0],
                    deficit_cost_per_mwh: 1_000.0,
                },
            ],
            interchange_limits: vec![],
            operational_limits: vec![],
            buses: vec![
                Bus {
                    id: BusId(1),
                    name: "BUS-1".into(),
                    submarket_id: SubmarketId(1),
                    angle_reference: true,
                    demand_mw: vec![0.0, 0.0],
                },
                Bus {
                    id: BusId(2),
                    name: "BUS-2".into(),
                    submarket_id: SubmarketId(2),
                    angle_reference: false,
                    demand_mw: vec![150.0, 150.0],
                },
            ],
            branches: vec![Branch {
                id: BranchId(1),
                name: "L1".into(),
                from_bus_id: BusId(1),
                to_bus_id: BusId(2),
                reactance_pu: 0.1,
                thermal_limit_mw: 200.0,
            }],
            thermal_plants: vec![ThermalPlant {
                id: ThermalPlantId(1),
                name: "UTE1".into(),
                submarket_id: SubmarketId(2),
                bus_id: BusId(2),
                units: vec![ThermalUnit {
                    id: ThermalUnitId(1),
                    name: "UTE1-1".into(),
                    min_generation_mw: 0.0,
                    max_generation_mw: 200.0,
                    startup_trajectory_mw: vec![50.0],
                    shutdown_trajectory_mw: vec![50.0],
                    min_up_time: 1,
                    min_down_time: 1,
                    startup_cost: 1.0,
                    shutdown_cost: 1.0,
                    variable_cost_per_mwh: 100.0,
                    initial_condition: ThermalInitialCondition {
                        is_on: false,
                        generation_mw: 0.0,
                        time_in_state: 1,
                        is_ramping_up: false,
                        is_ramping_down: false,
                    },
                }],
            }],
            hydro_plants: vec![HydroPlant {
                id: HydroPlantId(1),
                name: "UHE1".into(),
                submarket_id: SubmarketId(1),
                bus_id: BusId(1),
                upstream_plant_ids: vec![],
                downstream_plant_id: None,
                diversion_upstream_plant_ids: vec![],
                diversion_plant_id: None,
                fpha_segments: fpha_segments(),
                reservoir: Reservoir {
                    min_volume_hm3: 0.0,
                    max_volume_hm3: 500.0,
                    initial_volume_hm3: 200.0,
                },
                natural_inflow_hm3: vec![100.0, 100.0],
                spillage_cost_per_hm3: 0.1,
                groups: vec![HydroGroup {
                    id: HydroGroupId(1),
                    name: "CJ1".into(),
                    units: vec![HydroUnit {
                        id: HydroUnitId(1),
                        name: "UG1".into(),
                        min_generation_mw: 0.0,
                        max_generation_mw: 200.0,
                        max_turbining_hm3: 500.0,
                        startup_trajectory_mw: vec![10.0],
                        shutdown_trajectory_mw: vec![10.0],
                        min_up_time: 1,
                        min_down_time: 1,
                        startup_cost: 0.0,
                        shutdown_cost: 0.0,
                        initial_condition: HydroInitialCondition {
                            is_on: true,
                            generation_mw: 0.0,
                            time_in_state: 1,
                        },
                    }],
                }],
            }],
            wind_plants: vec![],
            solar_plants: vec![],
        }
    }

    #[test]
    fn runs_iterative_process_and_reports_steps() {
        let result = run_iterative_simulation(&build_system(), true)
            .expect("iterative simulation should solve the simple case");

        assert!(!result.steps.is_empty());
        assert_eq!(
            result
                .steps
                .iter()
                .filter(|step| step.stage == super::IterationStage::MixedIntegerLinearProgramming)
                .count(),
            1
        );
    }
}
