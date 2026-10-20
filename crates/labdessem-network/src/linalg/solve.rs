//! Linear system solver using Gaussian elimination.

use crate::error::NetworkError;

/// Solve a linear system Ax = b using Gaussian elimination with partial pivoting.
///
/// # Errors
///
/// Returns `NetworkError::InconsistentDimensions` if dimensions don't match.
/// Returns `NetworkError::SingularMatrix` if the matrix is singular.
#[allow(clippy::needless_pass_by_value)]
pub fn solve_linear_system(matrix: Vec<Vec<f64>>, rhs: Vec<f64>) -> Result<Vec<f64>, NetworkError> {
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_solve_linear_system() {
        let matrix = vec![vec![2.0, 1.0], vec![1.0, 3.0]];
        let rhs = vec![5.0, 5.0];
        let solution = solve_linear_system(matrix, rhs).expect("should solve");

        assert!((solution[0] - 2.0).abs() < 1e-9);
        assert!((solution[1] - 1.0).abs() < 1e-9);
    }

    #[test]
    fn test_solve_singular_fails() {
        let matrix = vec![vec![1.0, 2.0], vec![2.0, 4.0]];
        let rhs = vec![3.0, 6.0];
        assert!(solve_linear_system(matrix, rhs).is_err());
    }

    #[test]
    fn test_solve_empty() {
        let matrix: Vec<Vec<f64>> = vec![];
        let rhs: Vec<f64> = vec![];
        let result = solve_linear_system(matrix, rhs).expect("empty should succeed");
        assert!(result.is_empty());
    }
}
