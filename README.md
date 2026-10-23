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
    <a href="https://github.com/carlosribeiro06/labdessem_rust/releases/latest"><img src="https://img.shields.io/github/v/release/carlosribeiro06/labdessem_rust?label=version" alt="Latest Release"></a>
    <a href="LICENSE"><img src="https://img.shields.io/badge/license-MIT-blue.svg" alt="License"></a>
  </p>

  <p>
    <a href="https://carlosribeiro06.github.io/labdessem_rust/">Website</a> •
    <a href="docs/">Docs</a> •
    <a href="examples/24Barras/study_config.json">Study config</a> •
    <a href="CHANGELOG.md">Changelog</a>
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
- future cost function via Benders cuts read from binary file
- generic renewable plants with dispatch bounded by programmed generation
- submarket demand balance with optional network representation
- operational limit constraints and infeasibility reporting
- iterative network-constrained workflow with LP, MILP, LP-FIXED, and LP-CALC-CMO
- single-shot MILP workflow as an alternative execution mode
- nodal marginal cost (CMB) and submarket marginal operating cost (CMO) computation when network is enabled

## Workspace Structure

```text
labdessem_rust/
|-- crates/
|   |-- labdessem-cli
|   |-- labdessem-core
|   |-- labdessem-fpha
|   |-- labdessem-io
|   |-- labdessem-model
|   |-- labdessem-network
|   |-- labdessem-simulation
|   |-- labdessem-solver
|   `-- labdessem-uc
|-- docs/
|-- examples/
|-- Cargo.toml
`-- README.md
```

### Crates

- `labdessem-cli`
  Command-line interface for running studies. Produces the `labdessem` binary.

- `labdessem-core`
  Domain entities, identifiers, validations, horizon representation, and study data structures.

- `labdessem-fpha`
  FPHA (Função de Produção Hidrelétrica Aproximada) fitting pipeline. Computes hyperplanes from VHA geometry using convex hull fitting.

- `labdessem-io`
  File I/O. Reads study input (`study_config.json`, `CAD/`, `OPER/`) and writes all result CSVs.

- `labdessem-model`
  Indexing, variables, objective function, and algebraic constraints.

- `labdessem-network`
  DC network flow computation, PTDF matrices, flow cut constraints, and CMB/CMO dual extraction.

- `labdessem-simulation`
  End-to-end orchestration: solve workflow, network-cut iterations, and dual extraction.

- `labdessem-solver`
  Optimization backend integration through `good_lp`.

- `labdessem-uc`
  Solver-agnostic unit commitment formulation: thermal and hydro UC variables and constraints.

## Execution Modes

The study configuration supports two execution modes through `opcao_execucao`:

- `1`: `LP -> MILP -> LP-FIXED -> LP-CALC-CMO` (iterative network cuts)
- `2`: `MILP -> LP-CALC-CMO` (all network constraints upfront)

Both modes use the `LP-CALC-CMO` stage to extract valid dual information:

- submarket marginal operating cost (`PiDemanda`)
- hydro water balance dual (`PiBalHidr`)

When the network flag is active, mode 1 adds DC flow cuts iteratively for violated line-period pairs, while mode 2 includes all network constraints from the start. The `LP-CALC-CMO` stage always uses all flow constraints for correct dual extraction.

## Study Configuration

Each study case must contain a `study_config.json` file in its root directory.
See [`examples/24Barras/study_config.json`](examples/24Barras/study_config.json) for a reference.

Current keys:

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
  - `0`: FPHA disabled — use plant productivity times turbining
  - `1`: precomputed — read `OPER/OPER_FPHA.csv` and enforce FPHA cuts
  - `"computed"`: compute FPHA hyperplanes from VHA geometry at runtime

  When `"computed"` mode is active, the solver reads:
  - `CAD/CAD_GEOMETRIA_UHE.csv` — VHA geometry table (volume × turbining × generation)
  - `CAD/CAD_TAILRACE.csv` — tailrace level table (optional, for future expansion)
  - `CAD/CAD_FPHA_CONFIG.csv` — fitting parameters per plant (optional)

  The computed hyperplanes are injected into the hydro plant model and written to `RESULTADOS/resultado_fpha_computed.csv`.

- `FCF`
  Enables or disables future cost function:
  - `1`: read `OPER_FCF.bin` and add Benders cuts to the model
  - `0`: ignore future cost (default)

Example:

```json
{
  "$schema": "../../schemas/study_config.schema.json",
  "opcao_execucao": 2,
  "rede": 0,
  "UCT": 1,
  "UCH": 0,
  "TON_Residual": 0,
  "FPHA": 0,
  "FCF": 1
}
```

### JSON Schema

A JSON Schema is provided for IDE autocompletion and validation. Add `$schema` to your config file pointing to `schemas/study_config.schema.json`, or configure VS Code:

```json
// .vscode/settings.json
{
  "json.schemas": [
    {
      "fileMatch": ["**/study_config.json"],
      "url": "./schemas/study_config.schema.json"
    }
  ]
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
- `OPER_FCF.bin` when `FCF = 1`

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
- `resultado_fcf_eco.csv` when `FCF = 1`
- `resultado_fpha_computed.csv` when `FPHA = "computed"`

When network treatment is enabled, the run also writes:

- `resultado_rede.csv`
- `resultado_linhas_adicionadas.csv`
- `resultado_inviabilidade_rede.csv`
- `resultado_cmobar.csv` (nodal marginal costs per bus)

## Build and Run

### Install the CLI

```bash
cargo install --path crates/labdessem-cli
```

This installs the `labdessem` binary to `~/.cargo/bin/`.

### Run a study case

```bash
labdessem run examples/24Barras
```

### Run from the case directory

```bash
cd examples/24Barras
labdessem run
```

### Show help

```bash
labdessem --help
labdessem run --help
```

### Development (without installing)

```bash
cargo run -p labdessem-cli -- run examples/24Barras
```

### Check the workspace

```bash
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
