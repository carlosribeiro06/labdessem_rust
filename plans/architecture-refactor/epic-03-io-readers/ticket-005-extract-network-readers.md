# [005] Extract network and limits readers

## Context

### Background

Network (buses, branches) and limits (operational limits, residual costs,
interchange) readers are the remaining extraction targets.

### Relation to Epic

Fifth ticket in Epic 03. Extracts remaining readers.

### Current State

- Bus reading from `OPER_SBM.csv` with network data
- Branch reading from network files
- Operational limits from `OPER_REST_LIM.csv`
- Residual costs from `OPER_CUSTO_RESIDUAL.csv`

## Specification

### Requirements

1. Create `readers/network.rs` with `read_buses()`, `read_branches()`
2. Create `readers/submarket.rs` with `read_submarkets()`
3. Create `readers/limits.rs` with `read_operational_limits()`, `read_residual_costs()`, `read_interchange_limits()`
4. Update `study.rs`

### Outputs/Behavior

```rust
// readers/network.rs
pub fn read_buses(oper_path: &Path, horizon: &StudyHorizon) -> Result<Vec<Bus>, IoError>
pub fn read_branches(oper_path: &Path) -> Result<Vec<Branch>, IoError>

// readers/submarket.rs
pub fn read_submarkets(
    cad_path: &Path,
    oper_path: &Path,
    horizon: &StudyHorizon,
) -> Result<Vec<Submarket>, IoError>

// readers/limits.rs
pub fn read_operational_limits(...) -> Result<Vec<OperationalLimit>, IoError>
pub fn read_residual_costs(oper_path: &Path) -> Result<Vec<ResidualCost>, IoError>
pub fn read_interchange_limits(oper_path: &Path) -> Result<Vec<InterchangeLimit>, IoError>
```

## Acceptance Criteria

- [ ] All three files created with public functions
- [ ] `cargo test` passes

## Implementation Guide

### Key Files to Create

- `crates/labdessem-io/src/readers/network.rs`
- `crates/labdessem-io/src/readers/submarket.rs`
- `crates/labdessem-io/src/readers/limits.rs`

### Key Files to Modify

- `crates/labdessem-io/src/readers/mod.rs`
- `crates/labdessem-io/src/study.rs`

## Dependencies

- **Blocked By**: 001
- **Blocks**: 006

## Effort Estimate

**Points**: 2
**Confidence**: Medium
