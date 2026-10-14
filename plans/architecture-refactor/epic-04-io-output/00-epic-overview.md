# Epic 04: IO Output Directory

## Overview

Move CSV result writers from `labdessem-simulation` to `labdessem-io/output/`,
following the principle that all I/O operations belong in the I/O crate.

## Goals

- Create `output/` directory in `labdessem-io`
- Move result writing functions from simulation
- Update simulation to call `labdessem_io::output::write_results()`

## Architecture

### Before

```
labdessem-simulation/src/lib.rs  # 3000 lines, includes all CSV writers
```

### After

```
labdessem-io/src/output/
├── mod.rs
├── thermal.rs     # write_thermal_results()
├── hydro.rs       # write_hydro_results()
├── renewable.rs   # write_renewable_results()
├── pumping.rs     # write_pumping_results()
├── network.rs     # write_network_results()
└── summary.rs     # write_summary(), write_iteration_log()
```

## Tickets

| ID | Title | Est. | Dependencies |
|----|-------|------|--------------|
| 001 | Create output module structure | 20min | Epic 03 |
| 002 | Move thermal and hydro writers | 45min | 001 |
| 003 | Move renewable and pumping writers | 30min | 001 |
| 004 | Move network and summary writers | 30min | 001 |
| 005 | Update simulation exports | 20min | 002-004 |

## Success Criteria

- All result CSV files still generated correctly
- `labdessem-simulation/lib.rs` reduced by ~800 lines
- `cargo test` passes

## Estimated Effort

**Total**: 4-6 hours
