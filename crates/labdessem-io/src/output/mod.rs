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
//! ├── common.rs       # Shared utilities
//! ├── hydro.rs        # Hydro plant results
//! ├── network.rs      # Network results (buses, branches, CMO)
//! ├── pumping.rs      # Pumping plant results
//! ├── renewable.rs    # Renewable plant results
//! ├── summary.rs      # Iteration logs, constraint summaries
//! └── thermal.rs      # Thermal plant results
//! ```

// Modules to be implemented by subsequent tickets
