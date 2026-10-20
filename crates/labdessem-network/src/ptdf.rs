//! PTDF computation and linear algebra utilities.
//!
//! # Mathematical Background
//!
//! The Power Transfer Distribution Factor (PTDF) matrix relates bus power injections
//! to branch power flows in a DC power flow approximation:
//!
//! ```text
//! P_branch = PTDF × P_injection
//! ```
//!
//! where:
//! - `P_branch` is the vector of branch power flows
//! - `P_injection` is the vector of bus net power injections (generation - load)
//! - `PTDF = B_branch × A × B^(-1)`
//!
//! with:
//! - `B_branch` = diagonal matrix of branch susceptances (1/X)
//! - `A` = reduced incidence matrix (excluding slack bus)
//! - `B` = reduced bus susceptance matrix (excluding slack bus)

use std::collections::HashMap;

use labdessem_core::{ids::BusId, system::System};

// Re-import from linalg for public interface compatibility.
#[allow(unused_imports)]
use crate::linalg::{dot, invert_matrix, multiply_matrices, round_to_digits, solve_linear_system};

/// Build the reduced susceptance matrix excluding the slack bus.
///
/// The susceptance matrix B has elements:
/// - `B[i][i] = Σ(1/X_ij)` for all branches connected to bus i
/// - `B[i][j] = -1/X_ij` for branches between buses i and j
pub fn build_reduced_susceptance(
    system: &System,
    non_slack_bus_positions: &HashMap<BusId, usize>,
) -> Vec<Vec<f64>> {
    let size = non_slack_bus_positions.len();
    let mut matrix = vec![vec![0.0; size]; size];

    for branch in &system.branches {
        let susceptance = 1.0 / branch.reactance_pu;
        let from_pos = non_slack_bus_positions.get(&branch.from_bus_id).copied();
        let to_pos = non_slack_bus_positions.get(&branch.to_bus_id).copied();

        match (from_pos, to_pos) {
            (Some(i), Some(j)) => {
                matrix[i][i] += susceptance;
                matrix[j][j] += susceptance;
                matrix[i][j] -= susceptance;
                matrix[j][i] -= susceptance;
            }
            (Some(i), None) => {
                matrix[i][i] += susceptance;
            }
            (None, Some(j)) => {
                matrix[j][j] += susceptance;
            }
            (None, None) => {}
        }
    }

    matrix
}

/// Build the diagonal branch susceptance matrix.
///
/// Returns a diagonal matrix where `D[k][k] = 1/X_k` for branch k.
pub fn build_branch_susceptance_diag(system: &System) -> Vec<Vec<f64>> {
    let size = system.branches.len();
    let mut matrix = vec![vec![0.0; size]; size];

    for (branch_idx, branch) in system.branches.iter().enumerate() {
        matrix[branch_idx][branch_idx] = 1.0 / branch.reactance_pu;
    }

    matrix
}

/// Build the reduced incidence matrix excluding the slack bus.
///
/// The incidence matrix A has elements:
/// - `A[k][i] = +1` if branch k leaves bus i
/// - `A[k][i] = -1` if branch k enters bus i
/// - `A[k][i] = 0` otherwise
pub fn build_reduced_incidence(
    system: &System,
    non_slack_bus_positions: &HashMap<BusId, usize>,
) -> Vec<Vec<f64>> {
    let row_count = system.branches.len();
    let column_count = non_slack_bus_positions.len();
    let mut matrix = vec![vec![0.0; column_count]; row_count];

    for (branch_idx, branch) in system.branches.iter().enumerate() {
        if let Some(from_pos) = non_slack_bus_positions.get(&branch.from_bus_id) {
            matrix[branch_idx][*from_pos] = 1.0;
        }
        if let Some(to_pos) = non_slack_bus_positions.get(&branch.to_bus_id) {
            matrix[branch_idx][*to_pos] = -1.0;
        }
    }

    matrix
}
