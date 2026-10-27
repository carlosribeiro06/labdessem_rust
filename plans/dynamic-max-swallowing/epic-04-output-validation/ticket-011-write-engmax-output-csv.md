# TICKET-011 Write EngMax Output CSV

## Context

### Background

The EngMax feature computes dynamic maximum swallowing bounds per hydro plant and period based on head-dependent productivity. These bounds are used as upper limits on turbining variables in the optimization model. For auditability and post-processing, the computed bounds must be written to a CSV output file alongside other result files.

### Relation to Epic

This ticket delivers the output persistence layer for Epic 04. The validation ticket (012) depends on this output to verify end-to-end correctness.

### Current State

- `EngMaxBounds` struct exists in `labdessem-core/src/entities/engmax_bounds.rs` with fields: plant_idx, unit_idx, period, value
- `calculate_max_swallowing()` in `labdessem-fpha/src/calculator/mod.rs` returns computed bounds
- Output writers in `labdessem-io/src/writers/` follow the pattern: `build_*_records()` constructs row data, `write_*_results()` writes CSV
- `resultado_hidreletricas.csv` writer in `labdessem-io/src/writers/hydro.rs` serves as the reference pattern

## Specification

### Requirements

1. Create a writer module `labdessem-io/src/writers/engmax.rs` that exports computed EngMax bounds to CSV
2. Output file: `<case_path>/RESULTADOS/resultado_engmax.csv`
3. CSV columns (in order): `Usina`, `Unidade`, `Periodo`, `EngMax_hm3`
4. Column headers must match existing output naming conventions (Portuguese, no spaces)
5. Numeric precision: 6 decimal places for `EngMax_hm3`
6. Writer is only called when `study_config.EngMax == 1`

### Inputs

- `bounds: &EngMaxBounds` — computed bounds from the simulation
- `system: &System` — for plant names
- `output_dir: &Path` — the RESULTADOS directory path

### Outputs

- `resultado_engmax.csv` file written to `output_dir`
- Returns `Result<(), io::Error>`

### Error Handling

- Propagate I/O errors with context
- Log file path on write success at `info` level using `tracing`

## Acceptance Criteria

- [ ] Given an `EngMaxBounds` with bounds for 1 plant with 1 unit across 2 periods, when `write_engmax_results()` is called, then `resultado_engmax.csv` is created with 2 data rows plus 1 header row
- [ ] Given the output file exists, when opened, then the header row is `Usina,Unidade,Periodo,EngMax_hm3`
- [ ] Given a bound with `value = 1.234567890`, when written, then the value in CSV is `1.234568` (6 decimal places, rounded)
- [ ] Given `study_config.EngMax == 0`, when the simulation completes, then `resultado_engmax.csv` is NOT written
- [ ] Given a successful write, when logs are inspected, then an `info` message contains the output file path

## Implementation Guide

### Suggested Approach

1. Create `crates/labdessem-io/src/writers/engmax.rs`
2. Define `EngMaxRecord` struct with `#[derive(Serialize)]` for CSV serialization
3. Implement `build_engmax_records(bounds: &EngMaxBounds, system: &System) -> Vec<EngMaxRecord>`
4. Implement `write_engmax_results(records: &[EngMaxRecord], output_dir: &Path) -> Result<()>`
5. Register module in `crates/labdessem-io/src/writers/mod.rs`

### Key Files to Modify

- `crates/labdessem-io/src/writers/engmax.rs` — NEW
- `crates/labdessem-io/src/writers/mod.rs` — add `pub mod engmax;`

### Patterns to Follow

Reference `crates/labdessem-io/src/writers/hydro.rs`:
- Use `csv::Writer` for serialization
- Use `#[serde(rename = "...")]` for Portuguese column names
- Format floats inline during record construction

### Pitfalls to Avoid

- Do NOT call the writer when `EngMax == 0` — check config before calling
- Do NOT use `.unwrap()` on file operations — propagate errors

## Testing Requirements

### Unit Tests

In `crates/labdessem-io/src/writers/engmax.rs`:
- `test_build_engmax_records_formats_precision`
- `test_build_engmax_records_empty_bounds`

## Dependencies

- **Blocked By**: None (EngMaxBounds struct exists)
- **Blocks**: ticket-012-validate-end-to-end.md

## Effort Estimate

**Points**: 2
**Confidence**: High
