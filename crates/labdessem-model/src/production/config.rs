//! FPHA plant configuration types.

use super::grid::FittingBounds;
use super::reduction::PlaneReductionConfig;

pub const DEFAULT_MAX_PLANES: usize = 10;
pub const DEFAULT_VOLUME_POINTS: usize = 5;
pub const DEFAULT_FLOW_POINTS: usize = 5;
pub const DEFAULT_ANGLE_TOLERANCE_DEG: f64 = 1.0;

/// Fraction of the reference volume interval `[lo_ref, hi_ref]` the fit covers:
/// `[lo_ref + min_fraction * span, hi_ref - (1 - max_fraction) * span]`.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct VolumeWindow {
    pub min_fraction: f64,
    pub max_fraction: f64,
}

impl VolumeWindow {
    /// The whole reference interval.
    pub const FULL: Self = Self {
        min_fraction: 0.0,
        max_fraction: 1.0,
    };
}

/// Per-plant overrides of `CAD/CAD_FPHA_CONFIG.csv`; a `None` field keeps the default.
#[derive(Debug, Clone, Copy, Default, PartialEq)]
pub struct FphaConfigOverrides {
    /// Column `num_planes`.
    pub max_planes: Option<usize>,
    /// Column `tolerance`, in degrees.
    pub tolerance_deg: Option<f64>,
    /// Column `min_volume_pct`, a fraction in [0, 1].
    pub min_volume_fraction: Option<f64>,
    /// Column `max_volume_pct`, a fraction in [0, 1].
    pub max_volume_fraction: Option<f64>,
}

/// Per-plant FPHA configuration.
#[derive(Debug, Clone)]
pub struct FphaPlantConfig {
    pub max_planes: usize,
    pub volume_points: usize,
    pub flow_points: usize,
    pub reduction_config: PlaneReductionConfig,
    pub volume_window: VolumeWindow,
    /// Reference interval `(lo_ref, hi_ref)` in hm³ the window fractions refer to; `None` means
    /// the forebay table span.
    pub window_reference_hm3: Option<(f64, f64)>,
}

impl FphaPlantConfig {
    /// Create a config with the given parameters, the full volume window and no window reference.
    pub fn computed(
        max_planes: usize,
        volume_points: usize,
        flow_points: usize,
        reduction_config: PlaneReductionConfig,
    ) -> Self {
        Self {
            max_planes,
            volume_points,
            flow_points,
            reduction_config,
            volume_window: VolumeWindow::FULL,
            window_reference_hm3: None,
        }
    }

    /// Convert to FittingBounds for a given volume range.
    pub fn to_fitting_bounds(&self, v_min: f64, v_max: f64) -> FittingBounds {
        FittingBounds {
            v_min,
            v_max,
            n_volume_points: self.volume_points,
            n_flow_points: self.flow_points,
            n_spillage_points: 2,
            max_planes_per_hydro: self.max_planes,
            single_volume: (v_max - v_min).abs() < 1.0,
        }
    }
}
