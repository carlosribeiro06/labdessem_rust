//! FPHA production-function fit (RD-10, RD-22): sample the plant's production function over the
//! fit window, take the upper convex hull of the cloud, reduce and scale the planes, and fit the
//! spill coefficient. The formulation is in `docs/design/fpha-formulation.md`.

mod alpha;
mod config;
mod deviation;
mod error;
mod function;
mod grid;
mod hull_fit;
mod pipeline;
mod reduction;
mod secant;
mod selection;

pub use alpha::compute_alpha_fpha;
pub use config::{
    DEFAULT_ANGLE_TOLERANCE_DEG, DEFAULT_FLOW_POINTS, DEFAULT_MAX_PLANES, DEFAULT_VOLUME_POINTS,
    FphaConfigOverrides, FphaPlantConfig, VolumeWindow,
};
pub use deviation::FphaFitDeviation;
pub use error::FphaFittingError;
pub use function::ProductionFunction;
pub use grid::{FittingBounds, GridParams, build_grid};
pub use hull_fit::{RawPlane, fit_hull_planes};
pub use pipeline::{FitPlantParams, FphaPlantResult, fit_context, fit_plant};
pub use reduction::{PlaneReductionConfig, reduce_planes};
pub use secant::fit_gamma_s_for_planes;
