//! Characterization of audit finding FP-R02 (silent efficiency and hydraulic-loss defaults) and of
//! the missing tailrace, on the pre-fix pipeline.
//!
//! Computed FPHA builds each plant's production function from `CAD/CAD_UHE.csv` (`Eficiencia`,
//! `Perda.m`) and `CAD/CAD_TAILRACE.csv`. When that data is absent the pipeline neither warns nor
//! skips the plant: `ProductionFunction::new` substitutes efficiency 1.0 and loss 0 m, and a plant
//! without a tailrace row sees a 0 m tailrace level. Each baseline test removes the data from a
//! derived copy of `examples/24Barras`, measures how much the production function inflates against
//! an unmodified control run on the control fit grid, and prints the ratios the baseline evidence
//! report quotes. The control test shows the official inputs do carry the data, so the inflation
//! is a property of the removed input and not of the case.

use std::collections::BTreeMap;
use std::error::Error;

use labdessem_core::entities::HydroPlant;
use labdessem_core::system::System;
use labdessem_model::{Model, SolveMode};

use super::probe::{
    DerivedCase, EnvelopeRow, FitContext, computed_system, envelope_mw, fit_contexts,
    fit_grid_nodes, lp_fpha_rows,
};

/// Tailrace table the computed FPHA reads, relative to the case directory.
const TAILRACE_FILE: &str = "CAD/CAD_TAILRACE.csv";

/// Plant table carrying the `Eficiencia` and `Perda.m` columns, relative to the case directory.
const PLANTS_FILE: &str = "CAD/CAD_UHE.csv";

/// Plant whose tailrace row is removed: FURNAS in `examples/24Barras`.
const TAILRACE_PLANT_CODE: u32 = 3;

/// Hydro plants of `examples/24Barras`.
const HYDRO_PLANT_COUNT: usize = 4;

/// Control nodes whose FPH (or LP envelope) is below this value are skipped: the ratio is
/// ill-conditioned [MW].
const MIN_CONTROL_MW: f64 = 1.0;

/// A removed input counts as inflating the production function when the mean ratio exceeds this.
const MIN_MEAN_FPH_RATIO: f64 = 1.01;

/// Largest accepted difference between two net heads that must coincide [m].
const HEAD_TOLERANCE_M: f64 = 1e-9;

/// Largest accepted relative difference between two efficiencies that must coincide.
const EFFICIENCY_TOLERANCE: f64 = 1e-9;

/// One computed-FPHA run on a derived case: the system with the injected segments, the fit
/// context of every fitted plant and the LP model, as the `labdessem` binary would build them.
struct CaseRun {
    system: System,
    contexts: BTreeMap<i32, FitContext>,
    model: Model,
    plants_processed: usize,
    plants_skipped: usize,
}

impl CaseRun {
    fn new(case: &DerivedCase) -> Result<Self, Box<dyn Error>> {
        let (system, result) = computed_system(case.path())?;
        let contexts = fit_contexts(case.path(), &system)?;
        let model = Model::from_system(&system, SolveMode::LinearProgramming);
        Ok(Self {
            plants_processed: result.plants_processed,
            plants_skipped: result.plants_skipped,
            system,
            contexts,
            model,
        })
    }

    fn context(&self, code: u32) -> &FitContext {
        let key = i32::try_from(code).expect("plant code fits i32");
        assert!(
            self.contexts.contains_key(&key),
            "no fit context for plant {code}"
        );
        &self.contexts[&key]
    }

    /// Period-1 `hydro_fpha` rows of the plant named `plant_name`.
    fn envelope_rows(&self, plant_name: &str) -> Vec<EnvelopeRow> {
        let flow_conversion = self.system.horizon.period_duration_hours * 0.0036;
        lp_fpha_rows(&self.model, plant_name, flow_conversion)
    }
}

fn code_of(plant: &HydroPlant) -> u32 {
    u32::try_from(plant.id.0).expect("plant code fits u32")
}

fn plant_by_code(system: &System, code: u32) -> &HydroPlant {
    system
        .hydro_plants
        .iter()
        .find(|plant| code_of(plant) == code)
        .expect("the 24Barras system has a hydro plant with the requested code")
}

fn mean(values: &[f64]) -> f64 {
    values.iter().sum::<f64>() / values.len() as f64
}

/// Asserts that every hydro plant of `system` carries (`present`) or lacks (`!present`) both
/// `efficiency` and `hydraulic_loss_m`.
fn assert_physical_data_presence(system: &System, present: bool) {
    assert_eq!(
        system.hydro_plants.len(),
        HYDRO_PLANT_COUNT,
        "24Barras has {HYDRO_PLANT_COUNT} hydro plants"
    );
    for plant in &system.hydro_plants {
        assert_eq!(
            plant.efficiency.is_some(),
            present,
            "plant {} ({}): efficiency is {:?}, expected present={present}",
            plant.id.0,
            plant.name,
            plant.efficiency
        );
        assert_eq!(
            plant.hydraulic_loss_m.is_some(),
            present,
            "plant {} ({}): hydraulic_loss_m is {:?}, expected present={present}",
            plant.id.0,
            plant.name,
            plant.hydraulic_loss_m
        );
    }
}

/// `value` of the control plant, asserted present with a message naming the plant.
fn required(value: Option<f64>, quantity: &str, control: &FitContext) -> f64 {
    assert!(
        value.is_some(),
        "plant {} ({}): the control has no {quantity}",
        control.params.codigo,
        control.params.nome
    );
    value.expect("presence asserted above")
}

/// Fit-grid nodes of `control` with `q > 0` where its uncapped FPH is at least `MIN_CONTROL_MW`.
///
/// The control grid is the single node set of every comparison: the derived context must not
/// choose its own nodes.
fn qualifying_nodes(control: &FitContext) -> Vec<(f64, f64)> {
    fit_grid_nodes(control)
        .into_iter()
        .filter(|&(v, q)| q > 0.0 && control.pf.evaluate(v, q, 0.0) >= MIN_CONTROL_MW)
        .collect()
}

/// Mean and maximum of `derived FPH / control FPH` over `qualifying_nodes(control)`, with the
/// uncapped `evaluate` at `s = 0` (`evaluate_capped` would hide the inflation at Pmax).
fn fph_ratio(derived: &FitContext, control: &FitContext) -> (f64, f64) {
    let code = control.params.codigo;
    let name = &control.params.nome;
    let nodes = qualifying_nodes(control);
    assert!(
        !nodes.is_empty(),
        "plant {code} ({name}): no control node with q > 0 and FPH >= {MIN_CONTROL_MW} MW"
    );
    let ratios: Vec<f64> = nodes
        .iter()
        .map(|&(v, q)| derived.pf.evaluate(v, q, 0.0) / control.pf.evaluate(v, q, 0.0))
        .collect();
    let ratio_mean = mean(&ratios);
    let ratio_max = ratios.iter().copied().fold(f64::NEG_INFINITY, f64::max);
    assert!(
        ratio_mean.is_finite() && ratio_max.is_finite(),
        "plant {code} ({name}): FPH ratio is not finite (mean {ratio_mean}, max {ratio_max})"
    );
    (ratio_mean, ratio_max)
}

/// Mean of `derived LP envelope / control LP envelope` at `s = 0`, and the node count, over the
/// qualifying control nodes whose control envelope is finite and at least `MIN_CONTROL_MW`.
///
/// Informational: the ratio is not comparable with `fph_ratio`, because the hull fit samples
/// `evaluate_capped`, so for a plant whose derived FPH exceeds its installed capacity the planes
/// saturate near it and the LP envelope hides most of the inflation the uncapped ratio shows.
fn lp_envelope_ratio_mean(
    derived_rows: &[EnvelopeRow],
    control_rows: &[EnvelopeRow],
    control: &FitContext,
) -> (f64, usize) {
    let code = control.params.codigo;
    let name = &control.params.nome;
    assert!(
        !derived_rows.is_empty(),
        "plant {code} ({name}): the derived model has no hydro_fpha rows"
    );
    assert!(
        !control_rows.is_empty(),
        "plant {code} ({name}): the control model has no hydro_fpha rows"
    );
    let ratios: Vec<f64> = qualifying_nodes(control)
        .into_iter()
        .filter_map(|(v, q)| {
            let control_mw = envelope_mw(control_rows, v, q, 0.0);
            let derived_mw = envelope_mw(derived_rows, v, q, 0.0);
            (control_mw.is_finite() && control_mw >= MIN_CONTROL_MW).then(|| {
                assert!(
                    derived_mw.is_finite(),
                    "plant {code} ({name}) at v={v}, q={q}: derived envelope {derived_mw} MW is not finite"
                );
                derived_mw / control_mw
            })
        })
        .collect();
    assert!(
        !ratios.is_empty(),
        "plant {code} ({name}): no node with a finite control envelope >= {MIN_CONTROL_MW} MW"
    );
    (mean(&ratios), ratios.len())
}

/// Prints the single tagged line per affected plant that the baseline evidence report quotes.
fn print_ratios(
    tag: &str,
    plant: &HydroPlant,
    (fph_mean, fph_max): (f64, f64),
    lp_envelope_mean: f64,
) {
    eprintln!(
        "baseline_a3[{tag}] code={} name={} fph_ratio_mean={fph_mean:.6} \
         fph_ratio_max={fph_max:.6} lp_envelope_ratio_mean={lp_envelope_mean:.6}",
        plant.id.0, plant.name
    );
}

/// Asserts that `derived` (built without `Eficiencia` and `Perda.m`) is the control production
/// function with efficiency 1.0 and loss 0 m, at every qualifying control node.
///
/// Net head: `derived = control + control loss`, since the control net head is positive there.
/// Efficiency: `(G / h)` is `K * efficiency * q`, so the derived-to-control ratio of `G / h`
/// times the control efficiency must be 1 (derived efficiency 1.0) without needing `K`.
fn assert_defaults_are_neutral(derived: &FitContext, control: &FitContext) {
    let code = control.params.codigo;
    let name = &control.params.nome;
    let loss = required(control.params.hydraulic_loss_m, "Perda.m", control);
    let efficiency = required(control.params.efficiency, "Eficiencia", control);
    for (v, q) in qualifying_nodes(control) {
        let control_head = control.pf.net_head(v, q);
        let derived_head = derived.pf.net_head(v, q);
        assert!(
            (derived_head - (control_head + loss)).abs() <= HEAD_TOLERANCE_M,
            "plant {code} ({name}) at v={v}, q={q}: derived net head {derived_head} m, expected \
             the control {control_head} m plus the control loss {loss} m"
        );
        let control_unit = control.pf.evaluate(v, q, 0.0) / control_head;
        let derived_unit = derived.pf.evaluate(v, q, 0.0) / derived_head;
        let implied_unit_ratio = derived_unit / control_unit * efficiency;
        assert!(
            (implied_unit_ratio - 1.0).abs() <= EFFICIENCY_TOLERANCE,
            "plant {code} ({name}) at v={v}, q={q}: derived efficiency is not 1.0 (implied unit \
             ratio {implied_unit_ratio}, control efficiency {efficiency})"
        );
    }
}

/// Characterizes audit FP-R02 (and the missing tailrace) on the pre-fix code. Replaced by ticket-016.
///
/// A plant without a `CAD_TAILRACE.csv` row is fitted, not skipped or rejected: its context has no
/// tailrace model, so the net head is the forebay height minus the hydraulic loss at every node
/// (tailrace level 0 m), and the FPH exceeds the control one. If `build_fit_params` rejected the
/// missing row, the `Ok` expectation would fail.
#[test]
fn baseline_a3_missing_tailrace_row_uses_zero_level() {
    // Arrange
    let control_case = DerivedCase::new("computed").expect("control case from examples/24Barras");
    let derived_case = DerivedCase::new("computed").expect("derived case from examples/24Barras");
    let removed = derived_case
        .remove_plant_rows(TAILRACE_FILE, TAILRACE_PLANT_CODE)
        .expect("remove the plant 3 row from CAD_TAILRACE.csv");
    eprintln!("baseline_a3[info] code=3 removed_tailrace_rows={removed}");
    assert_eq!(
        removed, 1,
        "CAD_TAILRACE.csv has exactly one row for plant 3: the case would not be derived"
    );

    // Act
    let control = CaseRun::new(&control_case).expect("the unmodified case must compute");
    let derived = CaseRun::new(&derived_case)
        .expect("computed_system must succeed although plant 3 has no tailrace row");
    let control_ctx = control.context(TAILRACE_PLANT_CODE);
    let derived_ctx = derived.context(TAILRACE_PLANT_CODE);
    let plant = plant_by_code(&derived.system, TAILRACE_PLANT_CODE);
    let nodes = fit_grid_nodes(control_ctx);
    let fph = fph_ratio(derived_ctx, control_ctx);
    let (lp_mean, lp_nodes) = lp_envelope_ratio_mean(
        &derived.envelope_rows(&plant.name),
        &control.envelope_rows(&plant.name),
        control_ctx,
    );
    eprintln!(
        "baseline_a3[info] code=3 plants_processed={} plants_skipped={} fit_grid_nodes={} \
         fph_nodes={} lp_envelope_nodes={lp_nodes}",
        derived.plants_processed,
        derived.plants_skipped,
        nodes.len(),
        qualifying_nodes(control_ctx).len()
    );
    print_ratios("tailrace", plant, fph, lp_mean);

    // Assert
    assert_eq!(control.plants_skipped, 0, "the control must skip no plant");
    assert_eq!(derived.plants_skipped, 0, "no plant may be skipped");
    assert_eq!(derived.plants_processed, HYDRO_PLANT_COUNT);
    assert!(
        control_ctx.params.tailrace.is_some(),
        "the control plant 3 must carry a tailrace model"
    );
    assert!(
        derived_ctx.params.tailrace.is_none(),
        "plant 3 must have no tailrace model after its row was removed"
    );
    let loss = derived_ctx
        .params
        .hydraulic_loss_m
        .expect("plant 3 hydraulic_loss_m must be present in the official CAD_UHE.csv");
    assert!(!nodes.is_empty(), "plant 3 has an empty fit grid");
    for &(v, q) in &nodes {
        let expected = (derived_ctx.params.forebay.height(v) - loss).max(0.0);
        let actual = derived_ctx.pf.net_head(v, q);
        assert!(
            (actual - expected).abs() <= HEAD_TOLERANCE_M,
            "plant 3 at v={v}, q={q}: net head {actual} m, expected forebay height minus loss \
             {expected} m (tailrace level 0 m)"
        );
    }
    assert!(
        fph.0 > MIN_MEAN_FPH_RATIO,
        "plant 3: mean FPH ratio {:.6} is not above {MIN_MEAN_FPH_RATIO}",
        fph.0
    );
}

/// Characterizes audit FP-R02 (and the missing tailrace) on the pre-fix code. Replaced by ticket-016.
///
/// Without the `Perda.m` and `Eficiencia` columns every plant is fitted with `None` efficiency and
/// loss, which `ProductionFunction::new` turns into efficiency 1.0 and loss 0 m: the derived FPH
/// is the control one with exactly those two values swapped in, and exceeds it. If
/// `build_fit_params` rejected the missing data, the `Ok` expectation would fail.
#[test]
fn baseline_a3_missing_efficiency_and_loss_default_silently() {
    // Arrange
    let control_case = DerivedCase::new("computed").expect("control case from examples/24Barras");
    let derived_case = DerivedCase::new("computed").expect("derived case from examples/24Barras");
    derived_case
        .drop_columns(PLANTS_FILE, &["Perda.m", "Eficiencia"])
        .expect("drop Perda.m and Eficiencia from CAD_UHE.csv");

    // Act
    let control = CaseRun::new(&control_case).expect("the unmodified case must compute");
    let derived = CaseRun::new(&derived_case)
        .expect("computed_system must succeed although CAD_UHE.csv has no Perda.m and Eficiencia");

    // Assert
    assert_physical_data_presence(&control.system, true);
    assert_physical_data_presence(&derived.system, false);
    assert_eq!(control.plants_skipped, 0, "the control must skip no plant");
    assert_eq!(derived.plants_skipped, 0, "no plant may be skipped");
    assert_eq!(derived.plants_processed, HYDRO_PLANT_COUNT);
    for plant in &derived.system.hydro_plants {
        let code = code_of(plant);
        let control_ctx = control.context(code);
        let derived_ctx = derived.context(code);
        let fph = fph_ratio(derived_ctx, control_ctx);
        let (lp_mean, lp_nodes) = lp_envelope_ratio_mean(
            &derived.envelope_rows(&plant.name),
            &control.envelope_rows(&plant.name),
            control_ctx,
        );
        eprintln!(
            "baseline_a3[info] code={code} fit_grid_nodes={} fph_nodes={} lp_envelope_nodes={lp_nodes}",
            fit_grid_nodes(control_ctx).len(),
            qualifying_nodes(control_ctx).len()
        );
        print_ratios("efficiency_loss", plant, fph, lp_mean);

        assert!(
            derived_ctx.params.efficiency.is_none()
                && derived_ctx.params.hydraulic_loss_m.is_none(),
            "plant {code} ({}): the fit params must carry no efficiency and no loss",
            plant.name
        );
        assert!(
            derived_ctx.params.tailrace.is_some(),
            "plant {code} ({}): the tailrace row was not removed in this case",
            plant.name
        );
        assert!(
            fph.0 > MIN_MEAN_FPH_RATIO,
            "plant {code} ({}): mean FPH ratio {:.6} is not above {MIN_MEAN_FPH_RATIO}",
            plant.name,
            fph.0
        );
        assert_defaults_are_neutral(derived_ctx, control_ctx);
    }
}

/// Control for the physical-data baselines: the official inputs carry efficiency, hydraulic loss
/// and a tailrace model for every plant with units, so the inflation measured by the baselines
/// comes from the removed input. Stays valid after the fix.
#[test]
fn control_a3_official_inputs_carry_physical_data() {
    // Arrange
    let case = DerivedCase::new("computed").expect("control case from examples/24Barras");

    // Act
    let run = CaseRun::new(&case).expect("the unmodified case must compute");

    // Assert
    assert_eq!(run.plants_skipped, 0, "the control must skip no plant");
    assert_eq!(run.plants_processed, HYDRO_PLANT_COUNT);
    assert_physical_data_presence(&run.system, true);
    let mut plants_with_groups = 0;
    for plant in run
        .system
        .hydro_plants
        .iter()
        .filter(|plant| !plant.groups.is_empty())
    {
        let code = code_of(plant);
        assert!(
            plant
                .efficiency
                .is_some_and(|efficiency| efficiency > 0.0 && efficiency < 1.0),
            "plant {code} ({}): efficiency {:?} must be in (0, 1), so the 1.0 default differs",
            plant.name,
            plant.efficiency
        );
        assert!(
            plant.hydraulic_loss_m.is_some_and(|loss| loss > 0.0),
            "plant {code} ({}): hydraulic_loss_m {:?} must be positive, so the 0 m default differs",
            plant.name,
            plant.hydraulic_loss_m
        );
        assert!(
            run.context(code).params.tailrace.is_some(),
            "plant {code} ({}): the fit params must carry a tailrace model",
            plant.name
        );
        plants_with_groups += 1;
    }
    assert_eq!(
        plants_with_groups, HYDRO_PLANT_COUNT,
        "every 24Barras hydro plant has units"
    );
}
