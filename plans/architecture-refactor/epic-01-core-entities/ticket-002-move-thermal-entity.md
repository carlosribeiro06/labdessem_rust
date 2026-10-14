# [002] Move thermal entity to entities directory

## Context

### Background

The `thermal.rs` file at the root of `labdessem-core/src/` contains all thermal
plant related types. It should be moved into the new `entities/` directory.

### Relation to Epic

Second ticket in Epic 01. Moves the first entity file into the new structure.

### Current State

- `crates/labdessem-core/src/thermal.rs` exists at root level
- `lib.rs` has `pub mod thermal;` and re-exports types

## Specification

### Requirements

1. Move `thermal.rs` to `entities/thermal.rs`
2. Update `entities/mod.rs` to declare and re-export thermal types
3. Update `lib.rs` to re-export from `entities::thermal` instead of root `thermal`
4. Update imports in other crates if needed

### Inputs/Props

Existing file: `crates/labdessem-core/src/thermal.rs`

### Outputs/Behavior

- File moved to `crates/labdessem-core/src/entities/thermal.rs`
- Public API unchanged: `labdessem_core::ThermalPlant` still works

### Error Handling

N/A (file move)

## Acceptance Criteria

- [ ] `thermal.rs` is at `crates/labdessem-core/src/entities/thermal.rs`
- [ ] Old `crates/labdessem-core/src/thermal.rs` is deleted
- [ ] `entities/mod.rs` has `pub mod thermal;` and re-exports
- [ ] `lib.rs` re-exports thermal types from `entities`
- [ ] `cargo check --all-targets` passes
- [ ] `cargo test` passes

## Implementation Guide

### Suggested Approach

1. Move `thermal.rs` to `entities/thermal.rs`
2. Update `entities/mod.rs`:
   ```rust
   pub mod thermal;
   
   pub use thermal::{ThermalInitialCondition, ThermalPlant, ThermalUnit};
   ```
3. Update `lib.rs` to remove old `mod thermal;` and re-export from entities:
   ```rust
   mod entities;
   
   pub use entities::{ThermalInitialCondition, ThermalPlant, ThermalUnit};
   ```
4. Check for any internal imports within `thermal.rs` that need path updates

### Key Files to Modify

- `crates/labdessem-core/src/lib.rs`
- `crates/labdessem-core/src/entities/mod.rs`

### Key Files to Move

- `crates/labdessem-core/src/thermal.rs` → `crates/labdessem-core/src/entities/thermal.rs`

### Patterns to Follow

Keep the same internal structure of `thermal.rs`. Only update:
- `use crate::ids::*;` stays the same (relative to crate root)
- Module-level doc comment can stay

### Pitfalls to Avoid

- Don't change any struct definitions
- Ensure all public types are re-exported through `lib.rs`
- Check `use` statements in `thermal.rs` reference `crate::` not `super::`

## Testing Requirements

### Unit Tests

Existing tests in `thermal.rs` must continue to pass.

### Integration Tests

- `cargo test -p labdessem-core`
- `cargo test -p labdessem-io` (uses thermal types)
- `cargo test` (full workspace)

## Dependencies

- **Blocked By**: 001
- **Blocks**: None (can run in parallel with 003, 004)

## Effort Estimate

**Points**: 1
**Confidence**: High
