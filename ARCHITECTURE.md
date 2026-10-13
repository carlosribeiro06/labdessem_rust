# Architecture

Labdessem is a Cargo workspace of focused Rust crates for hydrothermal
dispatch optimization. This document describes the workspace structure
and the responsibility of each crate.

## Overview

The workspace solves day-ahead hydrothermal scheduling problems with:
- Thermal unit commitment with startup/shutdown trajectories
- Hydro cascade modeling with FPHA cuts
- Renewable generation dispatch
- Optional DC network flow constraints
- Future cost function (Benders cuts)

## Crate Responsibilities

### Foundation

**labdessem-core** — Domain model: thermal/hydro/renewable/pumping plants,
buses, branches, submarkets, the temporal model (`StudyHorizon`), and the
immutable `System` container. Provides `SystemBuilder` for validated
construction and `ValidationError` for structured error reporting.
No solver or I/O dependencies.

```
labdessem-core/src/
├── entities/          # One file per entity type
│   ├── thermal.rs
│   ├── hydro.rs
│   ├── renewable.rs
│   ├── pumping.rs
│   ├── bus.rs
│   ├── branch.rs
│   └── submarket.rs
├── system/
│   ├── mod.rs         # System struct
│   ├── builder.rs     # SystemBuilder
│   └── validate.rs    # validate_system()
├── ids.rs             # Typed IDs (ThermalPlantId, etc.)
└── error.rs           # CoreError, ValidationError
```

### Infrastructure

**labdessem-solver** — Solver abstraction via `good_lp` with HiGHS backend.
Provides `solve_lp_from_path()` and `solve_milp_from_path()` for LP and
MILP problems.

### I/O

**labdessem-io** — Case directory reader and result writer. Loads
`study_config.json` and CSV files from `CAD/` and `OPER/` directories,
builds `System` via entity readers. Writes result CSVs to `RESULTADOS/`.

```
labdessem-io/src/
├── readers/
│   ├── common.rs      # read_csv(), log utilities
│   └── mod.rs         # Reader infrastructure
├── study.rs           # Study loading orchestration
└── error.rs           # IoError
```

### Algorithm

**labdessem-model** — Optimization model construction: indexing, variables,
objective function, and constraints. Transforms `System` into a solvable
mathematical program.

```
labdessem-model/src/
├── indexing.rs        # Variable/constraint indexing
├── variables.rs       # Decision variables
├── objective.rs       # Objective function
└── constraints.rs     # All constraints
```

**labdessem-simulation** — End-to-end orchestration: iterative solve
workflow (LP → MILP → LP-FIXED → LP-CALC-CMO), network cut addition,
dual extraction, and result CSV generation.

### Entry Point

**labdessem-cli** — Command-line interface producing the `labdessem` binary.
Provides `labdessem run <case_dir>` subcommand with banner display and
TTY-aware colored output.

## Dependency Graph

```
                    ┌─────────────────┐
                    │  labdessem-core │
                    │    (domain)     │
                    └────────┬────────┘
                             │
           ┌─────────────────┼─────────────────┐
           │                 │                 │
           ▼                 ▼                 ▼
    ┌─────────────┐   ┌─────────────┐   ┌─────────────┐
    │labdessem-io │   │labdessem-   │   │labdessem-   │
    │  (I/O)      │   │   model     │   │   solver    │
    │             │   │(optimization)│  │  (HiGHS)    │
    └──────┬──────┘   └──────┬──────┘   └──────┬──────┘
           │                 │                 │
           └─────────────────┼─────────────────┘
                             │
                             ▼
                  ┌─────────────────────┐
                  │labdessem-simulation │
                  │  (orchestration)    │
                  └──────────┬──────────┘
                             │
                             ▼
                    ┌─────────────────┐
                    │  labdessem-cli  │
                    │   (entry)       │
                    └─────────────────┘
```

## Design Principles

1. **Solver-agnostic core**: `labdessem-core` has no solver dependencies.
   The domain model is pure data structures and validation logic.

2. **Validate at construction**: Invalid states are caught when building
   `System` via `SystemBuilder`, not at solve time. The `System` is
   immutable after construction.

3. **I/O isolation**: All file I/O (CSV reading, result writing) is in
   `labdessem-io`. Other crates work with in-memory data structures.

4. **Typed IDs**: Each entity type has its own ID type (e.g., `ThermalPlantId`,
   `HydroPlantId`) preventing accidental mixing of IDs.

5. **Structured errors**: `ValidationError` provides typed variants with
   context (entity type, field name, expected vs actual values) for
   better error messages and programmatic handling.

## Data Flow

```
┌─────────────────────────────────────────────────────────────────┐
│                        Case Directory                           │
│  study_config.json + CAD/*.csv + OPER/*.csv + OPER_FCF.bin     │
└─────────────────────────────────────────────────────────────────┘
                                │
                                ▼
                    ┌───────────────────────┐
                    │    labdessem-io       │
                    │  read_study_from_path │
                    └───────────┬───────────┘
                                │
                                ▼
                    ┌───────────────────────┐
                    │   labdessem-core      │
                    │   System (immutable)  │
                    └───────────┬───────────┘
                                │
                    ┌───────────┴───────────┐
                    │                       │
                    ▼                       ▼
          ┌─────────────────┐     ┌─────────────────┐
          │ labdessem-model │     │ labdessem-solver│
          │  build Model    │────▶│   solve LP/MILP │
          └─────────────────┘     └────────┬────────┘
                                           │
                                           ▼
                              ┌────────────────────────┐
                              │  labdessem-simulation  │
                              │   workflow execution   │
                              │   (iterate, add cuts)  │
                              └────────────┬───────────┘
                                           │
                                           ▼
                              ┌────────────────────────┐
                              │      RESULTADOS/       │
                              │   resultado_*.csv      │
                              └────────────────────────┘
```

## Execution Modes

The study configuration supports two execution modes:

- **Mode 1 (Iterative)**: `LP → MILP → LP-FIXED → LP-CALC-CMO`
  - LP iterations add network cuts for violated flows
  - Final LP-CALC-CMO extracts dual values (CMO, water value)

- **Mode 2 (Single MILP)**: Direct MILP solve without iteration

## Key Types

| Type | Crate | Description |
|------|-------|-------------|
| `System` | core | Immutable container for all power system data |
| `SystemBuilder` | core | Fluent builder for `System` construction |
| `StudyHorizon` | core | Temporal discretization (periods, durations) |
| `ValidationError` | core | Typed validation errors with context |
| `StudyConfig` | io | JSON configuration (execution mode, flags) |
| `IoError` | io | I/O and parsing errors |

## Testing

Run the test suite:

```bash
cargo test
```

Note: One test (`solves_milp_for_example_case` in labdessem-solver) may fail
due to solver precision issues. This is a known issue being investigated.

## Building

```bash
# Install CLI
cargo install --path crates/labdessem-cli

# Run a case
labdessem run examples/24Barras

# Development run
cargo run -p labdessem-cli -- run examples/24Barras
```
