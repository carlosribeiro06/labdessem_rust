//! Dynamic maximum swallowing bounds across the solves of one simulation run.
//!
//! [`MaxSwallowingState`] holds the bounds of one simulation run. `solve_stage` and
//! `solve_stage_without_network` (in `stages/solve.rs`) build each model from
//! [`MaxSwallowingState::bounds`] and call [`MaxSwallowingState::after_solve`] once after every solve of
//! the full-horizon re-solve sequence (LP cut loop, MILP, LP-FIXED, LP-CALC-CMO).
//!
//! The first solve uses the bounds of [`MaxSwallowingState::new`] (initial volume); every later solve
//! uses the bounds recalculated from the solution of the solve before it.

use labdessem_core::entities::{HydroPlant, MaxSwallowingBounds};
use labdessem_core::system::System;
use labdessem_model::hydro_geometry::{GeometryError, NetHeadModel};
use labdessem_model::indexing::{ConstraintOffsetParams, Indexing};
use labdessem_model::names::display_period;
use labdessem_model::swallowing::{
    PlantReference, net_head_m, partial_effective_head_groups, plant_bound_hm3, plant_head_model,
};
use labdessem_model::{ModelError, SolveMode};
use labdessem_solver::accessor::SolutionAccessor;
use labdessem_solver::types::LpSolution;
use tracing::{debug, info, warn};

use crate::error::SimulationError;
use crate::flow_units::volume_hm3_to_flow_m3s;
use crate::results::{MaxSwallowingDiagnostic, MaxSwallowingOutcome};
use crate::workflow::IterationStage;

/// Items kept in a [`WarningSummary`] sample.
const WARNING_SAMPLE_SIZE: usize = 5;

/// Count and first [`WARNING_SAMPLE_SIZE`] items of one max swallowing warning class, returned as
/// data so tests assert counts, never log text (ADR-013).
#[derive(Debug, Default, PartialEq, Eq)]
struct WarningSummary<T> {
    count: usize,
    sample: Vec<T>,
}

impl<T> WarningSummary<T> {
    fn record(&mut self, item: T) {
        self.count += 1;
        if self.sample.len() < WARNING_SAMPLE_SIZE {
            self.sample.push(item);
        }
    }
}

/// Plant outflow in m3/s with every unit turbining at its maximum.
///
/// This is `Q_ef` of [`PlantReference`] in m3/s, the first solve's tailrace flow with zero
/// spillage (ADR-007).
fn plant_max_outflow_m3s(plant: &HydroPlant, period_duration_hours: f64) -> f64 {
    volume_hm3_to_flow_m3s(
        PlantReference::from_plant(plant).q_ef_hm3,
        period_duration_hours,
    )
}

/// [`plant_head_model`] of every plant of `system`, indexed by plant.
fn system_geometry(system: &System) -> Result<Vec<Option<NetHeadModel>>, GeometryError> {
    system.hydro_plants.iter().map(plant_head_model).collect()
}

/// Average volume (hm3) of `plant_idx` over `period` in the solved model:
/// the mean of the volume at the start (slot `period`) and at the end
/// (slot `period + 1`) of the period (ADR-006).
fn period_average_volume_hm3(
    accessor: &SolutionAccessor<'_>,
    plant_idx: usize,
    period: usize,
) -> f64 {
    f64::midpoint(
        accessor.hydro_volume(plant_idx, period),
        accessor.hydro_volume(plant_idx, period + 1),
    )
}

/// Accessor over a solution of the model built for `system` in `solve_mode`. Max swallowing reads only
/// its volume, turbining and spillage columns.
fn primal_accessor<'a>(
    system: &System,
    solve_mode: SolveMode,
    solution: &'a LpSolution,
    indexing: &Indexing,
) -> SolutionAccessor<'a> {
    let params = ConstraintOffsetParams::from_system(system, solve_mode);
    SolutionAccessor::new(solution, indexing, &params)
}

/// Global unit-entry indices of the units of `plant_idx`, in input order.
fn plant_unit_entries(indexing: &Indexing, plant_idx: usize) -> Vec<usize> {
    indexing
        .hydro_unit_entries
        .iter()
        .enumerate()
        .filter(|(_, entry)| entry.plant_idx == plant_idx)
        .map(|(entry_idx, _)| entry_idx)
        .collect()
}

/// Turbining (hm3) of `period` summed over the plant's units (`unit_entries`).
fn plant_turbining_hm3(
    accessor: &SolutionAccessor<'_>,
    unit_entries: &[usize],
    period: usize,
) -> f64 {
    unit_entries
        .iter()
        .map(|&unit_idx| accessor.hydro_turbining(unit_idx, period))
        .sum()
}

/// Plant outflow (m3/s) of `period` in the solved model: the turbining of the plant's
/// units (`unit_entries`, global unit-entry indices) plus the plant spillage (D3, ADR-007).
/// Diversion and pumping are not part of it.
fn previous_outflow_m3s(
    accessor: &SolutionAccessor<'_>,
    unit_entries: &[usize],
    plant_idx: usize,
    period: usize,
    period_duration_hours: f64,
) -> f64 {
    volume_hm3_to_flow_m3s(
        plant_turbining_hm3(accessor, unit_entries, period)
            + accessor.hydro_spillage(plant_idx, period),
        period_duration_hours,
    )
}

/// Rejects a plant with `Hef.m <= 0` on some of its groups but not on all of them.
fn check_effective_heads(system: &System) -> Result<(), SimulationError> {
    for plant in &system.hydro_plants {
        let groups = partial_effective_head_groups(plant);
        if !groups.is_empty() {
            return Err(SimulationError::PartialEffectiveHead {
                plant: plant.name.clone(),
                groups,
            });
        }
    }
    Ok(())
}

/// Plants with at least one unit and `Hef.m <= 0` on every unit (their max swallowing is `Q_ef`
/// and their row never binds), warned once per run (R53, ADR-013).
///
/// Plants without units are not counted: they get bound 0 and no row. The partial case is
/// rejected earlier by [`check_effective_heads`].
fn summarise_inert_plants(system: &System) -> WarningSummary<String> {
    let mut summary = WarningSummary::default();
    for plant in &system.hydro_plants {
        let units = || plant.groups.iter().flat_map(|group| &group.units);
        if units().next().is_some() && units().all(|unit| unit.effective_head_m <= 0.0) {
            summary.record(plant.name.clone());
        }
    }

    if summary.count > 0 {
        warn!(
            count = summary.count,
            sample = ?summary.sample,
            "max swallowing: plants with Hef.m <= 0 on every unit are inert (bound = Q_ef, the row never binds)"
        );
    }

    summary
}

/// Bounds of the first solve: every period of every plant from the initial volume, and the
/// plant-periods (plant name, 1-based period) that got a zero bound from a non-positive net head.
fn initial_bounds(
    system: &System,
    geometry: &[Option<NetHeadModel>],
) -> (MaxSwallowingBounds, WarningSummary<(String, usize)>) {
    let periods = system.horizon.periods;
    let mut bounds = MaxSwallowingBounds::new(system.hydro_plants.len() * periods);
    let mut summary = WarningSummary::default();

    for (plant_idx, plant) in system.hydro_plants.iter().enumerate() {
        let reference = PlantReference::from_plant(plant);
        let net_head = net_head_m(
            geometry[plant_idx].as_ref(),
            reference.h_ef_m,
            plant.reservoir.initial_volume_hm3,
            plant_max_outflow_m3s(plant, system.horizon.period_duration_hours),
        );
        let bound = plant_bound_hm3(&reference, net_head);
        for period in 0..periods {
            if reference.h_ef_m > 0.0 && net_head <= 0.0 {
                summary.record((plant.name.clone(), display_period(period)));
            }
            bounds.set(plant_idx, period, bound);
        }
    }

    info!(
        plants = system.hydro_plants.len(),
        periods,
        total_bounds = bounds.len(),
        "Initialized max swallowing bounds from initial volumes"
    );
    if summary.count > 0 {
        warn!(
            count = summary.count,
            sample = ?summary.sample,
            "max swallowing: non-positive net head at the initial volume; these plant-periods get a zero bound"
        );
    }

    (bounds, summary)
}

/// Recalculate every bound of `max_swallowing_bounds` from `solution`, the solution of the previous
/// solve, with the geometry of `system`.
///
/// The bound for period `t` uses the previous solve's average volume over `t`,
/// `(V[t] + V[t + 1]) / 2` in the accessor slot layout (slot 0 = initial volume,
/// slot `t + 1` = end of period `t`). The first solve's bounds come from
/// [`MaxSwallowingState::new`] (initial volume).
///
/// When the plant has VHA and tailrace data, the net head is the VHA forebay
/// level at that volume, minus the tailrace polynomial level, minus the hydraulic
/// loss in metres. The tailrace level is taken at the previous solve's plant outflow
/// of period `t`: unit turbining plus plant spillage, in m3/s; diversion and pumping
/// are excluded (ADR-007). The first solve's bounds use the plant maximum outflow
/// with zero spillage. Without VHA and tailrace data the net head falls back to the
/// plant `h_ef`. The bound is the raw max swallowing, not capped by `Q_ef`.
///
/// Returns the plant-periods (plant name, 1-based period) that got a zero bound from a
/// non-positive net head.
fn recalculate_bounds(
    system: &System,
    geometry: &[Option<NetHeadModel>],
    solve_mode: SolveMode,
    solution: &LpSolution,
    indexing: &Indexing,
    max_swallowing_bounds: &mut MaxSwallowingBounds,
) -> WarningSummary<(String, usize)> {
    let accessor = primal_accessor(system, solve_mode, solution, indexing);
    let mut summary = WarningSummary::default();

    for (plant_idx, plant) in system.hydro_plants.iter().enumerate() {
        let reference = PlantReference::from_plant(plant);
        let unit_entries = plant_unit_entries(indexing, plant_idx);

        for period in 0..system.horizon.periods {
            let volume_hm3 = period_average_volume_hm3(&accessor, plant_idx, period);
            let tailrace_flow_m3s = previous_outflow_m3s(
                &accessor,
                &unit_entries,
                plant_idx,
                period,
                system.horizon.period_duration_hours,
            );
            let net_head = net_head_m(
                geometry[plant_idx].as_ref(),
                reference.h_ef_m,
                volume_hm3,
                tailrace_flow_m3s,
            );
            if reference.h_ef_m > 0.0 && net_head <= 0.0 {
                summary.record((plant.name.clone(), display_period(period)));
            }
            max_swallowing_bounds.set(plant_idx, period, plant_bound_hm3(&reference, net_head));
        }
    }

    info!(
        updated_count = system.hydro_plants.len() * system.horizon.periods,
        plants = system.hydro_plants.len(),
        periods = system.horizon.periods,
        "Recalculated max swallowing bounds"
    );
    if summary.count > 0 {
        warn!(
            count = summary.count,
            sample = ?summary.sample,
            "max swallowing: non-positive net head in the recalculated bounds; these plant-periods get a zero bound"
        );
    }

    summary
}

/// Excess (m3/s) of a plant's turbining over its recomputed max swallowing treated as zero (ADR-011).
const EXCESS_TOLERANCE_M3S: f64 = 0.01;

/// Largest relative excess the consistency diagnostic logs at INFO; above it, WARN (ADR-011).
const WARN_RELATIVE_EXCESS: f64 = 0.01;

fn diagnostic_warns(diagnostic: &MaxSwallowingDiagnostic) -> bool {
    diagnostic.max_relative_excess > WARN_RELATIVE_EXCESS || diagnostic.zero_bound_exceed_count > 0
}

/// Compares the turbining of `solution` with `recomputed` over every plant with units and every
/// model period, in input order, and logs the result once (ADR-011, R12).
///
/// Plants without units are skipped, like the max swallowing rows of the model.
fn consistency_diagnostic(
    system: &System,
    indexing: &Indexing,
    solve_mode: SolveMode,
    solution: &LpSolution,
    recomputed: &MaxSwallowingBounds,
) -> Result<MaxSwallowingDiagnostic, SimulationError> {
    let accessor = primal_accessor(system, solve_mode, solution, indexing);
    let duration_hours = system.horizon.period_duration_hours;
    let mut diagnostic = MaxSwallowingDiagnostic::default();

    for (plant_idx, plant) in system.hydro_plants.iter().enumerate() {
        let unit_entries = plant_unit_entries(indexing, plant_idx);
        if unit_entries.is_empty() {
            continue;
        }

        for period in 0..system.horizon.periods {
            let bound_hm3 = recomputed.get(plant_idx, period).ok_or_else(|| {
                ModelError::MissingMaxSwallowingBound {
                    plant: plant.name.clone(),
                    period,
                }
            })?;
            let excess_m3s = volume_hm3_to_flow_m3s(
                plant_turbining_hm3(&accessor, &unit_entries, period) - bound_hm3,
                duration_hours,
            );
            if excess_m3s <= EXCESS_TOLERANCE_M3S {
                continue;
            }

            diagnostic.exceed_count += 1;
            if bound_hm3 > 0.0 {
                diagnostic.max_relative_excess = diagnostic
                    .max_relative_excess
                    .max(excess_m3s / volume_hm3_to_flow_m3s(bound_hm3, duration_hours));
            } else {
                diagnostic.zero_bound_exceed_count += 1;
            }
            if excess_m3s > diagnostic.max_excess_m3s {
                diagnostic.max_excess_m3s = excess_m3s;
                diagnostic.worst = Some((plant.name.clone(), display_period(period)));
            }
        }
    }

    if diagnostic_warns(&diagnostic) {
        warn!(
            exceed_count = diagnostic.exceed_count,
            zero_bound_exceed_count = diagnostic.zero_bound_exceed_count,
            max_excess_m3s = diagnostic.max_excess_m3s,
            max_relative_excess = diagnostic.max_relative_excess,
            worst = ?diagnostic.worst,
            "max swallowing consistency: the final turbining exceeds the recomputed max swallowing bound"
        );
    } else {
        info!(
            exceed_count = diagnostic.exceed_count,
            zero_bound_exceed_count = diagnostic.zero_bound_exceed_count,
            max_excess_m3s = diagnostic.max_excess_m3s,
            max_relative_excess = diagnostic.max_relative_excess,
            worst = ?diagnostic.worst,
            "max swallowing consistency of the final solution"
        );
    }

    Ok(diagnostic)
}

/// One `after_solve` call recorded for the path-level tests (ADR-016).
#[cfg(test)]
#[derive(Debug, Clone)]
pub(crate) struct MaxSwallowingTraceEntry {
    pub(crate) stage: IterationStage,
    pub(crate) iteration: usize,
    pub(crate) used: MaxSwallowingBounds,
    pub(crate) objective: f64,
    pub(crate) produced: MaxSwallowingBounds,
}

/// Max swallowing bounds across the solves of one simulation run (R1, ADR-010).
pub(crate) struct MaxSwallowingState {
    // bounds the next solve builds its max swallowing rows from
    next: MaxSwallowingBounds,
    // bounds the last finished solve was built from
    used: MaxSwallowingBounds,
    // net head model of each plant, built once in `new`; indexed by plant
    geometry: Vec<Option<NetHeadModel>>,
    #[cfg(test)]
    trace: Vec<MaxSwallowingTraceEntry>,
}

impl MaxSwallowingState {
    /// Start a run: `next` holds the initial-volume bounds and `used` is empty.
    ///
    /// # Errors
    ///
    /// Returns [`SimulationError::PartialEffectiveHead`] when a plant has `Hef.m <= 0` on some
    /// of its groups but not on all of them, and [`SimulationError::Geometry`] when the VHA
    /// table of a plant with units is empty or has a duplicate volume.
    pub(crate) fn new(system: &System) -> Result<Self, SimulationError> {
        check_effective_heads(system)?;
        let geometry = system_geometry(system)?;
        summarise_inert_plants(system);
        Ok(Self {
            next: initial_bounds(system, &geometry).0,
            used: MaxSwallowingBounds::default(),
            geometry,
            #[cfg(test)]
            trace: Vec::new(),
        })
    }

    /// Bounds the next solve builds its max swallowing rows from.
    pub(crate) fn bounds(&self) -> &MaxSwallowingBounds {
        &self.next
    }

    /// Advance the state after a solve: the bounds that built the solve become `used` and
    /// `next` is recalculated from `solution`.
    ///
    /// Call exactly once per solve, on the solution the solve returns (the relaxed one when
    /// slack was used), with the `solve_mode` that solve used.
    pub(crate) fn after_solve(
        &mut self,
        system: &System,
        indexing: &Indexing,
        stage: IterationStage,
        iteration: usize,
        solve_mode: SolveMode,
        solution: &LpSolution,
    ) {
        std::mem::swap(&mut self.used, &mut self.next);
        recalculate_bounds(
            system,
            &self.geometry,
            solve_mode,
            solution,
            indexing,
            &mut self.next,
        );
        debug!(
            stage = stage.label(),
            iteration,
            solve_mode = ?solve_mode,
            "max swallowing bounds recalculated after solve"
        );
        #[cfg(test)]
        self.trace.push(MaxSwallowingTraceEntry {
            stage,
            iteration,
            used: self.used.clone(),
            objective: solution.objective,
            produced: self.next.clone(),
        });
    }

    /// The outcome of the run (ADR-011), called after its last solve (LP-CALC-CMO) with the
    /// solution of that solve: `used` built it, `recomputed` is the max swallowing of its solution and
    /// `diagnostic` compares its turbining with `recomputed`.
    ///
    /// `solve_mode` is the mode that solve used (LP-FIXED for LP-CALC-CMO).
    ///
    /// # Errors
    ///
    /// Returns [`SimulationError::Model`] when `recomputed` holds no bound for a plant with
    /// units and a model period.
    pub(crate) fn outcome(
        &self,
        system: &System,
        solve_mode: SolveMode,
        solution: &LpSolution,
    ) -> Result<MaxSwallowingOutcome, SimulationError> {
        let indexing = Indexing::from_system(system);
        Ok(MaxSwallowingOutcome {
            used: self.used.clone(),
            recomputed: self.next.clone(),
            diagnostic: consistency_diagnostic(
                system, &indexing, solve_mode, solution, &self.next,
            )?,
        })
    }

    #[cfg(test)]
    pub(crate) fn trace(&self) -> &[MaxSwallowingTraceEntry] {
        &self.trace
    }
}

#[cfg(test)]
// tests: a panic is the failure report
#[allow(clippy::unwrap_used, clippy::expect_used)]
mod tests {
    use super::{
        MaxSwallowingState, WarningSummary, consistency_diagnostic, diagnostic_warns,
        initial_bounds, plant_max_outflow_m3s, recalculate_bounds, summarise_inert_plants,
        system_geometry,
    };
    use crate::error::SimulationError;
    use crate::results::MaxSwallowingDiagnostic;
    use crate::workflow::IterationStage;
    use labdessem_core::entities::{
        HydroGroup, HydroInitialCondition, HydroPlant, HydroUnit, MaxSwallowingBounds, Reservoir,
        TurbineType,
    };
    use labdessem_core::ids::{BusId, HydroGroupId, HydroPlantId, HydroUnitId, SubmarketId};
    use labdessem_core::system::{StudyHorizon, Submarket, System};
    use labdessem_model::hydro_geometry::GeometryError;
    use labdessem_model::indexing::Indexing;
    use labdessem_model::swallowing::{
        PlantReference, net_head_m, plant_bound_hm3, plant_head_model,
    };
    use labdessem_model::{ModelError, SolveMode};
    use labdessem_solver::types::LpSolution;
    use std::error::Error;

    /// The bounds a new [`MaxSwallowingState`] gives the first solve.
    fn new_state_bounds(system: &System) -> Result<MaxSwallowingBounds, SimulationError> {
        MaxSwallowingState::new(system).map(|state| state.next)
    }

    fn build_test_system() -> System {
        System {
            horizon: StudyHorizon {
                periods: 2,
                period_duration_hours: 1.0,
                original_periods: 2,
                original_period_durations_hours: vec![1.0, 1.0],
                internal_to_original_period: vec![1, 2],
                internal_subperiod_index: vec![1, 1],
            },
            thermal_unit_commitment_enabled: false,
            hydro_unit_commitment_enabled: false,
            ton_residual_enabled: false,
            fpha_enabled: false,
            future_cost_enabled: false,
            residual_costs: vec![],
            future_cost_cuts: vec![],
            submarkets: vec![Submarket {
                id: SubmarketId(1),
                name: "SE".into(),
                demand_mw: vec![100.0, 100.0],
            }],
            interchange_limits: vec![],
            operational_limits: vec![],
            buses: vec![],
            branches: vec![],
            thermal_plants: vec![],
            hydro_plants: vec![HydroPlant {
                id: HydroPlantId(1),
                name: "UHE-1".into(),
                submarket_id: SubmarketId(1),
                bus_id: BusId(1),
                upstream_plant_ids: vec![],
                downstream_plant_id: None,
                diversion_upstream_plant_ids: vec![],
                diversion_plant_id: None,
                fpha_segments: vec![],
                reservoir: Reservoir {
                    min_volume_hm3: 10.0,
                    max_volume_hm3: 100.0,
                    initial_volume_hm3: 50.0,
                },
                natural_inflow_hm3: vec![1.0, 1.0],
                water_withdrawal_hm3: vec![0.0, 0.0],
                spillage_cost_per_hm3: 0.0,
                turbining_cost_per_hm3: 0.0,
                specific_productivity_mw_per_m3s: 1.0,
                hydraulic_loss_m: None,
                efficiency: None,
                groups: vec![HydroGroup {
                    id: HydroGroupId(1),
                    name: "CJ-1".into(),
                    units: vec![HydroUnit {
                        id: HydroUnitId(1),
                        name: "UG-1".into(),
                        min_generation_mw: 0.0,
                        max_generation_mw: 50.0,
                        max_turbining_hm3: 10.0,
                        effective_head_m: 50.0,
                        initial_condition: HydroInitialCondition {
                            is_on: true,
                            generation_mw: 0.0,
                            time_in_state: 1,
                        },
                    }],
                }],
                turbine_type: TurbineType::Francis,
                volume_level_table: None,
                tailrace_coefficients: None,
            }],
            pumping_plants: vec![],
            renewable_plants: vec![],
            inflow_history: vec![],
        }
    }

    #[test]
    fn initialize_creates_bounds_for_all_periods() {
        let system = build_test_system();
        let bounds = new_state_bounds(&system).expect("initialize bounds");

        // 1 plant × 2 periods = 2 bounds
        assert_eq!(bounds.len(), 2);

        assert!(bounds.get(0, 0).is_some());
        assert!(bounds.get(0, 1).is_some());
    }

    #[test]
    fn initialize_uses_initial_volume_for_head() {
        let system = build_test_system();
        let bounds = new_state_bounds(&system).expect("initialize bounds");

        // Single unit, no geometry: bound = Q_ef = 10.0
        let max_swallowing = bounds.get(0, 0).unwrap();
        assert!((max_swallowing - 10.0).abs() < 0.01);
    }

    /// `build_test_system` with plant `UHE-MULTI`: groups `CJ-1` (`UG-1`, `UG-2`) and `CJ-2`
    /// (`UG-3`, `UG-4`), every unit a copy of `UG-1` (10 hm3, `Hef.m` 50 m).
    fn build_multi_group_system() -> System {
        let mut system = build_test_system();
        let plant = &mut system.hydro_plants[0];
        plant.name = "UHE-MULTI".into();
        let template = plant.groups[0].units[0].clone();
        let unit = |n: usize| HydroUnit {
            id: HydroUnitId(n),
            name: format!("UG-{n}"),
            ..template.clone()
        };
        plant.groups = vec![
            HydroGroup {
                id: HydroGroupId(1),
                name: "CJ-1".into(),
                units: vec![unit(1), unit(2)],
            },
            HydroGroup {
                id: HydroGroupId(2),
                name: "CJ-2".into(),
                units: vec![unit(3), unit(4)],
            },
        ];
        system
    }

    fn set_effective_head(system: &mut System, group_idx: usize, head_m: f64) {
        for unit in &mut system.hydro_plants[0].groups[group_idx].units {
            unit.effective_head_m = head_m;
        }
    }

    /// One Francis plant with VHA and tailrace geometry: `Q_ef` = 0.72 hm3 (two units of
    /// 0.36 hm3 = 100 m3/s each over 1 h), plant max outflow 200 m3/s.
    fn geometry_plant_system(h_ef_m: f64, loss_m: Option<f64>) -> System {
        let mut system = build_test_system();
        let plant = &mut system.hydro_plants[0];
        plant.name = "UHE-GEO".into();
        plant.turbine_type = TurbineType::Francis;
        plant.reservoir = Reservoir {
            min_volume_hm3: 100.0,
            max_volume_hm3: 800.0,
            initial_volume_hm3: 250.0,
        };
        plant.volume_level_table = Some(vec![(100.0, 905.0), (400.0, 910.0), (800.0, 913.0)]);
        plant.tailrace_coefficients = Some([885.3, 0.0, 1.97e-5, -2.55e-8, 0.0]);
        plant.hydraulic_loss_m = loss_m;
        let unit = &mut plant.groups[0].units[0];
        unit.max_turbining_hm3 = 0.36;
        unit.effective_head_m = h_ef_m;
        let second = HydroUnit {
            id: HydroUnitId(2),
            name: "UG-2".into(),
            ..unit.clone()
        };
        plant.groups[0].units.push(second);
        system
    }

    fn assert_rel(actual: f64, expected: f64) {
        assert!(
            (actual - expected).abs() <= 1e-9 * expected.abs().max(1.0),
            "expected {expected}, got {actual}"
        );
    }

    /// Solution whose every volume slot is `volume_hm3`, every unit turbines
    /// `turbining_hm3` and every spillage is 0.0.
    fn uniform_solution(system: &System, volume_hm3: f64, turbining_hm3: f64) -> LpSolution {
        let horizon = system.horizon.periods;
        let indexing = Indexing::from_system(system);
        let offsets = indexing.compute_column_offsets(horizon);
        let mut primal = vec![0.0; offsets.total];
        for plant_idx in 0..indexing.hydro_plant_entries.len() {
            let start = offsets.hydro_volume + plant_idx * (horizon + 1);
            primal[start..=start + horizon].fill(volume_hm3);
        }
        let start = offsets.hydro_turbining;
        primal[start..start + indexing.hydro_units * horizon].fill(turbining_hm3);
        LpSolution {
            objective: 0.0,
            primal,
            dual: vec![],
            reduced_costs: vec![0.0; offsets.total],
            iterations: 0,
            solve_time_seconds: 0.0,
        }
    }

    /// D2(b) (ADR-006): the head volume of a period is the mean of its start and end volumes.
    fn d2_head_volume_hm3(start_hm3: f64, end_hm3: f64) -> f64 {
        f64::midpoint(start_hm3, end_hm3)
    }

    /// D3 (ADR-007): the tailrace flow of a period is the previous solve's plant outflow, unit
    /// turbining plus plant spillage, in m3/s; diversion and pumping are not part of it.
    fn d3_tailrace_flow_m3s(turbining_hm3: &[f64], spillage_hm3: f64, duration_hours: f64) -> f64 {
        (turbining_hm3.iter().sum::<f64>() + spillage_hm3) * 1e6 / (duration_hours * 3600.0)
    }

    #[test]
    fn initialize_stores_one_bound_per_plant_and_period() {
        let system = build_multi_group_system();
        let bounds = new_state_bounds(&system).expect("initialize bounds");

        assert_eq!(
            bounds.len(),
            system.hydro_plants.len() * system.horizon.periods
        );
    }

    #[test]
    fn initialize_gives_plant_without_units_a_zero_bound() {
        let mut system = build_multi_group_system();
        system.hydro_plants[0].groups.clear();
        let bounds = new_state_bounds(&system).expect("initialize bounds");

        assert_eq!(bounds.len(), system.horizon.periods);
        for period in 0..system.horizon.periods {
            assert!(bounds.get(0, period).unwrap().abs() < 1e-12);
        }
    }

    #[test]
    fn initialize_rejects_partial_effective_head() {
        let mut system = build_multi_group_system();
        set_effective_head(&mut system, 1, 0.0);

        let error = new_state_bounds(&system).expect_err("partial Hef.m must be rejected");

        assert!(matches!(
            &error,
            SimulationError::PartialEffectiveHead { plant, groups }
                if plant == "UHE-MULTI" && *groups == ["CJ-2"]
        ));
        assert_eq!(
            error.to_string(),
            "max swallowing: plant UHE-MULTI has Hef.m <= 0 on group(s) CJ-2 but not on every group; \
             set Hef.m for every group of the plant"
        );
    }

    #[test]
    fn initialize_keeps_plant_without_effective_head_inert() {
        let mut system = build_multi_group_system();
        set_effective_head(&mut system, 0, 0.0);
        set_effective_head(&mut system, 1, 0.0);

        let bounds = new_state_bounds(&system).expect("inert plant is accepted");

        // Q_ef = 4 units × 10.0
        for period in 0..system.horizon.periods {
            assert!((bounds.get(0, period).unwrap() - 40.0).abs() < 1e-12);
        }
    }

    #[test]
    fn inert_plant_summary_counts_only_all_zero_plants() {
        let mut system = build_multi_group_system();
        let mut inert = system.hydro_plants[0].clone();
        inert.id = HydroPlantId(2);
        inert.name = "UHE-INERT".into();
        for unit in inert.groups.iter_mut().flat_map(|group| &mut group.units) {
            unit.effective_head_m = 0.0;
        }
        let mut empty = system.hydro_plants[0].clone();
        empty.id = HydroPlantId(3);
        empty.name = "UHE-EMPTY".into();
        empty.groups = vec![];
        system.hydro_plants.push(inert);
        system.hydro_plants.push(empty);

        assert_eq!(
            summarise_inert_plants(&system),
            WarningSummary {
                count: 1,
                sample: vec!["UHE-INERT".to_owned()],
            }
        );
        assert!(MaxSwallowingState::new(&system).is_ok());
    }

    #[test]
    fn inert_plant_summary_is_empty_without_inert_plants() {
        let system = build_multi_group_system();

        assert_eq!(summarise_inert_plants(&system), WarningSummary::default());
    }

    #[test]
    fn plant_max_outflow_pins_furnas_first_solve_flow() {
        // 24Barras FURNAS over 0.5 h periods: six units of 188 m3/s in group 1 and two of 189 m3/s
        // in group 2, converted to hm3 as io's flow_m3s_to_hm3 does. The flat unit fold of
        // PlantReference gives 1505.9999999999998 m3/s, not 1506 (RD-23, R31).
        let mut system = build_multi_group_system();
        let plant = &mut system.hydro_plants[0];
        let template = plant.groups[0].units[0].clone();
        let unit = |n: usize, max_turbining_m3s: f64| HydroUnit {
            id: HydroUnitId(n),
            name: format!("UG-{n}"),
            max_turbining_hm3: max_turbining_m3s * 0.5 * 0.0036,
            ..template.clone()
        };
        plant.groups[0].units = (1..=6).map(|n| unit(n, 188.0)).collect();
        plant.groups[1].units = (7..=8).map(|n| unit(n, 189.0)).collect();

        let flow_m3s = plant_max_outflow_m3s(&system.hydro_plants[0], 0.5);

        assert_eq!(flow_m3s.to_bits(), 0x4097_87ff_ffff_ffff);
    }

    #[test]
    fn stored_bound_exceeds_q_ef_above_h_ef() {
        let system = geometry_plant_system(20.0, None);
        let periods = system.horizon.periods;
        // Net head 21.616 m > h_ef = 20 m and the bound is not capped by Q_ef = 0.72 hm3:
        // 0.72 * (21.616 / 20)^0.5 = 0.72 * 1.039615... = 0.748523025698
        let expected = 0.748_523_025_698;

        let initialized = new_state_bounds(&system).expect("initialize bounds");

        let indexing = Indexing::from_system(&system);
        let solution = uniform_solution(&system, 250.0, 0.36);
        let mut recalculated = MaxSwallowingBounds::new(periods);
        recalculate_bounds(
            &system,
            &system_geometry(&system).expect("valid geometry"),
            SolveMode::LinearProgramming,
            &solution,
            &indexing,
            &mut recalculated,
        );

        for bounds in [&initialized, &recalculated] {
            for period in 0..periods {
                let bound = bounds
                    .get(0, period)
                    .expect("bound stored for every period");
                assert!(bound > 0.72);
                assert_rel(bound, expected);
            }
        }
    }

    #[test]
    fn recalculate_uses_period_average_volume() {
        let system = geometry_plant_system(30.0, None);
        let periods = system.horizon.periods;
        let indexing = Indexing::from_system(&system);
        let mut solution = uniform_solution(&system, 0.0, 0.36);
        // Plant 0 volume slots: slot 0 = initial volume, slot t + 1 = end of period t.
        let slots = [250.0, 600.0, 400.0];
        let volume_start = indexing.compute_column_offsets(periods).hydro_volume;
        solution.primal[volume_start..volume_start + slots.len()].copy_from_slice(&slots);

        let mut bounds = MaxSwallowingBounds::new(periods);
        recalculate_bounds(
            &system,
            &system_geometry(&system).expect("valid geometry"),
            SolveMode::LinearProgramming,
            &solution,
            &indexing,
            &mut bounds,
        );

        // D2 head volumes: period 0 = (250 + 600) / 2 = 425, period 1 = (600 + 400) / 2 = 500.
        // Tailrace at 200 m3/s is 885.884 m, Q_ef = 0.72 hm3, h_ef = 30 m.
        // Period 0: forebay = 910 + (425 - 400) / 400 * 3 = 910.1875, net head = 24.3035,
        //   bound = 0.72 * (24.3035 / 30)^0.5 = 0.648046664986.
        // Period 1: forebay = 910 + (500 - 400) / 400 * 3 = 910.75, net head = 24.866,
        //   bound = 0.72 * (24.866 / 30)^0.5 = 0.655503226537.
        // Rejected rules (each gives a different pair):
        //   today's rule (250, 250):   0.611166 for both periods;
        //   start of period (250, 600): 0.611166 and 0.665315;
        //   end of period (600, 400):   0.665315 and 0.645542.
        let plant = &system.hydro_plants[0];
        let reference = PlantReference::from_plant(plant);
        let geometry = plant_head_model(plant).expect("valid geometry");
        let head_volumes = [
            d2_head_volume_hm3(slots[0], slots[1]),
            d2_head_volume_hm3(slots[1], slots[2]),
        ];
        assert_rel(head_volumes[0], 425.0);
        assert_rel(head_volumes[1], 500.0);

        let expected = [0.648_046_664_986, 0.655_503_226_537];
        for period in 0..periods {
            let bound = bounds
                .get(0, period)
                .expect("bound stored for every period");
            assert_rel(bound, expected[period]);
            let head_m = net_head_m(
                geometry.as_ref(),
                reference.h_ef_m,
                head_volumes[period],
                200.0,
            );
            assert_rel(bound, plant_bound_hm3(&reference, head_m));
        }
    }

    #[test]
    fn recalculate_uses_previous_plant_outflow() {
        let system = geometry_plant_system(30.0, None);
        let periods = system.horizon.periods;
        let indexing = Indexing::from_system(&system);
        let offsets = indexing.compute_column_offsets(periods);
        let mut solution = uniform_solution(&system, 250.0, 0.0);
        // Per period (hm3): turbining of each of the two units, plant spillage and diversion.
        // The diversion (0.09 hm3 = 25 m3/s) is there only to show that it is excluded.
        let unit_turbining = [0.18, 0.09];
        let spillage = [0.18, 0.72];
        for period in 0..periods {
            for unit in 0..2 {
                solution.primal[offsets.hydro_turbining + unit * periods + period] =
                    unit_turbining[period];
            }
            solution.primal[offsets.hydro_spillage + period] = spillage[period];
            solution.primal[offsets.hydro_diversion + period] = 0.09;
        }

        let mut bounds = MaxSwallowingBounds::new(periods);
        recalculate_bounds(
            &system,
            &system_geometry(&system).expect("valid geometry"),
            SolveMode::LinearProgramming,
            &solution,
            &indexing,
            &mut bounds,
        );

        // Every volume slot is 250 hm3, so the D2 head volume is 250 hm3 and the forebay is
        // 905 + (250 - 100) / 300 * 5 = 907.5 m in both periods. Q_ef = 0.72 hm3, h_ef = 30 m.
        // Period 0: Qjus = (0.18 + 0.18 + 0.18) * 1e6 / 3600 = 150 m3/s,
        //   tailrace = 885.3 + 1.97e-5 * 150^2 - 2.55e-8 * 150^3 = 885.6571875 m,
        //   net head = 21.8428125 m, bound = 0.72 * (21.8428125 / 30)^0.5 = 0.614364549759.
        // Period 1: Qjus = (0.09 + 0.09 + 0.72) * 1e6 / 3600 = 250 m3/s,
        //   tailrace = 886.1328125 m, net head = 21.3671875 m,
        //   bound = 0.72 * (21.3671875 / 30)^0.5 = 0.607638873016.
        // Rejected rules (each gives a different pair):
        //   plant max outflow (200, 200 m3/s): 0.611166 for both periods;
        //   turbining only (100, 50 m3/s):     0.616970 and 0.618725;
        //   outflow plus diversion (175, 275): 0.612823 and 0.605835.
        let plant = &system.hydro_plants[0];
        let reference = PlantReference::from_plant(plant);
        let geometry = plant_head_model(plant).expect("valid geometry");
        let flows = [
            d3_tailrace_flow_m3s(&[0.18, 0.18], 0.18, system.horizon.period_duration_hours),
            d3_tailrace_flow_m3s(&[0.09, 0.09], 0.72, system.horizon.period_duration_hours),
        ];
        assert_rel(flows[0], 150.0);
        assert_rel(flows[1], 250.0);

        let expected = [0.614_364_549_759, 0.607_638_873_016];
        for period in 0..periods {
            let bound = bounds
                .get(0, period)
                .expect("bound stored for every period");
            assert_rel(bound, expected[period]);
            let head_m = net_head_m(geometry.as_ref(), reference.h_ef_m, 250.0, flows[period]);
            assert_rel(bound, plant_bound_hm3(&reference, head_m));
        }
    }

    #[test]
    fn after_solve_swaps_buffers_and_records_trace() {
        let system = geometry_plant_system(30.0, None);
        let indexing = Indexing::from_system(&system);
        let mut state = MaxSwallowingState::new(&system).expect("initialize state");

        // Initial volume 250 hm3: 0.72 * (21.616 / 30)^0.5 = 0.611166491228
        assert_every_bound(state.bounds(), 2, 0.611_166_491_228);

        // Uniform 600 hm3 at 200 m3/s: forebay 911.5, net head 25.616,
        //   0.72 * (25.616 / 30)^0.5 = 0.665315323738
        let mut first = uniform_solution(&system, 600.0, 0.36);
        first.objective = 7.0;
        state.after_solve(
            &system,
            &indexing,
            IterationStage::LinearProgramming,
            1,
            SolveMode::LinearProgramming,
            &first,
        );

        // Uniform 400 hm3: forebay 910, net head 24.116,
        //   0.72 * (24.116 / 30)^0.5 = 0.645542004830
        let second = uniform_solution(&system, 400.0, 0.36);
        state.after_solve(
            &system,
            &indexing,
            IterationStage::MixedIntegerLinearProgramming,
            1,
            SolveMode::MixedIntegerLinearProgramming,
            &second,
        );

        let trace = state.trace();
        assert_eq!(trace.len(), 2);

        assert_eq!(trace[0].stage, IterationStage::LinearProgramming);
        assert_eq!(trace[0].iteration, 1);
        assert_rel(trace[0].objective, 7.0);
        assert_every_bound(&trace[0].used, 2, 0.611_166_491_228);
        assert_every_bound(&trace[0].produced, 2, 0.665_315_323_738);

        assert_eq!(
            trace[1].stage,
            IterationStage::MixedIntegerLinearProgramming
        );
        assert_eq!(trace[1].iteration, 1);
        assert_every_bound(&trace[1].used, 2, 0.665_315_323_738);
        assert_every_bound(&trace[1].produced, 2, 0.645_542_004_830);

        assert_every_bound(state.bounds(), 2, 0.645_542_004_830);
    }

    #[test]
    fn max_swallowing_outcome_holds_used_and_recomputed_bounds() {
        let system = geometry_plant_system(30.0, None);
        let indexing = Indexing::from_system(&system);
        let mut state = MaxSwallowingState::new(&system).expect("initialize state");

        let first = uniform_solution(&system, 600.0, 0.36);
        state.after_solve(
            &system,
            &indexing,
            IterationStage::MixedIntegerLinearProgramming,
            1,
            SolveMode::MixedIntegerLinearProgramming,
            &first,
        );

        let last = uniform_solution(&system, 400.0, 0.36);
        state.after_solve(
            &system,
            &indexing,
            IterationStage::LinearProgrammingCalculateCmo,
            1,
            SolveMode::LinearProgrammingWithFixedCommitment,
            &last,
        );

        let outcome = state
            .outcome(
                &system,
                SolveMode::LinearProgrammingWithFixedCommitment,
                &last,
            )
            .expect("outcome of the final solution");

        // `used` built the last solve, so it is what the first solution produced: uniform 600 hm3
        // at 200 m3/s: forebay 911.5, net head 25.616, 0.72 * (25.616 / 30)^0.5 = 0.665315323738
        assert_every_bound(&outcome.used, 2, 0.665_315_323_738);

        // `recomputed` is the max swallowing of the last solution: uniform 400 hm3: forebay 910, net head
        // 24.116, 0.72 * (24.116 / 30)^0.5 = 0.645542004830
        assert_every_bound(&outcome.recomputed, 2, 0.645_542_004_830);

        // The final turbining is two units at 0.36 hm3 over 1 h, 200 m3/s, against the bound
        // 0.645542 hm3, 179.317 m3/s, in both periods: both are counted and period 1 wins the tie.
        assert_eq!(outcome.diagnostic.exceed_count, 2);
        assert_eq!(outcome.diagnostic.worst, Some(("UHE-GEO".to_owned(), 1)));
        assert_eq!(
            outcome.diagnostic,
            consistency_diagnostic(
                &system,
                &indexing,
                SolveMode::LinearProgrammingWithFixedCommitment,
                &last,
                &outcome.recomputed
            )
            .expect("diagnostic of the final solution")
        );
    }

    fn bounds_from(rows: &[[f64; 2]]) -> MaxSwallowingBounds {
        let mut bounds = MaxSwallowingBounds::new(rows.len() * 2);
        for (plant_idx, row) in rows.iter().enumerate() {
            for (period, &bound_hm3) in row.iter().enumerate() {
                bounds.set(plant_idx, period, bound_hm3);
            }
        }
        bounds
    }

    fn diagnose(
        system: &System,
        turbining_hm3: f64,
        bounds: &MaxSwallowingBounds,
    ) -> MaxSwallowingDiagnostic {
        consistency_diagnostic(
            system,
            &Indexing::from_system(system),
            SolveMode::LinearProgrammingWithFixedCommitment,
            &uniform_solution(system, 50.0, turbining_hm3),
            bounds,
        )
        .expect("every plant with units has a bound")
    }

    // 0.36 hm3 over 1 h is 100 m3/s; 0.3528 hm3 is 98 m3/s; 0.3582 hm3 is 99.5 m3/s;
    // 0.359982 hm3 is 99.995 m3/s.
    #[test]
    fn consistency_diagnostic_measures_excess_over_positive_bound() {
        let system = build_test_system();

        // Period 1: 100 - 98 = 2 m3/s. Period 2: 100 - 99.995 = 0.005 m3/s, within the tolerance.
        let diagnostic = diagnose(&system, 0.36, &bounds_from(&[[0.3528, 0.359_982]]));

        assert_eq!(diagnostic.exceed_count, 1);
        assert_eq!(diagnostic.zero_bound_exceed_count, 0);
        assert_rel(diagnostic.max_excess_m3s, 2.0);
        assert_rel(diagnostic.max_relative_excess, 2.0 / 98.0);
        assert_eq!(diagnostic.worst, Some(("UHE-1".to_owned(), 1)));
        assert!(diagnostic_warns(&diagnostic));

        // Period 1: 100 - 99.5 = 0.5 m3/s, 0.5025% of the bound: at most 1%, so INFO.
        let diagnostic = diagnose(&system, 0.36, &bounds_from(&[[0.3582, 0.359_982]]));

        assert_eq!(diagnostic.exceed_count, 1);
        assert_rel(diagnostic.max_relative_excess, 0.5 / 99.5);
        assert!(!diagnostic_warns(&diagnostic));
    }

    #[test]
    fn consistency_diagnostic_counts_zero_bound_apart() {
        let system = build_test_system();

        let diagnostic = diagnose(&system, 0.36, &bounds_from(&[[0.0, 0.36]]));

        assert_eq!(diagnostic.exceed_count, 1);
        assert_eq!(diagnostic.zero_bound_exceed_count, 1);
        assert_rel(diagnostic.max_excess_m3s, 100.0);
        assert_eq!(diagnostic.max_relative_excess, 0.0);
        assert!(diagnostic.max_excess_m3s.is_finite());
        assert!(diagnostic.max_relative_excess.is_finite());
        assert_eq!(diagnostic.worst, Some(("UHE-1".to_owned(), 1)));
        assert!(diagnostic_warns(&diagnostic));
    }

    #[test]
    fn consistency_diagnostic_skips_tolerance_over_zero_bound() {
        let system = build_test_system();

        // 0.000018 hm3 over 1 h is 0.005 m3/s, within the tolerance over a zero bound.
        let diagnostic = diagnose(&system, 0.000_018, &bounds_from(&[[0.0, 0.0]]));

        assert_eq!(diagnostic, MaxSwallowingDiagnostic::default());
        assert!(!diagnostic_warns(&diagnostic));
    }

    #[test]
    fn consistency_diagnostic_first_plant_wins_a_tie() {
        let mut system = build_test_system();
        let mut second = system.hydro_plants[0].clone();
        second.id = HydroPlantId(2);
        second.name = "UHE-2".into();
        system.hydro_plants.push(second);

        // The same 2 m3/s excess at (UHE-1, period 2) and at (UHE-2, period 1).
        let diagnostic = diagnose(
            &system,
            0.36,
            &bounds_from(&[[0.36, 0.3528], [0.3528, 0.36]]),
        );

        assert_eq!(diagnostic.exceed_count, 2);
        assert_rel(diagnostic.max_excess_m3s, 2.0);
        assert_eq!(diagnostic.worst, Some(("UHE-1".to_owned(), 2)));
    }

    #[test]
    fn consistency_diagnostic_skips_plant_without_units() {
        let mut system = build_test_system();
        let mut empty = system.hydro_plants[0].clone();
        empty.id = HydroPlantId(2);
        empty.name = "UHE-EMPTY".into();
        empty.groups = vec![];
        system.hydro_plants.push(empty);

        // No bound is stored for the plant without units, as the initial bounds leave none.
        let diagnostic = diagnose(&system, 0.36, &bounds_from(&[[0.36, 0.36]]));

        assert_eq!(diagnostic, MaxSwallowingDiagnostic::default());
    }

    #[test]
    fn consistency_diagnostic_rejects_missing_bound() {
        let system = build_test_system();
        let mut bounds = MaxSwallowingBounds::new(1);
        bounds.set(0, 0, 0.36);

        let error = consistency_diagnostic(
            &system,
            &Indexing::from_system(&system),
            SolveMode::LinearProgrammingWithFixedCommitment,
            &uniform_solution(&system, 50.0, 0.36),
            &bounds,
        )
        .expect_err("period 2 has no bound");

        assert!(matches!(
            &error,
            SimulationError::Model(ModelError::MissingMaxSwallowingBound { plant, period })
                if plant == "UHE-1" && *period == 1
        ));
    }

    #[test]
    fn max_swallowing_state_rejects_malformed_vha_table() {
        let duplicate = vec![
            (100.0, 905.0),
            (400.0, 910.0),
            (400.0, 911.0),
            (800.0, 913.0),
        ];

        let mut empty = geometry_plant_system(30.0, None);
        empty.hydro_plants[0].volume_level_table = Some(vec![]);
        let error = MaxSwallowingState::new(&empty)
            .err()
            .expect("empty VHA table must be rejected");
        assert!(matches!(
            &error,
            SimulationError::Geometry(GeometryError::InsufficientPoints { hydro_name, count: 0 })
                if hydro_name == "UHE-GEO"
        ));

        let mut repeated = geometry_plant_system(30.0, None);
        repeated.hydro_plants[0].volume_level_table = Some(duplicate.clone());
        let error = MaxSwallowingState::new(&repeated)
            .err()
            .expect("duplicate VHA volume must be rejected");
        assert!(matches!(
            &error,
            SimulationError::Geometry(GeometryError::DuplicateVolume { hydro_name, volume_hm3 })
                if hydro_name == "UHE-GEO" && *volume_hm3 == 400.0
        ));
        assert_eq!(
            error.to_string(),
            "max swallowing: hydro UHE-GEO: duplicate volume 400 in geometry table"
        );
        assert!(
            error
                .source()
                .and_then(|source| source.downcast_ref::<GeometryError>())
                .is_some()
        );

        let mut without_tailrace = geometry_plant_system(30.0, None);
        without_tailrace.hydro_plants[0].volume_level_table = Some(duplicate);
        without_tailrace.hydro_plants[0].tailrace_coefficients = None;
        let error = MaxSwallowingState::new(&without_tailrace)
            .err()
            .expect("a malformed table is rejected without a tailrace polynomial");
        assert!(matches!(
            error,
            SimulationError::Geometry(GeometryError::DuplicateVolume { .. })
        ));

        let mut without_units = geometry_plant_system(30.0, None);
        without_units.hydro_plants[0].volume_level_table = Some(vec![]);
        without_units.hydro_plants[0].groups.clear();
        assert!(MaxSwallowingState::new(&without_units).is_ok());
    }

    #[test]
    fn max_swallowing_state_accepts_single_point_vha_table() {
        let mut system = geometry_plant_system(30.0, None);
        system.hydro_plants[0].volume_level_table = Some(vec![(250.0, 907.5)]);

        let state = MaxSwallowingState::new(&system).expect("single-point VHA table is accepted");

        // 907.5 - 885.884 = 21.616 m: 0.72 * (21.616 / 30)^0.5
        for period in 0..system.horizon.periods {
            assert_rel(
                state
                    .bounds()
                    .get(0, period)
                    .expect("bound stored for every period"),
                0.611_166_491_228,
            );
        }
        // The single point is a constant forebay of 907.5 m, not interpolated to 911.5 m.
        let geometry = system_geometry(&system).expect("valid geometry");
        assert_rel(net_head_m(geometry[0].as_ref(), 30.0, 600.0, 200.0), 21.616);
    }

    /// The initial and the recalculated bounds of `system` (uniform solution at 250 hm3, every
    /// unit turbining 0.36 hm3), each with its non-positive net-head summary.
    fn initial_and_recalculated(
        system: &System,
    ) -> [(MaxSwallowingBounds, WarningSummary<(String, usize)>); 2] {
        let geometry = system_geometry(system).expect("valid geometry");
        let initial = initial_bounds(system, &geometry);
        let solution = uniform_solution(system, 250.0, 0.36);
        let mut bounds = MaxSwallowingBounds::new(system.horizon.periods);
        let summary = recalculate_bounds(
            system,
            &geometry,
            SolveMode::LinearProgramming,
            &solution,
            &Indexing::from_system(system),
            &mut bounds,
        );
        [initial, (bounds, summary)]
    }

    fn assert_every_bound(bounds: &MaxSwallowingBounds, periods: usize, expected: f64) {
        for period in 0..periods {
            assert_rel(
                bounds
                    .get(0, period)
                    .expect("bound stored for every period"),
                expected,
            );
        }
    }

    #[test]
    fn non_positive_net_head_summary_counts_plant_periods() {
        let mut system = geometry_plant_system(30.0, None);
        system.hydro_plants[0].tailrace_coefficients = Some([920.0, 0.0, 1.97e-5, -2.55e-8, 0.0]);

        for (bounds, summary) in initial_and_recalculated(&system) {
            assert_eq!(
                summary,
                WarningSummary {
                    count: 2,
                    sample: vec![("UHE-GEO".to_owned(), 1), ("UHE-GEO".to_owned(), 2)],
                }
            );
            assert_every_bound(&bounds, system.horizon.periods, 0.0);
        }
    }

    #[test]
    fn non_positive_net_head_summary_is_empty_for_positive_head() {
        let system = geometry_plant_system(30.0, None);

        // Net head 21.616 m at 250 hm3: 0.72 * (21.616 / 30)^0.5
        for (bounds, summary) in initial_and_recalculated(&system) {
            assert_eq!(summary, WarningSummary::default());
            assert_every_bound(&bounds, system.horizon.periods, 0.611_166_491_228);
        }
    }

    #[test]
    fn non_positive_net_head_summary_is_empty_for_inert_plant() {
        let mut system = geometry_plant_system(0.0, None);
        system.hydro_plants[0].tailrace_coefficients = Some([920.0, 0.0, 1.97e-5, -2.55e-8, 0.0]);

        // h_ef = 0: the calculator returns Q_ef, never 0
        for (bounds, summary) in initial_and_recalculated(&system) {
            assert_eq!(summary, WarningSummary::default());
            assert_every_bound(&bounds, system.horizon.periods, 0.72);
        }
    }

    #[test]
    fn warning_summary_keeps_first_sample_items() {
        let mut summary = WarningSummary::default();
        for item in 0..7_usize {
            summary.record(item);
        }

        assert_eq!(
            summary,
            WarningSummary {
                count: 7,
                sample: vec![0, 1, 2, 3, 4],
            }
        );
    }
}
