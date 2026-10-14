# [002] Extract thermal constraints

## Context

Thermal constraints include generation limits, commitment logic, min up/down
times, and trajectory constraints.

## Specification

1. Create `constraints/thermal.rs` with `add_thermal_constraints()`
2. Move thermal-specific constraint functions from `constraints.rs`

## Acceptance Criteria

- [ ] `thermal.rs` exists
- [ ] Thermal constraint logic extracted
- [ ] `cargo test` passes

## Dependencies

- **Blocked By**: 001
- **Blocks**: None (can parallel with 003, 004)

## Effort Estimate

**Points**: 2
