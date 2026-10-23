//! Grid discretization for the (V, Q) production cloud.

use super::geometry::FittingBounds;
use super::production::ProductionFunction;

/// Tolerance for single-volume detection (hm³).
const V_EPS: f64 = 1e-9;

/// Absolute minimum half-width for synthesized volume samples (hm³).
const V_SYNTH_ABS: f64 = 1.0;

/// Fraction of useful storage for half-width (0.5%).
const V_SYNTH_FRAC: f64 = 0.005;

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
///
/// The bounds are already synthesized by `resolve_fitting_bounds`,
/// so this function simply returns the two endpoint volumes.
fn build_single_volume_axis(bounds: &FittingBounds) -> Vec<f64> {
    vec![bounds.v_min, bounds.v_max]
}

/// Resolve fitting bounds from plant geometry with single-volume synthesis.
///
/// For run-of-river plants (forebay span ≤ `V_EPS`), synthesizes a 2-point
/// volume range around the single volume using `useful_storage_hm3`:
/// - `half_width = max(0.5% of useful_storage, 1.0 hm³)`
/// - bounds span `[v0 - half_width, v0 + half_width]`
///
/// For single-point forebay tables, synthesized bounds may extend beyond the
/// table domain (extrapolation is valid since forebay height is constant).
pub fn resolve_fitting_bounds(
    forebay_v_min: f64,
    forebay_v_max: f64,
    useful_storage_hm3: f64,
    config_bounds: &FittingBounds,
) -> FittingBounds {
    let forebay_span = forebay_v_max - forebay_v_min;
    let single_volume = forebay_span <= V_EPS || config_bounds.n_volume_points == 1;

    if single_volume {
        let v0 = forebay_v_min;
        let useful = useful_storage_hm3.max(0.0);
        let half_width = (V_SYNTH_FRAC * useful).max(V_SYNTH_ABS);

        // If forebay table is single-point, allow extrapolation
        let (v_min, v_max) = if forebay_span <= V_EPS {
            (v0 - half_width, v0 + half_width)
        } else {
            (
                (v0 - half_width).clamp(forebay_v_min, forebay_v_max),
                (v0 + half_width).clamp(forebay_v_min, forebay_v_max),
            )
        };

        return FittingBounds {
            v_min,
            v_max,
            n_volume_points: 2,
            single_volume: true,
            ..*config_bounds
        };
    }

    FittingBounds {
        v_min: forebay_v_min,
        v_max: forebay_v_max,
        single_volume: false,
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
    fn resolve_bounds_detects_run_of_river_strict_tolerance() {
        let config = FittingBounds::default();

        // Exactly equal
        let bounds = resolve_fitting_bounds(500.0, 500.0, 0.0, &config);
        assert!(bounds.single_volume);

        // Within V_EPS
        let bounds = resolve_fitting_bounds(500.0, 500.0 + 1e-10, 0.0, &config);
        assert!(bounds.single_volume);

        // Beyond V_EPS — NOT single_volume
        let bounds = resolve_fitting_bounds(500.0, 500.001, 0.0, &config);
        assert!(!bounds.single_volume);
    }

    #[test]
    fn single_volume_synthesis_with_useful_storage() {
        let config = FittingBounds::default();

        // useful_storage = 1000.0 → half_width = max(5.0, 1.0) = 5.0
        let bounds = resolve_fitting_bounds(500.0, 500.0, 1000.0, &config);

        assert!(bounds.single_volume);
        assert_eq!(bounds.n_volume_points, 2);
        assert!((bounds.v_min - 495.0).abs() < 1e-9);
        assert!((bounds.v_max - 505.0).abs() < 1e-9);
    }

    #[test]
    fn single_volume_synthesis_minimum_width() {
        let config = FittingBounds::default();

        // useful_storage = 0.0 → half_width = max(0.0, 1.0) = 1.0
        let bounds = resolve_fitting_bounds(500.0, 500.0, 0.0, &config);

        assert!((bounds.v_min - 499.0).abs() < 1e-9);
        assert!((bounds.v_max - 501.0).abs() < 1e-9);
    }

    #[test]
    fn normal_plant_bounds_unchanged() {
        let config = FittingBounds {
            n_volume_points: 10,
            ..Default::default()
        };

        let bounds = resolve_fitting_bounds(0.0, 30000.0, 30000.0, &config);

        assert!(!bounds.single_volume);
        assert_eq!(bounds.v_min, 0.0);
        assert_eq!(bounds.v_max, 30000.0);
        assert_eq!(bounds.n_volume_points, 10);
    }

    #[test]
    fn single_volume_axis_uses_synthesized_bounds() {
        let bounds = FittingBounds {
            v_min: 495.0,
            v_max: 505.0,
            n_volume_points: 2,
            single_volume: true,
            ..Default::default()
        };

        let axis = build_single_volume_axis(&bounds);

        assert_eq!(axis.len(), 2);
        assert!((axis[0] - 495.0).abs() < 1e-9);
        assert!((axis[1] - 505.0).abs() < 1e-9);
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
