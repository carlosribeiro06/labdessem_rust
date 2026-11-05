# EngMax Formulation

This document is normative for EngMax. When it and the code disagree, the code wins and this document is fixed.

## Overview

EngMax (Engolimento Máximo Dinâmico) is the plant-level dynamic maximum swallowing of DESSEM (MM eq. 9.34). It bounds the total water a plant can turbine in a period by the water the turbines can swallow at the current net head. When the net head is lower than the effective head, the plant can process less water.

There is one EngMax value per (plant, period), not one per unit. The bounds are recalculated after every solve (see Update rule).

The CLI enables EngMax when `study_config.json` `EngMax` is not 0. Below the CLI, EngMax is enabled when the bounds are present (`Option<EngMaxBounds>`, ADR-022).

## Constraint

`build_engmax_constraints` adds one row per plant with units and per model period, named `engmax[p=<plant name>,t=<1-based period>]`:

```
Σ_u Q[u,t] ≤ EngMax[p,t]
```

The sum runs over the plant's units, and `Q[u,t]` is the unit's turbining in hm3 per period. Plants without units get no row.

The static per-unit turbining rows stay. The effective limit is therefore the tighter of the per-unit static limits and the plant EngMax row (D1).

## Plant reference values

`PlantReference::from_plant` derives three values per plant (MM eq. 9.34):

- `Q_ef`: the sum of every unit's maximum turbining.
- `h_ef`: the unit-count-weighted mean of the units' `Hef.m`.
- `α`: from `TurbineType::alpha`, by the deck `Turbina` code:

| `Turbina` code | Type | α |
| --- | --- | --- |
| 1 | Francis | 0.5 |
| 2 | Kaplan | 0.2 |
| 3 | Pelton (inferred from the deck, ADR-019) | 0.5 |
| other | Unknown | 0.5 |

An Unknown type is warned once by `TurbineType::from_deck_field`, except code 0 on a plant without units.

Effective-head cases:

- A plant with `Hef.m <= 0` on some units but not all is rejected at initialization with `SimulationError::PartialEffectiveHead { plant, groups }`. `groups` lists the groups that hold such units (ADR-002).
- A plant with `Hef.m <= 0` on every unit is inert. Its EngMax is `Q_ef` and its row never binds. All inert plants are counted in one summary warning when the run's `EngMaxState` is created.

## Maximum swallowing

`calculate_max_swallowing` computes:

```
EngMax = Q_ef × (h_l / h_ef)^α
```

Edge cases: it returns `Q_ef` when `h_ef <= 0` and 0 when `h_l <= 0`.

The value is raw and is never clamped to `Q_ef`. Since c54620f, `h_ef` is no longer capped by the net head, so EngMax exceeds `Q_ef` when `h_l > h_ef` (D1).

Plant-periods with a non-positive net head (and `h_ef > 0`) are counted in one summary warning per bound computation (ADR-013).

## Net head

`net_head_m` computes the net head:

```
h_l = h_forebay(V) - h_tailrace(Q) - h_loss
```

- `h_forebay(V)`: forebay level from the VHA table (volume to level), at the head volume `V` (D2).
- `h_tailrace(Q)`: tailrace level from the 4th-degree polynomial `a₀ + a₁Q + a₂Q² + a₃Q³ + a₄Q⁴`, where `Q` is the previous solve's plant outflow in m3/s (D3).
- `h_loss`: the plant's `Perda.m`, a loss in metres (D4).

The computation uses the labdessem-fpha `ProductionFunction` geometry, built once per run in `EngMaxState::new` (ADR-018).

Input files:

- `CAD_GEOMETRIA_UHE.csv`: the VHA table, with columns `codigo_usina`, `volume.hm3`, `cota.m`, `area.km2`.
- `CAD_TAILRACE.csv`: the polynomial coefficients, with columns `codigo_usina`, `a0.m`, `a1.m/m3s`, `a2.m/m3s2`, `a3.m/m3s3`, `a4.m/m3s4`.

Both files are optional. A plant with no unit, no VHA table or no tailrace polynomial uses `h_ef` as its net head.

A malformed VHA table with EngMax on fails initialization with `SimulationError::Geometry`.

## Update rule

Each solve's EngMax bounds are computed from the solution of the immediately preceding solve, whatever the solve mode (LP, MILP, LP-FIXED, LP-CALC-CMO) and whether or not a network is present. The only exception is the first solve of the process, which uses bounds computed from the initial volume.

Mechanism:

- `EngMaxState::new` computes the first solve's bounds.
- `solve_stage` and `solve_stage_without_network` (`crates/labdessem-simulation/src/stages/solve.rs`) build each model from `EngMaxState::bounds`, then call `EngMaxState::after_solve` exactly once after every solve. The call moves `next` to `used` and recalculates `next` from the solution. It is made on the relaxed solution when slacks were used.
- This covers both execution modes, with or without network (ADR-010).

Volume slot layout: each plant owns `horizon + 1` volume slots. Slot 0 is the initial volume and slot `t + 1` is the end of period `t`.

Head volume (D2, ADR-006): for period `t`, the head volume is the period average `(V[t] + V[t+1]) / 2` of the previous solve, in the slot layout above. The first solve uses the initial volume.

Tailrace flow (D3, ADR-007): the tailrace flow of period `t` is the previous solve's plant outflow, unit turbining plus plant spillage, in m3/s. Diversion and pumping are excluded. The first solve uses the plant maximum outflow (sum of unit maximum turbining) with zero spillage.

## Output

In `resultado_hidreletricas.csv`, `EngMaxHM3` and `EngMaxM3s` are written by `write_hydro_csv` (`crates/labdessem-io/src/output/hydro.rs`). The column preamble of that file is the normative wording (ADR-004, R59):

- (i) The columns hold the plant value per MM eq. 9.34. They are filled only on the plant aggregate row `99;99`, and are empty on unit rows and when EngMax is off.
- (ii) The value is the bound that built the final LP-CALC-CMO model (`used`).
- (iii) The value is raw, so it may exceed `TurbinamentoMaxHM3` / `TurbinamentoMaxM3s` on the same row.
- (iv) `EngMaxHM3` is summed over the internal periods of the original period. `EngMaxM3s` is that sum converted to m3/s over the original period's duration.

Column names, positions and number format are unchanged.

Outcome and diagnostic (ADR-011):

- After the final LP-CALC-CMO, `EngMaxState::outcome` fills `IterativeSimulationResult.engmax: Option<EngMaxOutcome { used, recomputed, diagnostic }>`, which is `None` when EngMax is off.
- `used` is the bound that built the final LP-CALC-CMO model, and `recomputed` is EngMax computed from its solution.
- `consistency_diagnostic` compares each plant's final turbining with `recomputed` in m3/s, over every plant with units and every period. It counts excesses above 0.01 m3/s, counts zero-bound excesses apart, and tracks the largest absolute and relative excess.
- It logs one INFO line, or a WARN line when the largest relative excess is over 1% or any zero-bound excess exists.
- The CLI prints an `EngMax:` line, plus `Max excess:` and `Max rel.:` when something is counted, in its Summary on stderr. The CLI logs at WARN by default, so `RUST_LOG=info` shows the INFO line (ADR-024).
- There is no fixed-point loop, no file and no extra CSV column.

To see the diagnostic, run `RUST_LOG=info labdessem run <case_dir>`.

## Failure behaviour

- A missing (plant, period) bound is an error: `ModelError::MissingEngMaxBound` from the model build and from the diagnostic, surfaced as `SimulationError::Model`, and `IoError::InvalidData` from the CSV writer (ADR-012).
- A partial effective head is rejected with `SimulationError::PartialEffectiveHead` (see Plant reference values).
- A malformed VHA table fails with `SimulationError::Geometry` (see Net head).
- EngMax rows get no slack. When a solve with engmax rows is `Infeasible`, `diagnose_engmax_infeasibility` (`crates/labdessem-simulation/src/stages/solve.rs`) re-solves the same model once without them. If that solve is feasible and exceeds some rows, the run fails with `SimulationError::EngMaxInfeasible { stage, iteration, rows }`. `rows` lists those `engmax[p,t]` names in plant-then-period order. Otherwise the original solver error is returned (ADR-017).

## Design decisions

### D1 Effective limit and raw EngMax

EngMax is not clamped to `Q_ef`, and the static per-unit rows stay beside the plant row, so the effective limit is the tighter of the two (ADR-001, ADR-005).

### D2 Head volume

The head volume of a period is the average of its start and end volumes in the previous solve (ADR-006).

### D3 Tailrace flow

The tailrace flow is the previous solve's plant outflow, turbining plus spillage, in m3/s (ADR-007).

### D4 Loss in metres

The loss term is `Perda.m`, a loss in metres, subtracted directly from the net head (ADR-008).

### D5 Tailrace backwater

Tailrace backwater is a recorded non-goal (ADR-009).

## Known limitations

- D5, tailrace backwater, is not modelled. Its effect was measured at no more than 0.3% of EngMax, on CAMARGOS in `examples/24Barras` only, so the figure is not a bound for every plant.
- D3 diverges from DESSEM in two ways (ADR-007 rework trigger), and this document does not claim full D3 parity with DESSEM:
  - Spillage always enters the tailrace flow here, while DESSEM leaves it out for plants whose hidr.dat field 30 is 0 (19 of 212 plants in the 2026-06-15 deck).
  - Lateral tailrace influences (MM eq. 9.25: ITAIPU, BELO MONTE, FONTES) are not modelled.

## Key Files

- `crates/labdessem-core/src/entities/engmax_bounds.rs`: `EngMaxBounds`, keyed by (plant, period).
- `crates/labdessem-core/src/entities/hydro.rs`: hydro plant fields and `TurbineType::alpha`.
- `crates/labdessem-fpha/src/calculator/mod.rs`: `calculate_max_swallowing`.
- `crates/labdessem-simulation/src/engmax.rs`: `EngMaxState`, plant references, bounds and the consistency diagnostic.
- `crates/labdessem-simulation/src/stages/solve.rs`: the per-solve hook and `diagnose_engmax_infeasibility`.
- `crates/labdessem-simulation/src/stages/orchestration.rs`: stage orchestration that owns the `EngMaxState`.
- `crates/labdessem-model/src/constraints/hydro.rs`: `build_engmax_constraints`.
- `crates/labdessem-io/src/readers/fpha_geometry.rs`: VHA and tailrace readers.
- `crates/labdessem-io/src/output/hydro.rs`: `write_hydro_csv` and the column preamble.
- `crates/labdessem-cli/src/main.rs`: the Summary, `render_engmax_summary`.

## References

- DESSEM Manual do Usuário — Section on hydro production function
- CEPEL documentation on turbine characteristics
- DESSEM Manual de Metodologia (MM), eq. 9.34 and 9.25
