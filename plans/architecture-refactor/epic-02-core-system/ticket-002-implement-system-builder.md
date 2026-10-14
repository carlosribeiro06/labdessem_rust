# [002] Implement SystemBuilder

## Context

### Background

Currently, `System` is constructed directly by populating fields. The Builder
pattern provides a cleaner API, allows validation at construction time, and
makes the `System` effectively immutable after creation.

### Relation to Epic

Second ticket in Epic 02. Implements the core builder pattern.

### Current State

Systems are constructed directly in `labdessem-io/src/study.rs` by populating
all fields.

## Specification

### Requirements

1. Implement `SystemBuilder` in `system/builder.rs`
2. Provide `with_*` methods for each entity collection
3. Provide `with_flags` method for configuration flags
4. `build()` method returns `Result<System, ValidationError>`
5. `System` fields should remain `pub` for reading but construction goes through builder

### Outputs/Behavior

```rust
let system = SystemBuilder::new(horizon)
    .with_submarkets(submarkets)
    .with_buses(buses)
    .with_branches(branches)
    .with_thermal_plants(thermals)
    .with_hydro_plants(hydros)
    .with_pumping_plants(pumpings)
    .with_renewable_plants(renewables)
    .with_operational_limits(limits)
    .with_interchange_limits(interchange)
    .with_flags(uct, uch, ton_residual, fpha, fcf)
    .build()?;
```

## Acceptance Criteria

- [ ] `SystemBuilder` struct exists in `system/builder.rs`
- [ ] All `with_*` methods implemented
- [ ] `build()` returns `Result<System, ValidationError>`
- [ ] `cargo check --all-targets` passes
- [ ] `cargo test` passes

## Implementation Guide

### Suggested Approach

```rust
// system/builder.rs

use super::{System, StudyHorizon, /* ... */};
use crate::entities::*;
use crate::error::ValidationError;

#[derive(Debug, Default)]
pub struct SystemBuilder {
    horizon: Option<StudyHorizon>,
    submarkets: Vec<Submarket>,
    buses: Vec<Bus>,
    branches: Vec<Branch>,
    thermal_plants: Vec<ThermalPlant>,
    hydro_plants: Vec<HydroPlant>,
    pumping_plants: Vec<PumpingPlant>,
    renewable_plants: Vec<RenewablePlant>,
    operational_limits: Vec<OperationalLimit>,
    interchange_limits: Vec<InterchangeLimit>,
    residual_costs: Vec<ResidualCost>,
    future_cost_cuts: Vec<FutureCostCut>,
    
    // Flags
    thermal_unit_commitment_enabled: bool,
    hydro_unit_commitment_enabled: bool,
    ton_residual_enabled: bool,
    fpha_enabled: bool,
    future_cost_enabled: bool,
}

impl SystemBuilder {
    pub fn new(horizon: StudyHorizon) -> Self {
        Self {
            horizon: Some(horizon),
            ..Default::default()
        }
    }

    pub fn with_submarkets(mut self, submarkets: Vec<Submarket>) -> Self {
        self.submarkets = submarkets;
        self
    }

    // ... other with_* methods

    pub fn with_flags(
        mut self,
        uct: bool,
        uch: bool,
        ton_residual: bool,
        fpha: bool,
        fcf: bool,
    ) -> Self {
        self.thermal_unit_commitment_enabled = uct;
        self.hydro_unit_commitment_enabled = uch;
        self.ton_residual_enabled = ton_residual;
        self.fpha_enabled = fpha;
        self.future_cost_enabled = fcf;
        self
    }

    pub fn build(self) -> Result<System, ValidationError> {
        let horizon = self.horizon.ok_or_else(|| {
            ValidationError::MissingField {
                entity_type: "System",
                entity_id: 0,
                field_name: "horizon",
            }
        })?;

        let system = System {
            horizon,
            submarkets: self.submarkets,
            buses: self.buses,
            // ... all other fields
        };

        // Validation will be added in ticket-003
        // super::validate::validate_system(&system)?;

        Ok(system)
    }
}
```

### Key Files to Modify

- `crates/labdessem-core/src/system/mod.rs` - add `mod builder;` and re-export
- `crates/labdessem-core/src/lib.rs` - re-export `SystemBuilder`

### Key Files to Create

- `crates/labdessem-core/src/system/builder.rs`

### Pitfalls to Avoid

- Don't add validation yet (that's ticket-003)
- Keep `System` fields public for reading
- Make sure all existing entity types are supported

## Testing Requirements

### Unit Tests

Add tests in `builder.rs`:
```rust
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn builder_requires_horizon() {
        let result = SystemBuilder::default().build();
        assert!(result.is_err());
    }

    #[test]
    fn builder_creates_system_with_horizon() {
        let horizon = /* create minimal horizon */;
        let result = SystemBuilder::new(horizon).build();
        assert!(result.is_ok());
    }
}
```

## Dependencies

- **Blocked By**: 001
- **Blocks**: 003, 004

## Effort Estimate

**Points**: 2
**Confidence**: High
