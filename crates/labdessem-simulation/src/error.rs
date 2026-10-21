//! Error types for simulation.

use std::{error::Error, fmt};

use labdessem_network::NetworkError;
use labdessem_solver::SolverError;

/// Errors that can occur during simulation.
#[derive(Debug)]
pub enum SimulationError {
    /// Error from the optimization solver.
    Solver(SolverError),

    /// Error from network model construction or computation.
    Network(NetworkError),
    /// Error in iterative process.
    IterativeProcess(String),
}

impl fmt::Display for SimulationError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Solver(error) => write!(f, "{error}"),

            Self::Network(error) => write!(f, "{error}"),
            Self::IterativeProcess(message) => write!(f, "{message}"),
        }
    }
}

impl Error for SimulationError {
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        match self {
            Self::Solver(error) => Some(error),

            Self::Network(error) => Some(error),
            Self::IterativeProcess(_) => None,
        }
    }
}

impl From<SolverError> for SimulationError {
    fn from(value: SolverError) -> Self {
        Self::Solver(value)
    }
}

impl From<NetworkError> for SimulationError {
    fn from(value: NetworkError) -> Self {
        Self::Network(value)
    }
}
