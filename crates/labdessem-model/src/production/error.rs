//! Error types for FPHA fitting.

use crate::hydro_geometry::GeometryError;
use thiserror::Error;

#[derive(Debug, Clone, Error)]
pub enum FphaFittingError {
    #[error(transparent)]
    Geometry(#[from] GeometryError),

    #[error("hydro {hydro_name}: hull computation failed: {reason}")]
    HullFailed { hydro_name: String, reason: String },

    #[error("hydro {hydro_name}: no hyperplanes produced")]
    NoHyperplanesProduced { hydro_name: String },

    #[error("hydro {hydro_name}: non-positive alpha={alpha}")]
    NonPositiveAlpha { hydro_name: String, alpha: f64 },

    #[error("hydro {hydro_name}: invalid coefficient for plane {plane_index}: {detail}")]
    InvalidCoefficient {
        hydro_name: String,
        plane_index: usize,
        detail: String,
    },

    #[error("hydro {hydro_name}: invalid fit volume window: {reason}")]
    InvalidFitWindow { hydro_name: String, reason: String },

    #[error(
        "hydro {hydro_name}: cannot reduce to {max_planes} planes ({remaining} remain after the last tolerance)"
    )]
    PlaneLimitUnreachable {
        hydro_name: String,
        max_planes: usize,
        remaining: usize,
    },
}
