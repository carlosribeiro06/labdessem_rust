# [003] Implement validation module

## Context

### Background

Validation logic is currently mixed with construction in `system.rs` and across
`labdessem-io`. Centralizing validation in a dedicated module improves testability
and makes validation rules explicit.

### Relation to Epic

Third ticket in Epic 02. Creates the validation infrastructure.

### Current State

Validation is scattered:
- Some checks happen during CSV parsing
- Some checks happen during System construction
- Error messages are generic strings

## Specification

### Requirements

1. Create `validate_system()` function in `system/validate.rs`
2. Implement validation helpers for:
   - Duplicate ID detection
   - Reference validation (e.g., bus_id exists)
   - Hydro cascade cycle detection
   - Dimension matching (arrays match horizon.periods)
3. Call validation from `SystemBuilder::build()`

### Outputs/Behavior

```rust
pub fn validate_system(system: &System) -> Result<(), ValidationError> {
    validate_no_duplicate_ids(system)?;
    validate_references(system)?;
    validate_hydro_cascade(system)?;
    validate_dimensions(system)?;
    Ok(())
}
```

## Acceptance Criteria

- [ ] `validate_system()` exists and is called from `build()`
- [ ] At least 4 validation checks implemented
- [ ] Invalid systems return appropriate `ValidationError`
- [ ] `cargo test` passes

## Implementation Guide

### Suggested Approach

```rust
// system/validate.rs

use super::System;
use crate::error::ValidationError;
use std::collections::HashSet;

pub fn validate_system(system: &System) -> Result<(), ValidationError> {
    validate_no_duplicate_ids(system)?;
    validate_references(system)?;
    validate_hydro_cascade(system)?;
    validate_dimensions(system)?;
    Ok(())
}

fn validate_no_duplicate_ids(system: &System) -> Result<(), ValidationError> {
    // Check thermal plant IDs
    let mut seen = HashSet::new();
    for plant in &system.thermal_plants {
        if !seen.insert(plant.id.0) {
            return Err(ValidationError::DuplicateId {
                entity_type: "ThermalPlant",
                id: plant.id.0,
            });
        }
    }
    // ... repeat for other entity types
    Ok(())
}

fn validate_references(system: &System) -> Result<(), ValidationError> {
    // Validate submarket_id references
    let submarket_ids: HashSet<_> = system.submarkets.iter().map(|s| s.id.0).collect();
    
    for plant in &system.thermal_plants {
        if !submarket_ids.contains(&plant.submarket_id.0) {
            return Err(ValidationError::InvalidReference {
                source_entity_type: "ThermalPlant",
                source_id: plant.id.0,
                field_name: "submarket_id",
                referenced_id: plant.submarket_id.0,
                expected_type: "Submarket",
            });
        }
    }
    // ... validate other references
    Ok(())
}

fn validate_hydro_cascade(system: &System) -> Result<(), ValidationError> {
    // Detect cycles in hydro upstream relationships
    // ... implementation
    Ok(())
}

fn validate_dimensions(system: &System) -> Result<(), ValidationError> {
    let periods = system.horizon.periods;
    
    for submarket in &system.submarkets {
        if submarket.demand_mw.len() != periods {
            return Err(ValidationError::DimensionMismatch {
                entity_type: "Submarket",
                entity_id: submarket.id.0,
                field_name: "demand_mw",
                expected: periods,
                actual: submarket.demand_mw.len(),
            });
        }
    }
    // ... check other arrays
    Ok(())
}
```

### Key Files to Modify

- `crates/labdessem-core/src/system/mod.rs` - add `mod validate;`
- `crates/labdessem-core/src/system/builder.rs` - call `validate_system()`

### Key Files to Create

- `crates/labdessem-core/src/system/validate.rs`

## Testing Requirements

### Unit Tests

```rust
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn detects_duplicate_thermal_ids() {
        // Create system with duplicate thermal plant IDs
        // Assert ValidationError::DuplicateId is returned
    }

    #[test]
    fn detects_invalid_submarket_reference() {
        // Create thermal plant with non-existent submarket_id
        // Assert ValidationError::InvalidReference is returned
    }
}
```

## Dependencies

- **Blocked By**: 002 (builder must exist)
- **Blocks**: 004

## Effort Estimate

**Points**: 2
**Confidence**: Medium
