# [003] Extract hydro constraints

## Context

Hydro constraints include hydraulic balance, FPHA cuts, pumping balance,
and generation limits.

## Specification

1. Create `constraints/hydro.rs` with `add_hydro_constraints()`
2. Move hydro-specific constraint functions from `constraints.rs`
3. Include pumping constraints here or in separate file

## Acceptance Criteria

- [ ] `hydro.rs` exists
- [ ] Hydro constraint logic extracted
- [ ] `cargo test` passes

## Dependencies

- **Blocked By**: 001
- **Blocks**: None

## Effort Estimate

**Points**: 2
