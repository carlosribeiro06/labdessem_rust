//! Grid discretization for the (V, Q) production cloud.

use super::geometry::FittingBounds;
use super::production::ProductionFunction;

/// Grid parameters for cloud sampling.
#[derive(Debug, Clone)]
pub struct GridParams {
    /// Volume sample points (hm³).
    pub v_points: Vec<f64>,
    /// Turbined flow sample points (m³/s).
    pub q_points: Vec<f64>,
    /// Spillage sample points (m³/s) — typically [0, max_spill] for secant.
    pub s_points: Vec<f64>,
}

/// Build a uniform (V, Q, S) grid for the production function.
///
/// Volume axis spans `[bounds.v_min, bounds.v_max]` with `n_volume_points` samples.
/// Flow axis spans `[0, max_turbined]` with `n_flow_points` samples.
/// Spillage axis spans `[0, max_spillage]` with `n_spillage_points` samples.
///
/// For `single_volume=true`, the volume axis is reduced to two points around
/// the midpoint for tangent sampling (run-of-river plants).
pub fn build_grid(pf: &ProductionFunction, bounds: &FittingBounds) -> GridParams {
    let v_points = if bounds.single_volume {
        build_single_volume_axis(bounds)
    } else {
        build_uniform_axis(bounds.v_min, bounds.v_max, bounds.n_volume_points)
    };

    let q_points = build_uniform_axis(0.0, pf.max_turbined_m3s(), bounds.n_flow_points);

    // Spillage axis for γₛ secant fitting
    let max_spillage = pf.max_turbined_m3s(); // Typical bound
    let s_points = build_uniform_axis(0.0, max_spillage, bounds.n_spillage_points);

    GridParams {
        v_points,
        q_points,
        s_points,
    }
}

/// Build a uniform axis from min to max with n points.
fn build_uniform_axis(min: f64, max: f64, n: usize) -> Vec<f64> {
    if n == 0 {
        return Vec::new();
    }
    if n == 1 {
        return vec![min];
    }

    let step = (max - min) / (n - 1) as f64;
    (0..n).map(|i| min + i as f64 * step).collect()
}

/// Build a two-point tangent axis for single-volume plants.
fn build_single_volume_axis(bounds: &FittingBounds) -> Vec<f64> {
    let v0 = (bounds.v_min + bounds.v_max) / 2.0;
    let delta = 2.0; // Small perturbation for tangent
    vec![v0 - delta, v0 + delta]
}

/// Resolve fitting bounds from plant geometry.
pub fn resolve_fitting_bounds(
    forebay_v_min: f64,
    forebay_v_max: f64,
    config_bounds: &FittingBounds,
) -> FittingBounds {
    let v_min = forebay_v_min;
    let v_max = forebay_v_max;
    let single_volume = (v_max - v_min).abs() < 10.0; // Run-of-river threshold

    FittingBounds {
        v_min,
        v_max,
        single_volume,
        ..*config_bounds
    }
}

#[cfg(test)]
#[allow(clippy::unwrap_used, clippy::expect_used, clippy::float_cmp)]
mod tests {
    use super::*;
    use crate::fitting::geometry::{ForebayTable, VhaRow};
    use crate::fitting::production::{ProductionFunction, TailraceSource};

    fn test_pf(max_turbined: f64) -> ProductionFunction {
        let rows = vec![
            VhaRow { volume_hm3: 0.0, height_m: 400.0, area_km2: 0.0 },
            VhaRow { volume_hm3: 1000.0, height_m: 420.0, area_km2: 0.0 },
        ];
        let forebay = ForebayTable::new(&rows, "Grid").unwrap();
        ProductionFunction::new(
            forebay,
            TailraceSource::Entity(None),
            None,
            None,
            max_turbined,
            1000.0,
            "Grid".to_owned(),
        )
    }

    #[test]
    fn uniform_volume_axis() {
        let pf = test_pf(900.0);
        let bounds = FittingBounds {
            v_min: 0.0,
            v_max: 1000.0,
            n_volume_points: 5,
            n_flow_points: 4,
            n_spillage_points: 2,
            max_planes_per_hydro: 10,
            single_volume: false,
        };

        let grid = build_grid(&pf, &bounds);

        assert_eq!(grid.v_points, vec![0.0, 250.0, 500.0, 750.0, 1000.0]);
    }

    #[test]
    fn uniform_flow_axis() {
        let pf = test_pf(900.0);
        let bounds = FittingBounds {
            v_min: 0.0,
            v_max: 1000.0,
            n_volume_points: 5,
            n_flow_points: 4,
            n_spillage_points: 2,
            max_planes_per_hydro: 10,
            single_volume: false,
        };

        let grid = build_grid(&pf, &bounds);

        assert_eq!(grid.q_points, vec![0.0, 300.0, 600.0, 900.0]);
    }

    #[test]
    fn single_volume_tangent_axis() {
        let pf = test_pf(900.0);
        let bounds = FittingBounds {
            v_min: 498.0,
            v_max: 502.0,
            n_volume_points: 10, // Ignored for single_volume
            n_flow_points: 4,
            n_spillage_points: 2,
            max_planes_per_hydro: 10,
            single_volume: true,
        };

        let grid = build_grid(&pf, &bounds);

        assert_eq!(grid.v_points.len(), 2);
        assert!((grid.v_points[0] - 498.0).abs() < 1e-9);
        assert!((grid.v_points[1] - 502.0).abs() < 1e-9);
    }

    #[test]
    fn spillage_axis() {
        let pf = test_pf(900.0);
        let bounds = FittingBounds {
            v_min: 0.0,
            v_max: 1000.0,
            n_volume_points: 5,
            n_flow_points: 4,
            n_spillage_points: 3,
            max_planes_per_hydro: 10,
            single_volume: false,
        };

        let grid = build_grid(&pf, &bounds);

        assert_eq!(grid.s_points, vec![0.0, 450.0, 900.0]);
    }

    #[test]
    fn resolve_bounds_detects_run_of_river() {
        let config = FittingBounds::default();

        let resolved = resolve_fitting_bounds(500.0, 505.0, &config);
        assert!(resolved.single_volume);

        let resolved_storage = resolve_fitting_bounds(0.0, 30000.0, &config);
        assert!(!resolved_storage.single_volume);
    }

    #[test]
    fn single_point_axis() {
        let axis = build_uniform_axis(100.0, 100.0, 1);
        assert_eq!(axis, vec![100.0]);
    }

    #[test]
    fn empty_axis() {
        let axis = build_uniform_axis(0.0, 100.0, 0);
        assert!(axis.is_empty());
    }
}
