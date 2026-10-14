# ticket-001: Extract Auxiliary Types to types.rs

## Context

### Background
The `system/mod.rs` contains three auxiliary structs (`InterchangeLimit`, `ResidualCost`, `FutureCostCut`) with their validation impls. These should be in a dedicated file.

### Relation to Epic
First extraction — moves small, self-contained types before the main System struct.

### Current State
These types are defined in `mod.rs` lines 33-115:
- `InterchangeLimit` (~15 lines)
- `ResidualCost` (~10 lines)
- `FutureCostCut` (~45 lines including validate impl)
- `impl InterchangeLimit::validate()` (~15 lines)
- `impl ResidualCost::validate()` (~10 lines)

## Specification

### Requirements
1. Create `system/types.rs`
2. Move `InterchangeLimit`, `ResidualCost`, `FutureCostCut` structs
3. Move their `impl` blocks with `validate()` methods
4. Add necessary imports in `types.rs`
5. Add `mod types;` to `mod.rs`
6. Add `pub use types::{InterchangeLimit, ResidualCost, FutureCostCut};` to `mod.rs`
7. Keep `InterchangeLimit` and `ResidualCost` internal usage by `System::validate()` working

### Types to Move
```rust
pub struct InterchangeLimit { ... }
impl InterchangeLimit { pub fn validate(&self) -> Result<(), CoreError> { ... } }

pub struct ResidualCost { ... }
impl ResidualCost { pub fn validate(&self) -> Result<(), CoreError> { ... } }

pub struct FutureCostCut { ... }
impl FutureCostCut { pub fn validate(&self, hydro_plant_count: usize) -> Result<(), CoreError> { ... } }
```

### Inputs/Props
- `crates/labdessem-core/src/system/mod.rs`

### Outputs/Behavior
- New `types.rs` with all three types
- Updated `mod.rs` with mod declaration and re-exports
- Same public API

### Error Handling
N/A — pure refactoring

## Acceptance Criteria
- [ ] `types.rs` exists with `InterchangeLimit`, `ResidualCost`, `FutureCostCut`
- [ ] `mod.rs` has `mod types;` declaration
- [ ] `mod.rs` re-exports: `pub use types::{InterchangeLimit, ResidualCost, FutureCostCut};`
- [ ] `cargo check -p labdessem-core` succeeds
- [ ] External import `use labdessem_core::system::FutureCostCut;` works (in `readers/future_cost.rs`)

## Implementation Guide

### Suggested Approach
1. Create `types.rs` with module doc comment
2. Copy the three structs and their impls
3. Add imports: `use crate::{error::CoreError, ids::SubmarketId};`
4. Add `mod types;` after existing mod declarations in `mod.rs`
5. Add re-export line
6. Remove the code from `mod.rs`
7. Run `cargo check -p labdessem-core`

### Key Files to Modify
- `crates/labdessem-core/src/system/types.rs` (create)
- `crates/labdessem-core/src/system/mod.rs` (modify)

### Patterns to Follow
```rust
// types.rs
//! Auxiliary types for the system module.

use crate::{error::CoreError, ids::SubmarketId};

#[derive(Debug, Clone, PartialEq)]
pub struct InterchangeLimit {
    pub from_submarket_id: SubmarketId,
    pub to_submarket_id: SubmarketId,
    pub max_flow_mw: f64,
    pub penalty_cost_per_mwh: f64,
}

impl InterchangeLimit {
    pub fn validate(&self) -> Result<(), CoreError> { ... }
}
// ... similar for ResidualCost, FutureCostCut
```

### Pitfalls to Avoid
- Don't forget to add `use crate::error::CoreError;` in `types.rs`
- Ensure visibility is `pub` for all types and methods

## Testing Requirements

### Unit Tests
N/A — code movement only

### Integration Tests
- `cargo check -p labdessem-core`
- Verify `readers/future_cost.rs` still imports `FutureCostCut`

## Dependencies
- **Blocked By**: None
- **Blocks**: ticket-003

## Effort Estimate
**Points**: 2
**Confidence**: High
