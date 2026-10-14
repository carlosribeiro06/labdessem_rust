# [003] Extract hydro reader

## Context

### Background

Hydro plant reading is the most complex, involving multiple CSV files:
- `CAD_UHE.csv` - hydro plants
- `CAD_CONJ_UHE.csv` - hydro groups
- `OPER_VAZAO.csv` - inflows
- `OPER_FPHA.csv` - FPHA cuts (optional)

### Relation to Epic

Third ticket in Epic 03. Extracts the largest reader.

### Current State

Functions in `study.rs`:
- `build_hydro_plants()` - main construction
- Multiple `*Row` structs for CSV parsing
- FPHA reading logic
- Cascade relationship handling

## Specification

### Requirements

1. Create `readers/hydro.rs`
2. Move all hydro-related structs and functions
3. Export `read_hydro_plants()` as public API
4. Handle FPHA optionally based on flag

### Outputs/Behavior

```rust
// readers/hydro.rs
pub fn read_hydro_plants(
    cad_path: &Path,
    oper_path: &Path,
    submarkets: &[Submarket],
    horizon: &StudyHorizon,
    network_enabled: bool,
    fpha_enabled: bool,
) -> Result<Vec<HydroPlant>, IoError>
```

## Acceptance Criteria

- [ ] `readers/hydro.rs` exists with `read_hydro_plants()`
- [ ] All hydro row structs moved
- [ ] FPHA reading included
- [ ] `cargo test` passes

## Implementation Guide

### Suggested Approach

1. Create `readers/hydro.rs`
2. Move hydro-related items:
   - `HydroPlantRow`, `HydroGroupRow`, `InflowRow`, `FphaRow` structs
   - `build_hydro_plants()` logic
   - FPHA reading logic
3. Keep cascade cycle detection in core (validation)

### Key Files to Create

- `crates/labdessem-io/src/readers/hydro.rs`

### Key Files to Modify

- `crates/labdessem-io/src/readers/mod.rs`
- `crates/labdessem-io/src/study.rs`

## Dependencies

- **Blocked By**: 001
- **Blocks**: 006

## Effort Estimate

**Points**: 3
**Confidence**: Medium
