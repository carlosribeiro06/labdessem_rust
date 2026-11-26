//! Error types for simulation.

use std::{error::Error, fmt};

use labdessem_model::ModelError;
use labdessem_model::hydro_geometry::GeometryError;
use labdessem_network::NetworkError;
use labdessem_solver::SolverError;

use crate::workflow::IterationStage;

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
    /// Max swallowing is enabled and a hydro plant has `Hef.m <= 0` on some of its groups but not on all of them.
    PartialEffectiveHead {
        /// Hydro plant name.
        plant: String,
        /// Groups holding a unit with `effective_head_m <= 0`, in input order.
        groups: Vec<String>,
    },
    /// Max swallowing is enabled and a plant's VHA table is malformed (ADR-018).
    Geometry(GeometryError),
    /// A solve with max swallowing rows is infeasible and the same model without them is
    /// feasible (ADR-017): `rows` names the max swallowing rows the max-swallowing-free solution
    /// exceeds, ordered by plant, then period.
    MaxSwallowingInfeasible {
        /// Stage whose solve failed.
        stage: IterationStage,
        /// Iteration of that stage.
        iteration: usize,
        /// `engmax[p=..,t=..]` names, ordered by plant, then period.
        rows: Vec<String>,
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
                "max swallowing: plant {plant} has Hef.m <= 0 on group(s) {} but not on every group; set Hef.m for every group of the plant",
                groups.join(", ")
            ),
            Self::Geometry(error) => write!(f, "max swallowing: {error}"),
            Self::MaxSwallowingInfeasible {
                stage,
                iteration,
                rows,
            } => write!(
                f,
                "max swallowing: {} #{iteration:02} is infeasible with the max swallowing rows and feasible without them; the solution without them exceeds {}",
                stage.label(),
                rows.join(", ")
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
            Self::Geometry(error) => Some(error),
            Self::IterativeProcess(_)
            | Self::PartialEffectiveHead { .. }
            | Self::MaxSwallowingInfeasible { .. } => None,
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

impl From<GeometryError> for SimulationError {
    fn from(value: GeometryError) -> Self {
        Self::Geometry(value)
    }
}

#[cfg(test)]
mod tests {
    use std::error::Error;

    use labdessem_model::ModelError;

    use super::SimulationError;
    use crate::workflow::IterationStage;

    #[test]
    fn max_swallowing_infeasible_display_lists_rows() {
        let error = SimulationError::MaxSwallowingInfeasible {
            stage: IterationStage::LinearProgramming,
            iteration: 1,
            rows: vec![
                "engmax[p=UHE-ENG,t=1]".into(),
                "engmax[p=UHE-ENG,t=2]".into(),
            ],
        };

        assert_eq!(
            error.to_string(),
            "max swallowing: LP #01 is infeasible with the max swallowing rows and feasible without them; the solution without them exceeds engmax[p=UHE-ENG,t=1], engmax[p=UHE-ENG,t=2]"
        );
        assert!(error.source().is_none());
    }

    #[test]
    fn model_error_converts_into_simulation_error() {
        let err = ModelError::MissingMaxSwallowingBound {
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
