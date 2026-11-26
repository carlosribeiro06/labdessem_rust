//! Similar-hyperplane reduction of the fitted FPHA plane set.

use super::hull_fit::RawPlane;

const ORIGIN_EPS: f64 = 1e-9;

/// Plane reduction configuration: consecutive planes whose normals differ by less than
/// `tolerance_deg` degrees are merged.
#[derive(Debug, Clone, Copy)]
pub struct PlaneReductionConfig {
    pub tolerance_deg: f64,
}

impl Default for PlaneReductionConfig {
    fn default() -> Self {
        Self { tolerance_deg: 1.0 }
    }
}

fn plane_normal(plane: &RawPlane) -> [f64; 3] {
    [plane.gamma_v, plane.gamma_q, -1.0]
}

fn angle_deg(a: &RawPlane, b: &RawPlane) -> f64 {
    let n1 = plane_normal(a);
    let n2 = plane_normal(b);
    let dot = n1[0] * n2[0] + n1[1] * n2[1] + n1[2] * n2[2];
    let norm1 = (n1[0] * n1[0] + n1[1] * n1[1] + n1[2] * n1[2]).sqrt();
    let norm2 = (n2[0] * n2[0] + n2[1] * n2[1] + n2[2] * n2[2]).sqrt();
    let cos = (dot / (norm1 * norm2)).clamp(-1.0, 1.0);
    cos.acos().to_degrees()
}

fn mean_plane(a: &RawPlane, b: &RawPlane) -> RawPlane {
    RawPlane {
        gamma_0: (a.gamma_0 + b.gamma_0) / 2.0,
        gamma_v: (a.gamma_v + b.gamma_v) / 2.0,
        gamma_q: (a.gamma_q + b.gamma_q) / 2.0,
        gamma_s: (a.gamma_s + b.gamma_s) / 2.0,
    }
}

/// Whether `plane` is the plane through the origin (γ₀ = 0 ∧ γᵥ = 0).
pub fn is_origin_plane(plane: &RawPlane) -> bool {
    plane.gamma_0.abs() <= ORIGIN_EPS && plane.gamma_v.abs() <= ORIGIN_EPS
}

fn reduce_planes_angle(planes: &[RawPlane], tolerance_deg: f64) -> Vec<RawPlane> {
    let mut out: Vec<RawPlane> = Vec::with_capacity(planes.len());

    for candidate in planes {
        let merged_tail = match out.last() {
            Some(tail)
                if !is_origin_plane(tail)
                    && !is_origin_plane(candidate)
                    && angle_deg(tail, candidate) < tolerance_deg =>
            {
                Some(mean_plane(tail, candidate))
            }
            _ => None,
        };

        if let Some(merged) = merged_tail {
            if let Some(tail) = out.last_mut() {
                *tail = merged;
            }
        } else {
            out.push(*candidate);
        }
    }

    out
}

/// Reduce a fitted plane set by the angle criterion.
pub fn reduce_planes(planes: &[RawPlane], config: &PlaneReductionConfig) -> Vec<RawPlane> {
    reduce_planes_angle(planes, config.tolerance_deg)
}

#[cfg(test)]
#[allow(clippy::unwrap_used, clippy::expect_used, clippy::float_cmp)]
mod tests {
    use super::*;

    fn plane(gamma_0: f64, gamma_v: f64, gamma_q: f64, gamma_s: f64) -> RawPlane {
        RawPlane {
            gamma_0,
            gamma_v,
            gamma_q,
            gamma_s,
        }
    }

    #[test]
    fn identical_normal_pair_merges_to_mean() {
        let a = plane(100.0, 0.002, 0.85, -0.01);
        let b = plane(200.0, 0.002, 0.85, -0.03);
        let reduced = reduce_planes_angle(&[a, b], 1.0);
        assert_eq!(reduced.len(), 1);
        let expected = mean_plane(&a, &b);
        assert_eq!(reduced[0].gamma_0.to_bits(), expected.gamma_0.to_bits());
    }

    #[test]
    fn below_tolerance_merges_at_or_above_does_not() {
        let a = plane(100.0, 0.0, 1.0, 0.0);
        let b = plane(100.0, 0.01, 1.0, 0.0);
        let theta = angle_deg(&a, &b);
        assert!(theta > 0.0);

        let below = reduce_planes_angle(&[a, b], theta + 1e-6);
        assert_eq!(below.len(), 1);

        let at = reduce_planes_angle(&[a, b], theta);
        assert_eq!(at.len(), 2);
    }

    #[test]
    fn greedy_cascade_three_planes_collapse_to_one() {
        let a = plane(100.0, 0.000, 1.0, -0.01);
        let b = plane(120.0, 0.001, 1.0, -0.02);
        let c = plane(140.0, 0.002, 1.0, -0.03);
        let reduced = reduce_planes_angle(&[a, b, c], 5.0);
        assert_eq!(reduced.len(), 1);
    }

    #[test]
    fn origin_plane_never_merged() {
        let origin = plane(0.0, 0.0, 1.0, 0.0);
        assert!(is_origin_plane(&origin));
        let neighbour = plane(0.0, 0.0001, 1.0, 0.0);

        let reduced = reduce_planes_angle(&[origin, neighbour], 90.0);
        assert_eq!(reduced.len(), 2);
        assert_eq!(reduced[0].gamma_0.to_bits(), origin.gamma_0.to_bits());
    }

    #[test]
    fn zero_tolerance_merges_nothing() {
        let a = plane(100.0, 0.002, 0.85, -0.01);
        let b = plane(200.0, 0.002, 0.85, -0.03);
        let reduced = reduce_planes_angle(&[a, b], 0.0);
        assert_eq!(reduced.len(), 2);
    }

    #[test]
    fn default_config_is_angle() {
        let config = PlaneReductionConfig::default();
        assert_eq!(config.tolerance_deg.to_bits(), 1.0_f64.to_bits());
    }
}
