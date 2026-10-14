# [004] Extract network entities from system.rs

## Context

### Background

The `system.rs` file in `labdessem-core` contains not only the `System` struct
but also several entity definitions that belong in the `entities/` directory:
`PumpingPlant`, `Bus`, `Branch`, and `Submarket`.

### Relation to Epic

Final ticket in Epic 01. Extracts remaining entities from `system.rs` into
dedicated files, completing the entities directory structure.

### Current State

`system.rs` (~1200 lines) contains:
- `System` struct (should stay)
- `Submarket` struct (should move)
- `Bus` struct (should move)
- `Branch` struct (should move)
- `PumpingPlant` struct (should move)
- Various helper types related to System (should stay)

## Specification

### Requirements

1. Create `entities/pumping.rs` with `PumpingPlant` struct
2. Create `entities/bus.rs` with `Bus` struct
3. Create `entities/branch.rs` with `Branch` struct
4. Create `entities/submarket.rs` with `Submarket` struct
5. Remove these structs from `system.rs`
6. Update `system.rs` to import from `entities`
7. Update `entities/mod.rs` with new modules and re-exports
8. Ensure `lib.rs` re-exports all types

### Inputs/Props

Structs to extract from `crates/labdessem-core/src/system.rs`:
- `Submarket`
- `Bus`
- `Branch`
- `PumpingPlant`

### Outputs/Behavior

New files created:
- `crates/labdessem-core/src/entities/pumping.rs`
- `crates/labdessem-core/src/entities/bus.rs`
- `crates/labdessem-core/src/entities/branch.rs`
- `crates/labdessem-core/src/entities/submarket.rs`

`system.rs` reduced by ~150-200 lines.

### Error Handling

N/A (code extraction)

## Acceptance Criteria

- [ ] `PumpingPlant` is in `entities/pumping.rs`
- [ ] `Bus` is in `entities/bus.rs`
- [ ] `Branch` is in `entities/branch.rs`
- [ ] `Submarket` is in `entities/submarket.rs`
- [ ] These types are removed from `system.rs`
- [ ] `system.rs` imports these types via `use crate::entities::*;`
- [ ] All types are re-exported through `lib.rs`
- [ ] `cargo check --all-targets` passes
- [ ] `cargo test` passes

## Implementation Guide

### Suggested Approach

1. Identify each struct's dependencies (which `ids` types it uses)
2. Create each entity file with appropriate `use` statements
3. Cut/paste struct definitions from `system.rs`
4. Add `use crate::entities::{...};` to `system.rs`
5. Update `entities/mod.rs` with new modules
6. Verify compilation incrementally

### Key Files to Create

#### `entities/pumping.rs`
```rust
//! Pumping station (elevatória) entity.

use crate::ids::{BusId, HydroPlantId, PumpingPlantId, SubmarketId};

/// A pumping station that moves water between two reservoirs.
#[derive(Debug, Clone)]
pub struct PumpingPlant {
    // ... copy fields from system.rs
}
```

#### `entities/bus.rs`
```rust
//! Electrical bus entity.

use crate::ids::{BusId, SubmarketId};

/// An electrical bus in the network.
#[derive(Debug, Clone)]
pub struct Bus {
    // ... copy fields from system.rs
}
```

#### `entities/branch.rs`
```rust
//! Transmission line (branch) entity.

use crate::ids::{BranchId, BusId};

/// A transmission line connecting two buses.
#[derive(Debug, Clone)]
pub struct Branch {
    // ... copy fields from system.rs
}
```

#### `entities/submarket.rs`
```rust
//! Submarket (subsistema) entity.

use crate::ids::SubmarketId;

/// A submarket (subsystem) in the power system.
#[derive(Debug, Clone)]
pub struct Submarket {
    // ... copy fields from system.rs
}
```

### Key Files to Modify

- `crates/labdessem-core/src/system.rs` - remove structs, add imports
- `crates/labdessem-core/src/entities/mod.rs` - add new modules
- `crates/labdessem-core/src/lib.rs` - update re-exports

### Patterns to Follow

- Each file has a module-level doc comment
- Each struct has a doc comment
- Use `crate::ids::*` for ID types
- Derive `Debug, Clone` as in original

### Pitfalls to Avoid

- Check for any methods implemented on these structs in `system.rs`
- Ensure any related enums or helper types move with the struct
- Don't break `System` struct which references these types

## Testing Requirements

### Unit Tests

No new tests needed. Existing tests must pass.

### Integration Tests

- `cargo test` (full workspace)
- Verify examples still run: `cargo run -p labdessem-cli -- run examples/24Barras`

## Dependencies

- **Blocked By**: 001 (entities directory must exist)
- **Blocks**: Epic 02 (System refactoring depends on entities being separated)

## Effort Estimate

**Points**: 2
**Confidence**: Medium (need to carefully identify struct boundaries in system.rs)
