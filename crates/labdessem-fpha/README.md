# labdessem-fpha

FPHA (Função de Produção Hidrelétrica Aproximada) fitting pipeline for labdessem.

## Overview

This crate computes FPHA hyperplanes from reservoir geometry (VHA tables) using convex hull fitting.

## Pipeline Stages

1. **Hull Fitting**: Build production cloud → convex hull → upper-envelope planes
2. **α Correction**: Least-squares scaling to minimize fit error
3. **γₛ Secant**: Per-plane lateral-flow coefficient via OLS
4. **Reduction**: Merge near-parallel planes (angle or distance method)
5. **Deviation**: Compute fit-quality metrics (max, mean, RMS)
6. **Validation**: Verify sign constraints (γᵥ ≥ 0, γᵩ ≥ 0, γₛ ≤ 0)

## Usage

### Read input files

```rust
use labdessem_fpha::{read_geometry_table, read_tailrace_table, read_fpha_config};

let geometry = read_geometry_table("CAD/CAD_GEOMETRIA_UHE.csv")?;
let tailrace = read_tailrace_table("CAD/CAD_TAILRACE.csv")?;
let config = read_fpha_config("CAD/CAD_FPHA_CONFIG.csv")?;
```

### Fit a plant

```rust
use labdessem_fpha::{fit_plant, FitPlantParams, FphaPlantConfig, PlaneReductionConfig};

let params = FitPlantParams {
    codigo: 1,
    nome: "FURNAS".to_string(),
    forebay: geometry[&1].clone(),
    tailrace: tailrace.get(&1).cloned(),
    hydraulic_loss_m: Some(2.0),
    efficiency: Some(0.92),
    max_turbined_m3s: 3_000.0,
    max_generation_mw: 5_000.0,
    long_term_mean_inflow_m3s: 1_500.0,
    config: FphaPlantConfig::computed(10, 6, 6, PlaneReductionConfig::default()),
};

let result = fit_plant(&params)?;
println!("Fitted {} planes, α = {:.4}", result.planes.len(), result.alpha);
```

### Write results

```rust
use labdessem_fpha::write_fpha_results;

write_fpha_results(&[result], "RESULTADOS/resultado_fpha_computed.csv")?;
```

## CLI Integration (Future)

To integrate with labdessem CLI:

1. Add `labdessem-fpha` dependency to `labdessem-cli/Cargo.toml`
2. In `run_and_print()`, after `read_study_from_path()`:
   - If `FPHA=1` and `CAD_GEOMETRIA_UHE.csv` exists, call `fit_plants()`
   - Convert `FphaPlantResult` to `HydroFphaSegment` and update `System`
3. After simulation, write `resultado_fpha_computed.csv`

## Input Files

| File | Description |
|------|-------------|
| `CAD_GEOMETRIA_UHE.csv` | Volume-Height-Area table per plant |
| `CAD_TAILRACE.csv` | Tailrace polynomial coefficients |
| `CAD_FPHA_CONFIG.csv` | Per-plant fitting configuration |

## Output Files

| File | Description |
|------|-------------|
| `resultado_fpha_computed.csv` | Fitted planes with coefficients and metrics |

## Dependencies

- `qhull` (vendored as submodule, compiled via build.rs)
- `csv`, `serde` for I/O
- `thiserror` for error handling
