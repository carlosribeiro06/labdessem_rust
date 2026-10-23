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

/// Per-plant FPHA configuration.
#[derive(Debug, Clone)]
pub struct FphaPlantConfig {
    pub mode: FphaMode,
    pub max_planes: usize,
    pub volume_points: usize,
    pub flow_points: usize,
    pub reduction_config: PlaneReductionConfig,
}

impl Default for FphaPlantConfig {
    fn default() -> Self {
        Self {
            mode: FphaMode::Precomputed,
            max_planes: 10,
            volume_points: 6,
            flow_points: 6,
            reduction_config: PlaneReductionConfig::default(),
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
