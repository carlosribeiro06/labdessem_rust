//! Power system model container and related types.
//!
//! The [`System`] struct is the immutable container for all power system data
//! used in optimization. It is constructed via [`SystemBuilder`].

mod builder;
mod helpers;
mod horizon;
mod operational_limit;
mod system;
#[cfg(test)]
mod tests;
mod types;
mod validate;

pub use builder::SystemBuilder;
pub use horizon::StudyHorizon;
pub use operational_limit::{OperationalLimit, OperationalLimitTarget, OperationalLimitVariable};
pub use system::System;
pub use types::{FutureCostCut, InterchangeLimit, ResidualCost};
pub use validate::validate_system;

// Re-export entities for backward compatibility
pub use crate::entities::{Branch, Bus, PumpingPlant, Submarket};
