# Epic 03: IO Readers Directory

## Overview

Split the monolithic `study.rs` (2100 lines) in `labdessem-io` into a `readers/`
directory with one file per entity type, improving maintainability and enabling
easier testing of individual readers.

## Goals

- Create `readers/` directory with dedicated reader modules
- Extract thermal plant reading logic to `readers/thermal.rs`
- Extract hydro plant reading logic to `readers/hydro.rs`
- Extract renewable plant reading logic to `readers/renewable.rs`
- Extract pumping, submarket, network readers
- Reduce `study.rs` to orchestration-only (~200 lines)

## Scope

### In Scope

- Splitting `study.rs` into multiple reader files
- Creating `readers/mod.rs` with public API
- Moving CSV parsing logic to appropriate reader modules

### Out of Scope

- Changing CSV format or parsing behavior
- Adding new validation logic
- Modifying the `SystemBuilder` API

## Architecture

### Before

```
labdessem-io/src/
├── study.rs     # 2100 lines - all reading logic
└── lib.rs
```

### After

```
labdessem-io/src/
├── readers/
│   ├── mod.rs         # Re-exports
│   ├── thermal.rs     # read_thermal_plants()
│   ├── hydro.rs       # read_hydro_plants()
│   ├── renewable.rs   # read_renewable_plants()
│   ├── pumping.rs     # read_pumping_plants()
│   ├── submarket.rs   # read_submarkets()
│   ├── network.rs     # read_buses(), read_branches()
│   └── limits.rs      # read_operational_limits(), read_residual_costs()
├── study.rs           # Orchestration only (~200 lines)
└── lib.rs
```

## Tickets

| ID | Title | Est. | Dependencies |
|----|-------|------|--------------|
| 001 | Create readers module structure | 20min | Epic 02 |
| 002 | Extract thermal reader | 45min | 001 |
| 003 | Extract hydro reader | 60min | 001 |
| 004 | Extract renewable and pumping readers | 30min | 001 |
| 005 | Extract network readers | 30min | 001 |
| 006 | Simplify study.rs orchestration | 30min | 002-005 |

## Success Criteria

- All tests pass
- `study.rs` under 300 lines
- Each reader file under 500 lines
- `read_study_from_path()` still works identically
- No changes to public API

## Estimated Effort

**Total**: 6-8 hours
