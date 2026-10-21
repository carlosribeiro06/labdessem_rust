//! Convex-hull FPHA fitter: production cloud → upper-envelope hyperplanes.

use super::geometry::FittingBounds;
use super::grid::build_grid;
use super::production::ProductionFunction;
use crate::hull::{convex_hull_3d, HullError};

/// An unscaled FPHA plane: generation = γ₀ + γᵥ·V + γᵩ·Q + γₛ·S.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct RawPlane {
    /// Intercept γ₀ (NOT scaled by α).
    pub gamma_0: f64,
    /// Volume gradient γᵥ [MW / hm³]. Non-negative for upper envelope.
    pub gamma_v: f64,
    /// Turbined-flow gradient γᵩ [MW / (m³/s)]. Non-negative for upper envelope.
    pub gamma_q: f64,
    /// Spillage gradient γₛ [MW / (m³/s)]. Set to 0.0 by hull fitter; written by secant.
    pub gamma_s: f64,
}

impl RawPlane {
    /// Evaluates the plane at (v, q, s).
    pub fn evaluate(&self, v: f64, q: f64, s: f64) -> f64 {
        self.gamma_0 + self.gamma_v * v + self.gamma_q * q + self.gamma_s * s
    }
}

/// Tolerance for upper-envelope facet selection.
const EPS_NZ: f64 = 1e-9;

/// Tolerance for snapping near-zero γᵥ to exactly 0.0 (single-volume plants).
const GAMMA_V_EPS: f64 = 1e-6;

/// Build the (V, Q, generation) production cloud at spillage = 0.
pub fn build_cloud(pf: &ProductionFunction, bounds: &FittingBounds) -> Vec<[f64; 3]> {
    let grid = build_grid(pf, bounds);

    let mut cloud = Vec::with_capacity(grid.v_points.len() * grid.q_points.len());
    for &v in &grid.v_points {
        for &q in &grid.q_points {
            let generation = pf.evaluate_capped(v, q, 0.0);
            cloud.push([v, q, generation]);
        }
    }

    cloud
}

/// Convert an upper-envelope facet to a plane: generation = γ₀ + γᵥ·V + γᵩ·Q.
fn facet_to_plane(normal: [f64; 3], offset: f64) -> RawPlane {
    let nz = normal[2];
    RawPlane {
        gamma_0: -offset / nz,
        gamma_v: -normal[0] / nz,
        gamma_q: -normal[1] / nz,
        gamma_s: 0.0,
    }
}

/// Fit upper-envelope FPHA planes from the production cloud via convex hull.
pub fn fit_hull_planes(
    pf: &ProductionFunction,
    bounds: &FittingBounds,
) -> Result<Vec<RawPlane>, HullError> {
    let cloud = build_cloud(pf, bounds);
    let facets = convex_hull_3d(&cloud)?;

    let mut planes: Vec<RawPlane> = facets
        .iter()
        .filter(|f| f.normal[2] > EPS_NZ) // Upper envelope: nz > 0
        .map(|f| facet_to_plane(f.normal, f.offset))
        .collect();

    // Run-of-river γᵥ = 0 contract
    if bounds.single_volume {
        for plane in &mut planes {
            if plane.gamma_v.abs() <= GAMMA_V_EPS {
                plane.gamma_v = 0.0;
            }
        }
    }

    // Dedup Qt triangulation (exact equality)
    let mut seen: Vec<(u64, u64, u64)> = Vec::with_capacity(planes.len());
    planes.retain(|p| {
        let key = (
            p.gamma_0.to_bits(),
            p.gamma_v.to_bits(),
            p.gamma_q.to_bits(),
        );
        if seen.contains(&key) {
            false
        } else {
            seen.push(key);
            true
        }
    });

    Ok(planes)
}

/// Evaluate the pointwise max of an envelope of planes at (v, q).
pub fn envelope_max(planes: &[RawPlane], v: f64, q: f64) -> f64 {
    planes
        .iter()
        .map(|p| p.gamma_0 + p.gamma_v * v + p.gamma_q * q)
        .fold(f64::NEG_INFINITY, f64::max)
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

    fn test_bounds() -> FittingBounds {
        FittingBounds {
            v_min: 0.0,
            v_max: 30_000.0,
            n_volume_points: 6,
            n_flow_points: 6,
            n_spillage_points: 2,
            max_planes_per_hydro: 10,
            single_volume: false,
        }
    }

    #[test]
    fn upper_envelope_is_outer_approximation() {
        let pf = concave_pf();
        let bounds = test_bounds();
        let planes = fit_hull_planes(&pf, &bounds).expect("valid hull");

        assert!(!planes.is_empty());

        let cloud = build_cloud(&pf, &bounds);
        for point in &cloud {
            let [v, q, generation] = *point;
            let envelope = envelope_max(&planes, v, q);
            assert!(
                envelope >= generation - 1e-8,
                "outer approximation violated at ({v}, {q})"
            );
        }
    }

    #[test]
    fn planes_have_valid_signs() {
        let pf = concave_pf();
        let bounds = test_bounds();
        let planes = fit_hull_planes(&pf, &bounds).expect("valid hull");

        for (idx, plane) in planes.iter().enumerate() {
            assert!(plane.gamma_v >= -1e-10, "plane {idx}: γᵥ < 0");
            assert!(plane.gamma_q >= -1e-10, "plane {idx}: γᵩ < 0");
            assert_eq!(plane.gamma_s, 0.0, "plane {idx}: γₛ ≠ 0");
        }
    }

    #[test]
    fn cloud_has_correct_size() {
        let pf = concave_pf();
        let bounds = test_bounds();
        let cloud = build_cloud(&pf, &bounds);

        assert_eq!(cloud.len(), 6 * 6);
    }

    #[test]
    fn single_volume_yields_zero_gamma_v() {
        let rows = vec![
            VhaRow { volume_hm3: 0.0, height_m: 400.0, area_km2: 0.0 },
            VhaRow { volume_hm3: 30_000.0, height_m: 400.0, area_km2: 0.0 }, // Flat
        ];
        let forebay = ForebayTable::new(&rows, "RunOfRiver").unwrap();
        let tailrace = TailraceModel::Polynomial {
            coefficients: vec![0.0, 0.0008, -2e-8],
        };
        let pf = ProductionFunction::new(
            forebay,
            TailraceSource::Entity(Some(tailrace)),
            Some(2.0),
            Some(0.92),
            3_000.0,
            5_000.0,
            "RunOfRiver".to_owned(),
        );

        let bounds = FittingBounds {
            v_min: 14_998.0,
            v_max: 15_002.0,
            n_volume_points: 2,
            n_flow_points: 6,
            n_spillage_points: 2,
            max_planes_per_hydro: 10,
            single_volume: true,
        };

        let planes = fit_hull_planes(&pf, &bounds).expect("valid hull");
        for plane in &planes {
            assert_eq!(plane.gamma_v.to_bits(), 0.0_f64.to_bits());
        }
    }

    #[test]
    fn determinism_same_input() {
        let pf = concave_pf();
        let bounds = test_bounds();

        let p1 = fit_hull_planes(&pf, &bounds).expect("hull 1");
        let p2 = fit_hull_planes(&pf, &bounds).expect("hull 2");

        assert_eq!(p1.len(), p2.len());
        for (a, b) in p1.iter().zip(&p2) {
            assert_eq!(a.gamma_0.to_bits(), b.gamma_0.to_bits());
            assert_eq!(a.gamma_v.to_bits(), b.gamma_v.to_bits());
            assert_eq!(a.gamma_q.to_bits(), b.gamma_q.to_bits());
        }
    }
}
