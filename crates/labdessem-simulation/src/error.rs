//! Error types for simulation.

use std::{error::Error, fmt};

use labdessem_model::ModelError;
use labdessem_network::NetworkError;
use labdessem_solver::SolverError;

/// Errors that can occur during simulation.
#[derive(Debug)]
pub enum SimulationError {
    /// Error from the optimization solver.
    Solver(SolverError),
    /// Error from network model construction or computation.
    Network(NetworkError),
    /// Error while building the optimization model.
    Model(ModelError),
    /// Error in iterative process.
    IterativeProcess(String),
    /// EngMax is enabled and a hydro plant has `Hef.m <= 0` on some of its groups but not on all of them.
    PartialEffectiveHead {
        /// Hydro plant name.
        plant: String,
        /// Groups holding a unit with `effective_head_m <= 0`, in input order.
        groups: Vec<String>,
    },
}

impl fmt::Display for SimulationError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Solver(error) => write!(f, "{error}"),
            Self::Network(error) => write!(f, "{error}"),
            Self::Model(error) => write!(f, "{error}"),
            Self::IterativeProcess(message) => write!(f, "{message}"),
            Self::PartialEffectiveHead { plant, groups } => write!(
                f,
                "EngMax: plant {plant} has Hef.m <= 0 on group(s) {} but not on every group; set Hef.m for every group of the plant",
                groups.join(", ")
            ),
        }
    }
}

impl Error for SimulationError {
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        match self {
            Self::Solver(error) => Some(error),
            Self::Network(error) => Some(error),
            Self::Model(error) => Some(error),
            Self::IterativeProcess(_) | Self::PartialEffectiveHead { .. } => None,
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

impl From<ModelError> for SimulationError {
    fn from(value: ModelError) -> Self {
        Self::Model(value)
    }
}

#[cfg(test)]
mod tests {
    use std::error::Error;

    use labdessem_model::ModelError;

    use super::SimulationError;

    #[test]
    fn model_error_converts_into_simulation_error() {
        let err = ModelError::MissingEngMaxBound {
            plant: "UHE-1".into(),
            period: 3,
        };

        let converted = SimulationError::from(err.clone());

        assert!(matches!(&converted, SimulationError::Model(inner) if *inner == err));
        assert_eq!(converted.to_string(), err.to_string());
        assert_eq!(
            converted
                .source()
                .and_then(|source| source.downcast_ref::<ModelError>()),
            Some(&err)
        );
    }
}
