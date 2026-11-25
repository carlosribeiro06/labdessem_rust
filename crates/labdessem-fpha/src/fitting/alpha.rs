//! Least-squares α correction on 30 evaluation points at spill = 0.
//!
//! The planes the LP uses (`FPHA_0`) are biased against the production function. A single scalar
//! `α_FPHA` minimizing `Σ (α·FPHA_0 − FPH)²` over 30 evaluation points of the fit window
//! corrects them: 6 volume × 5 flow points, or 2 × 15 for single-volume plants. The count is the
//! MM default (§9.16.11 step 4); the point distribution is a labdessem choice `[to confirm]`
//! against the MM/Libs text (ADR-033).

use super::geometry::FittingBounds;
use super::grid::{GridParams, build_grid};
use super::hull_fit::RawPlane;
use super::production::ProductionFunction;

const ALPHA_VOLUME_POINTS: usize = 6;
const ALPHA_FLOW_POINTS: usize = 5;
const ALPHA_SINGLE_VOLUME_FLOW_POINTS: usize = 15;

fn alpha_evaluation_grid(pf: &ProductionFunction, bounds: &FittingBounds) -> GridParams {
    let n_flow_points = if bounds.single_volume {
        ALPHA_SINGLE_VOLUME_FLOW_POINTS
    } else {
        ALPHA_FLOW_POINTS
    };
    build_grid(
        pf,
        &FittingBounds {
            n_volume_points: ALPHA_VOLUME_POINTS,
            n_flow_points,
            ..*bounds
        },
    )
}

/// Compute the least-squares α correction factor for the final FPHA planes.
///
/// # Contract — evaluation points, not the fit grid
///
/// α is fitted on its own 30 evaluation points (see the module doc), uniform over the fit window
/// `[bounds.v_min, bounds.v_max]` and `[0, Q_max]`. `bounds.n_volume_points` and
/// `bounds.n_flow_points` are ignored; only the window, `single_volume` and `Q_max` are read.
///
/// # Contract — pass the final planes
///
/// `planes` are the planes the LP will use (after the γₛ secant and the plane reduction), so α
/// depends on `max_planes` (ADR-033).
///
/// # Contract — regress the MIN envelope
///
/// `FPHA_0` is the pointwise **min** over `planes`. The LP applies
/// `g ≤ plane_k` for every plane, so the binding cap is the minimum.
///
/// # Contract — degenerate denominator returns α = 1.0
///
/// `Σ FPHA_0² == 0` (all-zero-production hydro) leaves the ratio undefined;
/// return `α = 1.0` rather than dividing by zero.
pub fn compute_alpha_fpha(
    planes: &[RawPlane],
    pf: &ProductionFunction,
    bounds: &FittingBounds,
) -> f64 {
    if planes.is_empty() {
        return 1.0;
    }

    let grid = alpha_evaluation_grid(pf, bounds);

    let mut sum_fpha0_fph = 0.0_f64;
    let mut sum_fpha0_sq = 0.0_f64;

    for &v in &grid.v_points {
        for &q in &grid.q_points {
            let fpha0 = planes
                .iter()
                .map(|p| p.evaluate(v, q, 0.0))
                .fold(f64::INFINITY, f64::min);
            let fph = pf.evaluate_capped(v, q, 0.0);
            sum_fpha0_fph += fpha0 * fph;
            sum_fpha0_sq += fpha0 * fpha0;
        }
    }

    if sum_fpha0_sq == 0.0 {
        1.0
    } else {
        sum_fpha0_fph / sum_fpha0_sq
    }
}

/// Scale the whole affine function of a raw hull plane by α.
///
/// Multiplies `γ₀, γᵥ, γᵩ, γₛ` alike: `FPHA = α·FPHA_0`.
pub fn scale_plane_affine(plane: &RawPlane, alpha: f64) -> RawPlane {
    RawPlane {
        gamma_0: plane.gamma_0 * alpha,
        gamma_v: plane.gamma_v * alpha,
        gamma_q: plane.gamma_q * alpha,
        gamma_s: plane.gamma_s * alpha,
    }
}

/// Apply α scaling to all planes.
pub fn scale_planes(planes: &[RawPlane], alpha: f64) -> Vec<RawPlane> {
    planes
        .iter()
        .map(|p| scale_plane_affine(p, alpha))
        .collect()
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
            VhaRow {
                volume_hm3: 0.0,
                height_m: 380.0,
                area_km2: 0.0,
            },
            VhaRow {
                volume_hm3: 10_000.0,
                height_m: 396.0,
                area_km2: 0.0,
            },
            VhaRow {
                volume_hm3: 20_000.0,
                height_m: 404.0,
                area_km2: 0.0,
            },
            VhaRow {
                volume_hm3: 30_000.0,
                height_m: 408.0,
                area_km2: 0.0,
            },
        ];
        let forebay = ForebayTable::new(&rows, "Concave").unwrap();
        let tailrace = TailraceModel::Polynomial {
            coefficients: vec![0.0, 0.0008, -2e-8],
        };
        ProductionFunction::new(
            forebay,
            TailraceSource::Entity(Some(tailrace)),
            Some(2.0),
            Some(0.92),
            3_000.0,
            5_000.0,
            "Concave".to_owned(),
        )
    }

    fn small_bounds() -> FittingBounds {
        FittingBounds {
            v_min: 0.0,
            v_max: 30_000.0,
            n_volume_points: 2,
            n_flow_points: 2,
            n_spillage_points: 2,
            max_planes_per_hydro: 10,
            single_volume: false,
        }
    }

    #[test]
    fn closed_form_alpha_matches_hand_computed_ratio() {
        let pf = concave_pf();
        let bounds = small_bounds();

        let planes = vec![RawPlane {
            gamma_0: 50.0,
            gamma_v: 0.01,
            gamma_q: 0.3,
            gamma_s: 0.0,
        }];

        let v_pts: Vec<f64> = (0..6).map(|i| 30_000.0 * f64::from(i) / 5.0).collect();
        let q_pts: Vec<f64> = (0..5)
            .map(|j| pf.max_turbined_m3s() * f64::from(j) / 4.0)
            .collect();
        let mut num = 0.0_f64;
        let mut den = 0.0_f64;
        for &v in &v_pts {
            for &q in &q_pts {
                let fpha0 = planes[0].evaluate(v, q, 0.0);
                let fph = pf.evaluate_capped(v, q, 0.0);
                num += fpha0 * fph;
                den += fpha0 * fpha0;
            }
        }
        let expected = num / den;

        let alpha = compute_alpha_fpha(&planes, &pf, &bounds);
        assert!((alpha - expected).abs() < 1e-12);
    }

    #[test]
    fn alpha_evaluation_grid_has_thirty_points() {
        let pf = concave_pf();
        let q_max = pf.max_turbined_m3s();

        let multi = alpha_evaluation_grid(&pf, &small_bounds());
        assert_eq!(multi.v_points.len(), 6);
        assert_eq!(multi.q_points.len(), 5);
        assert_eq!(multi.v_points.first(), Some(&0.0));
        assert_eq!(multi.v_points.last(), Some(&30_000.0));
        assert_eq!(multi.q_points.first(), Some(&0.0));
        assert_eq!(multi.q_points.last(), Some(&q_max));
        assert_eq!(multi.v_points.len() * multi.q_points.len(), 30);

        let single_bounds = FittingBounds {
            single_volume: true,
            ..small_bounds()
        };
        let single = alpha_evaluation_grid(&pf, &single_bounds);
        assert_eq!(single.v_points.len(), 2);
        assert_eq!(single.q_points.len(), 15);
        assert_eq!(single.v_points.len() * single.q_points.len(), 30);
    }

    #[test]
    fn scaling_is_full_affine() {
        let raw = RawPlane {
            gamma_0: 123.0,
            gamma_v: 0.04,
            gamma_q: 0.7,
            gamma_s: -0.2,
        };
        let alpha = 0.83_f64;
        let scaled = scale_plane_affine(&raw, alpha);

        assert!((scaled.gamma_0 - alpha * raw.gamma_0).abs() < 1e-12);
        assert!((scaled.gamma_v - alpha * raw.gamma_v).abs() < 1e-12);
        assert!((scaled.gamma_q - alpha * raw.gamma_q).abs() < 1e-12);
        assert!((scaled.gamma_s - alpha * raw.gamma_s).abs() < 1e-12);

        for &(v, q, s) in &[(0.0, 1.0, 0.0), (10_000.0, 500.0, 30.0)] {
            let scaled_val = scaled.evaluate(v, q, s);
            let alpha_raw_val = alpha * raw.evaluate(v, q, s);
            assert!((scaled_val - alpha_raw_val).abs() < 1e-12);
        }
    }

    #[test]
    fn degenerate_denominator_returns_neutral_alpha() {
        let rows = vec![
            VhaRow {
                volume_hm3: 0.0,
                height_m: 100.0,
                area_km2: 0.0,
            },
            VhaRow {
                volume_hm3: 30_000.0,
                height_m: 100.0,
                area_km2: 0.0,
            },
        ];
        let forebay = ForebayTable::new(&rows, "Zero").unwrap();
        let tailrace = TailraceModel::Polynomial {
            coefficients: vec![200.0],
        };
        let pf = ProductionFunction::new(
            forebay,
            TailraceSource::Entity(Some(tailrace)),
            None,
            None,
            3_000.0,
            0.0,
            "Zero".to_owned(),
        );
        let bounds = small_bounds();

        let planes = vec![RawPlane {
            gamma_0: 0.0,
            gamma_v: 0.0,
            gamma_q: 0.0,
            gamma_s: 0.0,
        }];

        let alpha = compute_alpha_fpha(&planes, &pf, &bounds);
        assert_eq!(alpha, 1.0);
    }

    #[test]
    fn empty_planes_returns_neutral_alpha() {
        let pf = concave_pf();
        let bounds = small_bounds();
        let alpha = compute_alpha_fpha(&[], &pf, &bounds);
        assert_eq!(alpha, 1.0);
    }

    #[test]
    fn regression_is_spillage_zero_only() {
        let pf = concave_pf();
        let bounds = small_bounds();

        let base = RawPlane {
            gamma_0: 50.0,
            gamma_v: 0.01,
            gamma_q: 0.3,
            gamma_s: 0.0,
        };
        let with_spill = RawPlane {
            gamma_s: -0.5,
            ..base
        };

        let alpha_base = compute_alpha_fpha(&[base], &pf, &bounds);
        let alpha_spill = compute_alpha_fpha(&[with_spill], &pf, &bounds);
        assert_eq!(alpha_base, alpha_spill);
    }

    #[test]
    fn scale_planes_applies_to_all() {
        let planes = vec![
            RawPlane {
                gamma_0: 100.0,
                gamma_v: 0.01,
                gamma_q: 0.5,
                gamma_s: -0.1,
            },
            RawPlane {
                gamma_0: 200.0,
                gamma_v: 0.02,
                gamma_q: 0.6,
                gamma_s: -0.2,
            },
        ];
        let alpha = 0.9;
        let scaled = scale_planes(&planes, alpha);

        assert_eq!(scaled.len(), 2);
        for (orig, sc) in planes.iter().zip(&scaled) {
            assert!((sc.gamma_0 - alpha * orig.gamma_0).abs() < 1e-12);
            assert!((sc.gamma_v - alpha * orig.gamma_v).abs() < 1e-12);
        }
    }
}
