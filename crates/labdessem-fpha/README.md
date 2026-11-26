# labdessem-fpha

FPHA (Função de Produção Hidrelétrica Aproximada) fitting pipeline for labdessem.

## Overview

This crate fits a set of planes to the exact hydro production function of one plant. The input is the reservoir geometry (VHA table), the tailrace polynomial, the efficiency, the hydraulic loss and the turbining and generation limits. Each plane is a linear bound on generation, `γ_0 + γ_V·V + γ_Q·Q + γ_S·S`, with `V` the reservoir volume, `Q` the turbined flow and `S` the spilled flow. The CLI runs the fit when `study_config.json` sets `"FPHA": "computed"`.

`fit_plant` returns the planes unscaled and the correction factor `alpha` as a separate field. The model multiplies all four coefficients by `alpha` once.

The formulation, the decisions ADR-025 to ADR-033, the failure policies and the known limitations are in [`docs/design/fpha-formulation.md`](../../docs/design/fpha-formulation.md).

## Pipeline Stages

`fit_plant` ([`src/pipeline.rs`](src/pipeline.rs)) runs these stages in this order:

1. **Context** (`fit_context`): builds the production function and the fit volume window. A window outside the VHA table fails with `InvalidFitWindow`. A single-point VHA table ignores the window.
2. **Hull fitting**: samples the production function on a volume-flow grid at spillage 0, builds the convex hull with qhull and keeps the upper facets as planes.
3. **γₛ secant**: fits the lateral-flow coefficient of each plane by least squares at the point where the plane binds. The planes stay unscaled.
4. **Reduction**: merges planes whose normals differ by less than the angle tolerance. While more than `max_planes` planes remain, it doubles the tolerance and reduces the original set again, at most 32 times, then fails with `PlaneLimitUnreachable`.
5. **α correction**: least-squares scale factor fitted on the final planes over 30 evaluation points (6 volumes by 5 flows, or 2 by 15 for a single-volume plant). `alpha` depends on `max_planes`.
6. **Deviation**: maximum, mean absolute and root-mean-square difference between `alpha × planes` and the production function on the fit grid.
7. **Validation**: rejects an empty plane set, `alpha ≤ 0`, `γ_V < 0`, `γ_Q < 0` and `γ_S > 0` beyond a tolerance of 1e-10.

## Usage

`fit_plant` takes one `FitPlantParams`: the plant's code and name, its `ForebayTable` and `TailraceModel` (`labdessem_model::hydro_geometry`), hydraulic loss, efficiency, turbining and generation limits, long-term mean inflow and `FphaPlantConfig`. The CLI builds it in `build_fit_params` (see CLI Integration).

`fit_plant` fails with `FphaFittingError`. The tailrace polynomial, the hydraulic loss and the efficiency are mandatory inputs; the CLI rejects a plant that lacks any of them (see the design document, Known Limitations).

## CLI Integration

In `labdessem run`, when `study_config.json` sets `"FPHA": "computed"`, `run_and_print` in [`crates/labdessem-cli/src/main.rs`](../labdessem-cli/src/main.rs) calls `compute_fpha`, which fits every hydro plant with generating units, then `finalize_fpha_injection`, which injects the planes as `fpha_segments` with `correction_factor = alpha`, turns FPHA on and re-validates the system, then `write_computed_results`, which writes `RESULTADOS/resultado_fpha_computed.csv`. All three live in [`crates/labdessem-cli/src/fpha_integration.rs`](../labdessem-cli/src/fpha_integration.rs). `compute_fpha` returns `FphaComputeError::PlantsNotFitted` listing every plant with units that cannot be fitted, and the CLI then exits with code 1 before writing any result.

## Input Files

The CLI computed mode reads these files from the case directory:

| File | Required | Content |
| --- | --- | --- |
| `CAD/CAD_GEOMETRIA_UHE.csv` | yes, for each plant with units | Volume, forebay level and area points of each plant (`read_vha_geometry`) |
| `CAD/CAD_TAILRACE.csv` | yes, for each plant with units | Degree-4 tailrace polynomial of each plant (`read_tailrace`) |
| `CAD/CAD_UHE.csv` | yes | `Vmin.hm3` and `Vmax.hm3` (window reference), `Eficiencia` and `Perda.m` |
| `CAD/CAD_FPHA_CONFIG.csv` | no | Per-plant plane limit, angle tolerance and volume window fractions (`read_fpha_config`) |
| `OPER/inflow_history.parquet` | no | Source of the long-term mean inflow; without it the CLI uses the deck mean |

A missing tailrace row, `Eficiencia` or `Perda.m` for a plant with units is an error; an explicit `Perda.m = 0` is valid. The defaults are `DEFAULT_MAX_PLANES = 10`, `DEFAULT_VOLUME_POINTS = 5`, `DEFAULT_FLOW_POINTS = 5` and `DEFAULT_ANGLE_TOLERANCE_DEG = 1.0`. The file formats, the window rule and the rejection messages are in the design document.

## Output Files

| File | Description |
| --- | --- |
| `RESULTADOS/resultado_fpha_computed.csv` | One row per fitted plane: `codigo_usina;nome_usina;plane_index;gamma_0;gamma_v;gamma_q;gamma_s;alpha;max_deviation_mw;mean_deviation_mw;rms_deviation_mw` |

The gamma columns hold the unscaled planes. The deviation columns describe `alpha × planes`. Copying the rows into `OPER/OPER_FPHA.csv` with `USIH = codigo_usina`, `SegFPHA = plane_index + 1`, `Fcorrec = alpha`, `Rhs.mw = gamma_0`, `Varm.mw/hm3 = gamma_v`, `Qtur.mw/m3s = gamma_q` and `Qlat.mw/m3s = gamma_s` reproduces the computed run.

## Dependencies

The crate declares these dependencies in [`Cargo.toml`](Cargo.toml):

- `thiserror` for the error types.
- `labdessem-model` (workspace crate): supplies the qhull convex hull (`labdessem_model::hull`) used by `src/fitting/hull_fit.rs`; model vendors qhull as a git submodule and compiles it with the C shim. If the build reports that the qhull source is missing, run `git submodule update --init --recursive`.
- Development dependency `labdessem-core` (workspace crate) with the `test-support` feature: supplies the golden-file harness used by `tests/golden.rs`.
