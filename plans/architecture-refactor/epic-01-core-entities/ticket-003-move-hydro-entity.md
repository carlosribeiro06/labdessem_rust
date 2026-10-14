# [003] Move hydro and renewable entities to entities directory

## Context

### Background

The `hydro.rs` and `renewable.rs` files at the root of `labdessem-core/src/`
contain hydro and renewable plant related types. They should be moved into
the new `entities/` directory.

### Relation to Epic

Third ticket in Epic 01. Moves the remaining existing entity files.

### Current State

- `crates/labdessem-core/src/hydro.rs` exists at root level
- `crates/labdessem-core/src/renewable.rs` exists at root level
- `lib.rs` has `pub mod hydro;` and `pub mod renewable;` with re-exports

## Specification

### Requirements

1. Move `hydro.rs` to `entities/hydro.rs`
2. Move `renewable.rs` to `entities/renewable.rs`
3. Update `entities/mod.rs` to declare and re-export hydro and renewable types
4. Update `lib.rs` to re-export from `entities` instead of root modules
5. Update imports in other crates if needed

### Inputs/Props

Existing files:
- `crates/labdessem-core/src/hydro.rs`
- `crates/labdessem-core/src/renewable.rs`

### Outputs/Behavior

- Files moved to `entities/` directory
- Public API unchanged: `labdessem_core::HydroPlant`, `labdessem_core::RenewablePlant` still work

### Error Handling

N/A (file move)

## Acceptance Criteria

- [ ] `hydro.rs` is at `crates/labdessem-core/src/entities/hydro.rs`
- [ ] `renewable.rs` is at `crates/labdessem-core/src/entities/renewable.rs`
- [ ] Old root-level files are deleted
- [ ] `entities/mod.rs` declares and re-exports both modules
- [ ] `lib.rs` re-exports from `entities`
- [ ] `cargo check --all-targets` passes
- [ ] `cargo test` passes

## Implementation Guide

### Suggested Approach

1. Move both files to `entities/`
2. Update `entities/mod.rs`:
   ```rust
   pub mod hydro;
   pub mod renewable;
   pub mod thermal;
   
   pub use hydro::{
       HydroFphaSegment, HydroGroup, HydroInitialCondition, HydroPlant, HydroUnit, Reservoir,
   };
   pub use renewable::RenewablePlant;
   pub use thermal::{ThermalInitialCondition, ThermalPlant, ThermalUnit};
   ```
3. Update `lib.rs` to remove old modules and re-export from entities
4. Verify all downstream crates compile

### Key Files to Modify

- `crates/labdessem-core/src/lib.rs`
- `crates/labdessem-core/src/entities/mod.rs`

### Key Files to Move

- `crates/labdessem-core/src/hydro.rs` → `crates/labdessem-core/src/entities/hydro.rs`
- `crates/labdessem-core/src/renewable.rs` → `crates/labdessem-core/src/entities/renewable.rs`

### Patterns to Follow

Same as ticket-002. Keep file contents unchanged, only update module paths.

### Pitfalls to Avoid

- `hydro.rs` may have more complex internal imports - verify `use crate::` paths work
- Ensure all public types from both modules are re-exported

## Testing Requirements

### Unit Tests

Existing tests in both files must continue to pass.

### Integration Tests

- `cargo test` (full workspace)

## Dependencies

- **Blocked By**: 001
- **Blocks**: None (can run in parallel with 002, 004)

## Effort Estimate

**Points**: 1
**Confidence**: High
