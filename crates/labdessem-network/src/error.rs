//! Network-specific error types.

use thiserror::Error;

/// Errors from DC network model construction or computation.
#[derive(Debug, Error)]
pub enum NetworkError {
    #[error("system does not define a reference bus for DC flow")]
    NoReferenceBus,

    #[error("reduced DC susceptance matrix is singular")]
    SingularMatrix,

    #[error("matrix dimension mismatch: {0}")]
    DimensionMismatch(String),

    #[error("matrix inversion requires a square matrix")]
    NonSquareMatrix,

    #[error("linear system dimensions are inconsistent")]
    InconsistentDimensions,
}
