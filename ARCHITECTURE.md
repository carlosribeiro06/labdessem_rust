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
`inflow_history.parquet`, builds `System` via entity readers. Writes the
result CSVs to `RESULTADOS/` through `write_all_results`, and
`resultado_fpha_computed.csv` through `write_fpha_computed`.

```
labdessem-io/src/
├── readers/
│   ├── mod.rs         # Reader orchestration
│   ├── common.rs      # read_csv(), log utilities
│   ├── horizon.rs     # StudyHorizon parsing
│   ├── thermal.rs     # Thermal plants and ramps
│   ├── hydro.rs       # Hydro plants, groups, units, FPHA
│   ├── fpha_geometry.rs   # FPHA fitting config (read_fpha_config); crate-private VHA geometry and tailrace readers
│   ├── inflow_history.rs  # Inflow history (Parquet)
│   ├── parquet_helpers.rs # Shared Parquet reader helpers
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
│   ├── hydro.rs       # Hydro results, including the max swallowing columns
│   ├── thermal.rs     # Thermal results writer
│   ├── renewable.rs   # Renewable results writer
│   ├── pumping.rs     # Pumping results writer
│   ├── summary.rs     # CMO and FCF economic results
│   ├── network.rs     # Network flow and nodal CMO results
│   ├── process.rs     # Iteration process results
│   ├── fpha_computed.rs # write_fpha_computed: the hyperplanes of the model fit
│   ├── infeasibility.rs # Infeasibility reports
│   ├── limits.rs      # Operational limits results
│   └── types.rs       # Output data transfer objects
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

**labdessem-model** — Optimization model construction: indexing, variables,
objective function, and constraints. Transforms `System` into a solvable
mathematical program. `ConstraintOffsetParams::from_system` derives the row
offsets of the model for a given solve mode. The crate also owns the hydro
production physics:

- `production` — the FPHA fit. `fit_system` fits every hydro plant with
  generating units and returns typed results (`SystemFit`, `PlantFit`,
  `FitFailureReason`); it prints nothing. `fit_plant` runs one plant in this
  order: volume window, convex hull via Qhull, γₛ secant, plane reduction
  within the plane limit, α correction fitted on 30 evaluation points of the
  final planes, deviation metrics and validation of the fitted planes. The
  defaults are at most 10 planes, a 5 × 5 volume-flow grid, an angle
  tolerance of 1.0 degree and a volume window over the whole `[Vmin, Vmax]`
  interval of the plant, which `CAD_FPHA_CONFIG.csv` changes per plant. The
  plane limit is enforced by repeating the reduction with a doubled
  tolerance.
- `swallowing` — the maximum swallowing formula (`calculate_max_swallowing`)
  and the plant head model used by the simulation's max swallowing bounds.
- `hydro_geometry` — the VHA forebay table, the tailrace polynomial and the
  net head, shared by `production` and `swallowing`.
- `hull` — the crate-private Qhull binding, the only module where `unsafe`
  is allowed. `build.rs` compiles the vendored qhull C sources of the
  `vendor/qhull` submodule.

```
labdessem-model/
├── build.rs           # Compiles the vendored qhull C sources and the C shim
├── csrc/              # C shim over libqhull_r (qhull_wrapper.c, qhull_wrapper.h)
├── vendor/qhull/      # Vendored qhull sources (git submodule)
├── tests/
│   ├── production_integration.rs # FPHA fit on synthetic plants and the 24Barras case
│   ├── production_regression.rs  # Fit determinism and numerical stability
│   └── unsafe_scope.rs           # Unsafe-code boundary of the crate
└── src/
    ├── lib.rs             # Public API re-exports
    ├── model.rs           # Model struct and SolveMode enum
    ├── indexing.rs        # Variable/constraint indexing and row offsets
    ├── error.rs           # ModelError
    ├── variables.rs       # Decision variables
    ├── objective.rs       # Objective function
    ├── names.rs           # Variable/constraint naming functions
    ├── constraints/
    │   ├── mod.rs         # Constraint orchestration
    │   ├── constraint_set.rs # Constraint set builder and accessors
    │   ├── types.rs       # Constraint type definitions
    │   ├── thermal.rs     # Thermal unit commitment constraints
    │   ├── hydro.rs       # Hydro balance, generation and max swallowing rows
    │   ├── demand.rs      # Submarket demand balance
    │   └── limits.rs      # Operational limit constraints
    ├── production/        # FPHA fit: fit_system, fit_plant and the fit stages
    ├── swallowing/        # Max swallowing formula and plant helpers
    ├── hydro_geometry/    # VHA forebay table, tailrace polynomial, net head
    ├── hull/              # Crate-private Qhull binding (the only unsafe module)
    └── tests.rs           # Unit tests
```

**labdessem-simulation** — End-to-end orchestration: iterative solve
workflow (LP → MILP → LP-FIXED → LP-CALC-CMO), network cut iterations
(using `labdessem-network`), dual extraction, and the max swallowing
bounds, recalculated after every solve with the `swallowing` formula of
`labdessem-model`. Result files are written by `labdessem-io`.

```
labdessem-simulation/src/
├── lib.rs             # Public API (run_simulation, run_simulation_with_max_swallowing)
├── stages/
│   ├── mod.rs
│   ├── orchestration.rs # run_simulation and the per-option workflows
│   ├── solve.rs       # solve_stage, solve_stage_without_network
│   ├── helpers.rs     # CMB/CMO assembly, cut and violation helpers
│   ├── common.rs      # Shared constants and column helpers
│   └── max_swallowing_tests.rs # Solver-backed max swallowing tests
├── max_swallowing.rs  # MaxSwallowingState: plant-level bounds, recalculated per solve
├── flow_units.rs      # hm3 to m3/s conversion
├── workflow.rs        # ExecutionOption and IterationStage enums
├── results.rs         # Result structures (IterativeSimulationResult, MaxSwallowingOutcome, etc.)
└── error.rs           # SimulationError
```

### Entry Point

**labdessem-cli** — Command-line interface producing the `labdessem` binary.
Provides `labdessem run <case_dir>` subcommand with banner display and
TTY-aware colored output. Installs one `tracing` subscriber on stderr: WARN
by default, `RUST_LOG` overrides the level. Prints the banner and the run
summary, including the max swallowing consistency line, on stderr.

With `FPHA="computed"`, `fpha_integration.rs` runs these steps before the
solve:

1. `compute_fpha` reads the overrides with `labdessem-io` `read_fpha_config`,
   keyed by `HydroPlantId`.
2. It calls `labdessem-model` `fit_system`, which returns typed results and
   prints nothing.
3. The CLI renders the results on stderr (each `FitFailureReason` included)
   and fails with the list of every plant it could not fit.
4. `finalize_fpha_injection` injects the fitted segments, turns FPHA on and
   re-validates the whole `System`.
5. `labdessem-io` `write_fpha_computed` writes `resultado_fpha_computed.csv`
   from the `System` and the fit results.

A failure exits with code 1 before any result is written.

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
| `labdessem-simulation` | core, model, network, solver |
| `labdessem-cli` | core, io, model, network, simulation, solver |

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

6. **Hydro production physics in `labdessem-model`**: the FPHA fit (`production`),
   the max swallowing formula (`swallowing`) and their shared geometry
   (`hydro_geometry`) live in the model crate, behind four one-way doors:
   - **qhull in the model crate**: the fit needs a 3-D convex hull, and building the
     vendored qhull C sources (`vendor/qhull`) from the model's `build.rs` keeps the
     whole fit in one crate, at the cost of a C compiler and the submodule
     (`git submodule update --init --recursive`) for `labdessem-model`, a native
     toolchain that `labdessem-io`, `labdessem-solver`, `labdessem-simulation` and
     `labdessem-cli` already need for HiGHS.
   - **One unsafe module**: the workspace forbids `unsafe_code`, and
     `labdessem-model` copies the workspace lint table with `unsafe_code = "deny"`,
     allows `unsafe` only on the crate-private `hull` module and forbids it on every
     other module, so the FFI stays in one place and `tests/unsafe_scope.rs` fails
     if that boundary moves.
   - **Shared `hydro_geometry`**: the FPHA fit and max swallowing read the same
     forebay table, tailrace polynomial and net head, so one definition serves both
     while each consumer keeps its own missing-data policy.
   - **Max swallowing vocabulary**: Rust identifiers and prose say max swallowing,
     while the serialized names (the `EngMax` key of `study_config.json`, the
     `engmax[` LP rows and the `EngMaxHM3` and `EngMaxM3s` columns) keep their
     spelling, so existing cases and outputs stay valid.

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
| `ModelError` | model | Model build errors (e.g. a missing max swallowing bound) |
| `SystemFit` | model | FPHA fit results of every plant (`PlantFit`), with the `FitFailureReason` of each plant not fitted |
| `MaxSwallowingState` | simulation | Max swallowing bounds per (plant, period), recalculated after every solve |
| `MaxSwallowingOutcome` | simulation | Max swallowing bounds that built the final LP-CALC-CMO model and the bounds recomputed from its solution |

## Testing

Run the test suite:

```bash
cargo test
```

The max swallowing tests live in `labdessem-simulation` (`max_swallowing.rs`
and `stages/max_swallowing_tests.rs`). The FPHA fit regression and
integration tests live in `labdessem-model` (`tests/production_regression.rs`
and `tests/production_integration.rs`), and `tests/unsafe_scope.rs` checks
the unsafe boundary of `hull`. The FPHA end-to-end cases live in
`labdessem-cli` (`tests/fpha_e2e.rs`).

## Building

```bash
# Fetch the qhull submodule (a new clone can use git clone --recurse-submodules)
git submodule update --init --recursive

# Install CLI
cargo install --path crates/labdessem-cli

# Run a case
labdessem run examples/24Barras

# Development run
cargo run -p labdessem-cli -- run examples/24Barras
```

## Known follow-ups

Deferred items, recorded but not scheduled, from the move of the FPHA engine into
`labdessem-model`; each bullet cites its code as `path:line`, and the R and RD ids are
requirement and decision ids of the relocation plan, which is not tracked.

- `clippy::module_inception` fires on `mod system;` at
  `crates/labdessem-core/src/system/mod.rs:10` (R30, RD-21), because the module
  `system` declares a child module of the same name. The relocation left it as
  found.
- `display_period` is defined three times in `labdessem-model`, at
  `crates/labdessem-model/src/names.rs:5`,
  `crates/labdessem-model/src/variables.rs:428` and
  `crates/labdessem-model/src/constraints/mod.rs:58`.
- The error `Display` impls are hand-written: `SimulationError` at
  `crates/labdessem-simulation/src/error.rs:45` and the cli `FphaComputeError`
  at `crates/labdessem-cli/src/fpha_integration.rs:49`.
- The `RESULTADOS/` directory is created in three places: `write_fpha_computed`
  at `crates/labdessem-io/src/output/fpha_computed.rs:23`, `write_all_results`
  at `crates/labdessem-io/src/output/orchestrator.rs:44` and the cli at
  `crates/labdessem-cli/src/main.rs:160`.
- `docs/mapa_conexoes_codigo.md` describes an older layout. It cites paths that
  no longer exist, for example `crates/labdessem-core/src/system.rs` and
  `crates/labdessem-model/src/constraints.rs`, and it does not name the
  relocated modules `production`, `swallowing`, `hydro_geometry` and `hull`.
- The `engmax[` row prefix is written in three places (R9, RD-11):
  `crates/labdessem-model/src/constraints/hydro.rs:466`,
  `crates/labdessem-model/src/constraints/constraint_set.rs:233` and
  `crates/labdessem-simulation/src/stages/solve.rs:529`. The tests
  `extend_with_max_swallowing_adds_plant_rows`,
  `extend_with_max_swallowing_sums_every_unit_of_the_plant`,
  `max_swallowing_infeasible_display_lists_rows` and
  `max_swallowing_infeasibility_names_exceeded_rows` pin the prefix, so a single
  definition removes a duplicated literal and closes no coverage gap.
- Schema drift (RD-24): `schemas/study_config.schema.json` is maintained by
  hand and already differs from the output of `generate_study_config_schema()`
  at `crates/labdessem-io/src/study.rs:251`, which the example
  `crates/labdessem-io/examples/generate_schema.rs` writes to that file. The
  file gives `EngMax` as `"enum": [0, 1]` and `FPHA` as a string enum; the
  generator gives `EngMax` `format: uint8` and `minimum: 0.0`, and gives `FPHA`
  an `allOf` `$ref` to `definitions.FphaOption`. No test compares the generated
  schema with the file: `generates_valid_json_schema` at
  `crates/labdessem-io/src/study.rs:344` only checks that the output is valid
  JSON with `$schema` and `properties`.
- The FPHA long-term mean inflow (MLT) is the mean of the incremental inflow,
  not DESSEM's total natural inflow per posto (`mlt.dat`); see
  `build_long_term_mean_inflow_table` at
  `crates/labdessem-model/src/production/assembly.rs:130` and
  `long_term_mean_inflow` at
  `crates/labdessem-model/src/production/assembly.rs:237`. On the 24Barras case
  FURNAS gets 590.8 m3/s against 886.2 m3/s in DESSEM, and ITUTINGA has MLT 0,
  so the spillage bound `S_max` of the fit falls back to 2 x Q_max. The evidence
  is the DESSEM review
  `ticket-029-move-fit-input-assembly-into-model.dessem-review.1.md`, a plan
  report that is not tracked. The follow-up is a posto MLT.
- `ignored_volume_window` at
  `crates/labdessem-model/src/production/assembly.rs:290` matches only a
  single-point VHA table. A multi-point table with reservoir Vmin == Vmax also
  drops a configured volume window without a warning: `fit_context` at
  `crates/labdessem-model/src/production/pipeline.rs:136` then passes a
  zero-width window, and `resolve_fitting_bounds` computes `forebay_span` at
  `crates/labdessem-model/src/production/grid.rs:115` and takes the
  run-of-river synthesis for `forebay_span <= V_EPS`. 24Barras is not affected.
  The evidence is the same DESSEM review,
  `ticket-029-move-fit-input-assembly-into-model.dessem-review.1.md`.
- `labdessem-io` declares `labdessem-solver` at
  `crates/labdessem-io/Cargo.toml:12`, and no non-test code uses it (only the
  test-only `verify_imports_test.rs` imports it, as unused imports). Cargo's
  `unused_dependencies` lint warns about it at every build.
- `labdessem-uc` declares `labdessem-core` at
  `crates/labdessem-uc/Cargo.toml:11` and uses none of it. The same
  `unused_dependencies` lint warns about it at every build.
- `fit_hull_planes` at `crates/labdessem-model/src/production/hull_fit.rs:60`
  is public and `crates/labdessem-model/src/production/mod.rs:32` re-exports it,
  but it returns `HullError` (`crates/labdessem-model/src/hull/mod.rs:19`),
  which lives in the crate-private `hull` module declared at
  `crates/labdessem-model/src/lib.rs:6`, so callers outside the crate cannot
  name it. The follow-up is to narrow the function to `pub(crate)`, which also
  moves its caller, the integration test `production_regression.rs`, into the
  crate, or to map the error to a public type.
- Test fixtures are duplicated. `storage_plant_params` at
  `crates/labdessem-model/tests/production_integration.rs:16` repeats
  `reference_params` at
  `crates/labdessem-model/tests/production_regression.rs:53` field for field,
  and the swallowing test builder `plant` at
  `crates/labdessem-model/src/swallowing/plant.rs:152` writes a full
  `HydroPlant` literal, while the `assembly.rs` tests start from `minimal_hydro`
  (for example at `crates/labdessem-model/src/production/assembly.rs:349`). The
  follow-up is one shared fixture home.
- The cli probe's test-only `fit_contexts` at
  `crates/labdessem-cli/src/fpha_integration/probe.rs:436` repeats the
  per-plant assembly of `fit_system` at
  `crates/labdessem-model/src/production/assembly.rs:82`: the mean inflow
  table, the `read_fpha_config` overrides, `build_fit_params` and `fit_context`.
  It does so because `FphaPlantResult` does not carry the `ProductionFunction`
  and the `FittingBounds`, so a change to the assembly must be repeated there by
  hand.
- `use super::*` remains in test modules: the nine files `alpha.rs`,
  `deviation.rs`, `function.rs`, `grid.rs`, `hull_fit.rs`, `pipeline.rs`,
  `reduction.rs`, `secant.rs` and `selection.rs` of
  `crates/labdessem-model/src/production/`, the io test module at
  `crates/labdessem-io/src/readers/fpha_geometry.rs:373`, and other test modules
  of the workspace. The newer test modules (`assembly.rs` and the cli
  `fpha_integration.rs` tests) import items by name.
- Three branches have no test that would fail if they changed: the negative
  `hydro_id` skip of `build_long_term_mean_inflow_table` at
  `crates/labdessem-model/src/production/assembly.rs:136`; `fit_system` setting
  `ignored_window` at `crates/labdessem-model/src/production/assembly.rs:110`
  (every model test passes no volume window, and the cli tests call
  `ignored_volume_window` directly); and the strict `>` tie-break of the secant
  search at `crates/labdessem-model/src/production/secant.rs:44`, which keeps
  the first grid point of equal generation (a flip to `>=` passes the suite).
- Three broken links in `docs/design/network-cuts.md`: the Key Files links at
  `docs/design/network-cuts.md:111` and the next two lines point to
  `crates/labdessem-simulation/src/network/{mod,ptdf,flow}.rs`, which do not
  exist. The network code is in `crates/labdessem-network`.
