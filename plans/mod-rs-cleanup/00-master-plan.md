# Master Plan: mod.rs Cleanup Refactoring

## Executive Summary

Refactor two `mod.rs` files that violate Rust convention (using mod.rs only as module index) by extracting substantive code to dedicated files. The `system/mod.rs` (~860 lines) contains struct definitions, validation logic, and tests. The `constraints/mod.rs` (~220 lines) contains type definitions and a large `ConstraintSet` implementation.

## Goals & Non-Goals

### Goals
- `mod.rs` files contain ONLY: module docs, `mod` declarations, and `pub use` re-exports
- Zero breaking changes to public API
- Maintain logical organization of related types
- Move tests alongside their code
- All existing imports continue working

### Non-Goals
- Changing the behavior of any function
- Adding new functionality
- Refactoring internal logic of methods
- Changing test coverage

## Architecture Overview

### Current State

**system/mod.rs (~860 lines)**:
- Module documentation (10 lines)
- `mod` declarations and `pub use` (15 lines)
- `InterchangeLimit` struct + impl (~30 lines)
- `ResidualCost` struct + impl (~20 lines)
- `FutureCostCut` struct + impl (~45 lines)
- `System` struct (~20 lines)
- `System::validate()` (~300 lines)
- Helper functions: `validate_unique_ids`, `validate_bus_and_submarket_membership` (~50 lines)
- Tests module (~300 lines)

**constraints/mod.rs (~220 lines)**:
- Module documentation (6 lines)
- `mod` declarations and imports (15 lines)
- `pub use` re-exports (10 lines)
- `ConstraintSense`, `LinearTerm`, `LinearConstraint` structs (~30 lines)
- `ConstraintSet` struct (~5 lines)
- `ConstraintSet::for_system()` (~60 lines)
- `ConstraintSet` filter methods (~70 lines)
- Helper functions: `term()`, `display_period()` (~10 lines)

### Target State

**system/** (after refactoring):
```
system/
├── mod.rs                  # ~25 lines: docs + mod + pub use only
├── builder.rs              # (unchanged)
├── horizon.rs              # (unchanged)
├── operational_limit.rs    # (unchanged)
├── validate.rs             # (unchanged)
├── types.rs                # InterchangeLimit, ResidualCost, FutureCostCut + their impls
├── system.rs               # System struct + impl System::validate() + tests
└── helpers.rs              # validate_unique_ids, validate_bus_and_submarket_membership
```

**constraints/** (after refactoring):
```
constraints/
├── mod.rs              # ~25 lines: docs + mod + pub use only
├── demand.rs           # (unchanged)
├── hydro.rs            # (unchanged)
├── limits.rs           # (unchanged)
├── thermal.rs          # (unchanged)
├── types.rs            # ConstraintSense, LinearTerm, LinearConstraint
├── constraint_set.rs   # ConstraintSet struct + impl + for_system() + filter methods
└── helpers.rs          # term(), display_period()
```

### Key Design Decisions

1. **Separate `types.rs` from main struct file** — Small auxiliary types (InterchangeLimit, ResidualCost) go in `types.rs`; the main `System` struct gets its own file.

2. **Helpers in dedicated file** — Functions used by validation but not part of System's API go in `helpers.rs`.

3. **Tests move with code** — System tests go to `system.rs` since they test System::validate().

4. **Preserve pub(crate) visibility** — Helper functions like `term()` and `display_period()` remain `pub(crate)`.

## Technical Approach

### Tech Stack
- Rust refactoring (module extraction)
- No new dependencies

### Component/Module Breakdown

See Target State above.

### Testing Strategy
- After each ticket: `cargo check --workspace`
- After each epic: `cargo test --workspace`
- Final verification: `cargo run -p labdessem-cli -- run examples/24Barras`

## Phases & Milestones

| Phase | Epic | Duration | Deliverable |
|-------|------|----------|-------------|
| 1 | Epic 01: System Module | ~30 min | Clean system/mod.rs (~25 lines) |
| 2 | Epic 02: Constraints Module | ~20 min | Clean constraints/mod.rs (~25 lines) |

## Risk Analysis

| Risk | Likelihood | Impact | Mitigation |
|------|------------|--------|------------|
| Circular imports | Low | High | Plan extraction order carefully |
| Missing re-export | Medium | Low | Test all existing imports |
| Visibility errors | Medium | Low | Match existing visibility modifiers |
| Test failures | Low | Medium | Run tests after each ticket |

## Success Metrics

1. Both `mod.rs` files < 30 lines
2. Zero changes to external imports
3. All 50+ tests pass
4. Example 24Barras: objective = 4497639.8364
5. Zero clippy warnings
