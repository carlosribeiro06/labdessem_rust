//! Error types for FPHA fitting.

use thiserror::Error;

#[derive(Debug, Clone, Error)]
pub enum FphaFittingError {
    #[error("hydro {hydro_name}: insufficient geometry points ({count} < 2)")]
    InsufficientPoints { hydro_name: String, count: usize },

    #[error("hydro {hydro_name}: duplicate volume {volume_hm3} in geometry table")]
    DuplicateVolume { hydro_name: String, volume_hm3: f64 },

    #[error("hydro {hydro_name}: degenerate production cloud")]
    DegenerateProductionCloud { hydro_name: String },

    #[error("hydro {hydro_name}: hull computation failed: {reason}")]
    HullFailed { hydro_name: String, reason: String },

    #[error("hydro {hydro_name}: plane validation failed: {reason}")]
    PlaneValidationFailed { hydro_name: String, reason: String },
}
