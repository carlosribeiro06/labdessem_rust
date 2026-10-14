# ticket-002: Extract ConstraintSet to constraint_set.rs

## Context

### Background
The `ConstraintSet` struct with its `for_system()` method and ~15 filter methods is the main type in the module and should have its own file.

### Relation to Epic
Main extraction — moves the largest piece of code (~130 lines).

### Current State
After ticket-001, `mod.rs` will still contain:
- `ConstraintSet` struct definition
- `impl ConstraintSet` with `for_system()` and filter methods
- Helper functions `term()` and `display_period()`

## Specification

### Requirements
1. Create `constraints/constraint_set.rs`
2. Move `ConstraintSet` struct
3. Move entire `impl ConstraintSet` block
4. Add necessary imports
5. Add `mod constraint_set;` to `mod.rs`
6. Add `pub use constraint_set::ConstraintSet;` to `mod.rs`

### Types to Move
```rust
#[derive(Debug, Clone, PartialEq)]
pub struct ConstraintSet {
    pub linear_constraints: Vec<LinearConstraint>,
}

impl ConstraintSet {
    pub fn for_system(system: &System, indexing: &Indexing, variables: &Variables, solve_mode: SolveMode) -> Self { ... }
    pub fn names(&self) -> Vec<&str> { ... }
    pub fn demand_balance(&self) -> Vec<&LinearConstraint> { ... }
    // ... ~15 more filter methods
}
```

### Inputs/Props
- `crates/labdessem-model/src/constraints/mod.rs` (after ticket-001)

### Outputs/Behavior
- New `constraint_set.rs` with ConstraintSet and all methods
- Updated `mod.rs` with mod declaration and re-export
- Same public API

### Error Handling
N/A — pure refactoring

## Acceptance Criteria
- [ ] `constraint_set.rs` exists with `ConstraintSet` struct and all impl methods
- [ ] `mod.rs` has `mod constraint_set;` declaration
- [ ] `mod.rs` re-exports: `pub use constraint_set::ConstraintSet;`
- [ ] `cargo check -p labdessem-model` succeeds
- [ ] External usage in `model.rs` and `stages.rs` works

## Implementation Guide

### Suggested Approach
1. Create `constraint_set.rs` with module doc comment
2. Copy `ConstraintSet` struct and entire `impl ConstraintSet` block
3. Add imports:
   ```rust
   use crate::{SolveMode, indexing::Indexing, variables::Variables};
   use labdessem_core::system::System;
   use super::{
       types::LinearConstraint,
       build_demand_balance_constraints,
       build_hydro_balance_constraints,
       // ... all builder functions
   };
   ```
4. Add `mod constraint_set;` to `mod.rs`
5. Add re-export line
6. Remove ConstraintSet from `mod.rs`
7. Run `cargo check -p labdessem-model`

### Key Files to Modify
- `crates/labdessem-model/src/constraints/constraint_set.rs` (create)
- `crates/labdessem-model/src/constraints/mod.rs` (modify)

### Patterns to Follow
```rust
// constraint_set.rs
//! ConstraintSet struct and builder logic.

use crate::{indexing::Indexing, variables::Variables, SolveMode};
use labdessem_core::system::System;

use super::{
    build_demand_balance_constraints, build_hydro_balance_constraints,
    build_hydro_commitment_channeling_constraints, build_hydro_fpha_constraints,
    build_hydro_generation_turbining_coupling_constraints, build_hydro_productivity_constraints,
    build_hydro_spillage_nonnegativity_constraints, build_hydro_turbining_limit_constraints,
    build_interchange_limit_constraints, build_future_cost_constraints,
    build_operational_limit_constraints, build_thermal_min_up_down_constraints,
    build_thermal_ramp_constraints,
    types::{ConstraintSense, LinearConstraint},
};

#[derive(Debug, Clone, PartialEq)]
pub struct ConstraintSet {
    pub linear_constraints: Vec<LinearConstraint>,
}

impl ConstraintSet {
    pub fn for_system(/* ... */) -> Self { /* ... */ }
    // ... all filter methods
}
```

### Pitfalls to Avoid
- The `for_system()` method calls many builder functions — ensure all are imported
- Import types from `super::types::{...}`

## Testing Requirements

### Unit Tests
N/A — code movement only

### Integration Tests
- `cargo check --workspace`
- `cargo test -p labdessem-model`

## Dependencies
- **Blocked By**: ticket-001
- **Blocks**: ticket-003

## Effort Estimate
**Points**: 2
**Confidence**: High
