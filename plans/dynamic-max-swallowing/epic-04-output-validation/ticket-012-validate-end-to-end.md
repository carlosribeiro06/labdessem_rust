# TICKET-012 Validate EngMax End-to-End

## Context

### Background

The EngMax feature spans multiple crates: configuration reading, bound calculation, constraint building, simulation integration, and output writing. End-to-end validation confirms that enabling `EngMax=1` in a study produces correct bounds, enforces them in the model, and writes the expected output file.

### Relation to Epic

This is the final validation ticket for the EngMax feature. It confirms the entire vertical slice works correctly before the feature is considered complete.

### Current State

- `examples/24Barras/` contains a complete study case with hydro plants
- `CAD_UHE.csv` has the `Turbina` column (turbine type identifier)
- `CAD_CONJ_UHE.csv` has `Hef.m` column (effective head in meters)
- `study_config.json` supports `EngMax` flag (0 or 1)
- The EngMax calculation, constraint building, simulation integration, and output writer are implemented

## Specification

### Requirements

1. Create an integration test that runs a full study with `EngMax=1` and verifies:
   - `resultado_engmax.csv` is written when EngMax=1
   - Bounds are computed and written to the file
2. Use `examples/24Barras` as the test case (or a dedicated test fixture derived from it)
3. Test must be runnable via `cargo test` in CI
4. Test configuration: `EngMax=1`

### Test Scenarios

1. **EngMax enabled**: Run with `EngMax=1`, verify output file is created
2. **Output verification**: Confirm `resultado_engmax.csv` exists and has correct structure

### Expected Behavior

- With `EngMax=1`, the output file should be created
- The file should have the correct header and data rows

## Acceptance Criteria

- [ ] Given `examples/24Barras` with `EngMax=1`, when `cargo test test_engmax_end_to_end` runs, then the test passes
- [ ] Given the test completes, when `RESULTADOS/resultado_engmax.csv` is read, then it contains a valid CSV header
- [ ] Given two runs (EngMax=0 vs EngMax=1), when the RESULTADOS directory is inspected, then `resultado_engmax.csv` only exists in the EngMax=1 run
- [ ] Given `cargo test --workspace`, when run, then the engmax integration test passes

## Implementation Guide

### Suggested Approach

1. Create integration test file `crates/labdessem-cli/tests/test_engmax_integration.rs`
2. Copy `examples/24Barras` to a temp directory for test isolation
3. Modify `study_config.json` programmatically to set `EngMax=1`
4. Run the solver via CLI or public API
5. Assert the output file exists and has correct structure

### Key Files to Modify

- `crates/labdessem-cli/tests/test_engmax_integration.rs` — NEW

### Patterns to Follow

Reference existing integration tests in the workspace.

### Pitfalls to Avoid

- Do NOT modify `examples/24Barras` directly — copy to temp dir
- Do NOT hardcode absolute paths — use relative paths from test context

## Testing Requirements

### Integration Tests

This ticket IS the integration test. The deliverable is the test itself.

Test function names:
- `test_engmax_output_created_when_enabled`

## Dependencies

- **Blocked By**: ticket-011-write-engmax-output-csv.md
- **Blocks**: None (final ticket)

## Effort Estimate

**Points**: 2
**Confidence**: High
