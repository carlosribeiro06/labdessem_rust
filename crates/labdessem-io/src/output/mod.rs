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
//! └── common.rs       # Shared CSV utilities
//! ```
//!
//! # Usage
//!
//! The common module provides CSV generation utilities:
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

pub mod common;

pub use common::{csv_with_header, volume_hm3_to_flow_m3s, write_csv_file};
