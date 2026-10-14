# [004] Extract renewable and pumping readers

## Context

### Background

Renewable and pumping plant readers are smaller and simpler. They can be
extracted together in a single ticket.

### Relation to Epic

Fourth ticket in Epic 03. Extracts two simpler readers.

### Current State

- `build_renewable_plants()` reads `CAD_REN.csv` and `OPER_REN.csv`
- `build_pumping_plants()` reads `CAD_USIE.csv`

## Specification

### Requirements

1. Create `readers/renewable.rs` with `read_renewable_plants()`
2. Create `readers/pumping.rs` with `read_pumping_plants()`
3. Move related row structs
4. Update `study.rs` to use new readers

### Outputs/Behavior

```rust
// readers/renewable.rs
pub fn read_renewable_plants(
    cad_path: &Path,
    oper_path: &Path,
    submarkets: &[Submarket],
    horizon: &StudyHorizon,
    network_enabled: bool,
) -> Result<Vec<RenewablePlant>, IoError>

// readers/pumping.rs
pub fn read_pumping_plants(
    cad_path: &Path,
    hydro_plants: &[HydroPlant],
    submarkets: &[Submarket],
    period_duration_hours: f64,
    network_enabled: bool,
) -> Result<Vec<PumpingPlant>, IoError>
```

## Acceptance Criteria

- [ ] Both files exist with public functions
- [ ] Related row structs moved
- [ ] `cargo test` passes

## Implementation Guide

### Key Files to Create

- `crates/labdessem-io/src/readers/renewable.rs`
- `crates/labdessem-io/src/readers/pumping.rs`

### Key Files to Modify

- `crates/labdessem-io/src/readers/mod.rs`
- `crates/labdessem-io/src/study.rs`

## Dependencies

- **Blocked By**: 001
- **Blocks**: 006

## Effort Estimate

**Points**: 2
**Confidence**: High
