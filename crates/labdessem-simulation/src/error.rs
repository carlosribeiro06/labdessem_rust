//! Error types for simulation.

use std::{error::Error, fmt};

use labdessem_io::IoError;
use labdessem_solver::SolverError;

/// Errors that can occur during simulation.
#[derive(Debug)]
pub enum SimulationError {
    /// Error from the optimization solver.
    Solver(SolverError),
    /// Error from I/O operations.
    Io(IoError),
    /// Invalid network configuration.
    InvalidNetwork(String),
    /// Error in iterative process.
    IterativeProcess(String),
}

impl fmt::Display for SimulationError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Solver(error) => write!(f, "{error}"),
            Self::Io(error) => write!(f, "{error}"),
            Self::InvalidNetwork(message) | Self::IterativeProcess(message) => {
                write!(f, "{message}")
            }
        }
    }
}

impl Error for SimulationError {
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        match self {
            Self::Solver(error) => Some(error),
            Self::Io(error) => Some(error),
            Self::InvalidNetwork(_) | Self::IterativeProcess(_) => None,
        }
    }
}

impl From<SolverError> for SimulationError {
    fn from(value: SolverError) -> Self {
        Self::Solver(value)
    }
}

impl From<IoError> for SimulationError {
    fn from(value: IoError) -> Self {
        Self::Io(value)
    }
}
