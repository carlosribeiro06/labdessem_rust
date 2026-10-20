//! Simulation stages orchestration module.
//!
//! This module contains the execution logic for different simulation stages:
//! - LP relaxation for iterative cut generation
//! - MILP for commitment decisions
//! - LP-FIXED with committed variables fixed
//! - LP-CALC-CMO for marginal cost extraction
//!
//! ## Module Structure
//!
//! - `orchestration` - Main simulation entry points and strategy dispatch
//! - `solve` - Solver interaction and stage execution
//! - `helpers` - Cut management, slack variables, CMO computation
//! - `common` - Shared constants and utilities

mod common;
mod helpers;
mod orchestration;
mod solve;

pub use orchestration::run_simulation;
