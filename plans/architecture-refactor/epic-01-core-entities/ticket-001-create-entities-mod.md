# [001] Create entities module structure

## Context

### Background

The `labdessem-core` crate contains domain entities scattered across multiple files
at the root level. Following the pattern from `cobre-rs`, we want to organize all
entities under a dedicated `entities/` directory.

### Relation to Epic

This is the foundational ticket for Epic 01. It creates the directory structure
and module file that subsequent tickets will populate.

### Current State

Entity files are at root level:
- `thermal.rs`
- `hydro.rs`
- `renewable.rs`
- Plus entities embedded in `system.rs`

## Specification

### Requirements

1. Create `crates/labdessem-core/src/entities/` directory
2. Create `crates/labdessem-core/src/entities/mod.rs` with module declarations
3. Update `crates/labdessem-core/src/lib.rs` to declare and re-export from `entities`

### Inputs/Props

None (directory/file creation only)

### Outputs/Behavior

New file structure:
```
crates/labdessem-core/src/
├── entities/
│   └── mod.rs
└── lib.rs (modified)
```

### Error Handling

N/A (structural change)

## Acceptance Criteria

- [ ] Directory `crates/labdessem-core/src/entities/` exists
- [ ] File `crates/labdessem-core/src/entities/mod.rs` exists with placeholder content
- [ ] `lib.rs` declares `mod entities;`
- [ ] `cargo check -p labdessem-core` passes

## Implementation Guide

### Suggested Approach

1. Create the entities directory
2. Create `mod.rs` with empty module declarations (commented out for now)
3. Add `mod entities;` to `lib.rs`
4. Verify compilation

### Key Files to Modify

- `crates/labdessem-core/src/lib.rs` - add `mod entities;`

### Key Files to Create

- `crates/labdessem-core/src/entities/mod.rs`

### Patterns to Follow

```rust
// entities/mod.rs
//! Power system entity definitions.
//!
//! Each entity type has its own module with the struct definition,
//! associated types, and entity-specific logic.

// Modules will be added by subsequent tickets:
// pub mod thermal;
// pub mod hydro;
// pub mod renewable;
// pub mod pumping;
// pub mod bus;
// pub mod branch;
// pub mod submarket;
```

### Pitfalls to Avoid

- Don't move any existing files yet (that's for subsequent tickets)
- Don't break existing imports (keep old re-exports working)

## Testing Requirements

### Unit Tests

None required (structural change)

### Integration Tests

- `cargo check -p labdessem-core`
- `cargo test -p labdessem-core`

## Dependencies

- **Blocked By**: None
- **Blocks**: 002, 003, 004

## Effort Estimate

**Points**: 1
**Confidence**: High
