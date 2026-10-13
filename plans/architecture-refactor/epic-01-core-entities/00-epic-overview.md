# Epic 01: Core Entities Directory

## Overview

Reorganize all domain entities in `labdessem-core` into a dedicated `entities/` directory,
with one file per entity type for better discoverability and maintainability.

## Goals

- Create `entities/` directory with centralized module exports
- Move existing entity files (`thermal.rs`, `hydro.rs`, `renewable.rs`) into `entities/`
- Extract `PumpingPlant`, `Bus`, `Branch`, `Submarket` from `system.rs` into dedicated files
- Maintain all existing public API through re-exports in `lib.rs`

## Scope

### In Scope

- File reorganization within `labdessem-core`
- Creating new entity files for types currently in `system.rs`
- Updating imports across all crates that depend on `labdessem-core`

### Out of Scope

- Changing entity struct definitions
- Adding new fields or methods
- Modifying validation logic

## Architecture

### Before

```
labdessem-core/src/
├── lib.rs
├── thermal.rs       # ThermalPlant, ThermalUnit, ThermalInitialCondition
├── hydro.rs         # HydroPlant, HydroGroup, HydroUnit, ...
├── renewable.rs     # RenewablePlant
├── system.rs        # System + PumpingPlant + Bus + Branch + Submarket + ...
├── ids.rs
└── error.rs
```

### After

```
labdessem-core/src/
├── lib.rs           # Re-exports from entities/*
├── entities/
│   ├── mod.rs       # Centralized exports
│   ├── thermal.rs
│   ├── hydro.rs
│   ├── renewable.rs
│   ├── pumping.rs   # NEW: extracted from system.rs
│   ├── bus.rs       # NEW: extracted from system.rs
│   ├── branch.rs    # NEW: extracted from system.rs
│   └── submarket.rs # NEW: extracted from system.rs
├── system.rs        # System struct only (reduced)
├── ids.rs
└── error.rs
```

## Tickets

| ID | Title | Est. | Dependencies |
|----|-------|------|--------------|
| 001 | Create entities module structure | 15min | - |
| 002 | Move thermal entity | 15min | 001 |
| 003 | Move hydro entity | 15min | 001 |
| 004 | Extract network entities from system.rs | 30min | 001 |

## Success Criteria

- All tests pass (`cargo test`)
- No changes to public API (same types exported from `labdessem_core`)
- `system.rs` reduced by ~400 lines
- Each entity file under 300 lines

## Estimated Effort

**Total**: 2-3 hours
