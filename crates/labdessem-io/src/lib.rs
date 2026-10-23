//! # labdessem-io
//!
//! File I/O for the LABDESSEM workspace.
//!
//! ## Input
//!
//! The [`study`] module loads study configuration and input data:
//! - `study_config.json` - Study configuration
//! - `CAD/` - Static registration data (plants, buses, lines)
//! - `OPER/` - Time-varying data (demand, inflows, limits)
//!
//! ## Output
//!
//! The [`output`] module writes simulation results to CSV files:
//! - [`output::write_all_results`] - Main entry point for writing all results
//! - Individual writers for hydro, thermal, renewable, pumping, network, etc.

pub mod error;
pub mod output;
pub mod readers;
pub mod study;

pub use error::IoError;
pub use readers::{
    read_fpha_config, read_tailrace, read_vha_geometry, FphaPlantConfig, TailraceData,
    VhaGeometryData, VhaPoint,
};
pub use study::{
    FphaOption, StudyConfig, generate_study_config_schema, read_study_config, read_study_from_path,
};

#[cfg(test)]
mod verify_imports_test;
