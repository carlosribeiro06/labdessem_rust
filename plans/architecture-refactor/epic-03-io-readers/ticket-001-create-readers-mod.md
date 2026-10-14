# [001] Create readers module structure

## Context

### Background

The `readers/` directory will contain one file per entity type, following the
pattern established in `cobre-io`. This ticket creates the directory structure.

### Relation to Epic

First ticket in Epic 03. Creates the module structure.

### Current State

All reading logic is in `study.rs` (2100 lines).

## Specification

### Requirements

1. Create `crates/labdessem-io/src/readers/` directory
2. Create `readers/mod.rs` with module declarations (initially commented out)
3. Add `mod readers;` to `lib.rs`

### Outputs/Behavior

```
crates/labdessem-io/src/
├── readers/
│   └── mod.rs
└── lib.rs (modified)
```

## Acceptance Criteria

- [ ] `readers/` directory exists
- [ ] `mod.rs` exists with placeholder content
- [ ] `cargo check -p labdessem-io` passes

## Implementation Guide

### Suggested Approach

```rust
// readers/mod.rs
//! CSV readers for each entity type.
//!
//! Each module provides a `read_*` function that reads CSV files
//! and returns the corresponding entity collection.

// Modules will be added by subsequent tickets:
// pub mod hydro;
// pub mod limits;
// pub mod network;
// pub mod pumping;
// pub mod renewable;
// pub mod submarket;
// pub mod thermal;
```

### Key Files to Create

- `crates/labdessem-io/src/readers/mod.rs`

### Key Files to Modify

- `crates/labdessem-io/src/lib.rs` - add `mod readers;`

## Dependencies

- **Blocked By**: Epic 02
- **Blocks**: 002, 003, 004, 005

## Effort Estimate

**Points**: 1
**Confidence**: High
