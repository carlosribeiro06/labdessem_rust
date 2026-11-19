//! Pins the computed-FPHA LP envelope after audit finding FP-02/FP-08 (alpha applied twice) was
//! fixed: `fit_plant` returns unscaled planes and the model applies alpha exactly once.
//!
//! Each test runs the computed-FPHA sequence on a derived copy of `examples/24Barras`, reads the
//! generation cap implied by the `hydro_fpha[...]` rows (`probe`), and compares it with the fitted
//! planes and with the fit-quality deviation the pipeline reports. Both tests print the measurement
//! table the post-fix evidence report quotes.

use labdessem_fpha::RawPlane;
use labdessem_model::{Model, SolveMode};

use super::probe::{
    DerivedCase, EnvelopeRow, FitContext, computed_system, envelope_mw, evaluation_grid_nodes,
    fit_contexts, fit_grid_nodes, lp_fpha_rows,
};

/// Nodes per axis of the uniform `(V, Q)` evaluation grid used for the LP-based deviation.
const EVALUATION_NODES_PER_AXIS: usize = 11;

/// Largest accepted difference between two deviations that must coincide [MW].
const DEVIATION_TOLERANCE_MW: f64 = 1e-9;

/// Largest accepted `|E_LP - alpha * min_k P_k|`, relative to `max(1, |alpha * min_k P_k|)`.
const ENVELOPE_RELATIVE_TOLERANCE: f64 = 1e-9;

/// A plant counts as materially scaled when `|alpha - 1|` exceeds this value.
const MATERIAL_ALPHA_OFFSET: f64 = 1e-3;

/// `(mean_abs, max_abs, rms)` of `fitted - exact` over a node set [MW].
type DeviationStats = (f64, f64, f64);

/// Everything the two tests assert on or print for one plant.
#[derive(Debug)]
struct PlantMeasurement {
    code: i32,
    name: String,
    alpha: f64,
    /// The planes as returned by `fit_plant`: unscaled, alpha is applied by the LP.
    planes: Vec<RawPlane>,
    /// Period-1 `hydro_fpha` rows of the LP model.
    rows: Vec<EnvelopeRow>,
    /// The `(V hm3, Q m3/s)` fit-grid nodes.
    fit_nodes: Vec<(f64, f64)>,
    /// Total turbining capacity `Q_max` [m3/s], also the upper end of the spillage axis.
    q_max: f64,
    /// `result.deviation`, as returned by `fit_plant`, on the fit grid.
    reported: DeviationStats,
    /// Deviation of `E_LP` on the fit grid.
    lp_fit: DeviationStats,
    /// Deviation of `E_LP` on the `EVALUATION_NODES_PER_AXIS` squared evaluation grid.
    lp_evaluation: DeviationStats,
}

/// Pointwise minimum of `planes` at `(v, q, s)`: the cap the LP derives from a plane set before
/// the correction factor.
fn min_plane_mw(planes: &[RawPlane], v: f64, q: f64, s: f64) -> f64 {
    planes
        .iter()
        .map(|plane| plane.evaluate(v, q, s))
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

/// Deviation of the LP envelope `E_LP` against the exact production function on `nodes`, `s = 0`.
fn lp_deviation(rows: &[EnvelopeRow], ctx: &FitContext, nodes: &[(f64, f64)]) -> DeviationStats {
    deviation_over(nodes, ctx, |v, q| envelope_mw(rows, v, q, 0.0))
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
            let lp_fit = lp_deviation(&rows, ctx, &fit_nodes);
            let lp_evaluation = lp_deviation(&rows, ctx, &evaluation_nodes);
            PlantMeasurement {
                code,
                alpha: plant_result.alpha,
                name: plant_result.nome,
                planes: plant_result.planes,
                rows,
                fit_nodes,
                q_max: ctx.pf.max_turbined_m3s(),
                reported: (
                    deviation.mean_abs_mw,
                    deviation.max_abs_mw,
                    deviation.rms_mw,
                ),
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
        eprintln!(
            "lp_envelope[{test_tag}] code={} name={} alpha={:.6} planes={} \
             reported_fit={} lp_fit={} lp_{n}x{n}={}",
            plant.code,
            plant.name,
            plant.alpha,
            plant.planes.len(),
            format_deviation(plant.reported),
            format_deviation(plant.lp_fit),
            format_deviation(plant.lp_evaluation),
            n = EVALUATION_NODES_PER_AXIS,
        );
    }
}

/// The LP envelope is `alpha * min_k P_k`, the envelope `fit_plant` measures its deviation on, so
/// the LP-based and the reported deviations coincide.
#[test]
fn lp_envelope_deviation_matches_reported_deviation() {
    // Arrange
    let plants = measure_plants();

    // Act
    print_table("deviation_matches_reported", &plants);

    // Assert
    for plant in &plants {
        let (reported_mean, reported_max, reported_rms) = plant.reported;
        let (lp_mean, lp_max, lp_rms) = plant.lp_fit;
        for (statistic, from_lp, reported) in [
            ("mean_abs", lp_mean, reported_mean),
            ("max_abs", lp_max, reported_max),
            ("rms", lp_rms, reported_rms),
        ] {
            assert!(
                (from_lp - reported).abs() <= DEVIATION_TOLERANCE_MW,
                "plant {} ({}): LP-based {statistic} {from_lp} MW differs from the reported \
                 {reported} MW",
                plant.code,
                plant.name
            );
        }
    }
    assert!(
        plants
            .iter()
            .any(|plant| (plant.alpha - 1.0).abs() > MATERIAL_ALPHA_OFFSET),
        "no plant has |alpha - 1| > {MATERIAL_ALPHA_OFFSET}: the deviation check would pass vacuously"
    );
}

/// `E_LP = alpha * min_k P_k` at every fit-grid node for `s` in `{0, Q_max / 2, Q_max}`, with `P_k`
/// the planes `fit_plant` returns. The `s > 0` points pin gamma_S, which carries alpha once.
#[test]
fn lp_envelope_matches_alpha_scaled_planes() {
    // Arrange
    let plants = measure_plants();

    // Act
    print_table("matches_alpha_scaled_planes", &plants);

    // Assert
    for plant in &plants {
        for &(v, q) in &plant.fit_nodes {
            for s in [0.0, 0.5 * plant.q_max, plant.q_max] {
                let from_lp = envelope_mw(&plant.rows, v, q, s);
                let expected = plant.alpha * min_plane_mw(&plant.planes, v, q, s);
                assert!(
                    (from_lp - expected).abs()
                        <= ENVELOPE_RELATIVE_TOLERANCE * expected.abs().max(1.0),
                    "plant {} ({}) at v={v}, q={q}, s={s}: E_LP = {from_lp}, alpha * min_k P_k = \
                     {expected}",
                    plant.code,
                    plant.name
                );
            }
        }
    }
    assert!(
        plants
            .iter()
            .any(|plant| (plant.alpha - 1.0).abs() > MATERIAL_ALPHA_OFFSET),
        "no plant has |alpha - 1| > {MATERIAL_ALPHA_OFFSET}: the alpha-once check would pass vacuously"
    );
}
