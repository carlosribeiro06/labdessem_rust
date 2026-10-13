//! CSV readers for each entity type.
//!
//! This module provides the infrastructure for entity-specific CSV readers.
//! Currently, the actual reading logic is in `study.rs`. This module structure
//! is prepared for incremental extraction of readers.
//!
//! # Planned Structure
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
//! - [x] common.rs - shared utilities (read_csv, log_read_file, file_label)
//! - [ ] thermal.rs - ThermalUnitRow, ThermalRampRow, build_thermal_plants
//! - [ ] hydro.rs - HydroPlantRow, HydroUnitRow, etc., build_hydro_plants
//! - [ ] renewable.rs - RenewableRow, build_renewables
//! - [ ] pumping.rs - PumpingPlantRow, build_pumping_plants
//! - [ ] submarket.rs - SubmarketRow, build_submarkets
//! - [ ] network.rs - BusOperationRow, BranchRow, build_buses, build_branches
//! - [ ] limits.rs - OperationalLimitRow, build_operational_limits

pub mod common;

pub use common::{file_label, log_read_file, read_csv};
