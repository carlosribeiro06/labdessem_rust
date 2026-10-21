//! DC network model and power flow algorithms.
//!
//! Provides PTDF computation, line flow calculation, violation detection,
//! and CMB/CMO dual extraction. Solution values arrive as pure slices (`&[f64]`).

pub mod constraints;
pub mod duals;
mod error;
pub mod flow;
pub mod linalg;
pub mod topology;
mod types;

pub use constraints::{FlowCutInput, GeneratorEntry, PumpingEntry, build_flow_cut_terms};
pub use duals::{CmbRecord, compute_cmb_per_bus, compute_cmo_from_cmb, extract_flow_duals};
pub use error::NetworkError;
pub use flow::{
    all_flow_constraints, compute_line_flows, constant_load_shift, detect_flow_violations,
};
pub use linalg::{dot, invert_matrix, multiply_matrices, round_to_digits, solve_linear_system};
pub use topology::model::{DcNetworkModel, FlowCutKey};
pub use topology::{
    build_branch_susceptance_diag, build_reduced_incidence, build_reduced_susceptance, compute_ptdf,
};
pub use types::{
    AddedFlowCutLineResult, ConstraintSense, FlowCutMapping, FlowViolation, LineFlowResult,
    NetworkConstraint, NetworkTerm,
};
