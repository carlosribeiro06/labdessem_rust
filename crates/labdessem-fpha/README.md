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

### Fit one plant

The snippet reads the three FPHA inputs with the `labdessem-io` readers, fits one plant with the default configuration, applies the `num_planes` override of `CAD/CAD_FPHA_CONFIG.csv` when present and writes `RESULTADOS/resultado_fpha_computed.csv`. Add `labdessem-fpha` and `labdessem-io` to the `[dependencies]` of the calling crate.

```rust
use std::path::Path;

use labdessem_fpha::config::{
    DEFAULT_ANGLE_TOLERANCE_DEG, DEFAULT_FLOW_POINTS, DEFAULT_MAX_PLANES, DEFAULT_VOLUME_POINTS,
};
use labdessem_fpha::{
    FitPlantParams, FphaPlantConfig, PlaneReductionConfig, fit_plant, forebay_from_vha,
    tailrace_from_data,
};
use labdessem_io::output::{ComputedFphaRecord, write_fpha_computed};
use labdessem_io::readers::{read_fpha_config, read_tailrace, read_vha_geometry};

fn fit_one_plant(case_dir: &Path) -> Result<(), Box<dyn std::error::Error>> {
    let code = 3_u32;
    let name = "FURNAS";

    // Each reader returns an empty map when its optional file is absent.
    let geometry = read_vha_geometry(case_dir)?;
    let tailrace = read_tailrace(case_dir)?;
    let overrides = read_fpha_config(case_dir)?;

    let vha = geometry
        .get(&code)
        .ok_or("plant has no rows in CAD/CAD_GEOMETRIA_UHE.csv")?;

    let mut config = FphaPlantConfig::computed(
        DEFAULT_MAX_PLANES,
        DEFAULT_VOLUME_POINTS,
        DEFAULT_FLOW_POINTS,
        PlaneReductionConfig::Angle {
            tolerance_deg: DEFAULT_ANGLE_TOLERANCE_DEG,
        },
    );
    if let Some(num_planes) = overrides.get(&code).and_then(|row| row.num_planes) {
        config.max_planes = num_planes;
    }

    let params = FitPlantParams {
        codigo: 3,
        nome: name.to_string(),
        forebay: forebay_from_vha(vha, name)?,
        tailrace: tailrace.get(&code).map(tailrace_from_data),
        // Illustrative values; the CLI reads them from CAD_UHE.csv and the unit tables.
        hydraulic_loss_m: Some(2.0),
        efficiency: Some(0.92),
        max_turbined_m3s: 3_000.0,
        max_generation_mw: 5_000.0,
        long_term_mean_inflow_m3s: 1_500.0,
        config,
    };

    let result = fit_plant(&params)?;
    println!(
        "{} planes, alpha = {:.4}",
        result.planes.len(),
        result.alpha
    );

    // The planes are unscaled: `alpha` is written beside them and applied once by the model.
    let records: Vec<ComputedFphaRecord> = result
        .planes
        .iter()
        .enumerate()
        .map(|(plane_index, plane)| ComputedFphaRecord {
            codigo_usina: code,
            nome_usina: result.nome.clone(),
            plane_index,
            gamma_0: plane.gamma_0,
            gamma_v: plane.gamma_v,
            gamma_q: plane.gamma_q,
            gamma_s: plane.gamma_s,
            alpha: result.alpha,
            max_deviation_mw: Some(result.deviation.max_abs_mw),
            mean_deviation_mw: Some(result.deviation.mean_abs_mw),
            rms_deviation_mw: Some(result.deviation.rms_mw),
        })
        .collect();
    write_fpha_computed(case_dir, &records)?;
    Ok(())
}
```

`fit_plant` fails with `FphaFittingError`. A library caller may pass `tailrace: None`, which gives a tailrace level of 0 m; the CLI rejects a plant without a tailrace row instead (see the design document, Known Limitations).

### Run the example

```bash
cargo run -p labdessem-fpha --example fpha_fit -- <case_dir>
```

The example fits every plant of `CAD/CAD_GEOMETRIA_UHE.csv` that has a row in `CAD/CAD_UHE_PARAMS.csv`, and prints a warning for each plant without one. It uses the defaults above and every field of `CAD/CAD_FPHA_CONFIG.csv`, as the CLI computed mode does. It differs in one point: it has no `CAD_UHE.csv`, so the volume window is a fraction of the VHA span. The case directory must contain:

| File | Required | Content |
| --- | --- | --- |
| `CAD/CAD_GEOMETRIA_UHE.csv` | yes | `codigo_usina;volume.hm3;cota.m;area.km2` |
| `CAD/CAD_TAILRACE.csv` | yes | `codigo_usina;a0.m;a1.m/m3s;a2.m/m3s2;a3.m/m3s3;a4.m/m3s4` |
| `CAD/CAD_FPHA_CONFIG.csv` | no | `codigo_usina;num_planes;tolerance;min_volume_pct;max_volume_pct` |
| `CAD/CAD_UHE_PARAMS.csv` | yes | `Codigo;Nome;MaxTurbinedM3s;MaxGenerationMW;HydraulicLossM;Efficiency;MeanInflowM3s` |

The example writes `<case_dir>/RESULTADOS/resultado_fpha_computed.csv`. `examples/24Barras` has no `CAD/CAD_UHE_PARAMS.csv`; copy the case and add one to use it.

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

- `labdessem-io` (workspace crate): supplies the readers and the writer used by `src/from_io.rs`, the example and the integration tests.
- `labdessem-core` (workspace crate): declared as a dependency; no source, test or example file of this crate imports it.
- `thiserror` for the error types. `serde` (with the `derive` feature) is used by `examples/fpha_fit.rs` to read `CAD_UHE_PARAMS.csv`.
- `qhull`, vendored as a git submodule in `vendor/qhull` and compiled by [`build.rs`](build.rs) with the C shim in `csrc/qhull_wrapper.c`. If the build reports that the qhull source is missing, run `git submodule update --init --recursive`.
- Build dependency `cc`, which compiles the qhull sources and the shim.
- Development dependency `tempfile`, declared in `[dev-dependencies]`; no test of this crate imports it.
