# [004] Extract results module

## Context

Extract result structs and summary types to dedicated module.

## Specification

1. Create `results.rs` with:
   - `SimulationResult`
   - `IterationStep`
   - `SolveSummary`
   - `SolveStage` enum
   - Other result-related types
2. Update `lib.rs` to only re-export

## Acceptance Criteria

- [ ] `results.rs` exists with all result types
- [ ] `lib.rs` under 100 lines (re-exports only)
- [ ] `cargo test` passes

## Verification

```bash
wc -l crates/labdessem-simulation/src/lib.rs
# Should be < 100
```

## Dependencies

- **Blocked By**: 001, 002, 003
- **Blocks**: Epic 06

## Effort Estimate

**Points**: 1
