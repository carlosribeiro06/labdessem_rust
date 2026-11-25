#[forbid(unsafe_code)]
pub mod constraints;
#[forbid(unsafe_code)]
mod error;
#[allow(unsafe_code)]
pub mod hull;
#[forbid(unsafe_code)]
pub mod indexing;
#[forbid(unsafe_code)]
mod model;
#[forbid(unsafe_code)]
pub mod names;
#[forbid(unsafe_code)]
pub mod objective;
#[forbid(unsafe_code)]
pub mod variables;

pub use error::ModelError;
pub use model::{Model, SolveMode};

#[cfg(test)]
#[forbid(unsafe_code)]
mod tests;
