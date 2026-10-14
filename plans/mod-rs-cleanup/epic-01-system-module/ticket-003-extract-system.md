# ticket-003: Extract System Struct to system.rs

## Context

### Background
The `System` struct is the main type in the module. After extracting auxiliary types and helpers, the System struct and its `validate()` method should move to a dedicated file.

### Relation to Epic
Main extraction — the largest piece of code (~320 lines of validate logic).

### Current State
After tickets 001 and 002, `mod.rs` will still contain:
- `System` struct definition (~20 fields)
- `impl System { pub fn validate(&self) -> Result<(), CoreError> }` (~300 lines)
- Tests module (~300 lines)

## Specification

### Requirements
1. Create `system/system.rs`
2. Move `System` struct definition
3. Move `impl System` block with `validate()` method
4. Update calls to helper functions: `helpers::validate_unique_ids()`, `helpers::validate_bus_and_submarket_membership()`
5. Add necessary imports
6. Add `mod system_def;` to `mod.rs` (using `system_def` to avoid conflict with `mod system;` being the directory)
7. Add `pub use system_def::System;` to `mod.rs`

### Types to Move
```rust
#[derive(Debug, Clone, PartialEq)]
pub struct System {
    pub horizon: StudyHorizon,
    pub thermal_unit_commitment_enabled: bool,
    // ... 16 more fields
}

impl System {
    pub fn validate(&self) -> Result<(), CoreError> {
        // ~300 lines of validation logic
    }
}
```

### Inputs/Props
- `crates/labdessem-core/src/system/mod.rs` (after tickets 001, 002)

### Outputs/Behavior
- New `system.rs` with System struct and impl
- Updated `mod.rs` with mod declaration and re-export
- Same public API

### Error Handling
N/A — pure refactoring

## Acceptance Criteria
- [ ] `system.rs` exists with `System` struct and `impl System`
- [ ] `validate()` calls helpers via `super::helpers::*` or import
- [ ] `mod.rs` has `mod system;` declaration
- [ ] `mod.rs` re-exports: `pub use system::System;` (if file is named system.rs, parent is already system/)
- [ ] `cargo check -p labdessem-core` succeeds
- [ ] External import `use labdessem_core::system::System;` works

## Implementation Guide

### Suggested Approach
1. Create `system.rs` with module doc comment
2. Copy the System struct and impl block
3. Add imports:
   ```rust
   use std::collections::HashSet;
   use crate::{
       entities::{HydroPlant, RenewablePlant, ThermalPlant, Bus, Branch, PumpingPlant, Submarket},
       error::CoreError,
       ids::{BusId, SubmarketId},
   };
   use super::{
       helpers::{validate_unique_ids, validate_bus_and_submarket_membership},
       types::{InterchangeLimit, ResidualCost, FutureCostCut},
       horizon::StudyHorizon,
       operational_limit::{OperationalLimit, OperationalLimitTarget},
   };
   ```
4. Add `mod system;` to `mod.rs`
5. Add `pub use system::System;` to re-exports
6. Remove System struct and impl from `mod.rs`
7. Run `cargo check -p labdessem-core`

### Key Files to Modify
- `crates/labdessem-core/src/system/system.rs` (create)
- `crates/labdessem-core/src/system/mod.rs` (modify)

### Patterns to Follow
```rust
// system.rs
//! The System struct and its validation logic.

use std::collections::HashSet;

use crate::{
    entities::{HydroPlant, RenewablePlant, ThermalPlant},
    error::CoreError,
    ids::{BusId, SubmarketId},
};

use super::{
    helpers::{validate_bus_and_submarket_membership, validate_unique_ids},
    types::{FutureCostCut, InterchangeLimit, ResidualCost},
    Branch, Bus, OperationalLimit, OperationalLimitTarget, PumpingPlant, StudyHorizon, Submarket,
};

#[derive(Debug, Clone, PartialEq)]
pub struct System {
    // fields
}

impl System {
    pub fn validate(&self) -> Result<(), CoreError> {
        // validation logic
    }
}
```

### Pitfalls to Avoid
- Don't name the file `system.rs` inside `system/` directory — Rust may get confused. Consider naming it `core.rs` or `definition.rs` if needed. Actually, `system.rs` inside `system/` module should work fine as `system/system.rs`.
- Import types from `super::` since we're in a submodule of `system`
- Keep the `#[allow(clippy::expect_used)]` annotations

## Testing Requirements

### Unit Tests
N/A — code movement only (tests move in ticket-004)

### Integration Tests
- `cargo check -p labdessem-core`
- `cargo check --workspace`

## Dependencies
- **Blocked By**: ticket-001, ticket-002
- **Blocks**: ticket-004

## Effort Estimate
**Points**: 2
**Confidence**: High
