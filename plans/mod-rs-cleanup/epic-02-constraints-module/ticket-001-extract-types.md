# ticket-001: Extract Constraint Types to types.rs

## Context

### Background
The `constraints/mod.rs` contains type definitions (`ConstraintSense`, `LinearTerm`, `LinearConstraint`) that should be in a dedicated file.

### Relation to Epic
First extraction — moves small, self-contained types.

### Current State
These types are in `mod.rs` lines ~30-65:
```rust
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ConstraintSense { ... }

#[derive(Debug, Clone, PartialEq)]
pub struct LinearTerm { ... }

#[derive(Debug, Clone, PartialEq)]
pub struct LinearConstraint { ... }
```

## Specification

### Requirements
1. Create `constraints/types.rs`
2. Move `ConstraintSense`, `LinearTerm`, `LinearConstraint`
3. Add `mod types;` to `mod.rs`
4. Add `pub use types::{ConstraintSense, LinearTerm, LinearConstraint};` to `mod.rs`

### Types to Move
```rust
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ConstraintSense {
    Equal,
    LessOrEqual,
    GreaterOrEqual,
}

#[derive(Debug, Clone, PartialEq)]
pub struct LinearTerm {
    pub variable: String,
    pub coefficient: f64,
}

#[derive(Debug, Clone, PartialEq)]
pub struct LinearConstraint {
    pub name: String,
    pub terms: Vec<LinearTerm>,
    pub sense: ConstraintSense,
    pub rhs: f64,
}
```

### Inputs/Props
- `crates/labdessem-model/src/constraints/mod.rs`

### Outputs/Behavior
- New `types.rs` with all three types
- Updated `mod.rs` with mod declaration and re-exports
- Same public API

### Error Handling
N/A — pure refactoring

## Acceptance Criteria
- [ ] `types.rs` exists with `ConstraintSense`, `LinearTerm`, `LinearConstraint`
- [ ] `mod.rs` has `mod types;` declaration
- [ ] `mod.rs` re-exports the types
- [ ] `cargo check -p labdessem-model` succeeds
- [ ] External import `use labdessem_model::constraints::LinearTerm;` works (in `simulation/network/flow.rs`)

## Implementation Guide

### Suggested Approach
1. Create `types.rs` with module doc comment
2. Copy the three type definitions (no imports needed — they're self-contained)
3. Add `mod types;` to `mod.rs` after existing mod declarations
4. Add re-export line
5. Remove the types from `mod.rs`
6. Run `cargo check -p labdessem-model`

### Key Files to Modify
- `crates/labdessem-model/src/constraints/types.rs` (create)
- `crates/labdessem-model/src/constraints/mod.rs` (modify)

### Patterns to Follow
```rust
// types.rs
//! Constraint type definitions.

/// Sense of a linear constraint.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ConstraintSense {
    Equal,
    LessOrEqual,
    GreaterOrEqual,
}

/// A term in a linear expression.
#[derive(Debug, Clone, PartialEq)]
pub struct LinearTerm {
    pub variable: String,
    pub coefficient: f64,
}

/// A linear constraint.
#[derive(Debug, Clone, PartialEq)]
pub struct LinearConstraint {
    pub name: String,
    pub terms: Vec<LinearTerm>,
    pub sense: ConstraintSense,
    pub rhs: f64,
}
```

### Pitfalls to Avoid
- These types have no dependencies — no imports needed
- Ensure derive macros are preserved

## Testing Requirements

### Unit Tests
N/A — code movement only

### Integration Tests
- `cargo check --workspace`

## Dependencies
- **Blocked By**: None
- **Blocks**: ticket-002

## Effort Estimate
**Points**: 1
**Confidence**: High
