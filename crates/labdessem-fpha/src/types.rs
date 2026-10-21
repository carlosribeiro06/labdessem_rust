//! Shared types for FPHA fitting.

/// Tailrace model for computing downstream water level.
#[derive(Debug, Clone, PartialEq)]
pub enum TailraceModel {
    /// Constant tailrace level (m).
    Constant { level_m: f64 },
    /// Polynomial tailrace: level = Σ coef[i] * q^i
    Polynomial { coefficients: Vec<f64> },
}

impl TailraceModel {
    /// Evaluate tailrace level (m) at outflow (m³/s).
    pub fn evaluate(&self, outflow_m3s: f64) -> f64 {
        match self {
            Self::Constant { level_m } => *level_m,
            Self::Polynomial { coefficients } => {
                let mut level = 0.0;
                let mut q_power = 1.0;
                for coef in coefficients {
                    level += coef * q_power;
                    q_power *= outflow_m3s;
                }
                level
            }
        }
    }
}
