//! FPHA fitting pipeline for computed hydro generation planes.
//!
//! This crate provides the convex hull fitting algorithm to compute
//! FPHA hyperplanes from reservoir geometry (VHA tables).

pub mod config;
pub mod error;
pub mod fitting;
pub mod from_io;
pub mod pipeline;

pub use config::FphaPlantConfig;
pub use error::FphaFittingError;
pub use fitting::{
    FittingBounds, FphaFitDeviation, GridParams, PlaneReductionConfig, ProductionFunction,
    RawPlane, build_cloud, build_grid, compute_alpha_fpha, compute_fit_deviation,
    fit_gamma_s_for_planes, fit_hull_planes, is_origin_plane, reduce_planes,
    resolve_fitting_bounds, resolve_s_max, scale_plane_affine, scale_planes,
    validate_fitted_planes,
};
pub use from_io::{forebay_from_vha, tailrace_from_data};
pub use pipeline::{FitPlantParams, FphaPlantResult, fit_plant};
