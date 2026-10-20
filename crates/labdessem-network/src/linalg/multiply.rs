//! Matrix multiplication.

use crate::error::NetworkError;

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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_multiply_2x2_matrices() {
        let a = vec![vec![1.0, 2.0], vec![3.0, 4.0]];
        let b = vec![vec![5.0, 6.0], vec![7.0, 8.0]];
        let result = multiply_matrices(&a, &b).unwrap();
        assert_eq!(result, vec![vec![19.0, 22.0], vec![43.0, 50.0]]);
    }

    #[test]
    fn test_multiply_identity() {
        let a = vec![vec![1.0, 2.0], vec![3.0, 4.0]];
        let i = vec![vec![1.0, 0.0], vec![0.0, 1.0]];
        let result = multiply_matrices(&a, &i).unwrap();
        assert_eq!(result, a);
    }

    #[test]
    fn test_multiply_dimension_mismatch() {
        let a = vec![vec![1.0, 2.0, 3.0]]; // 1x3
        let b = vec![vec![1.0, 2.0]]; // 1x2
        assert!(multiply_matrices(&a, &b).is_err());
    }

    #[test]
    fn test_multiply_empty() {
        let empty: Vec<Vec<f64>> = vec![];
        let result = multiply_matrices(&empty, &empty).unwrap();
        assert!(result.is_empty());
    }
}
