# ticket-002: Extract Helper Functions to helpers.rs

## Context

### Background
The `system/mod.rs` contains two helper functions used by `System::validate()` but not part of the public API. These should be in a dedicated file.

### Relation to Epic
Second extraction — moves internal helpers before the main System struct.

### Current State
These functions are at the end of `mod.rs`:
- `validate_unique_ids()` (~15 lines)
- `validate_bus_and_submarket_membership()` (~35 lines)

Both are private functions (no `pub`) used only by `System::validate()`.

## Specification

### Requirements
1. Create `system/helpers.rs`
2. Move `validate_unique_ids` and `validate_bus_and_submarket_membership`
3. Make them `pub(super)` so they're accessible from `system.rs` but not from outside
4. Add necessary imports
5. Add `mod helpers;` to `mod.rs` (no pub use — internal only)

### Functions to Move
```rust
fn validate_unique_ids(ids: impl IntoIterator<Item = usize>, message: &str) -> Result<(), CoreError>

fn validate_bus_and_submarket_membership(
    entity_id: usize,
    entity_name: &str,
    bus_id: BusId,
    submarket_id: SubmarketId,
    buses: &[Bus],
    bus_ids: &HashSet<BusId>,
    submarket_ids: &HashSet<SubmarketId>,
) -> Result<(), CoreError>
```

### Inputs/Props
- `crates/labdessem-core/src/system/mod.rs`

### Outputs/Behavior
- New `helpers.rs` with both functions
- Updated `mod.rs` with `mod helpers;`
- No public re-exports (internal use only)

### Error Handling
N/A — pure refactoring

## Acceptance Criteria
- [ ] `helpers.rs` exists with both helper functions
- [ ] Functions have `pub(super)` visibility
- [ ] `mod.rs` has `mod helpers;` declaration (no pub use)
- [ ] `cargo check -p labdessem-core` succeeds
- [ ] Functions are accessible from `system.rs` (to be created in ticket-003)

## Implementation Guide

### Suggested Approach
1. Create `helpers.rs` with module doc comment
2. Copy both functions
3. Change visibility from `fn` to `pub(super) fn`
4. Add imports: `use std::collections::HashSet; use crate::{error::CoreError, ids::{BusId, SubmarketId}, entities::Bus};`
5. Add `mod helpers;` to `mod.rs`
6. Remove the functions from `mod.rs`
7. Update `System::validate()` to call `helpers::validate_unique_ids()` etc.
8. Run `cargo check -p labdessem-core`

### Key Files to Modify
- `crates/labdessem-core/src/system/helpers.rs` (create)
- `crates/labdessem-core/src/system/mod.rs` (modify)

### Patterns to Follow
```rust
// helpers.rs
//! Internal helper functions for system validation.

use std::collections::HashSet;

use crate::{
    entities::Bus,
    error::CoreError,
    ids::{BusId, SubmarketId},
};

pub(super) fn validate_unique_ids(
    ids: impl IntoIterator<Item = usize>,
    message: &str,
) -> Result<(), CoreError> {
    // ...
}

pub(super) fn validate_bus_and_submarket_membership(
    // ...
) -> Result<(), CoreError> {
    // ...
}
```

### Pitfalls to Avoid
- Don't use `pub fn` — these are internal helpers, use `pub(super) fn`
- After removing from `mod.rs`, call them as `helpers::function_name()`

## Testing Requirements

### Unit Tests
N/A — code movement only

### Integration Tests
- `cargo check -p labdessem-core`

## Dependencies
- **Blocked By**: None
- **Blocks**: ticket-003

## Effort Estimate
**Points**: 1
**Confidence**: High
