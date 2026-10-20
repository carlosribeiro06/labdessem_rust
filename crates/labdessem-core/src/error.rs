//! Error types for labdessem-core.

use std::{error::Error, fmt};

/// Generic error type for core operations.
///
/// This is the current error type used throughout the codebase.
/// For new code, prefer using [`ValidationError`] for more structured errors.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum CoreError {
    Validation(String),
}

impl CoreError {
    pub fn validation(message: impl Into<String>) -> Self {
        Self::Validation(message.into())
    }
}

impl fmt::Display for CoreError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Validation(message) => write!(f, "{message}"),
        }
    }
}

impl Error for CoreError {}

/// Typed validation error with structured context.
///
/// This error type provides more detailed information about validation
/// failures, enabling pattern matching and better error messages.
#[derive(Debug, Clone)]
pub enum ValidationError {
    /// A required field is missing.
    MissingField {
        entity_type: &'static str,
        entity_id: usize,
        field_name: &'static str,
    },

    /// A cross-reference points to a non-existent entity.
    InvalidReference {
        source_entity_type: &'static str,
        source_id: usize,
        field_name: &'static str,
        referenced_id: usize,
        expected_type: &'static str,
    },

    /// Duplicate ID found in a collection.
    DuplicateId {
        entity_type: &'static str,
        id: usize,
    },

    /// Hydro cascade contains a cycle.
    HydroCascadeCycle { cycle_plant_ids: Vec<usize> },

    /// Array dimension does not match expected size.
    DimensionMismatch {
        entity_type: &'static str,
        entity_id: usize,
        field_name: &'static str,
        expected: usize,
        actual: usize,
    },

    /// A value is outside its valid range.
    ValueOutOfRange {
        entity_type: &'static str,
        entity_id: usize,
        field_name: &'static str,
        value: f64,
        min: Option<f64>,
        max: Option<f64>,
    },

    /// Thermal trajectory is invalid.
    InvalidThermalTrajectory {
        plant_name: String,
        unit_id: usize,
        trajectory_type: &'static str,
        reason: String,
    },

    /// Generic validation error (for migration from CoreError).
    Generic(String),
}

impl fmt::Display for ValidationError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::MissingField {
                entity_type,
                entity_id,
                field_name,
            } => {
                write!(
                    f,
                    "{entity_type} {entity_id}: missing required field '{field_name}'"
                )
            }
            Self::InvalidReference {
                source_entity_type,
                source_id,
                field_name,
                referenced_id,
                expected_type,
            } => {
                write!(
                    f,
                    "{source_entity_type} {source_id}: field '{field_name}' references \
                     non-existent {expected_type} {referenced_id}"
                )
            }
            Self::DuplicateId { entity_type, id } => {
                write!(f, "duplicate {entity_type} id: {id}")
            }
            Self::HydroCascadeCycle { cycle_plant_ids } => {
                let ids: Vec<_> = cycle_plant_ids
                    .iter()
                    .map(std::string::ToString::to_string)
                    .collect();
                write!(f, "hydro cascade contains cycle: [{}]", ids.join(" -> "))
            }
            Self::DimensionMismatch {
                entity_type,
                entity_id,
                field_name,
                expected,
                actual,
            } => {
                write!(
                    f,
                    "{entity_type} {entity_id}: field '{field_name}' has {actual} elements, \
                     expected {expected}"
                )
            }
            Self::ValueOutOfRange {
                entity_type,
                entity_id,
                field_name,
                value,
                min,
                max,
            } => {
                let range = match (min, max) {
                    (Some(lo), Some(hi)) => format!("[{lo}, {hi}]"),
                    (Some(lo), None) => format!(">= {lo}"),
                    (None, Some(hi)) => format!("<= {hi}"),
                    (None, None) => "valid range".to_string(),
                };
                write!(
                    f,
                    "{entity_type} {entity_id}: field '{field_name}' value {value} \
                     is outside {range}"
                )
            }
            Self::InvalidThermalTrajectory {
                plant_name,
                unit_id,
                trajectory_type,
                reason,
            } => {
                write!(
                    f,
                    "thermal plant '{plant_name}' unit {unit_id}: invalid {trajectory_type} \
                     trajectory: {reason}"
                )
            }
            Self::Generic(message) => write!(f, "{message}"),
        }
    }
}

impl Error for ValidationError {}

impl From<CoreError> for ValidationError {
    fn from(err: CoreError) -> Self {
        match err {
            CoreError::Validation(msg) => ValidationError::Generic(msg),
        }
    }
}

impl From<ValidationError> for CoreError {
    fn from(err: ValidationError) -> Self {
        CoreError::Validation(err.to_string())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn display_formats_missing_field() {
        let err = ValidationError::MissingField {
            entity_type: "ThermalPlant",
            entity_id: 42,
            field_name: "name",
        };
        assert_eq!(
            err.to_string(),
            "ThermalPlant 42: missing required field 'name'"
        );
    }

    #[test]
    fn display_formats_invalid_reference() {
        let err = ValidationError::InvalidReference {
            source_entity_type: "ThermalPlant",
            source_id: 1,
            field_name: "submarket_id",
            referenced_id: 999,
            expected_type: "Submarket",
        };
        assert!(err.to_string().contains("non-existent Submarket 999"));
    }

    #[test]
    fn display_formats_duplicate_id() {
        let err = ValidationError::DuplicateId {
            entity_type: "HydroPlant",
            id: 5,
        };
        assert_eq!(err.to_string(), "duplicate HydroPlant id: 5");
    }

    #[test]
    fn display_formats_dimension_mismatch() {
        let err = ValidationError::DimensionMismatch {
            entity_type: "Submarket",
            entity_id: 1,
            field_name: "demand_mw",
            expected: 48,
            actual: 24,
        };
        assert!(err.to_string().contains("24 elements, expected 48"));
    }

    #[test]
    fn converts_from_core_error() {
        let core_err = CoreError::validation("test error");
        let val_err: ValidationError = core_err.into();
        assert!(matches!(val_err, ValidationError::Generic(_)));
    }

    #[test]
    fn converts_to_core_error() {
        let val_err = ValidationError::DuplicateId {
            entity_type: "Bus",
            id: 10,
        };
        let core_err: CoreError = val_err.into();
        assert!(core_err.to_string().contains("duplicate Bus id: 10"));
    }
}
