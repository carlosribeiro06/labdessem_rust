//! Linear algebra utility functions.

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
    fn test_dot_product() {
        let a = vec![1.0, 2.0, 3.0];
        let b = vec![4.0, 5.0, 6.0];
        assert!((dot(&a, &b) - 32.0).abs() < 1e-9);
    }

    #[test]
    fn test_dot_empty() {
        let a: Vec<f64> = vec![];
        let b: Vec<f64> = vec![];
        assert!((dot(&a, &b) - 0.0).abs() < 1e-9);
    }

    #[test]
    fn test_round_to_digits() {
        assert!((round_to_digits(1.2345, 2) - 1.23).abs() < 1e-9);
        assert!((round_to_digits(1.2355, 2) - 1.24).abs() < 1e-9);
    }

    #[test]
    fn test_round_to_digits_negative() {
        assert!((round_to_digits(-1.2345, 2) - (-1.23)).abs() < 1e-9);
    }

    #[test]
    fn test_round_to_digits_zero_digits() {
        assert!((round_to_digits(3.14159, 0) - 3.0).abs() < 1e-9);
    }
}
