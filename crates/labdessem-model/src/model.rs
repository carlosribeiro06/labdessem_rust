//! Model construction from System.

use labdessem_core::system::System;

use crate::{constraints, indexing, objective, variables};

/// Solve mode determines which variables and constraints are active.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SolveMode {
    LinearProgramming,
    MixedIntegerLinearProgramming,
    LinearProgrammingWithFixedCommitment,
}

/// Optimization model built from a System.
#[derive(Debug)]
pub struct Model {
    pub solve_mode: SolveMode,
    pub indexing: indexing::Indexing,
    pub variables: variables::Variables,
    pub constraints: constraints::ConstraintSet,
    pub objective: objective::Objective,
}

impl Model {
    /// Build a model from a system and solve mode.
    pub fn from_system(system: &System, solve_mode: SolveMode) -> Self {
        let indexing = indexing::Indexing::from_system(system);
        let variables = variables::Variables::for_system(system, &indexing, solve_mode);
        let constraints =
            constraints::ConstraintSet::for_system(system, &indexing, &variables, solve_mode);
        let objective = objective::Objective::for_system(system, &indexing, &variables, solve_mode);

        Self {
            solve_mode,
            indexing,
            variables,
            constraints,
            objective,
        }
    }
}
