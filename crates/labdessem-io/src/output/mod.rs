//! CSV output writers for simulation results.
//!
//! This module provides functions to write result CSV files under the
//! RESULTADOS directory of each study case.

pub mod common;
pub mod helpers;
pub mod pumping;
pub mod renewable;
pub mod thermal;

pub use common::{csv_with_header, volume_hm3_to_flow_m3s, write_csv_file};
pub use pumping::write_pumping_csv;
pub use renewable::write_renewable_csv;
pub use thermal::write_thermal_csv;
