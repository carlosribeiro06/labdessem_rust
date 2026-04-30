<div align="center">
  <img src="docs/logo.svg" alt="LabDessem Rust logo" width="110">

  <h1>LABDESSEM RUST</h1>

  <p><strong>Open infrastructure for hydrothermal scheduling</strong></p>

  <p>
    A modular Rust workspace for thermal unit commitment, hydro scheduling,
    optional FPHA, renewables, pumping plants, operational limits, and
    network-aware dispatch studies.
  </p>

  <p>
    <a href="https://labdessem-rs.dev/">Website</a> •
    <a href="docs/">Docs</a> •
    <a href="crates/labdessem-io/study_config.json">Study config</a>
  </p>
</div>

## Current Scope

The current model supports:

- thermal unit commitment with startup and shutdown trajectories
- minimum up/down time for thermal units
- thermal residual TON cost treatment at the end of the horizon
- hydro scheduling with cascades, water diversion, pumping plants, and hydraulic balance
- two hydro generation formulations:
  - `FPHA = 1`: generation limited by FPHA cuts
  - `FPHA = 0`: generation linked to turbining through plant productivity
- generic renewable plants with dispatch bounded by programmed generation
- submarket demand balance with optional network representation
- operational limit constraints and infeasibility reporting
- iterative network-constrained workflow with LP, MILP, LP-FIXED, and LP-CALC-CMO
- single-shot MILP workflow as an alternative execution mode

## Workspace Structure

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
|-- docs/
|-- examples/
|-- Cargo.toml
`-- README.md
```

### Crates

- `labdessem-core`
  Domain entities, identifiers, validations, horizon representation, and study data structures.

- `labdessem-io`
  Case reader. Loads `study_config.json` and the CSV files under `CAD/` and `OPER/`.

- `labdessem-model`
  Indexing, variables, objective function, and algebraic constraints.

- `labdessem-solver`
  Optimization backend integration through `good_lp`.

- `labdessem-simulation`
  End-to-end orchestration: solve workflow, network-cut iterations, dual extraction, and CSV export.

- `labdessem-cli`
  Command-line entrypoint for running a study.

- `labdessem-common`
  Shared helpers used across workspace crates.

## Execution Modes

The study configuration supports two execution modes through `opcao_execucao`:

- `1`: `LP -> MILP -> LP-FIXED -> LP-CALC-CMO`
- `2`: single `MILP`

When the network flag is active, the iterative mode adds DC flow cuts only for violated line-period pairs. The final `LP-CALC-CMO` stage is solved with integer decisions fixed and is used to extract dual information such as:

- submarket marginal operating cost (`PiDemanda`)
- hydro water balance dual (`PiBalHidr`)

## Study Configuration

The default configuration file is:

[`crates/labdessem-io/study_config.json`](crates/labdessem-io/study_config.json)

Current keys:

- `case_path`
  Path to the study directory.

- `opcao_execucao`
  Execution strategy selector.

- `rede`
  Enables or disables electrical network treatment.

- `UCT`
  Enables or disables thermal unit commitment.

- `UCH`
  Enables or disables hydro commitment variables.

- `TON_Residual`
  Enables or disables thermal residual TON treatment in the objective and reports.

- `FPHA`
  Selects hydro generation formulation:
  - `1`: read `OPER_FPHA.csv` and enforce FPHA cuts
  - `0`: ignore FPHA cuts and use plant productivity times turbining

Example:

```json
{
  "case_path": "../../examples/24Barras",
  "opcao_execucao": 1,
  "rede": 1,
  "UCT": 1,
  "UCH": 0,
  "TON_Residual": 0,
  "FPHA": 0
}
```

## Input Data

Each study directory is organized with:

- `CAD/`
  Static registration data such as thermal plants, hydro plants, renewable plants, buses, lines, pumping plants, and thermal ramp trajectories.

- `OPER/`
  Time-varying data such as demand, inflows, renewable availability, operational limits, interchange limits, residual costs, and FPHA cuts.

Representative files currently used by the reader include:

- `CAD_UNID_UTE.csv`
- `CAD_RAMPAS_TERMICAS.csv`
- `CAD_UHE.csv`
- `CAD_CONJ_UHE.csv`
- `CAD_REN.csv`
- `CAD_USIE.csv`
- `OPER_SBM.csv`
- `OPER_VAZAO.csv`
- `OPER_REN.csv`
- `OPER_REST_LIM.csv`
- `OPER_CUSTO_RESIDUAL.csv`
- `OPER_FPHA.csv` when `FPHA = 1`

## Model Highlights

### Thermal

- unit-level generation variables
- binary commitment, startup, and shutdown variables when `UCT = 1`
- startup and shutdown trajectories read from `CAD_RAMPAS_TERMICAS.csv`
- minimum up/down times from input data, interpreted in hours
- residual end-of-horizon treatment controlled by `TON_Residual`

### Hydro

- plant, group, and unit representation
- hydraulic balance with upstream cascade mapping
- water diversion support
- pumping plants with electrical consumption
- turbining and spillage tracked in both `hm3` and `m3/s` in outputs
- optional FPHA-based generation envelope or productivity-based generation relation

### Renewables

- unified renewable representation
- generation bounded by programmed availability
- single output file for all renewable plants

### Network and Limits

- optional DC network flow evaluation
- iterative line-cut addition in execution mode `1`
- explicit operational limit input and infeasibility reporting

## Outputs

After a successful run, CSV files are written under:

```text
<case_path>/RESULTADOS
```

Current outputs include:

- `resultado_hidreletricas.csv`
- `resultado_termicas.csv`
- `resultado_renovaveis.csv`
- `resultado_elevatorias.csv`
- `resultado_cmosist.csv`
- `resultado_processo_iter.csv`
- `resultado_rest_lim.csv`
- `resultado_inviabilidade_lim.csv`

When network treatment is enabled, the run also writes:

- `resultado_rede.csv`
- `resultado_linhas_adicionadas.csv`
- `resultado_inviabilidade_rede.csv`

## Build and Run

### Run with the default configuration

```powershell
cargo run -p labdessem-cli
```

### Run with an explicit configuration path

```powershell
cargo run -p labdessem-cli -- crates/labdessem-io/study_config.json
```

### Check the workspace

```powershell
cargo check
```

## Solver Requirements

The workspace is configured around `good_lp` with the `HiGHS` backend.

On Windows, the current toolchain typically requires:

- CMake
- Visual Studio C++ Build Tools
- LLVM/Clang with `libclang`

If `libclang` is installed but not detected automatically:

```powershell
$env:LIBCLANG_PATH="C:\Program Files\LLVM\bin"
```

To persist it:

```powershell
setx LIBCLANG_PATH "C:\Program Files\LLVM\bin"
```

## Documentation

The repository also contains:

- project website sources in [`docs/`](docs)
- development guides and code connection maps for contributors

## License

See [`LICENSE`](LICENSE).
