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

use crate::error::NetworkError;

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

/// Invert a square matrix using Gaussian elimination.
///
/// # Errors
///
/// Returns `NetworkError::NonSquareMatrix` if the matrix is not square.
/// Returns `NetworkError::SingularMatrix` if the matrix is singular.
pub fn invert_matrix(matrix: &[Vec<f64>]) -> Result<Vec<Vec<f64>>, NetworkError> {
    let n = matrix.len();
    if n == 0 {
        return Ok(Vec::new());
    }

    if matrix.iter().any(|row| row.len() != n) {
        return Err(NetworkError::NonSquareMatrix);
    }

    let mut inverse = vec![vec![0.0; n]; n];
    for column in 0..n {
        let mut rhs = vec![0.0; n];
        rhs[column] = 1.0;
        let solution = solve_linear_system(matrix, &rhs)?;
        for row in 0..n {
            inverse[row][column] = solution[row];
        }
    }

    Ok(inverse)
}

/// Multiply two matrices.
///
/// # Errors
///
/// Returns `NetworkError::DimensionMismatch` if the matrices have incompatible dimensions.
pub fn multiply_matrices(
    left: &[Vec<f64>],
    right: &[Vec<f64>],
) -> Result<Vec<Vec<f64>>, NetworkError> {
    if left.is_empty() || right.is_empty() {
        return Ok(Vec::new());
    }

    let left_columns = left[0].len();
    let right_rows = right.len();
    let right_columns = right[0].len();

    if left.iter().any(|row| row.len() != left_columns)
        || right.iter().any(|row| row.len() != right_columns)
    {
        return Err(NetworkError::DimensionMismatch(
            "matrices must be rectangular".into(),
        ));
    }

    if left_columns != right_rows {
        return Err(NetworkError::DimensionMismatch(format!(
            "left is {}x{}, right is {}x{}",
            left.len(),
            left_columns,
            right_rows,
            right_columns
        )));
    }

    let mut product = vec![vec![0.0; right_columns]; left.len()];
    for (row_idx, left_row) in left.iter().enumerate() {
        for column_idx in 0..right_columns {
            let mut value = 0.0;
            for k in 0..left_columns {
                value += left_row[k] * right[k][column_idx];
            }
            product[row_idx][column_idx] = value;
        }
    }

    Ok(product)
}

/// Solve a linear system using Gaussian elimination with partial pivoting.
///
/// # Errors
///
/// Returns `NetworkError::InconsistentDimensions` if dimensions don't match.
/// Returns `NetworkError::SingularMatrix` if the matrix is singular.
pub fn solve_linear_system(matrix: &[Vec<f64>], rhs: &[f64]) -> Result<Vec<f64>, NetworkError> {
    let n = matrix.len();
    if n == 0 {
        return Ok(Vec::new());
    }
    if rhs.len() != n {
        return Err(NetworkError::InconsistentDimensions);
    }

    let mut augmented = vec![vec![0.0; n + 1]; n];
    for row in 0..n {
        for col in 0..n {
            augmented[row][col] = matrix[row][col];
        }
        augmented[row][n] = rhs[row];
    }

    for pivot in 0..n {
        let mut best_row = pivot;
        let mut best_value = augmented[pivot][pivot].abs();
        #[allow(clippy::needless_range_loop)]
        for row in (pivot + 1)..n {
            let candidate = augmented[row][pivot].abs();
            if candidate > best_value {
                best_row = row;
                best_value = candidate;
            }
        }

        if best_value <= 1e-12 {
            return Err(NetworkError::SingularMatrix);
        }

        if best_row != pivot {
            augmented.swap(best_row, pivot);
        }

        let pivot_value = augmented[pivot][pivot];
        #[allow(clippy::needless_range_loop)]
        for col in pivot..=n {
            augmented[pivot][col] /= pivot_value;
        }

        for row in 0..n {
            if row == pivot {
                continue;
            }

            let factor = augmented[row][pivot];
            if factor.abs() <= 1e-12 {
                continue;
            }

            #[allow(clippy::needless_range_loop)]
            for col in pivot..=n {
                augmented[row][col] -= factor * augmented[pivot][col];
            }
        }
    }

    Ok((0..n).map(|row| augmented[row][n]).collect())
}

/// Compute the dot product of two vectors.
#[inline]
pub fn dot(left: &[f64], right: &[f64]) -> f64 {
    left.iter().zip(right.iter()).map(|(a, b)| a * b).sum()
}

/// Round a value to the specified number of decimal digits.
#[inline]
pub fn round_to_digits(value: f64, digits: u32) -> f64 {
    #[allow(clippy::cast_possible_wrap)]
    let factor = 10_f64.powi(digits as i32);
    (value * factor).round() / factor
}


#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_invert_2x2_matrix() {
        // Known: [[4, 7], [2, 6]] has inverse [[0.6, -0.7], [-0.2, 0.4]]
        let matrix = vec![vec![4.0, 7.0], vec![2.0, 6.0]];
        let inverse = invert_matrix(&matrix).expect("should invert");

        assert!((inverse[0][0] - 0.6).abs() < 1e-9);
        assert!((inverse[0][1] - (-0.7)).abs() < 1e-9);
        assert!((inverse[1][0] - (-0.2)).abs() < 1e-9);
        assert!((inverse[1][1] - 0.4).abs() < 1e-9);
    }

    #[test]
    fn test_invert_identity_returns_identity() {
        let identity = vec![vec![1.0, 0.0], vec![0.0, 1.0]];
        let inverse = invert_matrix(&identity).expect("should invert identity");

        assert!((inverse[0][0] - 1.0).abs() < 1e-9);
        assert!((inverse[0][1] - 0.0).abs() < 1e-9);
        assert!((inverse[1][0] - 0.0).abs() < 1e-9);
        assert!((inverse[1][1] - 1.0).abs() < 1e-9);
    }

    #[test]
    fn test_invert_singular_matrix_fails() {
        // Singular: [[1, 2], [2, 4]] (row2 = 2*row1)
        let singular = vec![vec![1.0, 2.0], vec![2.0, 4.0]];
        assert!(invert_matrix(&singular).is_err());
    }

    #[test]
    fn test_invert_empty_matrix() {
        let empty: Vec<Vec<f64>> = vec![];
        let result = invert_matrix(&empty).expect("empty inversion should succeed");
        assert!(result.is_empty());
    }

    #[test]
    fn test_multiply_matrices() {
        // [[1, 2], [3, 4]] × [[5, 6], [7, 8]] = [[19, 22], [43, 50]]
        let a = vec![vec![1.0, 2.0], vec![3.0, 4.0]];
        let b = vec![vec![5.0, 6.0], vec![7.0, 8.0]];
        let c = multiply_matrices(&a, &b).expect("should multiply");

        assert!((c[0][0] - 19.0).abs() < 1e-9);
        assert!((c[0][1] - 22.0).abs() < 1e-9);
        assert!((c[1][0] - 43.0).abs() < 1e-9);
        assert!((c[1][1] - 50.0).abs() < 1e-9);
    }

    #[test]
    fn test_multiply_dimension_mismatch() {
        let a = vec![vec![1.0, 2.0, 3.0]]; // 1x3
        let b = vec![vec![1.0, 2.0]]; // 1x2
        assert!(multiply_matrices(&a, &b).is_err());
    }

    #[test]
    fn test_solve_linear_system() {
        // [[2, 1], [1, 3]] × [x, y] = [5, 5] → x=2, y=1
        let matrix = vec![vec![2.0, 1.0], vec![1.0, 3.0]];
        let rhs = vec![5.0, 5.0];
        let solution = solve_linear_system(&matrix, &rhs).expect("should solve");

        assert!((solution[0] - 2.0).abs() < 1e-9);
        assert!((solution[1] - 1.0).abs() < 1e-9);
    }

    #[test]
    fn test_dot_product() {
        let a = vec![1.0, 2.0, 3.0];
        let b = vec![4.0, 5.0, 6.0];
        // 1*4 + 2*5 + 3*6 = 32
        assert!((dot(&a, &b) - 32.0).abs() < 1e-9);
    }

    #[test]
    fn test_round_to_digits() {
        assert!((round_to_digits(1.2345, 2) - 1.23).abs() < 1e-9);
        assert!((round_to_digits(1.2355, 2) - 1.24).abs() < 1e-9);
        assert!((round_to_digits(-1.2345, 2) - (-1.23)).abs() < 1e-9);
    }
}
