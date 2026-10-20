//! Generic linear algebra utilities.

pub mod invert;
pub mod multiply;
pub mod solve;
pub mod utils;

pub use invert::invert_matrix;
pub use multiply::multiply_matrices;
pub use solve::solve_linear_system;
pub use utils::{dot, round_to_digits};
