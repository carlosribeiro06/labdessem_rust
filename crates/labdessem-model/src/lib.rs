pub mod constraints;
pub mod indexing;
mod model;
pub mod names;
pub mod objective;
pub mod variables;

pub use model::{Model, SolveMode};

#[cfg(test)]
mod tests;
