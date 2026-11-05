//! Solver-backed EngMax tests: R44 fixture, trace checker and path properties (ADR-016).

use std::collections::BTreeSet;

use labdessem_core::entities::{
    EngMaxBounds, HydroFphaSegment, HydroGroup, HydroInitialCondition, HydroPlant, HydroUnit,
    Reservoir, ThermalInitialCondition, ThermalPlant, ThermalUnit, TurbineType,
};
use labdessem_core::ids::{
    BranchId, BusId, HydroGroupId, HydroPlantId, HydroUnitId, SubmarketId, ThermalPlantId,
    ThermalUnitId,
};
use labdessem_core::system::{
    Branch, Bus, OperationalLimit, OperationalLimitTarget, OperationalLimitVariable, StudyHorizon,
    Submarket, System,
};
use labdessem_model::SolveMode;
use labdessem_network::{DcNetworkModel, FlowCutKey};
use labdessem_solver::{HighsSolver, SolverError, SolverInterface};

use crate::engmax::{EngMaxState, EngMaxTraceEntry, initialize_engmax_bounds};
use crate::error::SimulationError;
use crate::results::IterationReport;
use crate::workflow::{ExecutionOption, IterationStage};

use super::orchestration::run_simulation_with_state;
use super::solve::{solve_stage, solve_stage_without_network};

/// Thermal limit of the single line of [`engmax_network_fixture`].
#[derive(Debug, Clone, Copy)]
enum LineLimit {
    /// 40 MW, below what the hydro plant sends when nothing limits the line: the cut loop
    /// adds a flow cut and runs a second LP.
    Binding,
    /// 1.0e5 MW: no flow cut is ever generated.
    NonBinding,
}

/// R44 fixture: Iterative + network + EngMax with non-trivial head.
///
/// - 3 periods of 1.0 h, one submarket `SE` (90 MW per period), `BUS-1` (angle reference, hydro)
///   and `BUS-2` (demand 90 MW, thermal `UTE1` at 100 per MWh) joined by line `L1`.
/// - One plant `UHE-ENG` with two groups of one unit (`Q_ef` = 60 + 40 = 100 hm3, `h_ef` =
///   (60 + 70) / 2 = 65 m), a volume-level table and a tailrace polynomial, so the net head
///   (20 to 50 m) comes from the plant geometry and not from the `h_ef` fallback.
/// - `h_ef` = 65 m exceeds the maximum net head of 50 m, so EngMax < `Q_ef` on every solve.
/// - The single FPHA segment has `turbining_coefficient` 0.0036, which the FPHA row divides by
///   the hm3-to-m3/s conversion (0.0036 per hour): generation (MW) <= turbining (hm3 per 1-h
///   period). EngMax is therefore the hydro limit of the unconstrained first LP, and both
///   spillage and turbining costs are > 0, so the dispatch is unique.
/// - Inflow 50 hm3 per period and no withdrawal keep the reservoir inside [0, 500] whether the
///   hydro runs at EngMax or at the line limit.
/// - With `Binding`, the first LP sends more than 40 MW over `L1`, the cut loop adds a flow cut
///   and runs a second LP at the line limit, so the volume trajectory, and with it the bounds,
///   change between the two LPs. The single submarket keeps the no-network path feasible when
///   EngMax caps the hydro.
/// - Tuned value: the demand is 90 MW, not the specified 150 MW. `UTE1` starts off with a
///   one-step startup trajectory of 50 MW, so in period 1 it generates 0 or exactly 50 MW in
///   the MILP; 150 MW is out of reach of a hydro plant capped by EngMax (and by the 40 MW
///   line under `Binding`), and the MILP was infeasible. At 90 MW the first LP still runs the
///   hydro at EngMax (above the 40 MW line limit) and the MILP starts `UTE1` in period 1
///   (40 + 50 = 90). `thermal_limit_mw` keeps the specified 40.0.
fn engmax_network_fixture(line: LineLimit) -> System {
    let thermal_limit_mw = match line {
        LineLimit::Binding => 40.0,
        LineLimit::NonBinding => 1.0e5,
    };
    System {
        horizon: StudyHorizon {
            periods: 3,
            period_duration_hours: 1.0,
            original_periods: 3,
            original_period_durations_hours: vec![1.0; 3],
            internal_to_original_period: vec![1, 2, 3],
            internal_subperiod_index: vec![1, 1, 1],
        },
        thermal_unit_commitment_enabled: true,
        hydro_unit_commitment_enabled: true,
        ton_residual_enabled: false,
        fpha_enabled: true,
        future_cost_enabled: false,
        residual_costs: vec![],
        future_cost_cuts: vec![],
        submarkets: vec![Submarket {
            id: SubmarketId(1),
            name: "SE".into(),
            demand_mw: vec![90.0; 3],
        }],
        interchange_limits: vec![],
        operational_limits: vec![],
        buses: vec![
            Bus {
                id: BusId(1),
                name: "BUS-1".into(),
                submarket_id: SubmarketId(1),
                angle_reference: true,
                demand_mw: vec![0.0; 3],
            },
            Bus {
                id: BusId(2),
                name: "BUS-2".into(),
                submarket_id: SubmarketId(1),
                angle_reference: false,
                demand_mw: vec![90.0; 3],
            },
        ],
        branches: vec![Branch {
            id: BranchId(1),
            name: "L1".into(),
            from_bus_id: BusId(1),
            to_bus_id: BusId(2),
            reactance_pu: 0.1,
            thermal_limit_mw,
        }],
        thermal_plants: vec![ThermalPlant {
            id: ThermalPlantId(1),
            name: "UTE1".into(),
            submarket_id: SubmarketId(1),
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
                    time_in_ramp: 0,
                    is_ramping_up: false,
                    is_ramping_down: false,
                },
            }],
        }],
        hydro_plants: vec![HydroPlant {
            id: HydroPlantId(1),
            name: "UHE-ENG".into(),
            submarket_id: SubmarketId(1),
            bus_id: BusId(1),
            upstream_plant_ids: vec![],
            downstream_plant_id: None,
            diversion_upstream_plant_ids: vec![],
            diversion_plant_id: None,
            fpha_segments: vec![HydroFphaSegment {
                segment: 1,
                correction_factor: 1.0,
                rhs: 0.0,
                volume_coefficient: 0.0,
                turbining_coefficient: 0.0036,
                lateral_flow_coefficient: 0.0,
            }],
            reservoir: Reservoir {
                min_volume_hm3: 0.0,
                max_volume_hm3: 500.0,
                initial_volume_hm3: 200.0,
            },
            natural_inflow_hm3: vec![50.0; 3],
            water_withdrawal_hm3: vec![0.0; 3],
            spillage_cost_per_hm3: 0.1,
            turbining_cost_per_hm3: 0.01,
            specific_productivity_mw_per_m3s: 1.0,
            hydraulic_loss_m: None,
            efficiency: None,
            groups: vec![
                HydroGroup {
                    id: HydroGroupId(1),
                    name: "CJ1".into(),
                    units: vec![HydroUnit {
                        id: HydroUnitId(1),
                        name: "UG1".into(),
                        min_generation_mw: 0.0,
                        max_generation_mw: 200.0,
                        max_turbining_hm3: 60.0,
                        effective_head_m: 60.0,
                        initial_condition: HydroInitialCondition {
                            is_on: true,
                            generation_mw: 0.0,
                            time_in_state: 1,
                        },
                    }],
                },
                HydroGroup {
                    id: HydroGroupId(2),
                    name: "CJ2".into(),
                    units: vec![HydroUnit {
                        id: HydroUnitId(2),
                        name: "UG2".into(),
                        min_generation_mw: 0.0,
                        max_generation_mw: 200.0,
                        max_turbining_hm3: 40.0,
                        effective_head_m: 70.0,
                        initial_condition: HydroInitialCondition {
                            is_on: true,
                            generation_mw: 0.0,
                            time_in_state: 1,
                        },
                    }],
                },
            ],
            turbine_type: TurbineType::Francis,
            volume_level_table: Some(vec![(0.0, 400.0), (250.0, 420.0), (500.0, 430.0)]),
            tailrace_coefficients: Some([380.0, 0.0, 0.0, 0.0, 0.0]),
        }],
        pumping_plants: Vec::new(),
        renewable_plants: vec![],
        inflow_history: vec![],
    }
}

/// Largest `|a - b|` over every `(plant, period)` of `system`; infinite when the two sets
/// have different keys.
fn max_abs_diff(system: &System, a: &EngMaxBounds, b: &EngMaxBounds) -> f64 {
    if a.len() != b.len() {
        return f64::INFINITY;
    }
    let mut largest = 0.0_f64;
    for plant in 0..system.hydro_plants.len() {
        for period in 0..system.horizon.periods {
            match (a.get(plant, period), b.get(plant, period)) {
                (Some(x), Some(y)) => largest = largest.max((x - y).abs()),
                (None, None) => {}
                _ => return f64::INFINITY,
            }
        }
    }
    largest
}

fn bounds_equal(system: &System, a: &EngMaxBounds, b: &EngMaxBounds) -> bool {
    max_abs_diff(system, a, b) == 0.0
}

/// Cross-run tolerance (ADR-016): two LPs may land on values that differ in the last ulps.
const ABS_TOL_HM3: f64 = 1e-9;
const REL_TOL: f64 = 1e-6;

/// True when both sets hold the same keys and every pair satisfies
/// `|a - b| <= ABS_TOL_HM3 + REL_TOL * |b|`.
fn bounds_close(system: &System, a: &EngMaxBounds, b: &EngMaxBounds) -> bool {
    a.len() == b.len()
        && (0..system.hydro_plants.len()).all(|plant| {
            (0..system.horizon.periods).all(|period| {
                match (a.get(plant, period), b.get(plant, period)) {
                    (Some(x), Some(y)) => (x - y).abs() <= ABS_TOL_HM3 + REL_TOL * y.abs(),
                    _ => false,
                }
            })
        })
}

/// Checks the four ADR-016 rules on the data one run recorded: (i) the trace is aligned with
/// the steps, (ii) the first solve used the initial bounds, (iii) every later solve used what
/// the solve before it produced, (iv) the trace saw the objective the step reports.
fn assert_engmax_trace_consistent(
    system: &System,
    steps: &[IterationReport],
    trace: &[EngMaxTraceEntry],
) {
    assert_eq!(
        trace.len(),
        steps.len(),
        "rule i: trace has {} entries for {} steps",
        trace.len(),
        steps.len()
    );
    let initial = initialize_engmax_bounds(system).expect("fixture has full Hef.m");
    for (k, (entry, step)) in trace.iter().zip(steps).enumerate() {
        assert!(
            entry.stage == step.stage && entry.iteration == step.iteration,
            "rule i: trace[{k}] is {:?} #{}, step[{k}] is {:?} #{}",
            entry.stage,
            entry.iteration,
            step.stage,
            step.iteration
        );
        if k == 0 {
            assert!(
                bounds_equal(system, &entry.used, &initial),
                "rule ii: trace[0].used != initial bounds"
            );
        } else {
            assert!(
                bounds_equal(system, &entry.used, &trace[k - 1].produced),
                "rule iii: trace[{k}].used != trace[{}].produced",
                k - 1
            );
        }
        assert!(
            entry.objective == step.objective_value,
            "rule iv: trace[{k}].objective {} != steps[{k}].objective_value {}",
            entry.objective,
            step.objective_value
        );
    }
}

#[test]
fn cut_loop_feeds_each_lp_the_previous_lp_bounds() {
    let system = engmax_network_fixture(LineLimit::Binding);
    let mut state = EngMaxState::new(&system).expect("fixture has full Hef.m");

    let result =
        run_simulation_with_state(&system, true, ExecutionOption::Iterative, Some(&mut state))
            .expect("fixture simulation solves");
    let trace = state.trace();

    let lp_indices: Vec<usize> = result
        .steps
        .iter()
        .enumerate()
        .filter(|(_, step)| step.stage == IterationStage::LinearProgramming)
        .map(|(k, _)| k)
        .collect();
    assert!(
        lp_indices.len() >= 2,
        "fixture became vacuous: the cut loop ran {} LP(s), need at least 2",
        lp_indices.len()
    );
    assert!(
        result.accumulated_flow_cuts > 0,
        "fixture became vacuous: no flow cut was accumulated"
    );
    assert_eq!(
        trace.len(),
        result.steps.len(),
        "R6: the EngMax trace is not aligned with the steps: a solve stage skipped after_solve"
    );
    let lp_first = lp_indices[0];
    let lp_second = lp_indices[1];
    let lp_last = lp_indices[lp_indices.len() - 1];
    let milp = result
        .steps
        .iter()
        .position(|step| step.stage == IterationStage::MixedIntegerLinearProgramming)
        .expect("Iterative path solves one MILP");

    let initial = initialize_engmax_bounds(&system).expect("fixture has full Hef.m");
    assert!(
        max_abs_diff(&system, &initial, &trace[lp_first].produced) > 1e-6,
        "fixture became vacuous: the first LP did not move the bounds off the initial ones"
    );
    assert!(
        max_abs_diff(&system, &trace[lp_first].produced, &trace[lp_last].produced) > 1e-6,
        "fixture became vacuous: the last LP produced the same bounds as the first"
    );
    assert!(
        max_abs_diff(&system, &initial, &trace[lp_last].produced) > 1e-6,
        "fixture became vacuous: the last LP produced the initial bounds"
    );

    assert!(
        max_abs_diff(&system, &trace[lp_second].used, &trace[lp_first].used) > 1e-6,
        "R6: bounds did not change between consecutive LPs of the cut loop"
    );
    assert!(
        bounds_equal(&system, &trace[milp].used, &trace[lp_last].produced),
        "R6: the MILP did not receive the bounds the last LP produced"
    );

    assert_engmax_trace_consistent(&system, &result.steps, trace);
}

#[test]
fn engmax_trace_is_consistent_on_every_path() {
    let system = engmax_network_fixture(LineLimit::Binding);
    for (option, network) in [
        (ExecutionOption::Iterative, true),
        (ExecutionOption::Iterative, false),
        (ExecutionOption::SingleMilp, true),
        (ExecutionOption::SingleMilp, false),
    ] {
        let case = format!("{option:?} network={network}");
        println!("checking case {case}");
        let mut state = EngMaxState::new(&system).expect("fixture has full Hef.m");

        let result = run_simulation_with_state(&system, network, option, Some(&mut state))
            .map_err(|error| format!("{case}: {error}"))
            .expect("fixture simulation solves");

        assert!(
            !state.trace().is_empty(),
            "{case}: the run recorded no EngMax trace, so the checker would pass vacuously"
        );
        assert_engmax_trace_consistent(&system, &result.steps, state.trace());
    }
}

#[test]
fn engmax_bounds_match_between_network_and_no_network_paths() {
    let system = engmax_network_fixture(LineLimit::NonBinding);
    let mut state_net = EngMaxState::new(&system).expect("fixture has full Hef.m");
    let mut state_plain = EngMaxState::new(&system).expect("fixture has full Hef.m");

    let net = run_simulation_with_state(
        &system,
        true,
        ExecutionOption::Iterative,
        Some(&mut state_net),
    )
    .expect("network path solves");
    let plain = run_simulation_with_state(
        &system,
        false,
        ExecutionOption::Iterative,
        Some(&mut state_plain),
    )
    .expect("no-network path solves");

    let net_lps = net
        .steps
        .iter()
        .filter(|step| step.stage == IterationStage::LinearProgramming)
        .count();
    assert!(
        net_lps == 1 && net.accumulated_flow_cuts == 0,
        "fixture no longer isolates the paths: the network run ran {net_lps} LP(s) and added {} \
         flow cut(s), expected 1 LP and 0 cuts",
        net.accumulated_flow_cuts
    );

    let labels = |steps: &[IterationReport]| -> Vec<String> {
        steps
            .iter()
            .map(|step| format!("{}#{}", step.stage.label(), step.iteration))
            .collect()
    };
    let expected = ["LP#1", "MILP#1", "LP-CALC-CMO#1"];
    assert_eq!(labels(&net.steps), expected, "network stage labels");
    assert_eq!(labels(&plain.steps), expected, "no-network stage labels");

    let net_trace = state_net.trace();
    let plain_trace = state_plain.trace();
    assert_eq!(
        net_trace.len(),
        plain_trace.len(),
        "the two paths recorded a different number of solves"
    );
    for (k, (net_entry, plain_entry)) in net_trace.iter().zip(plain_trace).enumerate() {
        assert!(
            bounds_close(&system, &net_entry.used, &plain_entry.used),
            "solve {k}: used bounds differ between paths by {} hm3",
            max_abs_diff(&system, &net_entry.used, &plain_entry.used)
        );
    }
    let net_last = net_trace.last().expect("network run recorded solves");
    let plain_last = plain_trace.last().expect("no-network run recorded solves");
    assert!(
        bounds_close(&system, &net_last.produced, &plain_last.produced),
        "final produced bounds differ between paths by {} hm3",
        max_abs_diff(&system, &net_last.produced, &plain_last.produced)
    );
}

/// A floor no dispatch can meet (`UTE1` peaks at 200 MW), so the first solve is infeasible and
/// the operational-limit slack re-solve runs. The infeasibility has nothing to do with EngMax
/// rows, which carry no slack (ADR-017).
fn infeasible_thermal_floor() -> OperationalLimit {
    OperationalLimit {
        target: OperationalLimitTarget::ThermalPlant(ThermalPlantId(1)),
        plant_name: "UTE1".into(),
        variable: OperationalLimitVariable::Generation,
        start_period: 1,
        end_period: 3,
        lower_bound: Some(10_000.0),
        upper_bound: None,
    }
}

#[test]
fn relaxed_solve_stage_recalculates_once_on_returned_solution() {
    let mut system = engmax_network_fixture(LineLimit::NonBinding);
    system.operational_limits = vec![infeasible_thermal_floor()];
    let dc_network = DcNetworkModel::from_system(&system).expect("fixture network is valid");
    let cuts = BTreeSet::<FlowCutKey>::new();
    let mut state = EngMaxState::new(&system).expect("fixture has full Hef.m");
    let mut steps = Vec::new();
    let mut network_infeasibilities = Vec::new();
    let mut operational_limit_infeasibilities = Vec::new();
    let mut solver = HighsSolver::new().expect("HiGHS instance");

    let (solution, _violations, used_slacks) = solve_stage(
        &system,
        &dc_network,
        SolveMode::LinearProgramming,
        &cuts,
        None,
        Some(&mut state),
        IterationStage::LinearProgramming,
        1,
        &mut steps,
        &mut network_infeasibilities,
        &mut operational_limit_infeasibilities,
        &mut solver,
    )
    .expect("the slack re-solve makes the stage feasible");

    assert!(used_slacks, "the first solve was not infeasible");
    assert_eq!(steps.len(), 1);
    assert_eq!(steps[0].solution_status, "OPTIMAL_WITH_SLACK");
    assert_eq!(
        state.trace().len(),
        1,
        "R42: the infeasible first attempt must not count as a solve"
    );
    assert!(
        state.trace()[0].objective == solution.objective,
        "R42: the trace saw {} but the returned relaxed solution has {}",
        state.trace()[0].objective,
        solution.objective
    );
    assert_engmax_trace_consistent(&system, &steps, state.trace());
}

#[test]
fn relaxed_solve_stage_without_network_recalculates_once_on_returned_solution() {
    let mut system = engmax_network_fixture(LineLimit::NonBinding);
    system.operational_limits = vec![infeasible_thermal_floor()];
    let mut state = EngMaxState::new(&system).expect("fixture has full Hef.m");
    let mut steps = Vec::new();
    let mut operational_limit_infeasibilities = Vec::new();
    let mut solver = HighsSolver::new().expect("HiGHS instance");

    let solution = solve_stage_without_network(
        &system,
        SolveMode::LinearProgramming,
        None,
        Some(&mut state),
        IterationStage::LinearProgramming,
        1,
        &mut steps,
        &mut operational_limit_infeasibilities,
        &mut solver,
    )
    .expect("the slack re-solve makes the stage feasible");

    assert_eq!(steps.len(), 1);
    assert_eq!(
        steps[0].solution_status, "OPTIMAL_WITH_SLACK",
        "the first solve was not infeasible"
    );
    assert_eq!(
        state.trace().len(),
        1,
        "R42: the infeasible first attempt must not count as a solve"
    );
    assert!(
        state.trace()[0].objective == solution.objective,
        "R42: the trace saw {} but the returned relaxed solution has {}",
        state.trace()[0].objective,
        solution.objective
    );
    assert_engmax_trace_consistent(&system, &steps, state.trace());
}

/// `engmax_network_fixture(NonBinding)` with no thermal plant and a tailrace of 500 m:
/// - the forebay is at most 430 m, below the 500 m tailrace, so the net head is 0;
/// - `h_ef` = 65 > 0, so every EngMax bound is 0;
/// - with no thermal plant, the 90 MW demand needs 90 hm3 of turbining per period.
///
/// It is feasible without EngMax rows and infeasible with them.
fn engmax_zero_fixture() -> System {
    let mut system = engmax_network_fixture(LineLimit::NonBinding);
    system.thermal_plants = vec![];
    system.hydro_plants[0].tailrace_coefficients = Some([500.0, 0.0, 0.0, 0.0, 0.0]);
    system
}

/// `engmax_network_fixture(NonBinding)` with no thermal plant and a 500 MW demand, which
/// exceeds what `Q_ef` = 100 hm3 can generate, with or without EngMax rows.
fn infeasible_without_engmax_fixture() -> System {
    let mut system = engmax_network_fixture(LineLimit::NonBinding);
    system.thermal_plants = vec![];
    system.submarkets[0].demand_mw = vec![500.0; 3];
    system.buses[1].demand_mw = vec![500.0; 3];
    system
}

/// A generation cap that never binds; it only makes the slack guard true, so a failing
/// direct solve is followed by the slack re-solve.
fn hydro_generation_cap() -> OperationalLimit {
    OperationalLimit {
        target: OperationalLimitTarget::HydroPlant(HydroPlantId(1)),
        plant_name: "UHE-ENG".into(),
        variable: OperationalLimitVariable::Generation,
        start_period: 1,
        end_period: 3,
        lower_bound: None,
        upper_bound: Some(1000.0),
    }
}

/// What one stage call left behind: its result and the state the call touched.
struct StageRun {
    result: Result<(), SimulationError>,
    solve_count: u64,
    steps: Vec<IterationReport>,
    trace_len: usize,
    record_count: usize,
}

/// Runs iteration 1 of the LP stage on a fresh solver and fresh records, through
/// `solve_stage` (empty cuts) when `network` is true and `solve_stage_without_network`
/// otherwise, with EngMax on or off.
fn run_stage(system: &System, network: bool, engmax: bool) -> StageRun {
    let mut state = EngMaxState::new(system).expect("fixture has full Hef.m");
    let mut steps = Vec::new();
    let mut network_infeasibilities = Vec::new();
    let mut operational_limit_infeasibilities = Vec::new();
    let mut solver = HighsSolver::new().expect("HiGHS instance");
    let engmax = engmax.then_some(&mut state);

    let result = if network {
        let dc_network = DcNetworkModel::from_system(system).expect("fixture network is valid");
        solve_stage(
            system,
            &dc_network,
            SolveMode::LinearProgramming,
            &BTreeSet::<FlowCutKey>::new(),
            None,
            engmax,
            IterationStage::LinearProgramming,
            1,
            &mut steps,
            &mut network_infeasibilities,
            &mut operational_limit_infeasibilities,
            &mut solver,
        )
        .map(|_| ())
    } else {
        solve_stage_without_network(
            system,
            SolveMode::LinearProgramming,
            None,
            engmax,
            IterationStage::LinearProgramming,
            1,
            &mut steps,
            &mut operational_limit_infeasibilities,
            &mut solver,
        )
        .map(|_| ())
    };

    StageRun {
        result,
        solve_count: solver.statistics().solve_count,
        steps,
        trace_len: state.trace().len(),
        record_count: network_infeasibilities.len() + operational_limit_infeasibilities.len(),
    }
}

#[test]
fn engmax_infeasibility_names_exceeded_rows() {
    for (network, capped) in [(true, false), (true, true), (false, false), (false, true)] {
        let mut system = engmax_zero_fixture();
        if capped {
            system.operational_limits = vec![hydro_generation_cap()];
        }

        let run = run_stage(&system, network, true);
        let error = run.result.expect_err("stage must fail");

        assert!(
            matches!(
                &error,
                SimulationError::EngMaxInfeasible {
                    stage: IterationStage::LinearProgramming,
                    iteration: 1,
                    rows,
                } if rows == &[
                    "engmax[p=UHE-ENG,t=1]",
                    "engmax[p=UHE-ENG,t=2]",
                    "engmax[p=UHE-ENG,t=3]",
                ]
            ),
            "network={network} capped={capped}: unexpected error {error:?}"
        );
        assert!(
            run.steps.is_empty(),
            "network={network} capped={capped}: the diagnosis pushed an IterationReport"
        );
        assert_eq!(
            run.trace_len, 0,
            "network={network} capped={capped}: the diagnosis called after_solve"
        );
        assert_eq!(
            run.record_count, 0,
            "network={network} capped={capped}: the diagnosis recorded an infeasibility"
        );
        assert_eq!(
            run.solve_count,
            if capped { 3 } else { 2 },
            "network={network} capped={capped}: solve count"
        );
    }
}

#[test]
fn unrelated_infeasibility_keeps_solver_error() {
    let system = infeasible_without_engmax_fixture();
    for network in [true, false] {
        let run = run_stage(&system, network, true);
        let error = run.result.expect_err("stage must fail");

        assert!(
            matches!(error, SimulationError::Solver(SolverError::Infeasible)),
            "network={network}: unexpected error {error:?}"
        );
        assert_eq!(
            run.solve_count, 2,
            "network={network}: direct solve plus one diagnosis"
        );
        assert!(
            run.steps.is_empty(),
            "network={network}: a failed stage pushed an IterationReport"
        );
    }
}

#[test]
fn disabled_engmax_infeasibility_is_unchanged() {
    for network in [true, false] {
        let control = run_stage(&infeasible_without_engmax_fixture(), network, false);
        let error = control.result.expect_err("control must fail");
        assert!(
            matches!(error, SimulationError::Solver(SolverError::Infeasible)),
            "network={network}: unexpected error {error:?}"
        );
        assert_eq!(
            control.solve_count, 1,
            "network={network}: EngMax off must not run a diagnosis"
        );

        let zero = run_stage(&engmax_zero_fixture(), network, false);
        assert!(
            zero.result.is_ok(),
            "network={network}: the zero-bound fixture must be feasible without EngMax: {:?}",
            zero.result
        );
    }
}
