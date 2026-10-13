# [001] Create system module structure

## Context

### Background

After extracting entities to `entities/`, the `system.rs` file still contains
the `System` struct and helper types. This ticket creates the `system/` directory
structure for subsequent refactoring.

### Relation to Epic

First ticket in Epic 02. Creates the module structure.

### Current State

`system.rs` contains:
- `System` struct
- `StudyHorizon` and related types
- Various limit/constraint types
- Mixed validation logic

## Specification

### Requirements

1. Create `crates/labdessem-core/src/system/` directory
2. Create `system/mod.rs` with `System` struct and helper types
3. Create empty `system/builder.rs` (placeholder)
4. Create empty `system/validate.rs` (placeholder)
5. Update `lib.rs` to use `mod system;` instead of file
6. Delete old `system.rs`

### Outputs/Behavior

```
crates/labdessem-core/src/
├── system/
│   ├── mod.rs       # System struct moved here
│   ├── builder.rs   # Empty placeholder
│   └── validate.rs  # Empty placeholder
```

## Acceptance Criteria

- [ ] `system/` directory exists with three files
- [ ] `System` struct is in `system/mod.rs`
- [ ] Old `system.rs` is deleted
- [ ] `cargo check --all-targets` passes
- [ ] `cargo test` passes

## Implementation Guide

### Suggested Approach

1. Create `system/` directory
2. Move content of `system.rs` to `system/mod.rs`
3. Create placeholder files for `builder.rs` and `validate.rs`
4. Update `lib.rs`: change `mod system;` (file) to `mod system;` (directory)
5. Verify compilation

### Key Files to Create

- `crates/labdessem-core/src/system/mod.rs` (content from `system.rs`)
- `crates/labdessem-core/src/system/builder.rs` (placeholder)
- `crates/labdessem-core/src/system/validate.rs` (placeholder)

### Key Files to Delete

- `crates/labdessem-core/src/system.rs`

### Pitfalls to Avoid

- Ensure all `use crate::` paths in the moved code still work
- Update any imports that referenced `system::*`

## Testing Requirements

- `cargo test` passes

## Dependencies

- **Blocked By**: Epic 01 (entities must be extracted first)
- **Blocks**: 002, 003

## Effort Estimate

**Points**: 1
**Confidence**: High
