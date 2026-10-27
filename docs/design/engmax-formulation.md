# EngMax Formulation

Dynamic maximum swallowing constraint based on net head calculation.

## Overview

EngMax (Engolimento Máximo Dinâmico) adjusts the maximum turbining capacity of hydro units based on the current reservoir level. When the net head is lower than the effective head, the turbine can process less water. This constraint is recalculated between solve stages using the reservoir volumes from the optimization solution.

## Mathematical Formulation

### Net Head Calculation

The net head is the effective hydraulic head available for power generation:

```
h_l = h_forebay(V) - h_tailrace(Q) - h_loss
```

Where:
- `h_forebay(V)` — forebay level interpolated from VHA table (volume × level)
- `h_tailrace(Q)` — tailrace level from polynomial: `a₀ + a₁Q + a₂Q² + a₃Q³ + a₄Q⁴`
- `h_loss` — hydraulic losses in the penstock

### Maximum Swallowing Formula

The dynamic maximum turbining is:

```
Q_max = Q_ef × (h_l / h_ef)^α
```

Where:
- `Q_ef` — maximum turbining at effective head (from registration)
- `h_ef` — effective head (from registration)
- `h_l` — calculated net head
- `α` — turbine curve exponent:
  - Francis turbine: α = 0.5
  - Kaplan turbine: α = 0.2

### Constraint

For each unit and period:

```
Q[i,t] ≤ Q_max[i,t]
```

This constraint coexists with the static turbining limit. When `h_l < h_ef`, EngMax is binding. When `h_l ≥ h_ef`, the static limit applies (EngMax ≤ Q_ef by construction).

## Implementation

### Input Files

- `CAD_GEOMETRIA_UHE.csv` — VHA table with columns: `codigo_usina`, `volume.hm3`, `cota.m`, `area.km2`
- `CAD_TAILRACE.csv` — Polynomial coefficients: `codigo_usina`, `a0.m`, `a1.m/m3s`, `a2.m/m3s2`, `a3.m/m3s3`, `a4.m/m3s4`

Both files are optional. When unavailable for a plant, EngMax uses the effective head from registration.

### Inter-Stage Recalculation

1. Before first solve: initialize bounds using initial reservoir volumes
2. After each major solve (MILP, LP-FIXED): recalculate bounds using solution volumes
3. Rebuild model with updated EngMax constraints

Volume indexing:
- Period 0: use `initial_volume_hm3`
- Period t > 0: use `volume[t-1]` from solution (end of previous period)

### Unit Indexing

For multi-group plants, units are indexed with a flat index across groups:
- Plant with 2 groups × 2 units each = flat indices 0, 1, 2, 3
- EngMax bounds use `(plant_idx, unit_flat_idx, period)` as key

## Key Files

- `crates/labdessem-core/src/entities/engmax_bounds.rs` — Bounds storage
- `crates/labdessem-core/src/entities/hydro.rs` — Geometry fields in HydroPlant
- `crates/labdessem-fpha/src/calculator/mod.rs` — `calculate_max_swallowing()` function
- `crates/labdessem-simulation/src/engmax.rs` — Initialization and recalculation
- `crates/labdessem-model/src/constraints/hydro.rs` — `build_engmax_constraints()`
- `crates/labdessem-io/src/readers/fpha_geometry.rs` — VHA and tailrace readers

## Output

EngMax values appear in `resultado_hidreletricas.csv`:
- `EngMaxHM3` — dynamic maximum turbining in hm³
- `EngMaxM3s` — dynamic maximum turbining in m³/s

Columns are empty when `EngMax=0`.

## References

- DESSEM Manual do Usuário — Section on hydro production function
- CEPEL documentation on turbine characteristics
