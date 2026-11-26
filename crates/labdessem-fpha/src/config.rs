//! FPHA plant configuration types.

use crate::fitting::{FittingBounds, PlaneReductionConfig};

/// FPHA mode for a plant.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum FphaMode {
    /// Use precomputed planes from OPER_FPHA.csv.
    #[default]
    Precomputed,
    /// Compute planes from VHA geometry.
    Computed,
}

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

/// Per-plant FPHA configuration.
#[derive(Debug, Clone)]
pub struct FphaPlantConfig {
    pub mode: FphaMode,
    pub max_planes: usize,
    pub volume_points: usize,
    pub flow_points: usize,
    pub reduction_config: PlaneReductionConfig,
    pub volume_window: VolumeWindow,
    /// Reference interval `(lo_ref, hi_ref)` in hm³ the window fractions refer to; `None` means
    /// the forebay table span.
    pub window_reference_hm3: Option<(f64, f64)>,
}

impl Default for FphaPlantConfig {
    fn default() -> Self {
        Self {
            mode: FphaMode::Precomputed,
            max_planes: DEFAULT_MAX_PLANES,
            volume_points: DEFAULT_VOLUME_POINTS,
            flow_points: DEFAULT_FLOW_POINTS,
            reduction_config: PlaneReductionConfig {
                tolerance_deg: DEFAULT_ANGLE_TOLERANCE_DEG,
            },
            volume_window: VolumeWindow::FULL,
            window_reference_hm3: None,
        }
    }
}

impl FphaPlantConfig {
    /// Create a computed-mode config with the given parameters.
    pub fn computed(
        max_planes: usize,
        volume_points: usize,
        flow_points: usize,
        reduction_config: PlaneReductionConfig,
    ) -> Self {
        Self {
            mode: FphaMode::Computed,
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
