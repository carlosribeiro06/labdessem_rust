//! CSV and binary readers for each entity type.
//!
//! This module provides entity-specific readers extracted from `study.rs`.
//!
//! # Structure
//!
//! ```text
//! readers/
//! ├── mod.rs          # This file
//! ├── common.rs       # Shared utilities (csv parsing, period conversion)
//! ├── future_cost.rs  # read_future_cost_cuts() - FlatBuffers binary reader
//! ├── horizon.rs      # read_horizon() - Study horizon from OPER_DURACAO.csv
//! ├── hydro.rs        # read_hydro_plants()
//! ├── limits.rs       # read_operational_limits(), read_residual_costs(), read_interchange_limits()
//! ├── network.rs      # read_buses(), read_branches(), build_dummy_buses()
//! ├── pumping.rs      # read_pumping_plants()
//! ├── renewable.rs    # read_renewable_plants()
//! ├── submarket.rs    # read_submarkets()
//! └── thermal.rs      # read_thermal_plants()
//! ```

pub mod common;
pub mod future_cost;
pub mod horizon;
pub mod hydro;
pub mod limits;
pub mod network;
pub mod pumping;
pub mod renewable;
pub mod submarket;
pub mod thermal;

pub use common::read_csv;
pub use future_cost::read_future_cost_cuts;
pub use horizon::read_horizon;
pub use hydro::read_hydro_plants;
pub use limits::{read_interchange_limits, read_operational_limits, read_residual_costs};
pub use network::{build_dummy_buses, read_branches, read_buses};
pub use pumping::read_pumping_plants;
pub use renewable::read_renewable_plants;
pub use submarket::read_submarkets;
pub use thermal::read_thermal_plants;
