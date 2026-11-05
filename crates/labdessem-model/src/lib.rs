pub mod constraints;
mod error;
pub mod indexing;
mod model;
pub mod names;
pub mod objective;
pub mod variables;

pub use error::ModelError;
pub use model::{Model, SolveMode};

#[cfg(test)]
mod tests;
