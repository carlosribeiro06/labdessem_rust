//! Network constraint generation.
//!
//! Provides types and functions for building solver-independent
//! network flow constraints.

mod flow_cut;
mod input;

pub use input::{FlowCutInput, GeneratorEntry, PumpingEntry};
