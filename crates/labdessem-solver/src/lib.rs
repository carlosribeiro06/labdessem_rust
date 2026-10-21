//! Solver abstraction layer for `labdessem`.
//!
//! Provides a unified interface for LP/MILP solving through the [`SolverInterface`] trait,
//! with [`HighsSolver`] as the primary backend.

pub mod accessor;
pub mod backends;
pub mod basis_status;
pub mod conversion;
pub mod error;
pub mod profile;
pub mod statistics;
pub mod trait_def;
pub mod types;

pub use accessor::SolutionAccessor;
pub use backends::HighsSolver;
pub use basis_status::BasisStatus;
pub use conversion::model_to_lp_template;
pub use error::SolverError;
pub use profile::HighsProfile;
pub use statistics::SolverStatistics;
pub use trait_def::SolverInterface;
pub use types::{Basis, LpSolution, LpTemplate, RowBatch, SolutionView, VariableDomain};
