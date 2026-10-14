# Epic 02: Constraints Module Cleanup

## Overview

Extract all substantive code from `crates/labdessem-model/src/constraints/mod.rs` to dedicated files.

## Current State

The `constraints/mod.rs` (~220 lines) contains:
- Module documentation (~6 lines)
- `mod` declarations and imports (~15 lines)
- `pub use` re-exports (~10 lines)
- Type definitions: `ConstraintSense`, `LinearTerm`, `LinearConstraint` (~30 lines)
- `ConstraintSet` struct (~5 lines)
- `ConstraintSet::for_system()` (~60 lines)
- `ConstraintSet` filter methods (~70 lines)
- Helper functions: `term()`, `display_period()` (~10 lines)

## Target State

After this epic, `constraints/mod.rs` should contain ONLY:
- Module documentation
- `mod` declarations
- `pub use` re-exports

All types, impls, and functions will be in dedicated files.

## Tickets

| # | Ticket | Description | Points |
|---|--------|-------------|--------|
| 001 | [Extract Constraint Types](ticket-001-extract-types.md) | Move ConstraintSense, LinearTerm, LinearConstraint to types.rs | 1 |
| 002 | [Extract ConstraintSet](ticket-002-extract-constraint-set.md) | Move ConstraintSet struct and impl to constraint_set.rs | 2 |
| 003 | [Finalize mod.rs](ticket-003-finalize-mod.md) | Move helpers, clean mod.rs | 1 |

## Dependencies

```
ticket-001 ──> ticket-002 ──> ticket-003
```

## Quality Gate

- [ ] `cargo check --workspace` passes
- [ ] `cargo test --workspace` passes
- [ ] `constraints/mod.rs` < 30 lines
- [ ] All external imports unchanged
