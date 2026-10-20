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

mod error;

pub use error::NetworkError;
