//! Coefficient-sign and α validation of the fitted plane set.

use crate::error::FphaFittingError;
use super::hull_fit::RawPlane;

const SIGN_TOLERANCE: f64 = 1e-10;

/// Validate a fitted plane set and its α correction factor.
///
/// # Errors
///
/// | Condition | Error variant |
/// |-----------|---------------|
/// | `planes` is empty | `NoHyperplanesProduced` |
/// | `alpha <= 0` | `NonPositiveAlpha` |
/// | `gamma_v < -1e-10` | `InvalidCoefficient` |
/// | `gamma_q < -1e-10` | `InvalidCoefficient` |
/// | `gamma_s > 1e-10` | `InvalidCoefficient` |
pub fn validate_fitted_planes(
    planes: &[RawPlane],
    alpha: f64,
    hydro_name: &str,
) -> Result<(), FphaFittingError> {
    if planes.is_empty() {
        return Err(FphaFittingError::NoHyperplanesProduced {
            hydro_name: hydro_name.to_owned(),
        });
    }

    if alpha <= 0.0 {
        return Err(FphaFittingError::NonPositiveAlpha {
            hydro_name: hydro_name.to_owned(),
            alpha,
        });
    }

    for (idx, plane) in planes.iter().enumerate() {
        if plane.gamma_v < -SIGN_TOLERANCE {
            return Err(FphaFittingError::InvalidCoefficient {
                hydro_name: hydro_name.to_owned(),
                plane_index: idx,
                detail: format!(
                    "gamma_v={:.6e} must be >= 0 (more storage should increase production)",
                    plane.gamma_v
                ),
            });
        }
        if plane.gamma_q < -SIGN_TOLERANCE {
            return Err(FphaFittingError::InvalidCoefficient {
                hydro_name: hydro_name.to_owned(),
                plane_index: idx,
                detail: format!(
                    "gamma_q={:.6e} must be >= 0 (turbined flow should produce power)",
                    plane.gamma_q
                ),
            });
        }
        if plane.gamma_s > SIGN_TOLERANCE {
            return Err(FphaFittingError::InvalidCoefficient {
                hydro_name: hydro_name.to_owned(),
                plane_index: idx,
                detail: format!(
                    "gamma_s={:.6e} must be <= 0 (spillage should not increase production)",
                    plane.gamma_s
                ),
            });
        }
    }

    Ok(())
}

#[cfg(test)]
#[allow(clippy::unwrap_used, clippy::expect_used)]
mod tests {
    use super::*;

    fn plane(gamma_0: f64, gamma_v: f64, gamma_q: f64, gamma_s: f64) -> RawPlane {
        RawPlane { gamma_0, gamma_v, gamma_q, gamma_s }
    }

    #[test]
    fn valid_planes_pass() {
        let planes = vec![
            plane(100.0, 0.001, 0.5, -0.01),
            plane(50.0, 0.0, 0.3, 0.0),
        ];
        let result = validate_fitted_planes(&planes, 0.95, "ValidHydro");
        assert!(result.is_ok());
    }

    #[test]
    fn empty_planes_fails() {
        let result = validate_fitted_planes(&[], 1.0, "EmptyHydro");
        assert!(matches!(result, Err(FphaFittingError::NoHyperplanesProduced { .. })));
    }

    #[test]
    fn zero_alpha_fails() {
        let planes = vec![plane(100.0, 0.001, 0.5, -0.01)];
        let result = validate_fitted_planes(&planes, 0.0, "ZeroAlpha");
        assert!(matches!(result, Err(FphaFittingError::NonPositiveAlpha { .. })));
    }

    #[test]
    fn negative_alpha_fails() {
        let planes = vec![plane(100.0, 0.001, 0.5, -0.01)];
        let result = validate_fitted_planes(&planes, -0.5, "NegativeAlpha");
        assert!(matches!(result, Err(FphaFittingError::NonPositiveAlpha { .. })));
    }

    #[test]
    fn negative_gamma_v_fails() {
        let planes = vec![plane(100.0, -0.001, 0.5, -0.01)];
        let result = validate_fitted_planes(&planes, 1.0, "NegGammaV");
        assert!(matches!(result, Err(FphaFittingError::InvalidCoefficient { .. })));
    }

    #[test]
    fn negative_gamma_q_fails() {
        let planes = vec![plane(100.0, 0.001, -0.001, -0.01)];
        let result = validate_fitted_planes(&planes, 1.0, "NegGammaQ");
        assert!(matches!(result, Err(FphaFittingError::InvalidCoefficient { .. })));
    }

    #[test]
    fn positive_gamma_s_fails() {
        let planes = vec![plane(100.0, 0.001, 0.5, 0.001)];
        let result = validate_fitted_planes(&planes, 1.0, "PosGammaS");
        assert!(matches!(result, Err(FphaFittingError::InvalidCoefficient { .. })));
    }

    #[test]
    fn within_tolerance_passes() {
        let planes = vec![plane(100.0, -1e-11, -1e-11, 1e-11)];
        let result = validate_fitted_planes(&planes, 1.0, "Tolerance");
        assert!(result.is_ok());
    }

    #[test]
    fn exactly_at_tolerance_fails() {
        let planes = vec![plane(100.0, -1e-10 - 1e-15, 0.5, -0.01)];
        let result = validate_fitted_planes(&planes, 1.0, "AtTolerance");
        assert!(matches!(result, Err(FphaFittingError::InvalidCoefficient { .. })));
    }
}
