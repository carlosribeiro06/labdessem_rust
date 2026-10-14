# [004] Create typed ValidationError

## Context

### Background

The current `CoreError::InvalidData(String)` provides no structure for error
handling. Typed errors with specific variants enable pattern matching, better
error messages, and programmatic error handling.

### Relation to Epic

Final ticket in Epic 02. Completes the error handling improvement.

### Current State

```rust
// error.rs
pub enum CoreError {
    InvalidData(String),
}
```

## Specification

### Requirements

1. Replace or extend `CoreError` with typed `ValidationError` enum
2. Include variants for all validation failure types
3. Implement `Display` with descriptive messages
4. Implement `std::error::Error`

### Outputs/Behavior

```rust
pub enum ValidationError {
    MissingField { entity_type, entity_id, field_name },
    InvalidReference { source_entity_type, source_id, field_name, referenced_id, expected_type },
    DuplicateId { entity_type, id },
    HydroCascadeCycle { cycle_plant_ids },
    DimensionMismatch { entity_type, entity_id, field_name, expected, actual },
    ValueOutOfRange { entity_type, entity_id, field_name, value, min, max },
    InvalidThermalTrajectory { plant_name, unit_id, trajectory_type, reason },
}
```

## Acceptance Criteria

- [ ] `ValidationError` enum exists with at least 5 variants
- [ ] `Display` implementation provides descriptive messages
- [ ] `std::error::Error` is implemented
- [ ] All validation functions use `ValidationError`
- [ ] `cargo test` passes

## Implementation Guide

### Suggested Approach

```rust
// error.rs

use std::fmt;

#[derive(Debug, Clone)]
pub enum ValidationError {
    MissingField {
        entity_type: &'static str,
        entity_id: usize,
        field_name: &'static str,
    },
    
    InvalidReference {
        source_entity_type: &'static str,
        source_id: usize,
        field_name: &'static str,
        referenced_id: usize,
        expected_type: &'static str,
    },
    
    DuplicateId {
        entity_type: &'static str,
        id: usize,
    },
    
    HydroCascadeCycle {
        cycle_plant_ids: Vec<usize>,
    },
    
    DimensionMismatch {
        entity_type: &'static str,
        entity_id: usize,
        field_name: &'static str,
        expected: usize,
        actual: usize,
    },
    
    ValueOutOfRange {
        entity_type: &'static str,
        entity_id: usize,
        field_name: &'static str,
        value: f64,
        min: Option<f64>,
        max: Option<f64>,
    },
    
    InvalidThermalTrajectory {
        plant_name: String,
        unit_id: usize,
        trajectory_type: &'static str,
        reason: String,
    },
}

impl fmt::Display for ValidationError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::MissingField { entity_type, entity_id, field_name } => {
                write!(f, "{entity_type} {entity_id}: missing required field '{field_name}'")
            }
            Self::InvalidReference { 
                source_entity_type, source_id, field_name, referenced_id, expected_type 
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
                let ids: Vec<_> = cycle_plant_ids.iter().map(|id| id.to_string()).collect();
                write!(f, "hydro cascade contains cycle: [{}]", ids.join(" -> "))
            }
            Self::DimensionMismatch { 
                entity_type, entity_id, field_name, expected, actual 
            } => {
                write!(
                    f,
                    "{entity_type} {entity_id}: field '{field_name}' has {actual} elements, \
                     expected {expected}"
                )
            }
            Self::ValueOutOfRange { 
                entity_type, entity_id, field_name, value, min, max 
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
                plant_name, unit_id, trajectory_type, reason 
            } => {
                write!(
                    f,
                    "thermal plant '{plant_name}' unit {unit_id}: invalid {trajectory_type} \
                     trajectory: {reason}"
                )
            }
        }
    }
}

impl std::error::Error for ValidationError {}
```

### Key Files to Modify

- `crates/labdessem-core/src/error.rs` - replace/extend with `ValidationError`
- `crates/labdessem-core/src/lib.rs` - re-export `ValidationError`
- `crates/labdessem-core/src/system/validate.rs` - use `ValidationError`
- `crates/labdessem-core/src/system/builder.rs` - use `ValidationError`

### Pitfalls to Avoid

- Keep `CoreError` if other parts of the codebase use it
- Consider making `ValidationError` a separate type from `CoreError`

## Testing Requirements

### Unit Tests

```rust
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
}
```

## Dependencies

- **Blocked By**: 003
- **Blocks**: Epic 03 (IO readers will use ValidationError)

## Effort Estimate

**Points**: 2
**Confidence**: High
