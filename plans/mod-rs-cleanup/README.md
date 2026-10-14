# mod.rs Cleanup Plan

Refactor `mod.rs` files to follow Rust convention (index only, no substantive code).

## Epics

| # | Epic | Tickets | Focus |
|---|------|---------|-------|
| 01 | [System Module](epic-01-system-module/) | 4 | Extract types, System struct, helpers from `system/mod.rs` |
| 02 | [Constraints Module](epic-02-constraints-module/) | 3 | Extract types, ConstraintSet, helpers from `constraints/mod.rs` |

## Progress

Run with: `/implement-plan plans/mod-rs-cleanup`

## Quick Stats

- **Target**: Both `mod.rs` files < 30 lines
- **Current**: `system/mod.rs` ~860 lines, `constraints/mod.rs` ~220 lines
- **Approach**: Extract to dedicated files, update re-exports
