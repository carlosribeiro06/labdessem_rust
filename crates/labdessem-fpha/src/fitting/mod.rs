//! FPHA fitting pipeline modules.

mod alpha;
mod deviation;
mod geometry;
mod grid;
mod hull_fit;
mod production;
mod reduction;
mod rng;
mod secant;
mod selection;

pub use alpha::{compute_alpha_fpha, scale_plane_affine, scale_planes};
pub use deviation::{
    collect_fit_deviation_points, compute_fit_deviation, FphaDeviationPoint, FphaFitDeviation,
};
pub use geometry::{FittingBounds, ForebayTable, VhaRow};
pub use grid::{build_grid, resolve_fitting_bounds, GridParams};
pub use hull_fit::{build_cloud, envelope_max, fit_hull_planes, RawPlane};
pub use production::{ProductionFunction, TailraceSource};
pub use reduction::{is_origin_plane, reduce_planes, PlaneReductionConfig};
pub use secant::{fit_gamma_s_for_planes, resolve_s_max};
pub use selection::validate_fitted_planes;
