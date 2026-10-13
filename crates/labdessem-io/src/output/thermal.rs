//! Thermal plant result writer.
//!
//! This module is prepared for the thermal results writer. Currently, the
//! implementation resides in `labdessem-simulation/src/lib.rs` (function
//! `write_thermal_csv`) due to heavy dependencies on:
//!
//! - `SolveSummary` from labdessem-solver
//! - Variable naming functions from labdessem-model
//! - Thermal residence time calculation helpers
//!
//! # Migration Path
//!
//! To complete the extraction:
//! 1. Create a `ThermalResult` struct that holds pre-computed values
//! 2. Move value extraction logic to simulation
//! 3. Move CSV formatting logic here
//! 4. Export `write_thermal_results(results: &[ThermalResult], path: &Path)`

// Placeholder for future implementation
