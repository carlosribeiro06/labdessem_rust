//! FPHA fitting pipeline for computed hydro generation planes.
//!
//! This crate provides the convex hull fitting algorithm to compute
//! FPHA hyperplanes from reservoir geometry (VHA tables).

pub mod config;
pub mod error;
pub mod fitting;
pub mod hull;
pub mod io;
pub mod pipeline;
pub mod types;

pub use config::{FphaMode, FphaPlantConfig};
pub use error::FphaFittingError;
pub use fitting::{
    build_cloud, build_grid, collect_fit_deviation_points, compute_alpha_fpha, compute_fit_deviation,
    envelope_max, fit_gamma_s_for_planes, fit_hull_planes, is_origin_plane, reduce_planes,
    resolve_fitting_bounds, resolve_s_max, scale_plane_affine, scale_planes, validate_fitted_planes,
    FittingBounds, ForebayTable, FphaDeviationPoint, FphaFitDeviation, GridParams,
    PlaneReductionConfig, ProductionFunction, RawPlane, TailraceSource, VhaRow,
};
pub use hull::{convex_hull_3d, HullError, Hyperplane3d};
pub use io::{
    read_fpha_config, read_geometry_table, read_tailrace_table, write_fpha_results,
    FphaConfigRow, FphaPlantResult, GeometryRow, TailraceRow,
};
pub use pipeline::{fit_plant, fit_plants, FitPlantParams};
pub use types::TailraceModel;
