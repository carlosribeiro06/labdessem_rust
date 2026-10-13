//! CSV readers for each entity type.
//!
//! This module provides entity-specific CSV readers extracted from `study.rs`.
//!
//! # Structure
//!
//! ```text
//! readers/
//! ├── mod.rs          # This file
//! ├── common.rs       # Shared utilities (csv parsing, period conversion)
//! ├── thermal.rs      # read_thermal_plants()
//! ├── hydro.rs        # read_hydro_plants()
//! ├── renewable.rs    # read_renewable_plants()
//! ├── pumping.rs      # read_pumping_plants()
//! ├── submarket.rs    # read_submarkets()
//! ├── network.rs      # read_buses(), read_branches(), build_dummy_buses()
//! └── limits.rs       # read_operational_limits(), read_residual_costs(), read_interchange_limits()
//! ```

pub mod common;
pub mod hydro;
pub mod limits;
pub mod network;
pub mod pumping;
pub mod renewable;
pub mod submarket;
pub mod thermal;

// Re-export commonly used utilities for external callers
pub use common::read_csv;

pub use hydro::read_hydro_plants;
pub use limits::{read_interchange_limits, read_operational_limits, read_residual_costs};
pub use network::{build_dummy_buses, read_branches, read_buses};
pub use pumping::read_pumping_plants;
pub use renewable::read_renewable_plants;
pub use submarket::read_submarkets;
pub use thermal::read_thermal_plants;
