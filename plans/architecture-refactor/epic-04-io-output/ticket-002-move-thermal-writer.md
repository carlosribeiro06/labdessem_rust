# [002] Move thermal and hydro writers

## Context

Thermal and hydro result writers are the largest output functions.

## Specification

1. Create `output/thermal.rs` with `write_thermal_results()`
2. Create `output/hydro.rs` with `write_hydro_results()`
3. Move CSV writing logic from `labdessem-simulation/lib.rs`
4. Update simulation to import from `labdessem_io::output`

## Acceptance Criteria

- [ ] Both files created
- [ ] Result CSVs still generated correctly
- [ ] `cargo test` passes

## Key Files

### To Create
- `crates/labdessem-io/src/output/thermal.rs`
- `crates/labdessem-io/src/output/hydro.rs`

### To Modify
- `crates/labdessem-io/src/output/mod.rs`
- `crates/labdessem-simulation/src/lib.rs` (remove writer code)

## Dependencies

- **Blocked By**: 001
- **Blocks**: 005

## Effort Estimate

**Points**: 2
