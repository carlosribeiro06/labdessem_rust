//! Network constraint generation.
//!
//! Provides types and functions for building solver-independent
//! network flow constraints.

mod flow_cut;
mod input;

pub use flow_cut::build_flow_cut_terms;
pub use input::{FlowCutInput, GeneratorEntry, PumpingEntry};
