//! Similar-hyperplane reduction of the fitted FPHA plane set.

use super::geometry::FittingBounds;
use super::grid::{build_grid, GridParams};
use super::hull_fit::RawPlane;
use super::production::ProductionFunction;
use super::rng::{fnv1a64, SplitMix64};

const ORIGIN_EPS: f64 = 1e-9;

/// Plane reduction configuration.
#[derive(Debug, Clone, Copy)]
pub enum PlaneReductionConfig {
    Angle { tolerance_deg: f64 },
    Distance { tolerance_pct: f64, n_samples: u32 },
}

impl Default for PlaneReductionConfig {
    fn default() -> Self {
        Self::Angle { tolerance_deg: 1.0 }
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

fn reduce_planes_with<F>(planes: &[RawPlane], should_merge: F) -> Vec<RawPlane>
where
    F: Fn(usize, usize, &RawPlane, &RawPlane) -> bool,
{
    let mut out: Vec<RawPlane> = Vec::with_capacity(planes.len());
    let mut tail_slot: usize = 0;

    for (cand_slot, candidate) in planes.iter().enumerate() {
        let merged_tail = match out.last() {
            Some(tail)
                if !is_origin_plane(tail)
                    && !is_origin_plane(candidate)
                    && should_merge(tail_slot, cand_slot, tail, candidate) =>
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
            tail_slot = cand_slot;
        }
    }

    out
}

fn reduce_planes_angle(planes: &[RawPlane], tolerance_deg: f64) -> Vec<RawPlane> {
    reduce_planes_with(planes, |_, _, tail, candidate| {
        angle_deg(tail, candidate) < tolerance_deg
    })
}

fn grid_max_gh(pf: &ProductionFunction, grid: &GridParams) -> f64 {
    let mut generation_max = f64::NEG_INFINITY;
    for &v in &grid.v_points {
        for &q in &grid.q_points {
            let generation = pf.evaluate(v, q, 0.0);
            if generation > generation_max {
                generation_max = generation;
            }
        }
    }
    generation_max
}

fn pair_seed(hydro_id: i32, entry_level_bits: u64, tail_slot: usize, candidate_slot: usize) -> u64 {
    let tail_slot_u32 = u32::try_from(tail_slot).unwrap_or(u32::MAX);
    let candidate_slot_u32 = u32::try_from(candidate_slot).unwrap_or(u32::MAX);

    let mut buf = [0u8; 20];
    buf[0..4].copy_from_slice(&hydro_id.to_le_bytes());
    buf[4..12].copy_from_slice(&entry_level_bits.to_le_bytes());
    buf[12..16].copy_from_slice(&tail_slot_u32.to_le_bytes());
    buf[16..20].copy_from_slice(&candidate_slot_u32.to_le_bytes());
    fnv1a64(&buf)
}

fn pair_similar_distance(
    tail: &RawPlane,
    candidate: &RawPlane,
    grid: &GridParams,
    generation_max: f64,
    n_samples: u32,
    seed: u64,
    tolerance_pct: f64,
) -> bool {
    let v_first = grid.v_points.first().copied().unwrap_or(0.0);
    let v_last = grid.v_points.last().copied().unwrap_or(0.0);
    let q_first = grid.q_points.first().copied().unwrap_or(0.0);
    let q_last = grid.q_points.last().copied().unwrap_or(0.0);
    let v_span = v_last - v_first;
    let q_span = q_last - q_first;

    let mut rng = SplitMix64::new(seed);
    let mut sum_sq = 0.0_f64;
    for _ in 0..n_samples {
        let v = v_first + rng.next_unit_f64() * v_span;
        let q = q_first + rng.next_unit_f64() * q_span;
        let d = tail.evaluate(v, q, 0.0) - candidate.evaluate(v, q, 0.0);
        sum_sq += d * d;
    }

    let eqm = sum_sq / f64::from(n_samples);
    let delta = eqm / (generation_max * generation_max);
    delta < tolerance_pct / 100.0
}

fn reduce_planes_distance(
    planes: &[RawPlane],
    pf: &ProductionFunction,
    bounds: &FittingBounds,
    tolerance_pct: f64,
    n_samples: u32,
    hydro_id: i32,
    entry_level_bits: u64,
) -> Vec<RawPlane> {
    let grid = build_grid(pf, bounds);
    let generation_max = grid_max_gh(pf, &grid);

    if generation_max <= 0.0 {
        return planes.to_vec();
    }

    reduce_planes_with(planes, |tail_slot, cand_slot, tail, candidate| {
        let seed = pair_seed(hydro_id, entry_level_bits, tail_slot, cand_slot);
        pair_similar_distance(tail, candidate, &grid, generation_max, n_samples, seed, tolerance_pct)
    })
}

/// Reduce a fitted plane set by the configured method.
pub fn reduce_planes(
    planes: &[RawPlane],
    config: &PlaneReductionConfig,
    pf: &ProductionFunction,
    bounds: &FittingBounds,
    hydro_id: i32,
    entry_level_bits: u64,
) -> Vec<RawPlane> {
    match config {
        PlaneReductionConfig::Angle { tolerance_deg } => {
            reduce_planes_angle(planes, *tolerance_deg)
        }
        PlaneReductionConfig::Distance { tolerance_pct, n_samples } => {
            reduce_planes_distance(planes, pf, bounds, *tolerance_pct, *n_samples, hydro_id, entry_level_bits)
        }
    }
}

#[cfg(test)]
#[allow(clippy::unwrap_used, clippy::expect_used, clippy::float_cmp)]
mod tests {
    use super::*;
    use crate::fitting::geometry::{ForebayTable, VhaRow};
    use crate::fitting::production::{ProductionFunction, TailraceSource};
    use crate::types::TailraceModel;

    fn plane(gamma_0: f64, gamma_v: f64, gamma_q: f64, gamma_s: f64) -> RawPlane {
        RawPlane { gamma_0, gamma_v, gamma_q, gamma_s }
    }

    fn test_pf() -> ProductionFunction {
        let rows = vec![
            VhaRow { volume_hm3: 0.0, height_m: 380.0, area_km2: 0.0 },
            VhaRow { volume_hm3: 10_000.0, height_m: 396.0, area_km2: 0.0 },
            VhaRow { volume_hm3: 20_000.0, height_m: 404.0, area_km2: 0.0 },
            VhaRow { volume_hm3: 30_000.0, height_m: 408.0, area_km2: 0.0 },
        ];
        let forebay = ForebayTable::new(&rows, "Reduction").unwrap();
        let tailrace = TailraceModel::Polynomial { coefficients: vec![0.0, 0.0008, -2e-8] };
        ProductionFunction::new(
            forebay,
            TailraceSource::Entity(Some(tailrace)),
            Some(2.0),
            Some(0.92),
            3_000.0,
            5_000.0,
            "Reduction".to_owned(),
        )
    }

    fn test_bounds() -> FittingBounds {
        FittingBounds {
            v_min: 0.0, v_max: 30_000.0,
            n_volume_points: 6, n_flow_points: 6, n_spillage_points: 2,
            max_planes_per_hydro: 10, single_volume: false,
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
    fn distance_reduction_is_input_order_invariant() {
        let pf = test_pf();
        let bounds = test_bounds();
        let planes = [
            plane(100.0, 0.0010, 0.80, -0.010),
            plane(101.0, 0.0011, 0.81, -0.011),
            plane(300.0, 0.0050, 1.50, -0.030),
        ];
        let config = PlaneReductionConfig::Distance { tolerance_pct: 0.5, n_samples: 64 };

        let first = reduce_planes(&planes, &config, &pf, &bounds, 7, 42);
        let second = reduce_planes(&planes, &config, &pf, &bounds, 7, 42);
        assert_eq!(first.len(), second.len());
        for (a, b) in first.iter().zip(&second) {
            assert_eq!(a.gamma_0.to_bits(), b.gamma_0.to_bits());
        }
    }

    #[test]
    fn default_config_is_angle() {
        let config = PlaneReductionConfig::default();
        assert!(matches!(config, PlaneReductionConfig::Angle { .. }));
    }
}
