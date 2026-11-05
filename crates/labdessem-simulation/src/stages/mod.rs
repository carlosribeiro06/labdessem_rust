//! Simulation stages orchestration.

mod common;
mod helpers;
mod orchestration;
mod solve;

#[cfg(test)]
// Rationale: tests use expect() for clearer failure messages
#[allow(clippy::unwrap_used, clippy::expect_used)]
mod engmax_tests;

pub use orchestration::{run_simulation, run_simulation_with_engmax};
