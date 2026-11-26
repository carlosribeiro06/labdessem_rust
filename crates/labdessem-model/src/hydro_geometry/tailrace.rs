//! Tailrace rating polynomial.

/// Tailrace level polynomial: `level(q) = Σ coefficients[i] · q^i`, level in m, outflow `q` in m³/s.
#[derive(Debug, Clone, PartialEq)]
pub struct TailraceModel {
    /// Polynomial coefficients, constant term first.
    pub coefficients: Vec<f64>,
}

impl TailraceModel {
    /// Evaluate tailrace level (m) at outflow (m³/s).
    pub fn evaluate(&self, outflow_m3s: f64) -> f64 {
        let mut level = 0.0;
        let mut q_power = 1.0;
        for coef in &self.coefficients {
            level += coef * q_power;
            q_power *= outflow_m3s;
        }
        level
    }
}
