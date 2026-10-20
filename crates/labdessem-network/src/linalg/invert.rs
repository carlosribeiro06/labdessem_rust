//! Matrix inversion using Gaussian elimination.

use crate::error::NetworkError;
use crate::linalg::solve::solve_linear_system;

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
        let solution = solve_linear_system(matrix.to_vec(), rhs)?;
        for row in 0..n {
            inverse[row][column] = solution[row];
        }
    }

    Ok(inverse)
}

#[cfg(test)]
mod tests {
    use super::*;

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
    fn test_invert_2x2_matrix() {
        let matrix = vec![vec![4.0, 7.0], vec![2.0, 6.0]];
        let inverse = invert_matrix(&matrix).expect("should invert");

        assert!((inverse[0][0] - 0.6).abs() < 1e-9);
        assert!((inverse[0][1] - (-0.7)).abs() < 1e-9);
        assert!((inverse[1][0] - (-0.2)).abs() < 1e-9);
        assert!((inverse[1][1] - 0.4).abs() < 1e-9);
    }

    #[test]
    fn test_invert_singular_matrix_fails() {
        let singular = vec![vec![1.0, 2.0], vec![2.0, 4.0]];
        assert!(invert_matrix(&singular).is_err());
    }

    #[test]
    fn test_invert_empty_matrix() {
        let empty: Vec<Vec<f64>> = vec![];
        let result = invert_matrix(&empty).expect("empty inversion should succeed");
        assert!(result.is_empty());
    }
}
