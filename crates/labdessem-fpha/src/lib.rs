//! FPHA fitting pipeline for computed hydro generation planes.
//!
//! This crate provides the convex hull fitting algorithm to compute
//! FPHA hyperplanes from reservoir geometry (VHA tables).

pub mod error;
pub mod fitting;
pub mod hull;
pub mod types;

pub use error::FphaFittingError;
pub use fitting::{
    build_cloud, build_grid, envelope_max, fit_hull_planes, resolve_fitting_bounds, FittingBounds,
    ForebayTable, GridParams, ProductionFunction, RawPlane, TailraceSource, VhaRow,
};
pub use hull::{convex_hull_3d, HullError, Hyperplane3d};
pub use types::TailraceModel;
