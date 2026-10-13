# Epic 06: Model Constraints Split

## Overview

Split `constraints.rs` (1500 lines) in `labdessem-model` into domain-specific
modules for better maintainability.

## Goals

- Create `constraints/` directory
- Separate thermal, hydro, network, demand, and limits constraints
- Reduce cognitive load when working on specific constraint types

## Architecture

### After

```
labdessem-model/src/
├── lib.rs
├── model.rs            # Model struct (extracted from lib.rs)
├── indexing.rs
├── variables.rs
├── objective.rs
└── constraints/
    ├── mod.rs          # Re-exports, add_all_constraints()
    ├── thermal.rs      # add_thermal_constraints()
    ├── hydro.rs        # add_hydro_constraints()
    ├── network.rs      # add_network_constraints()
    ├── demand.rs       # add_demand_balance_constraints()
    └── limits.rs       # add_operational_limit_constraints()
```

## Tickets

| ID | Title | Est. | Dependencies |
|----|-------|------|--------------|
| 001 | Create constraints module structure | 20min | Epic 05 |
| 002 | Extract thermal constraints | 30min | 001 |
| 003 | Extract hydro constraints | 30min | 001 |
| 004 | Extract network and demand constraints | 30min | 001 |

## Success Criteria

- Original `constraints.rs` deleted
- Each constraint file under 400 lines
- `cargo test` passes

## Estimated Effort

**Total**: 3-4 hours
