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
//! ├── network.rs      # read_buses(), read_branches()
//! └── limits.rs       # read_operational_limits()
//! ```
//!
//! # Migration Status
//!
//! - [x] common.rs - shared utilities (read_csv, helpers)
//! - [x] thermal.rs - ThermalUnitRow, ThermalRampRow, read_thermal_plants
//! - [x] hydro.rs - HydroPlantRow, HydroUnitRow, etc., read_hydro_plants
//! - [x] renewable.rs - RenewableRow, read_renewable_plants
//! - [x] pumping.rs - PumpingPlantRow, read_pumping_plants
//! - [ ] submarket.rs - SubmarketRow, read_submarkets
//! - [ ] network.rs - BusOperationRow, BranchRow, read_buses, read_branches
//! - [ ] limits.rs - OperationalLimitRow, read_operational_limits

pub mod common;
pub mod hydro;
pub mod pumping;
pub mod renewable;
pub mod thermal;

// Re-export commonly used utilities for external callers
pub use common::read_csv;
pub use hydro::read_hydro_plants;
pub use pumping::read_pumping_plants;
pub use renewable::read_renewable_plants;
pub use thermal::read_thermal_plants;
