//! Per-plane lateral-flow secant γₛ on the spillage axis.

use super::geometry::FittingBounds;
use super::grid::build_grid;
use super::hull_fit::RawPlane;
use super::production::ProductionFunction;

const N_LATERAL_FLOW_SAMPLES: usize = 9;
const GAMMA_S_SNAP_EPS: f64 = 1e-10;

/// Resolve `S_max` for secant fit: `2 × long_term_mean_inflow` if > 0, else `2 × max_turbined`.
pub fn resolve_s_max(long_term_mean_inflow_m3s: f64, max_turbined_m3s: f64) -> f64 {
    if long_term_mean_inflow_m3s > 0.0 {
        2.0 * long_term_mean_inflow_m3s
    } else {
        2.0 * max_turbined_m3s
    }
}

/// Find the representative operating point `(V, Q)` where a plane is the binding constraint.
///
/// # Contract — anchor at argmin, not argmax
///
/// The plane anchors where it is the active binding bound — the pointwise **min**
/// over planes (the LP applies `g ≤ plane_k`).
fn representative_operating_point(
    plane: &RawPlane,
    planes: &[RawPlane],
    pf: &ProductionFunction,
    bounds: &FittingBounds,
) -> Option<(f64, f64)> {
    let grid = build_grid(pf, bounds);

    let mut best: Option<(f64, f64, f64)> = None;
    for &v in &grid.v_points {
        for &q in &grid.q_points {
            let this_val = plane.evaluate(v, q, 0.0);
            let min_val = planes
                .iter()
                .map(|p| p.evaluate(v, q, 0.0))
                .fold(f64::INFINITY, f64::min);

            if this_val - min_val <= 1e-8 {
                let generation = pf.evaluate(v, q, 0.0);
                if best.is_none() || generation > best.unwrap().2 {
                    best = Some((v, q, generation));
                }
            }
        }
    }

    best.map(|(v, q, _)| (v, q))
}

/// Fit the OLS slope of generation vs lateral_flow at a single point.
///
/// # Contract — γₛ ≤ 0
///
/// Spillage raises tailrace and lowers generation. Near-zero slopes snap to 0.0.
fn fit_gamma_s(pf: &ProductionFunction, v_rep: f64, q_rep: f64, s_max: f64) -> f64 {
    if s_max <= 0.0 {
        return 0.0;
    }

    #[allow(clippy::cast_possible_truncation)]
    let denom = f64::from((N_LATERAL_FLOW_SAMPLES - 1) as u32);

    let mut sum_x = 0.0_f64;
    let mut sum_y = 0.0_f64;
    for i in 0..N_LATERAL_FLOW_SAMPLES {
        #[allow(clippy::cast_possible_truncation)]
        let lateral_flow = f64::from(i as u32) * s_max / denom;
        let generation = pf.evaluate(v_rep, q_rep, lateral_flow);
        sum_x += lateral_flow;
        sum_y += generation;
    }

    #[allow(clippy::cast_precision_loss)]
    let n = N_LATERAL_FLOW_SAMPLES as f64;
    let mean_x = sum_x / n;
    let mean_y = sum_y / n;

    let mut cov = 0.0_f64;
    let mut var_x = 0.0_f64;
    for i in 0..N_LATERAL_FLOW_SAMPLES {
        #[allow(clippy::cast_possible_truncation)]
        let lateral_flow = f64::from(i as u32) * s_max / denom;
        let generation = pf.evaluate(v_rep, q_rep, lateral_flow);
        let dx = lateral_flow - mean_x;
        cov += dx * (generation - mean_y);
        var_x += dx * dx;
    }

    if var_x == 0.0 {
        return 0.0;
    }

    let slope = cov / var_x;
    if slope > -GAMMA_S_SNAP_EPS {
        0.0
    } else {
        slope
    }
}

/// Fit `γₛ` in place for every plane.
pub fn fit_gamma_s_for_planes(
    planes: &mut [RawPlane],
    pf: &ProductionFunction,
    bounds: &FittingBounds,
    long_term_mean_inflow_m3s: f64,
) {
    let s_max = resolve_s_max(long_term_mean_inflow_m3s, pf.max_turbined_m3s());

    let snapshot: Vec<RawPlane> = planes.to_vec();
    for plane in planes.iter_mut() {
        plane.gamma_s = match representative_operating_point(plane, &snapshot, pf, bounds) {
            Some((v_rep, q_rep)) => fit_gamma_s(pf, v_rep, q_rep, s_max),
            None => 0.0,
        };
    }
}

#[cfg(test)]
#[allow(clippy::unwrap_used, clippy::expect_used, clippy::float_cmp, clippy::cast_precision_loss)]
mod tests {
    use super::*;
    use crate::fitting::geometry::{ForebayTable, VhaRow};
    use crate::fitting::production::{ProductionFunction, TailraceSource};
    use crate::types::TailraceModel;

    fn spill_sensitive_pf() -> ProductionFunction {
        let rows = vec![
            VhaRow { volume_hm3: 0.0, height_m: 380.0, area_km2: 0.0 },
            VhaRow { volume_hm3: 10_000.0, height_m: 396.0, area_km2: 0.0 },
            VhaRow { volume_hm3: 20_000.0, height_m: 404.0, area_km2: 0.0 },
            VhaRow { volume_hm3: 30_000.0, height_m: 408.0, area_km2: 0.0 },
        ];
        let forebay = ForebayTable::new(&rows, "SpillSensitive").unwrap();
        let tailrace = TailraceModel::Polynomial {
            coefficients: vec![5.0, 0.001],
        };
        ProductionFunction::new(
            forebay,
            TailraceSource::Entity(Some(tailrace)),
            Some(0.0),
            Some(0.9),
            3_000.0,
            5_000.0,
            "SpillSensitive".to_owned(),
        )
    }

    fn flat_tailrace_pf() -> ProductionFunction {
        let rows = vec![
            VhaRow { volume_hm3: 0.0, height_m: 400.0, area_km2: 0.0 },
            VhaRow { volume_hm3: 30_000.0, height_m: 420.0, area_km2: 0.0 },
        ];
        let forebay = ForebayTable::new(&rows, "Flat").unwrap();
        let tailrace = TailraceModel::Polynomial { coefficients: vec![10.0] };
        ProductionFunction::new(
            forebay,
            TailraceSource::Entity(Some(tailrace)),
            None,
            Some(1.0),
            3_000.0,
            5_000.0,
            "Flat".to_owned(),
        )
    }

    fn small_bounds() -> FittingBounds {
        FittingBounds {
            v_min: 0.0,
            v_max: 30_000.0,
            n_volume_points: 3,
            n_flow_points: 3,
            n_spillage_points: 2,
            max_planes_per_hydro: 10,
            single_volume: false,
        }
    }

    #[test]
    fn s_max_resolution_picks_mlt_then_falls_back() {
        assert_eq!(resolve_s_max(1_500.0, 3_000.0), 3_000.0);
        assert_eq!(resolve_s_max(0.0, 3_000.0), 6_000.0);
        assert_eq!(resolve_s_max(-1.0, 2_000.0), 4_000.0);
    }

    #[test]
    fn gamma_s_is_strictly_negative_for_rising_tailrace() {
        let pf = spill_sensitive_pf();
        let bounds = small_bounds();

        let mut planes = vec![RawPlane {
            gamma_0: 0.0,
            gamma_v: 0.001,
            gamma_q: 0.1,
            gamma_s: 0.0,
        }];

        fit_gamma_s_for_planes(&mut planes, &pf, &bounds, 0.0);
        assert!(planes[0].gamma_s < 0.0);
    }

    #[test]
    fn flat_tailrace_yields_zero_slope() {
        let pf = flat_tailrace_pf();
        let bounds = small_bounds();
        let mut planes = vec![RawPlane {
            gamma_0: 0.0,
            gamma_v: 0.0005,
            gamma_q: 0.2,
            gamma_s: 0.0,
        }];
        fit_gamma_s_for_planes(&mut planes, &pf, &bounds, 0.0);
        assert_eq!(planes[0].gamma_s, 0.0);
    }

    #[test]
    fn all_planes_satisfy_sign_rule() {
        let pf = spill_sensitive_pf();
        let bounds = small_bounds();
        let mut planes = vec![
            RawPlane { gamma_0: 0.0, gamma_v: 0.001, gamma_q: 0.12, gamma_s: 0.0 },
            RawPlane { gamma_0: 50.0, gamma_v: 0.0008, gamma_q: 0.08, gamma_s: 0.0 },
        ];
        fit_gamma_s_for_planes(&mut planes, &pf, &bounds, 0.0);
        for plane in &planes {
            assert!(plane.gamma_s <= 1e-10);
        }
    }

    #[test]
    fn gamma_s_matches_hand_computed_ols() {
        let pf = spill_sensitive_pf();
        let bounds = small_bounds();

        let mut planes = vec![RawPlane {
            gamma_0: 0.0,
            gamma_v: 0.001,
            gamma_q: 0.1,
            gamma_s: 0.0,
        }];

        let (v_rep, q_rep) = representative_operating_point(&planes[0], &planes, &pf, &bounds)
            .expect("active plane");

        let s_max = resolve_s_max(0.0, pf.max_turbined_m3s());
        let denom = (N_LATERAL_FLOW_SAMPLES - 1) as f64;
        let xs: Vec<f64> = (0..N_LATERAL_FLOW_SAMPLES)
            .map(|i| i as f64 * s_max / denom)
            .collect();
        let ys: Vec<f64> = xs.iter().map(|&x| pf.evaluate(v_rep, q_rep, x)).collect();
        let n = N_LATERAL_FLOW_SAMPLES as f64;
        let mean_x = xs.iter().sum::<f64>() / n;
        let mean_y = ys.iter().sum::<f64>() / n;
        let cov: f64 = xs.iter().zip(&ys).map(|(&x, &y)| (x - mean_x) * (y - mean_y)).sum();
        let var_x: f64 = xs.iter().map(|&x| (x - mean_x).powi(2)).sum();
        let expected = cov / var_x;

        fit_gamma_s_for_planes(&mut planes, &pf, &bounds, 0.0);
        assert!((planes[0].gamma_s - expected).abs() < 1e-10);
    }
}
