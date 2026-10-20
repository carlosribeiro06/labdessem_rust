//! Dual value extraction and marginal cost computation.
//!
//! Implements CMB (Custo Marginal de Barra) and CMO (Custo Marginal de Operação)
//! computation from LP/MILP dual values.

mod cmb;
mod cmo;
mod extraction;

pub use cmb::{CmbRecord, compute_cmb_per_bus};
pub use cmo::compute_cmo_from_cmb;
pub use extraction::extract_flow_duals;
