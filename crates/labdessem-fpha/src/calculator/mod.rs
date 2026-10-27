//! Maximum swallowing (EngMax) calculator.

/// Calculate dynamic maximum swallowing from head ratio.
///
/// Formula: Q_ef × (h_l / h_ef)^α
///
/// # Arguments
/// * `max_turbining_m3s` - Maximum turbining at effective head (Q_ef) in m³/s
/// * `effective_head_m` - Effective head (h_ef) in meters
/// * `net_head_m` - Net head (h_l) in meters
/// * `alpha` - Turbine curve exponent (α)
///
/// # Returns
/// Maximum swallowing (EngMax) in m³/s
#[must_use]
pub fn calculate_max_swallowing(
    max_turbining_m3s: f64,
    effective_head_m: f64,
    net_head_m: f64,
    alpha: f64,
) -> f64 {
    if effective_head_m <= 0.0 {
        return max_turbining_m3s;
    }
    if net_head_m <= 0.0 {
        tracing::warn!(net_head_m, "near-zero net head, returning zero EngMax");
        return 0.0;
    }
    max_turbining_m3s * (net_head_m / effective_head_m).powf(alpha)
}

#[cfg(test)]
mod tests {
    use super::*;

    const EPSILON: f64 = 1e-2;

    #[test]
    fn normal_case_francis_alpha() {
        // Q_ef=100, h_ef=50, h_l=40, α=0.5
        // Expected: 100 × (40/50)^0.5 = 100 × 0.8944... ≈ 89.44
        let result = calculate_max_swallowing(100.0, 50.0, 40.0, 0.5);
        assert!((result - 89.44).abs() < EPSILON);
    }

    #[test]
    fn net_head_exceeds_effective_head() {
        // Q_ef=100, h_ef=50, h_l=60, α=0.5
        // Expected: 100 × (60/50)^0.5 = 100 × 1.0954... ≈ 109.54
        let result = calculate_max_swallowing(100.0, 50.0, 60.0, 0.5);
        assert!((result - 109.54).abs() < EPSILON);
    }

    #[test]
    fn effective_head_zero_fallback() {
        // h_ef=0 → return Q_ef unchanged
        let result = calculate_max_swallowing(100.0, 0.0, 40.0, 0.5);
        assert!((result - 100.0).abs() < f64::EPSILON);
    }

    #[test]
    fn net_head_zero_fallback() {
        // h_l=0 → return 0.0 with warning
        let result = calculate_max_swallowing(100.0, 50.0, 0.0, 0.5);
        assert!((result - 0.0).abs() < f64::EPSILON);
    }

    #[test]
    fn normal_case_kaplan_alpha() {
        // Q_ef=100, h_ef=50, h_l=40, α=0.2
        // Expected: 100 × (40/50)^0.2 = 100 × 0.9564... ≈ 95.64
        let result = calculate_max_swallowing(100.0, 50.0, 40.0, 0.2);
        assert!((result - 95.64).abs() < EPSILON);
    }
}
