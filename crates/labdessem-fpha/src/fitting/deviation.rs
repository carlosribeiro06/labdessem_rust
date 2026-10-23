//! FPHA fit-quality deviation diagnostic.

use super::geometry::FittingBounds;
use super::grid::build_grid;
use super::hull_fit::RawPlane;
use super::production::ProductionFunction;

const WARN_RELATIVE_THRESHOLD: f64 = 0.05;

/// Aggregate FPHA-vs-exact deviation over the spill = 0 fit grid.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct FphaFitDeviation {
    /// Mean of `|fitted − exact|` over the grid [MW].
    pub mean_abs_mw: f64,
    /// Maximum of `|fitted − exact|` over the grid [MW].
    pub max_abs_mw: f64,
    /// RMS deviation [MW].
    pub rms_mw: f64,
    /// Mean signed residual `fitted − exact` [MW].
    pub mean_signed_mw: f64,
    /// `mean_abs_mw / peak` (dimensionless, `≥ 0`).
    pub relative: f64,
}

impl FphaFitDeviation {
    const NEUTRAL: Self = Self {
        mean_abs_mw: 0.0,
        max_abs_mw: 0.0,
        rms_mw: 0.0,
        mean_signed_mw: 0.0,
        relative: 0.0,
    };

    /// Whether this fit's relative deviation warrants an operator warning.
    pub fn exceeds_warn_threshold(&self) -> bool {
        self.relative > WARN_RELATIVE_THRESHOLD
    }
}

/// Measure the deviation of the fitted envelope from the exact production function.
///
/// # Contract — compare the MIN envelope
///
/// The fitted value is the pointwise **min** over `planes` (the LP binds on minimum).
pub fn compute_fit_deviation(
    planes: &[RawPlane],
    pf: &ProductionFunction,
    bounds: &FittingBounds,
) -> FphaFitDeviation {
    if planes.is_empty() {
        return FphaFitDeviation::NEUTRAL;
    }

    let grid = build_grid(pf, bounds);

    let mut sum_abs = 0.0_f64;
    let mut sum_signed = 0.0_f64;
    let mut sum_sq = 0.0_f64;
    let mut max_abs = 0.0_f64;
    let mut peak_fph = 0.0_f64;
    let mut count = 0_usize;

    for &v in &grid.v_points {
        for &q in &grid.q_points {
            let fitted = planes
                .iter()
                .map(|p| p.evaluate(v, q, 0.0))
                .fold(f64::INFINITY, f64::min);
            let exact = pf.evaluate_capped(v, q, 0.0);
            let signed = fitted - exact;
            let abs = signed.abs();
            sum_abs += abs;
            sum_signed += signed;
            sum_sq += signed * signed;
            max_abs = max_abs.max(abs);
            peak_fph = peak_fph.max(exact);
            count += 1;
        }
    }

    #[allow(clippy::cast_precision_loss)]
    let n = count.max(1) as f64;
    let mean_abs_mw = sum_abs / n;
    let rms_mw = (sum_sq / n).sqrt();
    let relative = if peak_fph > 0.0 { mean_abs_mw / peak_fph } else { 0.0 };

    FphaFitDeviation {
        mean_abs_mw,
        max_abs_mw: max_abs,
        rms_mw,
        mean_signed_mw: sum_signed / n,
        relative,
    }
}

/// Per-point deviation detail.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct FphaDeviationPoint {
    pub v: f64,
    pub q: f64,
    pub fph_exact: f64,
    pub fpha_fitted: f64,
    pub deviation: f64,
    pub relative_to_peak: f64,
}

/// Collect per-point deviation details.
pub fn collect_fit_deviation_points(
    planes: &[RawPlane],
    pf: &ProductionFunction,
    bounds: &FittingBounds,
) -> Vec<FphaDeviationPoint> {
    if planes.is_empty() {
        return Vec::new();
    }

    let grid = build_grid(pf, bounds);
    let mut points: Vec<FphaDeviationPoint> =
        Vec::with_capacity(grid.v_points.len() * grid.q_points.len());
    let mut peak_fph = 0.0_f64;

    for &v in &grid.v_points {
        for &q in &grid.q_points {
            let fpha_fitted = planes
                .iter()
                .map(|p| p.evaluate(v, q, 0.0))
                .fold(f64::INFINITY, f64::min);
            let fph_exact = pf.evaluate_capped(v, q, 0.0);
            let deviation = fpha_fitted - fph_exact;
            peak_fph = peak_fph.max(fph_exact);
            points.push(FphaDeviationPoint {
                v, q, fph_exact, fpha_fitted, deviation,
                relative_to_peak: 0.0,
            });
        }
    }

    if peak_fph > 0.0 {
        for point in &mut points {
            point.relative_to_peak = point.deviation.abs() / peak_fph;
        }
    }

    points
}

#[cfg(test)]
#[allow(clippy::unwrap_used, clippy::expect_used, clippy::float_cmp)]
mod tests {
    use super::*;
    use crate::fitting::geometry::{ForebayTable, VhaRow};
    use crate::fitting::production::{ProductionFunction, TailraceSource};
    use crate::types::TailraceModel;

    fn concave_pf() -> ProductionFunction {
        let rows = vec![
            VhaRow { volume_hm3: 0.0, height_m: 380.0, area_km2: 0.0 },
            VhaRow { volume_hm3: 10_000.0, height_m: 396.0, area_km2: 0.0 },
            VhaRow { volume_hm3: 20_000.0, height_m: 404.0, area_km2: 0.0 },
            VhaRow { volume_hm3: 30_000.0, height_m: 408.0, area_km2: 0.0 },
        ];
        let forebay = ForebayTable::new(&rows, "Concave").unwrap();
        let tailrace = TailraceModel::Polynomial { coefficients: vec![0.0, 0.0008, -2e-8] };
        ProductionFunction::new(
            forebay,
            TailraceSource::Entity(Some(tailrace)),
            Some(2.0), Some(0.92), 3_000.0, 5_000.0, "Concave".to_owned(),
        )
    }

    fn small_bounds() -> FittingBounds {
        FittingBounds {
            v_min: 0.0, v_max: 30_000.0,
            n_volume_points: 2, n_flow_points: 2, n_spillage_points: 2,
            max_planes_per_hydro: 10, single_volume: false,
        }
    }

    #[test]
    fn empty_planes_is_neutral() {
        let pf = concave_pf();
        let bounds = small_bounds();
        let dev = compute_fit_deviation(&[], &pf, &bounds);
        assert_eq!(dev, FphaFitDeviation::NEUTRAL);
        assert!(!dev.exceeds_warn_threshold());
    }

    #[test]
    fn matches_hand_computed_deviation() {
        let pf = concave_pf();
        let bounds = small_bounds();

        let planes = vec![RawPlane {
            gamma_0: 50.0, gamma_v: 0.01, gamma_q: 0.3, gamma_s: 0.0,
        }];

        let v_pts = [0.0_f64, 30_000.0];
        let q_pts = [0.0_f64, pf.max_turbined_m3s()];
        let mut sum_abs = 0.0_f64;
        let mut max_abs = 0.0_f64;
        let mut peak = 0.0_f64;
        for &v in &v_pts {
            for &q in &q_pts {
                let fitted = planes[0].evaluate(v, q, 0.0);
                let exact = pf.evaluate_capped(v, q, 0.0);
                let signed = fitted - exact;
                sum_abs += signed.abs();
                max_abs = max_abs.max(signed.abs());
                peak = peak.max(exact);
            }
        }
        let n = 4.0_f64;
        let expected_mean_abs = sum_abs / n;

        let dev = compute_fit_deviation(&planes, &pf, &bounds);
        assert!((dev.mean_abs_mw - expected_mean_abs).abs() < 1e-9);
        assert!((dev.max_abs_mw - max_abs).abs() < 1e-9);
        assert!((dev.relative - expected_mean_abs / peak).abs() < 1e-9);
    }

    #[test]
    fn zero_production_has_zero_relative() {
        let rows = vec![
            VhaRow { volume_hm3: 0.0, height_m: 100.0, area_km2: 0.0 },
            VhaRow { volume_hm3: 30_000.0, height_m: 100.0, area_km2: 0.0 },
        ];
        let forebay = ForebayTable::new(&rows, "Zero").unwrap();
        let tailrace = TailraceModel::Polynomial { coefficients: vec![200.0] };
        let pf = ProductionFunction::new(
            forebay,
            TailraceSource::Entity(Some(tailrace)),
            None, None, 3_000.0, 0.0, "Zero".to_owned(),
        );
        let bounds = small_bounds();

        let planes = vec![RawPlane {
            gamma_0: 100.0, gamma_v: 0.0, gamma_q: 0.0, gamma_s: 0.0,
        }];
        let dev = compute_fit_deviation(&planes, &pf, &bounds);
        assert!(dev.mean_abs_mw > 0.0);
        assert_eq!(dev.relative, 0.0);
        assert!(!dev.exceeds_warn_threshold());
    }

    #[test]
    fn large_overestimate_warns() {
        let pf = concave_pf();
        let bounds = small_bounds();
        let planes = vec![RawPlane {
            gamma_0: 1.0e6, gamma_v: 0.0, gamma_q: 0.0, gamma_s: 0.0,
        }];
        let dev = compute_fit_deviation(&planes, &pf, &bounds);
        assert!(dev.relative > WARN_RELATIVE_THRESHOLD);
        assert!(dev.exceeds_warn_threshold());
    }

    #[test]
    fn collect_points_match_grid_size() {
        let pf = concave_pf();
        let bounds = small_bounds();
        let planes = vec![RawPlane {
            gamma_0: 50.0, gamma_v: 0.01, gamma_q: 0.3, gamma_s: 0.0,
        }];
        let points = collect_fit_deviation_points(&planes, &pf, &bounds);
        assert_eq!(points.len(), 4);
    }

    #[test]
    fn rms_is_computed() {
        let pf = concave_pf();
        let bounds = small_bounds();
        let planes = vec![RawPlane {
            gamma_0: 50.0, gamma_v: 0.01, gamma_q: 0.3, gamma_s: 0.0,
        }];
        let dev = compute_fit_deviation(&planes, &pf, &bounds);
        assert!(dev.rms_mw >= 0.0);
    }
}
