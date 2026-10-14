# ticket-004: Finalize mod.rs and Move Tests

## Context

### Background
After extracting all types and impls, we need to finalize `mod.rs` and handle the test module.

### Relation to Epic
Final cleanup — ensures `mod.rs` is index-only.

### Current State
After tickets 001-003, `mod.rs` will contain:
- Module documentation
- `mod` declarations
- `pub use` re-exports
- Tests module (~300 lines)

## Specification

### Requirements
1. Move tests to `system.rs` (with the code they test)
2. Clean up `mod.rs` to contain ONLY:
   - Module documentation (//! comments)
   - `mod` declarations
   - `pub use` re-exports
3. Verify all public types are re-exported
4. Remove any leftover imports from `mod.rs`

### Final mod.rs Structure
```rust
//! Power system model container and related types.
//!
//! The [`System`] struct is the immutable container for all power system data
//! used in optimization. It is constructed via [`SystemBuilder`].
//!
//! # Module Structure
//!
//! - [`horizon`](horizon) - Study horizon and temporal discretization
//! - [`operational_limit`](operational_limit) - Operational limit constraints
//! - [`builder`](builder) - System construction API
//! - [`types`](types) - Auxiliary types (InterchangeLimit, ResidualCost, FutureCostCut)
//! - [`system`](system) - System struct and validation

mod builder;
mod helpers;
mod horizon;
mod operational_limit;
mod system;
mod types;
mod validate;

pub use builder::SystemBuilder;
pub use horizon::StudyHorizon;
pub use operational_limit::{OperationalLimit, OperationalLimitTarget, OperationalLimitVariable};
pub use system::System;
pub use types::{FutureCostCut, InterchangeLimit, ResidualCost};
pub use validate::validate_system;

// Re-export entities for backward compatibility
pub use crate::entities::{Branch, Bus, PumpingPlant, Submarket};
```

### Inputs/Props
- `crates/labdessem-core/src/system/mod.rs` (after tickets 001-003)
- `crates/labdessem-core/src/system/system.rs` (to receive tests)

### Outputs/Behavior
- Clean `mod.rs` (~30 lines)
- Tests in `system.rs`
- All external imports working

### Error Handling
N/A — pure refactoring

## Acceptance Criteria
- [ ] `mod.rs` < 30 lines
- [ ] `mod.rs` contains ONLY docs, mod declarations, pub use re-exports
- [ ] Tests moved to `system.rs`
- [ ] `cargo check --workspace` succeeds
- [ ] `cargo test -p labdessem-core` passes
- [ ] All external imports work unchanged

## Implementation Guide

### Suggested Approach
1. Move the `#[cfg(test)] mod tests { ... }` block to the end of `system.rs`
2. Update test imports to use `super::*` pattern
3. Clean up `mod.rs` — remove `use std::collections::HashSet;` and other unused imports
4. Verify the structure matches the Final mod.rs Structure above
5. Run `cargo test -p labdessem-core`
6. Run `cargo test --workspace`

### Key Files to Modify
- `crates/labdessem-core/src/system/system.rs` (add tests)
- `crates/labdessem-core/src/system/mod.rs` (cleanup)

### Patterns to Follow
```rust
// At end of system.rs
#[cfg(test)]
#[allow(clippy::unwrap_used, clippy::expect_used)]
mod tests {
    use super::*;  // imports System, etc.
    use crate::{
        entities::{...},
        ids::{...},
    };
    use super::super::{types::*, Branch, Bus, ...};  // from parent module

    // ... test functions
}
```

### Pitfalls to Avoid
- Tests need to import `InterchangeLimit`, `FutureCostCut` etc. from the right path
- Some imports may need adjustment from `super::*` to explicit paths

## Testing Requirements

### Unit Tests
- All 17+ tests in the tests module must pass

### Integration Tests
- `cargo test --workspace`
- `cargo run -p labdessem-cli -- run examples/24Barras`

## Dependencies
- **Blocked By**: ticket-003
- **Blocks**: None (end of epic)

## Effort Estimate
**Points**: 1
**Confidence**: High
