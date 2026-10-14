# [002] Extract thermal reader

## Context

### Background

Thermal plant reading involves parsing `CAD_UNID_UTE.csv` and optionally
`CAD_RAMPAS_TERMICAS.csv`. This logic should be in its own module.

### Relation to Epic

Second ticket in Epic 03. Extracts the first and most complex reader.

### Current State

Functions in `study.rs`:
- `build_thermal_plants()` - main construction logic
- `read_thermal_trajectory_table()` - reads ramp CSV
- `ThermalUnitRow` struct - CSV row representation
- Supporting helper functions

## Specification

### Requirements

1. Create `readers/thermal.rs`
2. Move thermal-related structs and functions from `study.rs`
3. Export `read_thermal_plants()` as the public API
4. Update `study.rs` to use `readers::thermal::read_thermal_plants()`

### Outputs/Behavior

```rust
// readers/thermal.rs
pub fn read_thermal_plants(
    cad_path: &Path,
    submarkets: &[Submarket],
    period_duration_hours: f64,
    network_enabled: bool,
    uct_enabled: bool,
) -> Result<Vec<ThermalPlant>, IoError>
```

## Acceptance Criteria

- [ ] `readers/thermal.rs` exists with `read_thermal_plants()`
- [ ] `ThermalUnitRow` and trajectory reading is in `thermal.rs`
- [ ] `study.rs` imports and uses `read_thermal_plants()`
- [ ] `cargo test -p labdessem-io` passes
- [ ] `cargo test` passes (full workspace)

## Implementation Guide

### Suggested Approach

1. Create `readers/thermal.rs`
2. Move these items from `study.rs`:
   - `ThermalUnitRow` struct
   - `read_thermal_trajectory_table()` (rename to `read_trajectories()`)
   - `build_thermal_plants()` logic → integrate into `read_thermal_plants()`
3. Add appropriate `use` statements:
   ```rust
   use std::collections::HashMap;
   use std::path::Path;
   use labdessem_core::entities::{ThermalPlant, ThermalUnit, ThermalInitialCondition};
   use labdessem_core::ids::{BusId, SubmarketId, ThermalPlantId, ThermalUnitId};
   use crate::error::IoError;
   ```
4. Update `readers/mod.rs`:
   ```rust
   pub mod thermal;
   pub use thermal::read_thermal_plants;
   ```
5. Update `study.rs` to call `readers::read_thermal_plants()`

### Key Files to Create

- `crates/labdessem-io/src/readers/thermal.rs`

### Key Files to Modify

- `crates/labdessem-io/src/readers/mod.rs` - add module
- `crates/labdessem-io/src/study.rs` - remove thermal code, import from readers

### Pitfalls to Avoid

- Ensure all helper functions used by thermal reading are also moved
- Check for shared utilities that should stay in `study.rs`
- Keep CSV delimiter configuration consistent

## Testing Requirements

### Unit Tests

Existing thermal tests should continue to pass.

### Integration Tests

- `cargo run -p labdessem-cli -- run examples/24Barras`

## Dependencies

- **Blocked By**: 001
- **Blocks**: 006

## Effort Estimate

**Points**: 2
**Confidence**: Medium
