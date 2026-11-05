//! Solver interaction and stage execution functions.

use std::collections::{BTreeSet, HashMap};
use std::io::{self, Write};
use std::time::Instant;

use labdessem_core::entities::EngMaxBounds;
use labdessem_core::system::System;
use labdessem_model::indexing::{ColumnOffsets, Indexing};
use labdessem_model::{Model, SolveMode};
use labdessem_network::{DcNetworkModel, FlowCutKey, detect_flow_violations};
use labdessem_solver::{
    HighsSolver, LpSolution, SolverError, SolverInterface, model_to_lp_template,
};

use crate::engmax::EngMaxState;
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
    col_offsets: &ColumnOffsets,
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
}

fn stage_model(
    system: &System,
    solve_mode: SolveMode,
    commitment_source: Option<&LpSolution>,
    indexing: &Indexing,
    col_offsets: &ColumnOffsets,
    engmax_bounds: Option<&EngMaxBounds>,
) -> Result<Model, SimulationError> {
    let mut model = Model::from_system(system, solve_mode);
    if let Some(commitment_source) = commitment_source {
        apply_commitment_fixes(&mut model, commitment_source, col_offsets);
    }
    if let Some(bounds) = engmax_bounds {
        model
            .constraints
            .extend_with_engmax(system, indexing, &model.variables, bounds)?;
    }
    Ok(model)
}

/// Largest excess (hm3) of a plant's turbining sum over its EngMax bound treated as zero.
const ENGMAX_EXCEED_TOL_HM3: f64 = 1e-6;

/// Diagnose an infeasible solve of `failing_model`, which carries engmax rows (ADR-017):
/// solve `engmax_free_model` (the same model without engmax rows, with the same
/// relaxations) once and name the engmax rows of `failing_model` its solution exceeds.
///
/// Returns `SimulationError::EngMaxInfeasible` when the EngMax-free model is feasible and
/// some row is exceeded, and `SimulationError::Solver(error)` (the original error,
/// unchanged) otherwise.
fn diagnose_engmax_infeasibility(
    solver: &mut HighsSolver,
    failing_model: &Model,
    engmax_free_model: &Model,
    col_offsets: &ColumnOffsets,
    stage: IterationStage,
    iteration: usize,
    error: SolverError,
) -> SimulationError {
    let Ok(solution) = solve_model_direct(solver, engmax_free_model) else {
        return SimulationError::Solver(error);
    };
    let turbining: HashMap<&str, f64> = engmax_free_model
        .variables
        .hydro_turbining
        .iter()
        .enumerate()
        .map(|(idx, variable)| {
            (
                variable.name.as_str(),
                solution.primal[col_offsets.hydro_turbining + idx],
            )
        })
        .collect();
    let mut rows = Vec::new();
    for row in failing_model.constraints.engmax() {
        let mut lhs = 0.0;
        for term in &row.terms {
            let Some(value) = turbining.get(term.variable.as_str()) else {
                return SimulationError::Solver(error);
            };
            lhs += term.coefficient * value;
        }
        if lhs - row.rhs > ENGMAX_EXCEED_TOL_HM3 {
            rows.push(row.name.clone());
        }
    }
    if rows.is_empty() {
        SimulationError::Solver(error)
    } else {
        SimulationError::EngMaxInfeasible {
            stage,
            iteration,
            rows,
        }
    }
}

/// Run LP cut loop until no violations or stalled.
pub(crate) fn run_lp_cut_loop(
    system: &System,
    dc_network: &DcNetworkModel,
    accumulated_cuts: &mut BTreeSet<FlowCutKey>,
    mut engmax: Option<&mut EngMaxState>,
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
            engmax.as_deref_mut(),
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

/// Solve one stage over the full horizon. `engmax` is read to build the model (normal and
/// relaxed) and then advanced, once, on the solution this function returns.
#[allow(clippy::too_many_arguments)]
pub(crate) fn solve_stage(
    system: &System,
    dc_network: &DcNetworkModel,
    solve_mode: SolveMode,
    accumulated_cuts: &BTreeSet<FlowCutKey>,
    commitment_source: Option<&LpSolution>,
    engmax: Option<&mut EngMaxState>,
    stage: IterationStage,
    iteration: usize,
    steps: &mut Vec<IterationReport>,
    network_infeasibilities: &mut Vec<NetworkInfeasibilityRecord>,
    operational_limit_infeasibilities: &mut Vec<OperationalLimitInfeasibilityRecord>,
    solver: &mut HighsSolver,
) -> Result<(LpSolution, Vec<FlowViolation>, bool), SimulationError> {
    println!("Resolvendo {} #{:02}...", stage.label(), iteration);
    io::stdout().flush().ok();

    let indexing = Indexing::from_system(system);
    let col_offsets = indexing.compute_column_offsets(system.horizon.periods);

    let build_model =
        |bounds: Option<&EngMaxBounds>, relaxed: bool| -> Result<Model, SimulationError> {
            let mut model = stage_model(
                system,
                solve_mode,
                commitment_source,
                &indexing,
                &col_offsets,
                bounds,
            )?;
            add_flow_cuts(&mut model, system, accumulated_cuts, dc_network, relaxed);
            if relaxed {
                add_operational_limit_slacks(&mut model);
            }
            Ok(model)
        };
    let model = build_model(engmax.as_deref().map(EngMaxState::bounds), false)?;
    let (variable_count, constraint_count) = model_dimensions(&model);

    let started_at = Instant::now();
    let mut used_slacks = false;
    let summary = match solve_model_direct(solver, &model) {
        Ok(summary) => summary,
        Err(SolverError::Infeasible | SolverError::Unbounded)
            if !accumulated_cuts.is_empty() || !system.operational_limits.is_empty() =>
        {
            used_slacks = true;
            println!(
                "{} #{:02} inviavel com restricoes de rede/limite. Adicionando folgas...",
                stage.label(),
                iteration
            );
            io::stdout().flush().ok();

            let relaxed_model = build_model(engmax.as_deref().map(EngMaxState::bounds), true)?;

            let relaxed_summary = match solve_model_direct(solver, &relaxed_model) {
                Ok(summary) => summary,
                Err(SolverError::Infeasible) if engmax.is_some() => {
                    return Err(diagnose_engmax_infeasibility(
                        solver,
                        &relaxed_model,
                        &build_model(None, true)?,
                        &col_offsets,
                        stage,
                        iteration,
                        SolverError::Infeasible,
                    ));
                }
                Err(error) => return Err(error.into()),
            };
            network_infeasibilities.extend(collect_network_infeasibilities(
                system,
                accumulated_cuts,
                &relaxed_model.variables,
                &relaxed_summary,
                stage,
                iteration,
            ));
            operational_limit_infeasibilities.extend(collect_operational_limit_infeasibilities(
                system,
                &relaxed_model.variables,
                &relaxed_summary,
                stage,
                iteration,
            ));
            relaxed_summary
        }
        Err(SolverError::Infeasible) if engmax.is_some() => {
            return Err(diagnose_engmax_infeasibility(
                solver,
                &model,
                &build_model(None, false)?,
                &col_offsets,
                stage,
                iteration,
                SolverError::Infeasible,
            ));
        }
        Err(error) => return Err(error.into()),
    };
    let solve_time = started_at.elapsed();
    if let Some(state) = engmax {
        state.after_solve(system, &indexing, stage, iteration, solve_mode, &summary);
    }
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

/// Solve one stage over the full horizon without the network. `engmax` is read to build the
/// model (normal and relaxed) and then advanced, once, on the solution this function returns.
#[allow(clippy::too_many_arguments)]
pub(crate) fn solve_stage_without_network(
    system: &System,
    solve_mode: SolveMode,
    commitment_source: Option<&LpSolution>,
    engmax: Option<&mut EngMaxState>,
    stage: IterationStage,
    iteration: usize,
    steps: &mut Vec<IterationReport>,
    operational_limit_infeasibilities: &mut Vec<OperationalLimitInfeasibilityRecord>,
    solver: &mut HighsSolver,
) -> Result<LpSolution, SimulationError> {
    println!("Resolvendo {} #{:02}...", stage.label(), iteration);
    io::stdout().flush().ok();

    let indexing = Indexing::from_system(system);
    let col_offsets = indexing.compute_column_offsets(system.horizon.periods);

    let build_model =
        |bounds: Option<&EngMaxBounds>, relaxed: bool| -> Result<Model, SimulationError> {
            let mut model = stage_model(
                system,
                solve_mode,
                commitment_source,
                &indexing,
                &col_offsets,
                bounds,
            )?;
            if relaxed {
                add_operational_limit_slacks(&mut model);
            }
            Ok(model)
        };
    let model = build_model(engmax.as_deref().map(EngMaxState::bounds), false)?;
    let (variable_count, constraint_count) = model_dimensions(&model);

    let started_at = Instant::now();
    let summary = match solve_model_direct(solver, &model) {
        Ok(summary) => summary,
        Err(SolverError::Infeasible | SolverError::Unbounded)
            if !system.operational_limits.is_empty() =>
        {
            println!(
                "{} #{:02} inviavel com restricoes de limite. Adicionando folgas...",
                stage.label(),
                iteration
            );
            io::stdout().flush().ok();

            let relaxed_model = build_model(engmax.as_deref().map(EngMaxState::bounds), true)?;

            let relaxed_summary = match solve_model_direct(solver, &relaxed_model) {
                Ok(summary) => summary,
                Err(SolverError::Infeasible) if engmax.is_some() => {
                    return Err(diagnose_engmax_infeasibility(
                        solver,
                        &relaxed_model,
                        &build_model(None, true)?,
                        &col_offsets,
                        stage,
                        iteration,
                        SolverError::Infeasible,
                    ));
                }
                Err(error) => return Err(error.into()),
            };
            operational_limit_infeasibilities.extend(collect_operational_limit_infeasibilities(
                system,
                &relaxed_model.variables,
                &relaxed_summary,
                stage,
                iteration,
            ));
            relaxed_summary
        }
        Err(SolverError::Infeasible) if engmax.is_some() => {
            return Err(diagnose_engmax_infeasibility(
                solver,
                &model,
                &build_model(None, false)?,
                &col_offsets,
                stage,
                iteration,
                SolverError::Infeasible,
            ));
        }
        Err(error) => return Err(error.into()),
    };
    let solve_time = started_at.elapsed();
    if let Some(state) = engmax {
        state.after_solve(system, &indexing, stage, iteration, solve_mode, &summary);
    }

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

#[cfg(test)]
// tests: a panic is the failure report
#[allow(clippy::unwrap_used, clippy::expect_used)]
mod tests {
    use super::{solve_stage, solve_stage_without_network, stage_model};
    use crate::error::SimulationError;
    use crate::results::OperationalLimitInfeasibilityRecord;
    use crate::workflow::IterationStage;
    use labdessem_core::entities::EngMaxBounds;
    use labdessem_core::ids::{BranchId, BusId, ThermalPlantId};
    use labdessem_core::system::{
        Branch, OperationalLimit, OperationalLimitTarget, OperationalLimitVariable, System,
    };
    use labdessem_core::test_support::{minimal_bus, minimal_system_with_hydro};
    use labdessem_model::indexing::Indexing;
    use labdessem_model::{ModelError, SolveMode};
    use labdessem_network::{DcNetworkModel, FlowCutKey};
    use labdessem_solver::HighsSolver;
    use std::collections::BTreeSet;

    #[test]
    fn stage_model_without_engmax_adds_no_engmax_rows() {
        let system = minimal_system_with_hydro();
        let indexing = Indexing::from_system(&system);
        let col_offsets = indexing.compute_column_offsets(system.horizon.periods);

        let model = stage_model(
            &system,
            SolveMode::LinearProgramming,
            None,
            &indexing,
            &col_offsets,
            None,
        )
        .expect("no EngMax bounds to build");

        assert_eq!(model.constraints.engmax().len(), 0);
    }

    #[test]
    fn stage_model_with_engmax_builds_rows_from_the_given_bounds() {
        let system = minimal_system_with_hydro();
        let indexing = Indexing::from_system(&system);
        let col_offsets = indexing.compute_column_offsets(system.horizon.periods);
        // Copied into the rows, not computed: they differ from the plant's computed bound.
        let mut bounds = EngMaxBounds::new(2);
        bounds.set(0, 0, 11.0);
        bounds.set(0, 1, 13.0);

        let model = stage_model(
            &system,
            SolveMode::LinearProgramming,
            None,
            &indexing,
            &col_offsets,
            Some(&bounds),
        )
        .expect("all bounds are present");

        let rows = model.constraints.engmax();
        assert_eq!(rows.len(), 2);
        let plant = &system.hydro_plants[0].name;
        for (period, expected) in [(1, 11.0), (2, 13.0)] {
            let name = format!("engmax[p={plant},t={period}]");
            let row = rows
                .iter()
                .find(|row| row.name == name)
                .expect("engmax row for every period");
            assert_eq!(row.rhs, expected);
        }
    }

    #[test]
    fn stage_model_with_incomplete_bounds_is_model_error() {
        let system = minimal_system_with_hydro();
        let indexing = Indexing::from_system(&system);
        let col_offsets = indexing.compute_column_offsets(system.horizon.periods);
        // horizon 2: period 1 has no bound
        let mut bounds = EngMaxBounds::new(2);
        bounds.set(0, 0, 11.0);

        let result = stage_model(
            &system,
            SolveMode::LinearProgramming,
            None,
            &indexing,
            &col_offsets,
            Some(&bounds),
        );

        assert!(matches!(
            result,
            Err(SimulationError::Model(ModelError::MissingEngMaxBound {
                period: 1,
                ..
            }))
        ));
    }

    /// `minimal_system_with_hydro()` with both unit-commitment flags on, so a
    /// `LinearProgramming` model drops the commitment columns that `Indexing` still counts.
    fn uc_system() -> System {
        let mut system = minimal_system_with_hydro();
        system.thermal_unit_commitment_enabled = true;
        system.hydro_unit_commitment_enabled = true;
        system
    }

    /// Generation limit on `TERM-1` over periods 1-2. A 10 000 MW floor is out of reach
    /// (demand is 100 MW), so the relaxed re-solve sets each lower slack to 9 900 MW.
    fn term_1_generation_limit(lower: Option<f64>, upper: Option<f64>) -> OperationalLimit {
        OperationalLimit {
            target: OperationalLimitTarget::ThermalPlant(ThermalPlantId(1)),
            plant_name: "TERM-1".into(),
            variable: OperationalLimitVariable::Generation,
            start_period: 1,
            end_period: 2,
            lower_bound: lower,
            upper_bound: upper,
        }
    }

    /// Moves the 100 MW demand to a new reference bus `BUS-2` and joins it to `BUS-1` (both
    /// generators) by line `L1` limited to 10 MW: every dispatch sends 100 MW over `L1`, so a
    /// flow cut on it needs a 90 MW slack in every period.
    fn with_forced_line_overload(mut system: System) -> System {
        system.buses[0].demand_mw = vec![0.0; 2];
        system.buses[0].angle_reference = false;
        let mut demand_bus = minimal_bus(2, 1, 100.0, 2);
        demand_bus.angle_reference = true;
        system.buses.push(demand_bus);
        system.branches.push(Branch {
            id: BranchId(1),
            name: "L1".into(),
            from_bus_id: BusId(1),
            to_bus_id: BusId(2),
            reactance_pu: 0.1,
            thermal_limit_mw: 10.0,
        });
        system
    }

    /// Flow cuts on `L1` in both periods.
    fn line_cuts() -> BTreeSet<FlowCutKey> {
        (0..2)
            .map(|period| FlowCutKey {
                branch_idx: 0,
                period,
            })
            .collect()
    }

    fn assert_term_1_floor_records(records: &[OperationalLimitInfeasibilityRecord]) {
        let periods: Vec<usize> = records.iter().map(|record| record.period).collect();
        assert_eq!(periods, [1, 2], "records: {records:?}");
        for record in records {
            assert_eq!(record.plant_name, "TERM-1", "records: {records:?}");
            assert_eq!(
                record.variable,
                OperationalLimitVariable::Generation,
                "records: {records:?}"
            );
            assert!(
                (record.lower_violation - 9900.0).abs() <= 1e-6,
                "records: {records:?}"
            );
            assert!(record.upper_violation.abs() <= 1e-6, "records: {records:?}");
        }
    }

    #[test]
    fn slack_columns_lp_mode_reports_limit_slack_without_network() {
        let mut system = uc_system();
        system
            .operational_limits
            .push(term_1_generation_limit(Some(10_000.0), Some(20_000.0)));
        let mut steps = Vec::new();
        let mut limit_records = Vec::new();
        let mut solver = HighsSolver::new().expect("HiGHS instance");

        solve_stage_without_network(
            &system,
            SolveMode::LinearProgramming,
            None,
            None,
            IterationStage::LinearProgramming,
            1,
            &mut steps,
            &mut limit_records,
            &mut solver,
        )
        .expect("the slack re-solve makes the stage feasible");

        assert_term_1_floor_records(&limit_records);
        assert_eq!(steps[0].solution_status, "OPTIMAL_WITH_SLACK");
    }

    #[test]
    fn slack_columns_lower_only_limit_is_attributed_to_lower_side() {
        let mut system = uc_system();
        system
            .operational_limits
            .push(term_1_generation_limit(Some(10_000.0), None));
        let mut steps = Vec::new();
        let mut limit_records = Vec::new();
        let mut solver = HighsSolver::new().expect("HiGHS instance");

        solve_stage_without_network(
            &system,
            SolveMode::MixedIntegerLinearProgramming,
            None,
            None,
            IterationStage::MixedIntegerLinearProgramming,
            1,
            &mut steps,
            &mut limit_records,
            &mut solver,
        )
        .expect("the slack re-solve makes the stage feasible");

        assert_term_1_floor_records(&limit_records);
    }

    #[test]
    fn slack_columns_lp_mode_reports_flow_slack() {
        let system = with_forced_line_overload(uc_system());
        let dc_network = DcNetworkModel::from_system(&system).expect("BUS-2 is the reference");
        let mut steps = Vec::new();
        let mut network_records = Vec::new();
        let mut limit_records = Vec::new();
        let mut solver = HighsSolver::new().expect("HiGHS instance");

        solve_stage(
            &system,
            &dc_network,
            SolveMode::LinearProgramming,
            &line_cuts(),
            None,
            None,
            IterationStage::LinearProgramming,
            1,
            &mut steps,
            &mut network_records,
            &mut limit_records,
            &mut solver,
        )
        .expect("the slack re-solve makes the stage feasible");

        let periods: Vec<usize> = network_records.iter().map(|record| record.period).collect();
        assert_eq!(periods, [1, 2], "records: {network_records:?}");
        for record in &network_records {
            assert_eq!(record.branch_name, "L1", "records: {network_records:?}");
            assert!(
                (record.violation_mw - 90.0).abs() <= 1e-6,
                "records: {network_records:?}"
            );
        }
        assert!(limit_records.is_empty(), "records: {limit_records:?}");
    }

    #[test]
    fn slack_columns_limit_slack_follows_flow_slack() {
        let mut system = with_forced_line_overload(uc_system());
        system
            .operational_limits
            .push(term_1_generation_limit(Some(10_000.0), Some(20_000.0)));
        let dc_network = DcNetworkModel::from_system(&system).expect("BUS-2 is the reference");
        let mut steps = Vec::new();
        let mut network_records = Vec::new();
        let mut limit_records = Vec::new();
        let mut solver = HighsSolver::new().expect("HiGHS instance");

        solve_stage(
            &system,
            &dc_network,
            SolveMode::MixedIntegerLinearProgramming,
            &line_cuts(),
            None,
            None,
            IterationStage::MixedIntegerLinearProgramming,
            1,
            &mut steps,
            &mut network_records,
            &mut limit_records,
            &mut solver,
        )
        .expect("the slack re-solve makes the stage feasible");

        assert_eq!(network_records.len(), 2, "records: {network_records:?}");
        assert_term_1_floor_records(&limit_records);
    }
}
