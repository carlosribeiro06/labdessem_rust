//! Solver error types.

use std::{error::Error, fmt};

/// Terminal failure modes from a solve attempt.
#[derive(Debug, Clone)]
pub enum SolverError {
    /// LP has no feasible solution.
    Infeasible,

    /// Objective is unbounded (no finite optimum).
    Unbounded,

    /// Numerical difficulty after retry attempts exhausted.
    NumericalDifficulty {
        /// Description of the numerical issue.
        message: String,
    },

    /// Wall-clock time limit exceeded.
    TimeLimitExceeded {
        /// Elapsed time in seconds before termination.
        elapsed_seconds: f64,
    },

    /// Simplex iteration limit exceeded.
    IterationLimit {
        /// Number of iterations performed.
        iterations: u64,
    },

    /// Internal solver error.
    InternalError {
        /// Error description from the solver.
        message: String,
        /// Solver-specific error code, if available.
        error_code: Option<i32>,
    },

    /// Offered basis is inconsistent with LP structure.
    BasisInconsistent,

    /// Offered basis has different row count than current LP.
    BasisRowCountMismatch {
        /// Expected number of rows.
        expected: usize,
        /// Actual number of rows in basis.
        got: usize,
    },

    /// I/O error from underlying system.
    Io(String),
}

impl fmt::Display for SolverError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Infeasible => write!(f, "LP is infeasible"),
            Self::Unbounded => write!(f, "LP is unbounded"),
            Self::NumericalDifficulty { message } => {
                write!(f, "numerical difficulty: {message}")
            }
            Self::TimeLimitExceeded { elapsed_seconds } => {
                write!(f, "time limit exceeded after {elapsed_seconds:.2}s")
            }
            Self::IterationLimit { iterations } => {
                write!(f, "iteration limit exceeded after {iterations} iterations")
            }
            Self::InternalError {
                message,
                error_code,
            } => {
                if let Some(code) = error_code {
                    write!(f, "internal solver error (code {code}): {message}")
                } else {
                    write!(f, "internal solver error: {message}")
                }
            }
            Self::BasisInconsistent => write!(f, "basis is inconsistent with LP structure"),
            Self::BasisRowCountMismatch { expected, got } => {
                write!(
                    f,
                    "basis row count mismatch: expected {expected}, got {got}"
                )
            }
            Self::Io(message) => write!(f, "I/O error: {message}"),
        }
    }
}

impl Error for SolverError {}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn solver_error_display_infeasible() {
        let err = SolverError::Infeasible;
        assert_eq!(err.to_string(), "LP is infeasible");
    }

    #[test]
    fn solver_error_display_unbounded() {
        let err = SolverError::Unbounded;
        assert_eq!(err.to_string(), "LP is unbounded");
    }

    #[test]
    fn solver_error_display_numerical() {
        let err = SolverError::NumericalDifficulty {
            message: "scaling failed".into(),
        };
        let s = err.to_string();
        assert!(s.contains("numerical difficulty"));
        assert!(s.contains("scaling failed"));
    }

    #[test]
    fn solver_error_display_time_limit() {
        let err = SolverError::TimeLimitExceeded {
            elapsed_seconds: 123.456,
        };
        let s = err.to_string();
        assert!(s.contains("time limit"));
        assert!(s.contains("123.46"));
    }

    #[test]
    fn solver_error_display_iteration_limit() {
        let err = SolverError::IterationLimit { iterations: 50000 };
        let s = err.to_string();
        assert!(s.contains("iteration limit"));
        assert!(s.contains("50000"));
    }

    #[test]
    fn solver_error_display_internal_with_code() {
        let err = SolverError::InternalError {
            message: "memory allocation failed".into(),
            error_code: Some(-1),
        };
        let s = err.to_string();
        assert!(s.contains("internal solver error"));
        assert!(s.contains("code -1"));
        assert!(s.contains("memory allocation failed"));
    }

    #[test]
    fn solver_error_display_internal_without_code() {
        let err = SolverError::InternalError {
            message: "unknown failure".into(),
            error_code: None,
        };
        let s = err.to_string();
        assert!(s.contains("internal solver error"));
        assert!(s.contains("unknown failure"));
        assert!(!s.contains("code"));
    }

    #[test]
    fn solver_error_display_basis_inconsistent() {
        let err = SolverError::BasisInconsistent;
        assert!(err.to_string().contains("inconsistent"));
    }

    #[test]
    fn solver_error_display_basis_row_mismatch() {
        let err = SolverError::BasisRowCountMismatch {
            expected: 100,
            got: 50,
        };
        let s = err.to_string();
        assert!(s.contains("100"));
        assert!(s.contains("50"));
    }

    #[test]
    fn solver_error_display_io() {
        let err = SolverError::Io("file not found".into());
        let s = err.to_string();
        assert!(s.contains("I/O error"));
        assert!(s.contains("file not found"));
    }

    #[test]
    fn solver_error_is_std_error() {
        fn assert_error<E: std::error::Error>() {}
        assert_error::<SolverError>();
    }

    #[test]
    fn solver_error_is_clone() {
        let err = SolverError::Infeasible;
        let cloned = err.clone();
        assert_eq!(err.to_string(), cloned.to_string());
    }
}
