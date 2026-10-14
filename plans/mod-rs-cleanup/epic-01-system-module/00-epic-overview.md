# Epic 01: System Module Cleanup

## Overview

Extract all substantive code from `crates/labdessem-core/src/system/mod.rs` to dedicated files.

## Current State

The `system/mod.rs` (~860 lines) contains:
- Module documentation (~10 lines)
- `mod` declarations and `pub use` re-exports (~15 lines)
- `InterchangeLimit`, `ResidualCost`, `FutureCostCut` structs + impls (~95 lines)
- `System` struct + `impl System::validate()` (~320 lines)
- Helper functions: `validate_unique_ids`, `validate_bus_and_submarket_membership` (~50 lines)
- Test module (~300 lines)

## Target State

After this epic, `system/mod.rs` should contain ONLY:
- Module documentation
- `mod` declarations
- `pub use` re-exports

All structs, impls, functions, and tests will be in dedicated files.

## Tickets

| # | Ticket | Description | Points |
|---|--------|-------------|--------|
| 001 | [Extract Auxiliary Types](ticket-001-extract-auxiliary-types.md) | Move InterchangeLimit, ResidualCost, FutureCostCut to types.rs | 2 |
| 002 | [Extract Helper Functions](ticket-002-extract-helpers.md) | Move validate_unique_ids, validate_bus_and_submarket_membership to helpers.rs | 1 |
| 003 | [Extract System Struct](ticket-003-extract-system.md) | Move System struct and impl to system.rs | 2 |
| 004 | [Finalize mod.rs](ticket-004-finalize-mod.md) | Clean mod.rs, add all re-exports, move tests | 1 |

## Dependencies

```
ticket-001 ──┬──> ticket-003 ──> ticket-004
ticket-002 ──┘
```

## Quality Gate

- [ ] `cargo check --workspace` passes
- [ ] `cargo test --workspace` passes
- [ ] `system/mod.rs` < 30 lines
- [ ] All external imports unchanged
