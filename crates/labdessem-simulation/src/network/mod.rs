//! DC network model for power flow analysis.
//!
//! This module re-exports types and functions from `labdessem-network` crate
//! and provides glue code that bridges the network crate with `labdessem-model`.

mod flow;

// Re-export everything from labdessem-network
pub use labdessem_network::{
    CmbRecord, DcNetworkModel, FlowCutKey, all_flow_constraints, compute_cmb_per_bus,
    compute_cmo_from_cmb, compute_line_flows, constant_load_shift, detect_flow_violations,
    extract_flow_duals,
};

// flow_terms_for_cut stays here (depends on Model)
pub use flow::flow_terms_for_cut;
