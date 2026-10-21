//! FPHA fitting pipeline modules.

mod geometry;
mod grid;
mod hull_fit;
mod production;

pub use geometry::{FittingBounds, ForebayTable, VhaRow};
pub use grid::{build_grid, resolve_fitting_bounds, GridParams};
pub use hull_fit::{build_cloud, envelope_max, fit_hull_planes, RawPlane};
pub use production::{ProductionFunction, TailraceSource};
