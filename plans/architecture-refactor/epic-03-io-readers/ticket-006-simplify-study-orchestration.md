# [006] Simplify study.rs orchestration

## Context

### Background

After extracting all readers, `study.rs` should only contain orchestration
logic that coordinates the readers and builds the system.

### Relation to Epic

Final ticket in Epic 03. Completes the reader extraction.

### Current State

After tickets 002-005, `study.rs` should have all reader functions removed
but may still have helper functions and configuration logic.

## Specification

### Requirements

1. Remove any remaining reader-specific code from `study.rs`
2. Ensure `read_study_from_path()` uses all reader modules
3. Use `SystemBuilder` to construct the system
4. Keep `study.rs` under 300 lines

### Outputs/Behavior

`study.rs` should look like:
```rust
//! Study loading orchestration.

use std::path::Path;
use labdessem_core::system::{System, SystemBuilder};
use crate::readers::*;
use crate::error::IoError;

pub fn read_study_from_path(case_path: impl AsRef<Path>) -> Result<System, IoError> {
    let case_path = case_path.as_ref();
    let config = read_study_config(case_path.join("study_config.json"))?;
    let cad_path = case_path.join("CAD");
    let oper_path = case_path.join("OPER");
    
    let horizon = read_horizon(&oper_path)?;
    let submarkets = read_submarkets(&cad_path, &oper_path, &horizon)?;
    let buses = read_buses(&oper_path, &horizon, config.rede != 0)?;
    // ... other readers
    
    SystemBuilder::new(horizon)
        .with_submarkets(submarkets)
        .with_buses(buses)
        // ... other with_* calls
        .build()
        .map_err(IoError::from)
}
```

## Acceptance Criteria

- [ ] `study.rs` under 300 lines
- [ ] All reading delegated to `readers/*`
- [ ] Uses `SystemBuilder`
- [ ] `cargo test` passes
- [ ] Example runs: `labdessem run examples/24Barras`

## Implementation Guide

### Suggested Approach

1. Review remaining code in `study.rs`
2. Move any remaining helpers to appropriate reader modules
3. Simplify `read_study_from_path()` to pure orchestration
4. Verify line count

### Key Files to Modify

- `crates/labdessem-io/src/study.rs`

### Verification

```bash
wc -l crates/labdessem-io/src/study.rs
# Should be < 300
```

## Dependencies

- **Blocked By**: 002, 003, 004, 005
- **Blocks**: Epic 04

## Effort Estimate

**Points**: 2
**Confidence**: High
