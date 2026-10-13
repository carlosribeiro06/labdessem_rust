# Master Plan: Architecture Refactor

## Executive Summary

Refactor the labdessem workspace to follow professional software engineering patterns,
improving maintainability, testability, and code organization. Based on analysis of
cobre-rs patterns, this refactor reorganizes entities, separates concerns, improves
error handling, and adds architectural documentation.

## Goals & Non-Goals

### Goals

- Reorganize `labdessem-core` entities into dedicated `entities/` directory
- Implement `SystemBuilder` pattern for validated construction
- Replace generic string errors with typed `ValidationError` variants
- Split monolithic `study.rs` (2100 lines) into `readers/` modules
- Move CSV writers from `labdessem-simulation` to `labdessem-io/output/`
- Split `labdessem-simulation/lib.rs` (3000 lines) into focused modules
- Split `labdessem-model/constraints.rs` into type-specific modules
- Create `ARCHITECTURE.md` documenting workspace structure

### Non-Goals

- Adding new model features or constraints
- Changing solver backend or optimization logic
- Modifying input/output file formats
- Performance optimization (pure refactor)

## Architecture Overview

### Current State

```
labdessem-core/src/
├── hydro.rs, thermal.rs, renewable.rs  # Entities scattered
├── system.rs                            # 1200 lines, mixed concerns
└── error.rs                             # Generic string errors

labdessem-io/src/
├── study.rs                             # 2100 lines monolith
└── lib.rs

labdessem-simulation/src/
└── lib.rs                               # 3000 lines, simulation + output

labdessem-model/src/
├── constraints.rs                       # 1500 lines
└── lib.rs                               # 1400 lines
```

### Target State

```
labdessem-core/src/
├── entities/
│   ├── mod.rs, thermal.rs, hydro.rs, renewable.rs
│   ├── pumping.rs, bus.rs, branch.rs, submarket.rs
├── system/
│   ├── mod.rs, builder.rs, validate.rs
├── horizon.rs
└── error.rs                             # Typed ValidationError

labdessem-io/src/
├── readers/
│   ├── mod.rs, thermal.rs, hydro.rs, renewable.rs
│   ├── pumping.rs, submarket.rs, network.rs, limits.rs
├── output/
│   ├── mod.rs, thermal.rs, hydro.rs, renewable.rs
│   ├── pumping.rs, network.rs, summary.rs
├── validation/
│   ├── mod.rs, referential.rs
└── study.rs                             # Orchestration only

labdessem-simulation/src/
├── lib.rs                               # Re-exports only
├── workflow.rs, iteration.rs
├── network_cuts.rs, dual_extraction.rs
└── results.rs

labdessem-model/src/
├── constraints/
│   ├── mod.rs, thermal.rs, hydro.rs
│   ├── network.rs, demand.rs, limits.rs
└── model.rs

docs/
└── ARCHITECTURE.md
```

### Key Design Decisions

1. **Entity-per-file**: Each domain entity type gets its own file for discoverability
2. **SystemBuilder**: Separates construction from validation, enables fluent API
3. **Typed errors**: Structured error variants with context for better debugging
4. **IO owns output**: CSV writers belong in labdessem-io, not simulation
5. **Modules by domain**: Constraints grouped by thermal/hydro/network, not by type

## Technical Approach

### Tech Stack

- Rust (existing workspace)
- No new dependencies required

### Component/Module Breakdown

| Component | Lines | Action |
|-----------|-------|--------|
| `labdessem-core/system.rs` | ~1200 | Split into `entities/` + `system/` |
| `labdessem-io/study.rs` | ~2100 | Extract to `readers/` modules |
| `labdessem-simulation/lib.rs` | ~3000 | Split into workflow/iteration/results |
| `labdessem-model/constraints.rs` | ~1500 | Split by domain (thermal/hydro/network) |

### Data Flow

```
study_config.json + CSVs
        │
        ▼
    readers/*        (labdessem-io)
        │
        ▼
   SystemBuilder     (labdessem-core)
        │
        ▼
     System          (labdessem-core, immutable)
        │
        ▼
     Model           (labdessem-model)
        │
        ▼
    Solver           (labdessem-solver)
        │
        ▼
   workflow/*        (labdessem-simulation)
        │
        ▼
    output/*         (labdessem-io)
```

### Testing Strategy

- All existing tests must pass after each epic
- No new test coverage required (pure refactor)
- Run `cargo test` after each ticket
- Run `cargo check --all-targets` for compilation verification

## Phases & Milestones

| Phase | Epic | Description | Est. Hours |
|-------|------|-------------|------------|
| 1 | Epic 1 | Core entities directory | 2-3h |
| 1 | Epic 2 | Core system + builder | 3-4h |
| 2 | Epic 3 | IO readers directory | 6-8h |
| 2 | Epic 4 | IO output directory | 4-6h |
| 3 | Epic 5 | Simulation split | 4-5h |
| 3 | Epic 6 | Model constraints split | 3-4h |
| 4 | Epic 7 | Documentation | 2h |
| **Total** | | | **24-32h** |

## Risk Analysis

| Risk | Likelihood | Impact | Mitigation |
|------|------------|--------|------------|
| Breaking changes to public API | Medium | High | Keep re-exports in lib.rs |
| Circular dependencies | Low | High | Plan module structure carefully |
| Test failures from moved code | Medium | Medium | Run tests after each ticket |
| Merge conflicts with develop | Low | Medium | Small, focused commits |

## Success Metrics

- All 32 existing tests pass
- `cargo clippy` clean
- No increase in compile time > 10%
- Each crate's lib.rs under 200 lines
- No file over 1000 lines (down from 3000)
