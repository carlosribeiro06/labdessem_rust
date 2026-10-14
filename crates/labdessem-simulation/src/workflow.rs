//! Workflow execution modes and entry points.

use crate::error::SimulationError;

/// Iteration stage in the simulation workflow.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum IterationStage {
    LinearProgramming,
    MixedIntegerLinearProgramming,
    LinearProgrammingWithFixedCommitment,
    LinearProgrammingCalculateCmo,
}

impl IterationStage {
    pub fn label(&self) -> &'static str {
        match self {
            Self::LinearProgramming => "LP",
            Self::MixedIntegerLinearProgramming => "MILP",
            Self::LinearProgrammingWithFixedCommitment => "LP-FIXED",
            Self::LinearProgrammingCalculateCmo => "LP-CALC-CMO",
        }
    }
}

/// Execution mode for the simulation.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ExecutionOption {
    /// LP → MILP → LP-FIXED → LP-CALC-CMO with network cut iteration.
    Iterative,
    /// Single MILP solve.
    SingleMilp,
}

impl ExecutionOption {
    /// Parse execution option from config value.
    pub fn from_config_value(value: u8) -> Result<Self, SimulationError> {
        match value {
            1 => Ok(Self::Iterative),
            2 => Ok(Self::SingleMilp),
            other => Err(SimulationError::IterativeProcess(format!(
                "opcao_execucao invalida: {other}. Use 1 para LP -> MILP -> LP-FIXED -> LP-CALC-CMO ou 2 para MILP unico"
            ))),
        }
    }

    /// Human-readable description of the execution mode.
    pub fn description(&self) -> &'static str {
        match self {
            Self::Iterative => "LP -> MILP -> LP-FIXED -> LP-CALC-CMO",
            Self::SingleMilp => "MILP unico",
        }
    }
}
