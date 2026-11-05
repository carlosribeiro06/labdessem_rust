# Architecture

Labdessem is a Cargo workspace of focused Rust crates for hydrothermal
dispatch optimization. This document describes the workspace structure
and the responsibility of each crate.

## Overview

The workspace solves day-ahead hydrothermal scheduling problems with:
- Thermal unit commitment with startup/shutdown trajectories
- Hydro cascade modeling with FPHA cuts (tabulated, or computed at runtime
  from the reservoir geometry)
- Dynamic maximum swallowing (EngMax), recalculated after every solve
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
`study_config.json`, CSV files from `CAD/` and `OPER/` and the optional
`inflow_history.parquet`, builds `System` via entity readers. Writes every
result CSV to `RESULTADOS/` through `write_all_results`.

```
labdessem-io/src/
├── readers/
│   ├── mod.rs         # Reader orchestration
│   ├── common.rs      # read_csv(), log utilities
│   ├── horizon.rs     # StudyHorizon parsing
│   ├── thermal.rs     # Thermal plants and ramps
│   ├── hydro.rs       # Hydro plants, groups, units, FPHA
│   ├── fpha_geometry.rs   # VHA geometry and tailrace tables
│   ├── inflow_history.rs  # Inflow history (Parquet)
│   ├── renewable.rs   # Renewable plants
│   ├── pumping.rs     # Pumping plants
│   ├── submarket.rs   # Submarkets and demand
│   ├── network.rs     # Buses and branches
│   ├── limits.rs      # Operational limits
│   └── future_cost.rs # FCF binary file reader
├── output/
│   ├── mod.rs         # Output module exports
│   ├── common.rs      # csv_with_header(), write_csv_file()
│   ├── helpers.rs     # Period conversion utilities
│   ├── orchestrator.rs # write_all_results
│   ├── hydro.rs       # Hydro results, including the EngMax columns
│   ├── thermal.rs     # Thermal results writer
│   ├── renewable.rs   # Renewable results writer
│   ├── pumping.rs     # Pumping results writer
│   ├── summary.rs     # CMO and FCF economic results
│   ├── network.rs     # Network flow and nodal CMO results
│   ├── process.rs     # Iteration process results
│   ├── fpha_computed.rs # Computed FPHA hyperplanes
│   ├── infeasibility.rs # Infeasibility reports
│   └── limits.rs      # Operational limits results
├── study.rs           # Study loading orchestration
└── error.rs           # IoError
```

### Algorithm

**labdessem-uc** — Solver-agnostic unit commitment formulation: thermal
commitment, startup and shutdown variables and constraints, and hydro
commitment with its channeling constraints. Used by `labdessem-model`.

**labdessem-network** — DC network model and flow algorithms: incidence and
susceptance matrices, PTDF computation, line flow evaluation, violation
detection, flow cut constraints, and CMB/CMO dual extraction. Solution
values arrive as plain slices, so the crate has no solver dependency.

**labdessem-fpha** — FPHA fitting pipeline. Computes hydro production
hyperplanes from the VHA geometry (convex hull via Qhull, α correction, γₛ
secant, plane reduction, deviation metrics). Also holds the net head and
maximum swallowing calculator used by EngMax.

**labdessem-model** — Optimization model construction: indexing, variables,
objective function, and constraints. Transforms `System` into a solvable
mathematical program. `ConstraintOffsetParams::from_system` derives the row
offsets of the model for a given solve mode.

```
labdessem-model/src/
├── lib.rs             # Public API re-exports
├── model.rs           # Model struct and SolveMode enum
├── indexing.rs        # Variable/constraint indexing and row offsets
├── error.rs           # ModelError
├── variables.rs       # Decision variables
├── objective.rs       # Objective function
├── names.rs           # Variable/constraint naming functions
├── constraints/
│   ├── mod.rs         # Constraint orchestration
│   ├── thermal.rs     # Thermal unit commitment constraints
│   ├── hydro.rs       # Hydro balance, generation and EngMax rows
│   ├── demand.rs      # Submarket demand balance
│   └── limits.rs      # Operational limit constraints
└── tests.rs           # Unit tests
```

**labdessem-simulation** — End-to-end orchestration: iterative solve
workflow (LP → MILP → LP-FIXED → LP-CALC-CMO), network cut iterations
(using `labdessem-network`), dual extraction, and the EngMax bounds that
are recalculated after every solve. Result files are written by
`labdessem-io`.

```
labdessem-simulation/src/
├── lib.rs             # Public API (run_simulation, run_simulation_with_engmax)
├── stages/
│   ├── mod.rs
│   ├── orchestration.rs # run_simulation and the per-option workflows
│   ├── solve.rs       # solve_stage, solve_stage_without_network
│   ├── helpers.rs     # CMB/CMO assembly, cut and violation helpers
│   └── common.rs      # Shared constants and column helpers
├── engmax.rs          # EngMaxState: plant-level bounds, recalculated per solve
├── flow_units.rs      # hm3 to m3/s conversion
├── workflow.rs        # ExecutionOption and IterationStage enums
├── results.rs         # Result structures (IterativeSimulationResult, EngMaxOutcome, etc.)
└── error.rs           # SimulationError
```

### Entry Point

**labdessem-cli** — Command-line interface producing the `labdessem` binary.
Provides `labdessem run <case_dir>` subcommand with banner display and
TTY-aware colored output. Installs one `tracing` subscriber on stderr: WARN
by default, `RUST_LOG` overrides the level. Prints the banner and the run
summary, including the EngMax consistency line, on stderr.

## Dependency Graph

Each crate depends only on the crates in its row (library dependencies; test-only
dependencies are left out).

| Crate | Depends on |
|-------|------------|
| `labdessem-core` | none |
| `labdessem-uc` | core |
| `labdessem-network` | core |
| `labdessem-model` | core, uc |
| `labdessem-solver` | model |
| `labdessem-io` | core, model, solver |
| `labdessem-fpha` | core, io |
| `labdessem-simulation` | core, fpha, model, network, solver |
| `labdessem-cli` | all of the above |

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
  - LP-CALC-CMO uses all flow constraints for dual extraction

- **Mode 2 (Single MILP)**: `MILP → LP-CALC-CMO`
  - MILP includes all network constraints upfront
  - LP-CALC-CMO extracts dual values with fixed commitments

Both modes use LP-CALC-CMO with all flow constraints to ensure valid dual extraction (CMO, water value).

## Key Types

| Type | Crate | Description |
|------|-------|-------------|
| `System` | core | Immutable container for all power system data |
| `SystemBuilder` | core | Fluent builder for `System` construction |
| `StudyHorizon` | core | Temporal discretization (periods, durations) |
| `ValidationError` | core | Typed validation errors with context |
| `StudyConfig` | io | JSON configuration (execution mode, flags) |
| `IoError` | io | I/O and parsing errors |
| `ModelError` | model | Model build errors (e.g. a missing EngMax bound) |
| `EngMaxState` | simulation | EngMax bounds per (plant, period), recalculated after every solve |
| `EngMaxOutcome` | simulation | Bound that built the final LP-CALC-CMO model and the bound recomputed from its solution |

## Testing

Run the test suite:

```bash
cargo test
```

The EngMax tests live in `labdessem-simulation` (`engmax.rs` and
`stages/engmax_tests.rs`); the FPHA fitting regression tests are in
`labdessem-fpha/tests`.

## Building

```bash
# Install CLI
cargo install --path crates/labdessem-cli

# Run a case
labdessem run examples/24Barras

# Development run
cargo run -p labdessem-cli -- run examples/24Barras
```
