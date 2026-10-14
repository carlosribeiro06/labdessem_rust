# ticket-003: Finalize mod.rs and Extract Helpers

## Context

### Background
After extracting types and ConstraintSet, we need to handle the helper functions and finalize `mod.rs`.

### Relation to Epic
Final cleanup — ensures `mod.rs` is index-only.

### Current State
After tickets 001-002, `mod.rs` will contain:
- Module documentation
- `mod` declarations
- `pub use` re-exports
- Helper functions: `term()`, `display_period()` (~10 lines)

## Specification

### Requirements
1. Move `term()` and `display_period()` to a dedicated location
2. These are `pub(crate)` — used by submodules (demand.rs, hydro.rs, thermal.rs, limits.rs)
3. Clean up `mod.rs` to contain ONLY docs, mod declarations, pub use

### Options for helpers location
- **Option A**: Create `helpers.rs` in constraints/ (cleanest)
- **Option B**: Put in `types.rs` (types + helpers together)

**Recommendation**: Option A — separate `helpers.rs` for clarity.

### Functions to Move
```rust
pub(crate) fn term(variable: &Variable, coefficient: f64) -> LinearTerm {
    LinearTerm {
        variable: variable.name.clone(),
        coefficient,
    }
}

pub(crate) fn display_period(period: usize) -> usize {
    period + 1
}
```

### Final mod.rs Structure
```rust
//! Model constraints organized by domain.
//!
//! Constraints are split into submodules by type:
//! - `thermal`: Thermal unit commitment, ramps, min up/down times
//! - `hydro`: Hydraulic balance, FPHA, turbining limits
//! - `demand`: Demand balance and interchange limits
//! - `limits`: Operational limits and future cost cuts
//! - `types`: Constraint type definitions
//! - `constraint_set`: ConstraintSet builder and filter methods

mod constraint_set;
mod demand;
mod helpers;
mod hydro;
mod limits;
mod thermal;
mod types;

pub use constraint_set::ConstraintSet;
pub use demand::{build_demand_balance_constraints, build_interchange_limit_constraints};
pub use hydro::{...};
pub use limits::{build_future_cost_constraints, build_operational_limit_constraints};
pub use thermal::{build_thermal_min_up_down_constraints, build_thermal_ramp_constraints};
pub use types::{ConstraintSense, LinearConstraint, LinearTerm};

pub(crate) use helpers::{display_period, term};
```

### Inputs/Props
- `crates/labdessem-model/src/constraints/mod.rs` (after tickets 001-002)

### Outputs/Behavior
- New `helpers.rs` with `term()` and `display_period()`
- Clean `mod.rs` (~35 lines)
- All submodules can import helpers via `super::term` or `super::display_period`

### Error Handling
N/A — pure refactoring

## Acceptance Criteria
- [ ] `helpers.rs` exists with `term()` and `display_period()`
- [ ] `mod.rs` < 40 lines
- [ ] `mod.rs` contains ONLY docs, mod declarations, pub use/pub(crate) use
- [ ] Submodules (demand.rs, hydro.rs, thermal.rs, limits.rs) can use helpers
- [ ] `cargo check --workspace` succeeds
- [ ] `cargo test --workspace` passes

## Implementation Guide

### Suggested Approach
1. Create `helpers.rs` with:
   ```rust
   //! Helper functions for constraint construction.
   
   use crate::variables::Variable;
   use super::types::LinearTerm;
   
   pub(crate) fn term(variable: &Variable, coefficient: f64) -> LinearTerm { ... }
   pub(crate) fn display_period(period: usize) -> usize { ... }
   ```
2. Add `mod helpers;` to `mod.rs`
3. Add `pub(crate) use helpers::{term, display_period};` (so submodules can use `super::term`)
4. Update submodules to use `super::term` and `super::display_period`
5. Remove helpers from `mod.rs`
6. Clean up any remaining imports in `mod.rs`
7. Run `cargo check --workspace`
8. Run `cargo test --workspace`

### Key Files to Modify
- `crates/labdessem-model/src/constraints/helpers.rs` (create)
- `crates/labdessem-model/src/constraints/mod.rs` (modify)
- `crates/labdessem-model/src/constraints/demand.rs` (update import if needed)
- `crates/labdessem-model/src/constraints/hydro.rs` (update import if needed)
- `crates/labdessem-model/src/constraints/thermal.rs` (update import if needed)
- `crates/labdessem-model/src/constraints/limits.rs` (update import if needed)

### Patterns to Follow
Submodules should import helpers as:
```rust
// In demand.rs, hydro.rs, etc.
use super::{term, display_period};
```

### Pitfalls to Avoid
- The helpers are `pub(crate)` — they're used within the crate but not exported
- Make sure `Variable` import is correct in helpers.rs

## Testing Requirements

### Unit Tests
N/A — code movement only

### Integration Tests
- `cargo test --workspace`
- `cargo run -p labdessem-cli -- run examples/24Barras`

## Dependencies
- **Blocked By**: ticket-002
- **Blocks**: None (end of epic)

## Effort Estimate
**Points**: 1
**Confidence**: High
