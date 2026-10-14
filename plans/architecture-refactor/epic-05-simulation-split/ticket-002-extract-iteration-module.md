# [002] Extract iteration module

## Context

Extract iterative solve logic (LP → MILP → LP-FIXED → LP-CALC-CMO).

## Specification

1. Create `iteration.rs` with:
   - `run_iterative_workflow()`
   - `run_single_milp()`
2. Handle the solve loop with network cut iterations

## Acceptance Criteria

- [ ] `iteration.rs` exists
- [ ] Both workflow functions implemented
- [ ] `cargo test` passes

## Dependencies

- **Blocked By**: 001
- **Blocks**: None

## Effort Estimate

**Points**: 2
