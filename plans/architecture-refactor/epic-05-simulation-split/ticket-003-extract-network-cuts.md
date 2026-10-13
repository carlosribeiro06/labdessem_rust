# [003] Extract network cuts module

## Context

Extract network flow violation detection and cut addition logic.

## Specification

1. Create `network_cuts.rs` with:
   - `check_flow_violations()` - detect violated branch flows
   - `add_violated_flow_cuts()` - add constraints to model
   - `FlowViolation` struct

## Acceptance Criteria

- [ ] `network_cuts.rs` exists
- [ ] Flow violation logic extracted
- [ ] `cargo test` passes

## Dependencies

- **Blocked By**: 001
- **Blocks**: None

## Effort Estimate

**Points**: 2
