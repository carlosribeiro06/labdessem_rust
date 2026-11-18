//! Characterization of audit finding FP-02/FP-08 (alpha applied twice) on the computed-FPHA LP
//! envelope, on the pre-fix pipeline.
//!
//! Each test runs the computed-FPHA sequence on a derived copy of `examples/24Barras`, reads the
//! generation cap implied by the `hydro_fpha[...]` rows (`probe`), and compares it with the unscaled
//! fitted planes recovered from the returned ones and with the fit-quality deviation the pipeline
//! reports. Both tests print the measurement table the baseline evidence report quotes.

use labdessem_fpha::{FphaPlantResult, RawPlane};
use labdessem_model::{Model, SolveMode};

use super::probe::{
    DerivedCase, EnvelopeRow, FitContext, computed_system, envelope_mw, evaluation_grid_nodes,
    fit_contexts, fit_grid_nodes, lp_fpha_rows,
};

/// Nodes per axis of the uniform `(V, Q)` evaluation grid used for the LP-based deviation.
const EVALUATION_NODES_PER_AXIS: usize = 11;

/// Nodes whose intended generation is below this value are skipped: the ratio is ill-conditioned.
const MIN_INTENDED_MW: f64 = 1.0;

/// Largest accepted `|E_LP / intended - alpha|`.
const RATIO_TOLERANCE: f64 = 1e-9;

/// Largest accepted difference between two deviations that must coincide [MW].
const DEVIATION_TOLERANCE_MW: f64 = 1e-9;

/// A plant counts as materially scaled when `|alpha - 1|` exceeds this value.
const MATERIAL_ALPHA_OFFSET: f64 = 1e-3;

/// Smallest difference between the LP-based and the reported maximum deviation that counts as
/// "the diagnostic misses the LP envelope" [MW].
const MISSED_DEVIATION_MW: f64 = 1e-3;

/// `(mean_abs, max_abs, rms)` of `fitted - exact` over a node set [MW].
type DeviationStats = (f64, f64, f64);

/// `E_LP / intended` at one `(V hm3, Q m3/s)` fit-grid node with `s = 0`.
#[derive(Debug)]
struct RatioSample {
    v: f64,
    q: f64,
    ratio: f64,
}

/// Everything the two tests assert on or print for one plant.
#[derive(Debug)]
struct PlantMeasurement {
    code: i32,
    name: String,
    alpha: f64,
    plane_count: usize,
    /// `E_LP / intended` at the qualifying fit-grid nodes (`q > 0`, `intended >= 1 MW`).
    ratios: Vec<RatioSample>,
    /// `result.deviation`, as returned by `fit_plant`, on the fit grid.
    reported: DeviationStats,
    /// Deviation of `min_k P_k` (the planes as returned) on the fit grid.
    planes_fit: DeviationStats,
    /// Deviation of `E_LP` on the fit grid.
    lp_fit: DeviationStats,
    /// Deviation of `E_LP` on the `EVALUATION_NODES_PER_AXIS` squared evaluation grid.
    lp_evaluation: DeviationStats,
}

/// Recovers the unscaled fitted plane `U_k` from a plane `P_k` returned by `fit_plant`.
///
/// Valid only while `fit_plant` scales all four coefficients by `alpha` before the gamma_S secant
/// fit replaces gamma_S: `P_k = (alpha * g0, alpha * gv, alpha * gq, gamma_S)`, hence
/// `U_k = (g0 / alpha, gv / alpha, gq / alpha, gamma_S)`.
fn unscaled_pre_fix(plane: &RawPlane, alpha: f64) -> RawPlane {
    RawPlane {
        gamma_0: plane.gamma_0 / alpha,
        gamma_v: plane.gamma_v / alpha,
        gamma_q: plane.gamma_q / alpha,
        gamma_s: plane.gamma_s,
    }
}

/// Pointwise minimum of `planes` at `(v, q)` with `s = 0`: the cap the LP derives from a plane set.
fn min_plane_mw(planes: &[RawPlane], v: f64, q: f64) -> f64 {
    planes
        .iter()
        .map(|plane| plane.evaluate(v, q, 0.0))
        .fold(f64::INFINITY, f64::min)
}

/// Mean absolute, maximum absolute and RMS of `fitted(v, q) - exact(v, q)` over `nodes`, with the
/// formulas of `labdessem_fpha::compute_fit_deviation` so every deviation printed here is
/// comparable with the reported one.
fn deviation_over(
    nodes: &[(f64, f64)],
    ctx: &FitContext,
    fitted: impl Fn(f64, f64) -> f64,
) -> DeviationStats {
    let mut sum_abs = 0.0_f64;
    let mut sum_sq = 0.0_f64;
    let mut max_abs = 0.0_f64;
    let mut count = 0.0_f64;
    for &(v, q) in nodes {
        let signed = fitted(v, q) - ctx.pf.evaluate_capped(v, q, 0.0);
        let abs = signed.abs();
        sum_abs += abs;
        sum_sq += signed * signed;
        max_abs = max_abs.max(abs);
        count += 1.0;
    }
    let n = count.max(1.0);
    (sum_abs / n, max_abs, (sum_sq / n).sqrt())
}

/// Deviation of the minimum of `planes` against the exact production function on `nodes`.
fn plane_deviation(planes: &[RawPlane], ctx: &FitContext, nodes: &[(f64, f64)]) -> DeviationStats {
    deviation_over(nodes, ctx, |v, q| min_plane_mw(planes, v, q))
}

/// Deviation of the LP envelope `E_LP` against the exact production function on `nodes`, `s = 0`.
fn lp_deviation(rows: &[EnvelopeRow], ctx: &FitContext, nodes: &[(f64, f64)]) -> DeviationStats {
    deviation_over(nodes, ctx, |v, q| envelope_mw(rows, v, q, 0.0))
}

/// `E_LP / intended` at the qualifying nodes, with `intended = alpha * min_k U_k(v, q, 0)`.
fn envelope_ratios(
    result: &FphaPlantResult,
    rows: &[EnvelopeRow],
    nodes: &[(f64, f64)],
) -> Vec<RatioSample> {
    let unscaled: Vec<RawPlane> = result
        .planes
        .iter()
        .map(|plane| unscaled_pre_fix(plane, result.alpha))
        .collect();
    nodes
        .iter()
        .filter_map(|&(v, q)| {
            let intended = result.alpha * min_plane_mw(&unscaled, v, q);
            (q > 0.0 && intended >= MIN_INTENDED_MW).then(|| RatioSample {
                v,
                q,
                ratio: envelope_mw(rows, v, q, 0.0) / intended,
            })
        })
        .collect()
}

/// Runs the computed pipeline on a derived case and measures every fitted plant.
fn measure_plants() -> Vec<PlantMeasurement> {
    let case = DerivedCase::new("computed").expect("derived case from examples/24Barras");
    let (system, result) = computed_system(case.path()).expect("computed system");
    let contexts = fit_contexts(case.path(), &system).expect("fit contexts");
    let model = Model::from_system(&system, SolveMode::LinearProgramming);
    let flow_conversion = system.horizon.period_duration_hours * 0.0036;

    result
        .plant_results
        .into_iter()
        .map(|plant_result| {
            let code = plant_result.codigo;
            assert!(
                contexts.contains_key(&code),
                "no fit context for plant {code} ({})",
                plant_result.nome
            );
            let ctx = &contexts[&code];
            let rows = lp_fpha_rows(&model, &plant_result.nome, flow_conversion);
            assert!(
                !rows.is_empty(),
                "no period-1 LP rows for plant {code} ({})",
                plant_result.nome
            );
            let fit_nodes = fit_grid_nodes(ctx);
            let evaluation_nodes = evaluation_grid_nodes(ctx, EVALUATION_NODES_PER_AXIS);
            let deviation = plant_result.deviation;
            let ratios = envelope_ratios(&plant_result, &rows, &fit_nodes);
            let planes_fit = plane_deviation(&plant_result.planes, ctx, &fit_nodes);
            let lp_fit = lp_deviation(&rows, ctx, &fit_nodes);
            let lp_evaluation = lp_deviation(&rows, ctx, &evaluation_nodes);
            PlantMeasurement {
                code,
                alpha: plant_result.alpha,
                plane_count: plant_result.planes.len(),
                name: plant_result.nome,
                ratios,
                reported: (
                    deviation.mean_abs_mw,
                    deviation.max_abs_mw,
                    deviation.rms_mw,
                ),
                planes_fit,
                lp_fit,
                lp_evaluation,
            }
        })
        .collect()
}

/// `max_abs/mean_abs/rms` in MW with six decimals.
fn format_deviation((mean_abs, max_abs, rms): DeviationStats) -> String {
    format!("{max_abs:.6}/{mean_abs:.6}/{rms:.6}")
}

/// Prints one self-describing line per plant. Deviations read `max_abs/mean_abs/rms` in MW.
fn print_table(test_tag: &str, plants: &[PlantMeasurement]) {
    for plant in plants {
        let (ratio_min, ratio_max) = plant
            .ratios
            .iter()
            .fold((f64::INFINITY, f64::NEG_INFINITY), |(low, high), sample| {
                (low.min(sample.ratio), high.max(sample.ratio))
            });
        eprintln!(
            "baseline_a1[{test_tag}] code={} name={} alpha={:.6} planes={} \
             ratio_min={ratio_min:.9} ratio_max={ratio_max:.9} \
             reported_fit={} lp_fit={} lp_{n}x{n}={}",
            plant.code,
            plant.name,
            plant.alpha,
            plant.plane_count,
            format_deviation(plant.reported),
            format_deviation(plant.lp_fit),
            format_deviation(plant.lp_evaluation),
            n = EVALUATION_NODES_PER_AXIS,
        );
    }
}

/// Characterizes audit FP-02/FP-08 on the pre-fix code. Replaced by ticket-011.
///
/// If alpha were applied once (planes carrying alpha, `E_LP = min_k P_k`), `E_LP / intended` would
/// be 1 and the tolerance check would fail for every plant with `|alpha - 1| > 1e-9`.
/// `U_k` is rebuilt as `P_k / alpha`, so the ratio against `min_k P_k` is alpha by construction:
/// the check shows that the LP applies alpha on top of the returned planes; that they already carry
/// alpha is the pre-fix contract encoded in `unscaled_pre_fix`, corroborated by the next test.
#[test]
fn baseline_a1_lp_envelope_applies_alpha_twice() {
    // Arrange
    let plants = measure_plants();

    // Act
    print_table("applies_alpha_twice", &plants);

    // Assert
    for plant in &plants {
        for sample in &plant.ratios {
            assert!(
                (sample.ratio - plant.alpha).abs() <= RATIO_TOLERANCE,
                "plant {} ({}) at v={}, q={}: E_LP / intended = {}, expected alpha = {}",
                plant.code,
                plant.name,
                sample.v,
                sample.q,
                sample.ratio,
                plant.alpha
            );
        }
    }
    assert!(
        plants
            .iter()
            .any(|plant| (plant.alpha - 1.0).abs() > MATERIAL_ALPHA_OFFSET),
        "no plant has |alpha - 1| > {MATERIAL_ALPHA_OFFSET}: the double-alpha check would pass vacuously"
    );
}

/// Characterizes audit FP-02/FP-08 on the pre-fix code. Replaced by ticket-011.
///
/// If alpha were applied once (`E_LP = min_k P_k`), the LP-based and the reported maximum
/// deviation would coincide and the final assertion would fail for every plant with
/// `|alpha - 1| > 1e-3`. The reported deviation equals that of the returned planes, so the gap to
/// the LP-based one is the extra alpha the diagnostic never sees (it corroborates the first test).
#[test]
fn baseline_a1_reported_deviation_differs_from_lp_envelope() {
    // Arrange
    let plants = measure_plants();

    // Act
    print_table("reported_deviation", &plants);

    // Assert
    for plant in &plants {
        let (reported_mean, reported_max, reported_rms) = plant.reported;
        let (planes_mean, planes_max, planes_rms) = plant.planes_fit;
        for (statistic, from_planes, reported) in [
            ("mean_abs", planes_mean, reported_mean),
            ("max_abs", planes_max, reported_max),
            ("rms", planes_rms, reported_rms),
        ] {
            assert!(
                (from_planes - reported).abs() <= DEVIATION_TOLERANCE_MW,
                "plant {} ({}): reported {statistic} {reported} MW differs from the deviation of \
                 min_k P_k {from_planes} MW",
                plant.code,
                plant.name
            );
        }
        if (plant.alpha - 1.0).abs() > MATERIAL_ALPHA_OFFSET {
            let (_, lp_max, _) = plant.lp_fit;
            assert!(
                (lp_max - reported_max).abs() > MISSED_DEVIATION_MW,
                "plant {} ({}): LP-based max_abs {lp_max} MW is within {MISSED_DEVIATION_MW} MW of \
                 the reported {reported_max} MW",
                plant.code,
                plant.name
            );
        }
    }
}
