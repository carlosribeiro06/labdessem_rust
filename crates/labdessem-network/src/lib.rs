//! DC network model and power flow algorithms.
//!
//! This crate provides:
//! - DC network model construction from system data
//! - PTDF (Power Transfer Distribution Factor) matrix computation
//! - Line flow calculation and violation detection
//! - CMB (Custo Marginal de Barra) and CMO computation
//!
//! # Architecture
//!
//! The crate is independent of the LP/MILP model formulation. It receives
//! solution values as pure slices (`&[f64]`) rather than solver-specific types.

mod cmb;
mod dual_extraction;
mod error;
mod flow;
mod model;
mod ptdf;
mod types;

pub use cmb::{CmbRecord, compute_cmb_per_bus, compute_cmo_from_cmb};
pub use dual_extraction::extract_flow_duals;
pub use error::NetworkError;
pub use flow::{
    all_flow_constraints, compute_line_flows, constant_load_shift, detect_flow_violations,
};
pub use model::{DcNetworkModel, FlowCutKey};
pub use ptdf::{
    build_branch_susceptance_diag, build_reduced_incidence, build_reduced_susceptance, dot,
    invert_matrix, multiply_matrices, round_to_digits, solve_linear_system,
};
pub use types::{AddedFlowCutLineResult, FlowCutMapping, FlowViolation, LineFlowResult};
