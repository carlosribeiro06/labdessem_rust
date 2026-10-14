# FPHA Formulation

This document describes the Hydro Production Function Approximation (FPHA) used in labdessem.

## Overview

FPHA is a piecewise linear approximation of the nonlinear relationship between hydro generation and the operational variables (turbining, volume, spillage). It linearizes the hydro production function for use in mixed-integer linear programming.

When `FPHA = 1` in `study_config.json`, the model reads FPHA segments from `OPER_FPHA.csv` and adds linear constraints that approximate the hydro generation envelope.

When `FPHA = 0`, the model uses a simpler productivity-based coupling where generation equals turbining times a constant productivity factor.

## Mathematical Formulation

### FPHA Constraints (FPHA = 1)

For each hydro plant, period, and FPHA segment:

```
generation ≤ α × turbining + β × volume + γ × spillage + δ
```

Where:
- `α` = turbining coefficient × correction factor / flow conversion
- `β` = volume coefficient × correction factor
- `γ` = lateral flow coefficient × correction factor / flow conversion
- `δ` = RHS × correction factor

The flow conversion factor converts from m³/s to hm³ per period:
```
flow_conversion = period_duration_hours × 0.0036
```

### Productivity Coupling (FPHA = 0)

When FPHA is disabled, a simpler linear relationship is used:

```
generation = productivity × turbining / flow_conversion
```

Where `productivity` is the unit's generation productivity in MW·s/m³.

## FPHA Segment Structure

Each segment in `OPER_FPHA.csv` contains:

| Field | Description |
|-------|-------------|
| `codigo_usina` | Plant ID |
| `segmento` | Segment number |
| `coef_turbinamento` | Turbining coefficient |
| `coef_volume` | Volume coefficient |
| `coef_vazao_lateral` | Lateral flow coefficient |
| `rhs` | Right-hand side constant |
| `fator_correcao` | Correction factor |

## Implementation

The FPHA constraints are built in `build_hydro_fpha_constraints()`:

1. For each hydro plant and period
2. For each FPHA segment defined for that plant
3. Create a constraint: `generation ≤ linear combination of variables`

The constraint ensures generation stays within the linearized envelope defined by all segments.

## Key Files

- [`crates/labdessem-model/src/constraints/hydro.rs`](../../crates/labdessem-model/src/constraints/hydro.rs) - `build_hydro_fpha_constraints()`
- [`crates/labdessem-io/src/readers/fpha.rs`](../../crates/labdessem-io/src/readers/fpha.rs) - FPHA segment reader
- [`crates/labdessem-core/src/entities/hydro.rs`](../../crates/labdessem-core/src/entities/hydro.rs) - `HydroFphaSegment` struct

## References

The FPHA approach is based on the methodology used in DESSEM (Brazilian short-term hydrothermal dispatch model) developed by CEPEL.
