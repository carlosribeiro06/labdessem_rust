# LABDESSEM Rust

`LABDESSEM Rust` is a professional Rust implementation of a reduced-scale day-ahead power system scheduling model inspired by DESSEM.

The project is organized as a modular workspace and is designed to support:

- thermal unit commitment
- hydro scheduling with cascaded reservoirs
- renewable generation with curtailment decisions
- submarket demand balance
- linearized network analysis with iterative flow cuts
- LP, MILP, and LP with fixed integer decisions

## Overview

The current solution workflow follows an iterative network-constrained process:

1. Solve an `LP`.
2. Compute DC power flows.
3. Detect line capacity violations.
4. Add flow cuts only for violated line-period pairs.
5. Repeat until the `LP` is network-feasible.
6. Solve a `MILP` with accumulated cuts.
7. Recompute flows.
8. If new violations appear, solve `LP with fixed commitment` until no new violations remain.
9. Export the main operational results to CSV.

This gives the project a practical decomposition strategy while keeping the optimization model modular and extensible.

## Workspace Structure

The repository is organized as a Rust workspace:

```text
labdessem_rust/
|-- crates/
|   |-- labdessem-cli
|   |-- labdessem-common
|   |-- labdessem-core
|   |-- labdessem-io
|   |-- labdessem-model
|   |-- labdessem-simulation
|   `-- labdessem-solver
|-- examples/
|   |-- 3Barras
|   `-- caso_base
|-- Cargo.toml
`-- README.md
```

### Crates

- `labdessem-core`
  Domain layer. Defines the power system data structures, identifiers, validations, generation assets, network, submarkets, reservoirs, and study horizon.

- `labdessem-model`
  Mathematical model layer. Defines indexing, variables, constraints, objective function, and solve modes.

- `labdessem-io`
  Input layer. Reads study data from a JSON configuration file that points to a case directory containing CSV files.

- `labdessem-solver`
  Solver integration layer. Builds and solves the optimization model using `good_lp`.

- `labdessem-simulation`
  Orchestration layer. Runs the iterative workflow, computes DC flows, checks violations, accumulates flow cuts, and exports result CSVs.

- `labdessem-cli`
  Command-line entrypoint for reading a study, running the iterative workflow, and writing outputs.

- `labdessem-common`
  Shared utilities and common abstractions used across crates.

## Mathematical Scope

### Generation Technologies

- Thermal plants
  Each plant contains thermal units. The model is prepared for thermal unit commitment.

- Hydro plants
  Each plant contains groups and hydro units. Reservoirs include minimum, maximum, and initial storage. Plants may be connected in cascade with multiple upstream plants.

- Wind and solar plants
  These are modeled at plant level. Their available generation is input data, and the model decides how much to dispatch or curtail.

### Network

The project includes:

- buses
- transmission lines
- submarket aggregation
- DC flow evaluation
- iterative line flow cuts based on detected violations

### Solve Modes

The modeling layer supports three solve modes:

- `LinearProgramming`
- `MixedIntegerLinearProgramming`
- `LinearProgrammingWithFixedCommitment`

## Current Results Export

After a successful run, the simulation writes CSV outputs under:

```text
<case_path>/RESULTADOS
```

The current export includes:

- hydro results
  volume, generation, turbining, and spillage for all periods

- thermal results
  generation, on/off status, and residence times for all periods

- network results
  power flow for every line and every period, including from-bus and to-bus

- wind results
  generation for all periods

- solar results
  generation for all periods

## Input Data

The study is driven by a JSON file located at:

[`crates/labdessem-io/study_config.json`](c:/Users/carlo/OneDrive/Documentos/git/labdessem_rust/crates/labdessem-io/study_config.json)

Example:

```json
{
  "case_path": "../../examples/caso_base"
}
```

The `case_path` must point to a study directory that follows the expected CSV layout used by the reader in `labdessem-io`.

## Build and Run

### Default Execution

Run the CLI with the default study configuration:

```powershell
cargo run -p labdessem-cli
```

### Explicit Configuration Path

Run the CLI with a specific JSON configuration file:

```powershell
cargo run -p labdessem-cli -- crates/labdessem-io/study_config.json
```

### Check the Workspace

```powershell
cargo check
```

## Solver Requirements

The project is currently configured to use `good_lp` with the `HiGHS` backend.

On Windows, a working build environment requires:

- CMake
- Visual Studio C++ Build Tools
- LLVM/Clang with `libclang`

In particular, `highs-sys` requires `bindgen`, which depends on `libclang`. If `libclang` is installed but not detected automatically, define:

```powershell
$env:LIBCLANG_PATH="C:\Program Files\LLVM\bin"
```

If you want this permanently:

```powershell
setx LIBCLANG_PATH "C:\Program Files\LLVM\bin"
```

## Runtime Output

During execution, the CLI prints the iterative solution process in real time, including:

- current stage
- iteration number
- objective value
- number of flow violations
- solve time

This makes it easier to trace the progression of:

- `LP`
- `MILP`
- `LP-FIXED`

## Design Goals

This project is being built with a professional engineering standard in mind:

- clear separation between domain, model, IO, solver, and simulation
- explicit validations in the domain layer
- reproducible study execution through configuration-driven input
- scalable architecture for future model growth
- traceable operational outputs through CSV exports

## Roadmap

The current implementation establishes the core architecture. Natural next steps include:

- fuller thermal unit commitment logic
- fuller hydro unit commitment logic
- tighter integration between optimization and network constraints
- richer reporting and scenario management
- broader automated test coverage

## License

This repository includes a [`LICENSE`](c:/Users/carlo/OneDrive/Documentos/git/labdessem_rust/LICENSE) file at the root of the workspace.
