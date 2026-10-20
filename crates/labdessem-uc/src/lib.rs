//! Unit Commitment formulation for LABDESSEM.
//!
//! This crate provides solver-agnostic construction of variables and constraints
//! for thermal and hydro unit commitment problems.

pub mod hydro;
pub mod thermal;
pub mod types;

pub use types::{CommitmentDomain, ConstraintSense, UcConstraint, UcTerm, UcVariable};
