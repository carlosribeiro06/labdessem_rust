//! CSV output writers for simulation results.
//!
//! This module provides functions to write result CSV files under the
//! RESULTADOS directory of each study case.
//!
//! # Structure
//!
//! ```text
//! output/
//! ├── mod.rs          # This file
//! ├── common.rs       # Shared CSV utilities (csv_with_header, write_csv_file)
//! ├── hydro.rs        # Hydro plant results (stub)
//! ├── network.rs      # Network results (stub)
//! ├── pumping.rs      # Pumping plant results (stub)
//! ├── renewable.rs    # Renewable plant results (stub)
//! ├── summary.rs      # CMO, iteration logs, FCF (stub)
//! └── thermal.rs      # Thermal plant results (stub)
//! ```
//!
//! # Usage
//!
//! The common module provides CSV generation utilities used by all writers:
//!
//! ```ignore
//! use labdessem_io::output::{csv_with_header, write_csv_file};
//!
//! let csv = csv_with_header(
//!     "Col1;Col2;Col3",
//!     &[("Col1", "description 1"), ("Col2", "description 2")],
//! );
//! write_csv_file("output.csv", csv)?;
//! ```
//!
//! # Note
//!
//! Most writer modules are stubs. The actual implementations remain in
//! `labdessem-simulation` due to dependencies on `SolveSummary` and model
//! variable naming. See individual module documentation for migration paths.

pub mod common;
pub mod hydro;
pub mod network;
pub mod pumping;
pub mod renewable;
pub mod summary;
pub mod thermal;

pub use common::{csv_with_header, volume_hm3_to_flow_m3s, write_csv_file};
