//! FPHA fitting pipeline modules.

mod alpha;
mod deviation;
mod grid;
mod hull_fit;
mod production;
mod reduction;
mod secant;
mod selection;

pub use alpha::{compute_alpha_fpha, scale_plane_affine, scale_planes};
pub use deviation::{
    FphaDeviationPoint, FphaFitDeviation, collect_fit_deviation_points, compute_fit_deviation,
};
pub use grid::{FittingBounds, GridParams, build_grid, resolve_fitting_bounds};
pub use hull_fit::{RawPlane, build_cloud, envelope_max, fit_hull_planes};
pub use production::ProductionFunction;
pub use reduction::{PlaneReductionConfig, is_origin_plane, reduce_planes};
pub use secant::{fit_gamma_s_for_planes, resolve_s_max};
pub use selection::validate_fitted_planes;
