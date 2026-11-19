# FPHA Formulation

This document describes how labdessem models hydro generation with the FPHA (Função de Produção Hidrelétrica Aproximada), as the code implements it. The code is normative: when this document and the code disagree, the code wins and this document is fixed. The before/after values of each behavior change are in the `[Unreleased]` section of [`CHANGELOG.md`](../../CHANGELOG.md).

Terms used below:

- FPH: the exact hydro production function, in MW (`ProductionFunction::evaluate`, [`production.rs`](../../crates/labdessem-fpha/src/fitting/production.rs)).
- FPHA: a set of planes `γ_0 + γ_V·V + γ_Q·Q + γ_S·S`, each a linear bound on generation, fitted to the FPH. `V` is the reservoir volume in hm3, `Q` the turbined flow and `S` the spilled flow, both in m3/s.
- MM, MU, Libs: the DESSEM Methodology Manual (22.3.0), the DESSEM User Manual (22.3.2) and the Libs manual (11.0.0). The decision briefs cite them through secondary summaries; the manual PDFs were not re-read. A DESSEM claim in this document carries a section number or `[to confirm]`.
- MLT: the long-term mean inflow of a plant in m3/s, used as the spillage range of the `γ_S` fit.
- VHA: the volume-height-area table of a plant, read from `CAD/CAD_GEOMETRIA_UHE.csv`.

The FPH is `K · η · Q · h_net` with `K = 0.00981` MW/(m3/s · m), `η` the plant efficiency, `Q` clamped to `[0, Q_max]` and

```
h_net = max(forebay(V) − tailrace(Q + S) − loss, 0)
```

`forebay(V)` is the linear interpolation of the VHA table, clamped (not extrapolated) at the table ends. `tailrace(Q + S)` is one degree-4 polynomial in the total outflow (ADR-032). `loss` is `Perda.m` in metres. `evaluate_capped` returns the FPH limited to the plant installed capacity `Pmax`.

## Modes

The `FPHA` key of `study_config.json` takes one of three strings. A number (the earlier `0` and `1` flags) fails the parse of the file in `read_study_config`.

| `FPHA` value | Source of the planes | Hydro generation rows | Inputs |
| --- | --- | --- | --- |
| `"constant_productivity"` | none | `hydro_productivity[p,t]`: `Σ_u g[u,t] = Prod.mw/m3s / conv · Σ_u q[u,t]` | `Prod.mw/m3s` of `CAD/CAD_UHE.csv` |
| `"precomputed"` | rows of `OPER/OPER_FPHA.csv` | `hydro_fpha[p,seg,t]` and `hydro_generation_turbining_coupling[p,g,u,t]` | `OPER/OPER_FPHA.csv` |
| `"computed"` | `compute_fpha` fits each plant with `fit_plant` | the same rows as `"precomputed"` | `CAD/CAD_GEOMETRIA_UHE.csv`, `CAD/CAD_TAILRACE.csv`, `Eficiencia` and `Perda.m` of `CAD/CAD_UHE.csv`, optional `CAD/CAD_FPHA_CONFIG.csv`, `OPER/inflow_history.parquet` or the deck mean for the MLT |

`conv = period_duration_hours · 0.0036` converts a flow in m3/s to hm3 per period. The variables `g[u,t]` (MW), `q[u,t]` and `S[t]` (hm3 per period) are per unit `u` and period `t`.

The computed mode writes `RESULTADOS/resultado_fpha_computed.csv`. It does not read `OPER/OPER_FPHA.csv`: the deck is read with FPHA off, and `finalize_fpha_injection` turns FPHA on after the planes are injected. The CLI runs `compute_fpha`, `finalize_fpha_injection` and `write_computed_results` in this order in `run_and_print` ([`main.rs`](../../crates/labdessem-cli/src/main.rs)), before the solve.

## LP Constraint

`build_hydro_fpha_constraints` ([`hydro.rs`](../../crates/labdessem-model/src/constraints/hydro.rs)) adds one `<=` row per plant, period and segment, named `hydro_fpha[p=<plant name>,seg=<segment>,t=<1-based period>]`. Precomputed and computed segments use the same builder (ADR-030):

```
Σ_u g[u,t] − c·γ_Q/conv · Σ_u q[u,t] − c·γ_V/2 · V[t] − c·γ_V/2 · V[t+1] − c·γ_S/conv · S[t] ≤ c·γ_0
```

- `c` is the `correction_factor` of the segment (`alpha` in computed mode, `Fcorrec` in `OPER_FPHA.csv`). The row multiplies all four coefficients by `c` once.
- The plane is evaluated at the period-mean volume `(V[t] + V[t+1]) / 2`, so the volume coefficient is split in half over the two volume slots of the period: `V[t]` is the start slot and `V[t+1]` the end slot. Each plant owns `horizon + 1` volume slots; slot 0 is the initial volume.
- In the first period slot 0 is the fixed initial volume `VolInic`. The row has no slot-0 term, and `c·γ_V/2 · VolInic` is added to the right-hand side.
- The spillage `S[t]` enters as the lateral flow of the plane. `γ_S` is not positive, so the coefficient of `S[t]` on the left-hand side is not negative.

Before ADR-030 the row read the end-of-period volume `V[t+1]` only, with the full `c·γ_V`. The CHANGELOG records the change and the measured effect on `examples/24Barras`.

`GeracaoMaxPontoMW` of `resultado_hidreletricas.csv` (`hydro_generation_max_at_point_mw`, [`helpers.rs`](../../crates/labdessem-io/src/output/helpers.rs)) evaluates the planes at the same period-mean volume. `VolumeHM3` of that file stays the end-of-period volume.

A second row couples generation to turbining. `build_hydro_generation_turbining_coupling_constraints` adds, for each unit and period, `g[u,t] ≤ P_max_prod · q[u,t]` with `P_max_prod` the largest `c·γ_Q/conv` over the segments of the plant whose value is finite and positive. A plant with no such segment gets no coupling row.

## Computed Fit Pipeline

`compute_fpha` ([`fpha_integration.rs`](../../crates/labdessem-cli/src/fpha_integration.rs)) fits every hydro plant with generating units and fails when any such plant cannot be fitted:

1. It excludes plants without generating units before any geometry or `inflow_history.parquet` lookup, and counts them in `plants_skipped` (ADR-026).
2. It reads `CAD_GEOMETRIA_UHE.csv`, `CAD_TAILRACE.csv` and `CAD_FPHA_CONFIG.csv`.
3. For each plant with units, `build_fit_params` builds `FitPlantParams`:
   - `Q_max` is the sum of the maximum turbining of every unit, converted to m3/s with the period duration. `Pmax` is the sum of the units' maximum generation.
   - The tailrace polynomial, `Eficiencia` and `Perda.m` are required. An absent one is a missing input, not a default.
   - The MLT is the plant mean of `OPER/inflow_history.parquet`. When the file is absent the MLT is the mean of the deck `natural_inflow_hm3` converted to m3/s, and the CLI prints `Warning: inflow_history.parquet not found; using deck mean for MLT`.
   - The window reference is the `[Vmin.hm3, Vmax.hm3]` interval of the plant in `CAD/CAD_UHE.csv`.
4. It calls `fit_plant` and records each failure with its reason. After the last plant it returns `PlantsNotFitted` with every failure, or the results.

`fit_plant` ([`pipeline.rs`](../../crates/labdessem-fpha/src/pipeline.rs)) runs these stages in this order:

| Stage | Function | Behavior |
| --- | --- | --- |
| Context | `fit_context` | Builds the production function and the fit window `[lo_ref + min_fraction · span, hi_ref − (1 − max_fraction) · span]`. The window must lie inside the VHA table and `min_fraction` must be below `max_fraction`, else `InvalidFitWindow`. A single-point VHA table ignores the window and uses two volumes around the point with half-width 1 hm3 (ADR-028). |
| Hull | `fit_hull_planes` | Samples the capped FPH at spillage 0 on a uniform grid of `volume_points` volumes over the window and `flow_points` flows over `[0, Q_max]`, builds the 3D convex hull with qhull, keeps the upper facets as planes `γ_0 + γ_V·V + γ_Q·Q` and removes exact duplicates. For a single-volume plant a `γ_V` within 1e-6 of 0 is set to 0. |
| Secant | `fit_gamma_s_for_planes` | For each plane, finds the grid points where the plane is the lowest of all planes (the LP binds on the minimum) and takes the one with the highest uncapped FPH. Fits `γ_S` as the least-squares slope of the uncapped FPH against spillage over 9 samples on `[0, S_max]`, with `S_max = 2 · MLT` (or `2 · Q_max` when the MLT is 0). A slope above −1e-10 becomes 0. The target is the unscaled FPH `[to confirm]`. |
| Reduction | `reduce_within_plane_limit` | Merges consecutive planes whose normals differ by less than the angle tolerance (default 1.0 degrees) into their mean plane, never merging the plane through the origin. While more than `max_planes` planes remain, doubles the tolerance and reduces the original plane set again, at most 32 times. A first pass within the limit is returned unchanged. When the limit is not reached, or is 0, the stage fails with `PlaneLimitUnreachable` (ADR-029). |
| Alpha | `compute_alpha_fpha` | Fits `alpha` on the final planes over 30 evaluation points (see below). |
| Deviation | `compute_fit_deviation` | Measures the maximum, mean absolute and root-mean-square difference, over the fit grid at spillage 0, between the lowest of the planes scaled by `alpha` and the capped FPH. |
| Validation | `validate_fitted_planes` | Fails with `NoHyperplanesProduced` for an empty plane set, `NonPositiveAlpha` for `alpha ≤ 0` and `InvalidCoefficient` for `γ_V < −1e-10`, `γ_Q < −1e-10` or `γ_S > 1e-10`. |

Alpha (ADR-033) minimizes `Σ (alpha · FPHA_0 − FPH)²`, so `alpha = Σ FPHA_0 · FPH / Σ FPHA_0²`. `FPHA_0` is the lowest of the final planes at spillage 0 and `FPH` is the capped FPH. The 30 evaluation points are a uniform grid of 6 volumes over the fit window and 5 flows over `[0, Q_max]`, or 2 volumes and 15 flows for a single-volume plant. `volume_points` and `flow_points` do not change them. The count 30 is the MM default (§9.16.11, step 4); the point layout is a labdessem choice `[to confirm]`. When `Σ FPHA_0²` is 0, `alpha` is 1.0. Because the planes are the reduced ones, `alpha` depends on `max_planes`.

The turbining axis is `[0, Q_max]` with `Q_max` the sum of the units' maximum turbining, and the FPH is capped at `Pmax` in the hull cloud, in alpha and in the deviation. The secant and the distance reduction use the uncapped FPH (ADR-031).

## Alpha Contract and OPER_FPHA Copy Rule

`fit_plant` returns the planes unscaled. `FphaPlantResult::planes` hold the fitted `γ_0`, `γ_V`, `γ_Q` and `γ_S`, and `alpha` is a separate field. The model applies `alpha` once to all four coefficients:

- `convert_to_fpha_segments` sets `segment = plane_index + 1`, `correction_factor = alpha`, `rhs = γ_0`, `volume_coefficient = γ_V`, `turbining_coefficient = γ_Q` and `lateral_flow_coefficient = γ_S`.
- `build_hydro_fpha_constraints` multiplies each of them by `correction_factor` (see LP Constraint).

`write_fpha_computed` ([`fpha_computed.rs`](../../crates/labdessem-io/src/output/fpha_computed.rs)) writes the file after a column-description preamble and the header `codigo_usina;nome_usina;plane_index;gamma_0;gamma_v;gamma_q;gamma_s;alpha;max_deviation_mw;mean_deviation_mw;rms_deviation_mw`. The gamma columns are the unscaled planes. The three deviation columns describe `alpha × planes` against the FPH on the fit grid. `plane_index` is 0-based. The writer prints `gamma_0`, `gamma_q` and `alpha` with 6 decimals, `gamma_v` and `gamma_s` with 9 and the deviations with 3.

Copying the rows of a plant into `OPER/OPER_FPHA.csv` reproduces the computed run with this rule:

| `resultado_fpha_computed.csv` | `OPER_FPHA.csv` |
| --- | --- |
| `codigo_usina` | `USIH = codigo_usina` |
| `plane_index` + 1 | `SegFPHA = plane_index + 1` |
| `alpha` | `Fcorrec = alpha` |
| `gamma_0` | `Rhs.mw = gamma_0` |
| `gamma_v` | `Varm.mw/hm3 = gamma_v` |
| `gamma_q` | `Qtur.mw/m3s = gamma_q` |
| `gamma_s` | `Qlat.mw/m3s = gamma_s` |

The `OPER_FPHA.csv` header is `USIH;Nome;SegFPHA;Fcorrec;Rhs.mw;Varm.mw/hm3;Qtur.mw/m3s;Qlat.mw/m3s`. The reader matches the seven named columns without regard to case, ignores `Nome` and skips rows whose first field is empty or starts with `-`. `Varm.mw/hm3` is a coefficient on the period-mean volume.

The test `computed_csv_round_trip_reproduces_objective` ([`fpha_e2e.rs`](../../crates/labdessem-cli/tests/fpha_e2e.rs)) checks that the copy reproduces the computed objective within a relative tolerance of 1e-5. A copy with `Fcorrec = 1` is a different model: on `examples/24Barras` the CHANGELOG records 2374967.2890 R$ for it against 2440503.4875 R$ for the computed run.

## Fitting Configuration

Defaults come from named constants in [`config.rs`](../../crates/labdessem-fpha/src/config.rs):

| Constant | Value | Meaning |
| --- | --- | --- |
| `DEFAULT_MAX_PLANES` | 10 | Plane limit enforced by the reduction stage |
| `DEFAULT_VOLUME_POINTS` | 5 | Volume points of the fit grid (MM §9.16.11 default) |
| `DEFAULT_FLOW_POINTS` | 5 | Flow points of the fit grid (MM §9.16.11 default) |
| `DEFAULT_ANGLE_TOLERANCE_DEG` | 1.0 | Angle tolerance of the reduction, in degrees (Libs §15.6.1) |

The default window is the whole `[Vmin.hm3, Vmax.hm3]` interval of `CAD/CAD_UHE.csv` (fractions 0 and 1). The defaults apply to every plant with a volume span, with or without `CAD/CAD_FPHA_CONFIG.csv` (ADR-028). labdessem has no regulation-type input and no `NPTV` or `NPTQ` column.

`CAD/CAD_FPHA_CONFIG.csv` is optional and has one row per plant. `read_fpha_config` ([`fpha_geometry.rs`](../../crates/labdessem-io/src/readers/fpha_geometry.rs)) reads `;`-delimited rows with this header:

```
codigo_usina;num_planes;tolerance;min_volume_pct;max_volume_pct
1;;5.0;;
0;2;;;
3;;;0.5;0.6
```

| Column | Accepted aliases | Effect on the plant |
| --- | --- | --- |
| `codigo_usina` | `Codigo`, `CODIGO`, `codigo` | Plant code of `CAD_UHE.csv` |
| `num_planes` | `MaxPlanes`, `max_planes` | Plane limit, an integer of at least 1 |
| `tolerance` | `ReductionTolerance`, `Tolerance` | Angle tolerance of the reduction in degrees, not negative |
| `min_volume_pct` | `MinVolumePct` | Lower window bound as a fraction in [0, 1] of the `CAD_UHE.csv` interval |
| `max_volume_pct` | `MaxVolumePct` | Upper window bound as a fraction in [0, 1] of the `CAD_UHE.csv` interval |

An empty cell keeps the default. Despite the `_pct` suffix, the window bounds are fractions, not percentages. The reference of the window is the `CAD_UHE.csv` `[Vmin.hm3, Vmax.hm3]` interval, not the VHA span. A window bound on a plant with a single-point VHA table has no effect and the CLI prints the warning listed under Failure Policies. The example `fpha_fit` has no `CAD_UHE.csv`, so it takes the window as a fraction of the VHA span.

`CAD/CAD_TAILRACE.csv` has the header `codigo_usina;a0.m;a1.m/m3s;a2.m/m3s2;a3.m/m3s3;a4.m/m3s4`, one row per plant, giving the tailrace level in metres as a degree-4 polynomial of the total outflow in m3/s. `CAD/CAD_GEOMETRIA_UHE.csv` has the header `codigo_usina;volume.hm3;cota.m;area.km2`.

## Failure Policies

A failure stops the run before any result is written. The CLI prints `Error: <message>` and exits with code 1. Study-read errors carry no further prefix. Errors that `compute_fpha` converts to `FphaComputeError::Io`, such as a bad `CAD_FPHA_CONFIG.csv`, carry the prefix `I/O error: `. The warning row is the one entry that does not stop the run.

| Message | Error type | Raised by | Notes |
| --- | --- | --- | --- |
| `computed FPHA could not be fitted for <n> hydro plant(s) with generating units:` | `FphaComputeError::PlantsNotFitted` | `compute_fpha` | One line follows per plant: `plant <id> (<name>): <reason>`. A reason is `no rows in CAD/CAD_GEOMETRIA_UHE.csv`, `missing inputs: <inputs>`, `invalid fit parameters: <detail>` or `fitting failed: <detail>` (`FitFailureReason`). |
| `row in CAD/CAD_TAILRACE.csv` | `FphaInput::Tailrace`, inside `FitFailureReason::MissingInputs` | `compute_fpha` through `build_fit_params` | The plant has no tailrace row. |
| `Eficiencia in CAD/CAD_UHE.csv (column absent or empty)` | `FphaInput::Efficiency`, inside `MissingInputs` | `compute_fpha` through `build_fit_params` | An explicit `Eficiencia` is required. |
| `Perda.m in CAD/CAD_UHE.csv (column absent or empty)` | `FphaInput::HydraulicLoss`, inside `MissingInputs` | `compute_fpha` through `build_fit_params` | An explicit `Perda.m = 0` is valid; only an absent column or an empty cell is missing. |
| `hydro <name>: cannot reduce to <n> planes (<k> remain after the last tolerance)` | `FphaFittingError::PlaneLimitUnreachable`, inside `FittingFailed` | `fit_plant`, called by `compute_fpha` | Raise `num_planes` or `tolerance` of the plant. |
| `hydro <name>: invalid fit volume window: <reason>` | `FphaFittingError::InvalidFitWindow`, inside `FittingFailed` | `fit_context`, called by `compute_fpha` | The fractions are outside [0, 1], `min_volume_pct` is not below `max_volume_pct`, or the window leaves the VHA table. |
| `hydro <id> (<name>) missing from inflow_history.parquet` | `FphaComputeError::MissingHydro` | `compute_fpha` | Fails at the first plant, without collecting the others, when the file exists and lacks the plant. |
| `system validation failed after FPHA injection: <detail>` | `FphaComputeError::InvalidSystem` | `finalize_fpha_injection` | Validates the whole system with FPHA on, so a plant with units and no segment fails here (ADR-026). |
| `Warning: plant <id> (<name>) has a single-point table in CAD/CAD_GEOMETRIA_UHE.csv, so min_volume_pct <a> and max_volume_pct <b> in CAD/CAD_FPHA_CONFIG.csv are ignored` | none (stderr warning) | `compute_fpha` | An unset bound prints `unset`. The fit continues. |
| `OPER_FPHA.csv has FPHA rows for hydro plant(s) without generating units in CAD_CONJ_UHE.csv: <code> (<name>), ...` | `IoError::InvalidData` | study read, precomputed mode | Lists every affected plant in ascending code order. |
| `plant <id> has non-finite <column> <value> at line <n> in <file>` | `IoError::InvalidData` | study read for `CAD_GEOMETRIA_UHE.csv` and `CAD_TAILRACE.csv`; `compute_fpha` for `CAD_FPHA_CONFIG.csv` | Rejects `NaN`, `inf` and `-inf`. |
| `plant <id> has duplicate rows at lines <a> and <b> in <file>` | `IoError::InvalidData` | study read for `CAD_TAILRACE.csv`; `compute_fpha` for `CAD_FPHA_CONFIG.csv` | One row per plant. |
| `plant <id> has duplicate volume <v> at lines <a> and <b> in CAD_GEOMETRIA_UHE.csv` | `IoError::InvalidData` | study read | Two points of one plant with the same `volume.hm3`. |
| `plant <id> has num_planes 0 (must be at least 1) at line <n> in CAD_FPHA_CONFIG.csv` | `IoError::InvalidData` | `compute_fpha` | The plane limit is at least 1. |
| `plant <id> has min_volume_pct <a> not below max_volume_pct <b> at line <n> in CAD_FPHA_CONFIG.csv` | `IoError::InvalidData` | `compute_fpha` | Checked only when both bounds are given. |

The `CAD_FPHA_CONFIG.csv` reader also rejects a negative `tolerance` (`plant <id> has negative tolerance <v> at line <n> in CAD_FPHA_CONFIG.csv`) and a window bound outside [0, 1] (`plant <id> has invalid min_volume_pct <v> (must be 0-1) at line <n> in CAD_FPHA_CONFIG.csv`, and the same for `max_volume_pct`).

`build_fit_params` returns `FphaComputeError::MissingFphaInputs` for a plant with missing inputs, and `compute_fpha` records it as `FitFailureReason::MissingInputs`. `FphaComputeError::Fitting` carries a forebay-table construction failure, which `compute_fpha` records as `invalid fit parameters`.

Policies behind the table:

- ADR-026: a plant with generating units that cannot be fitted stops the run, and the error lists every such plant. Plants without generating units are not fitted.
- ADR-027: computed mode never substitutes a 0 m tailrace, an efficiency of 1.0 or a zero loss. The policy covers computed fitting only; EngMax keeps its defaults.
- ADR-029: a plane limit that the reduction cannot reach is an error, not a silent truncation.

## Decisions

Each row summarizes a decision brief in [`docs/auditoria/fpha-decisions/`](../auditoria/fpha-decisions/). All nine briefs are accepted and each accepted option is implemented, or needs no code (ADR-031).

| ADR | Title | Status | Summary |
| --- | --- | --- | --- |
| ADR-025 | Apply the FPHA correction factor alpha exactly once in computed mode | accepted | Accepted 2026-11-18. Option (b): the fit returns unscaled planes and `correction_factor = alpha`; the model applies `alpha` once. [Brief](../auditoria/fpha-decisions/ADR-025-alpha-application.md). |
| ADR-026 | Policy for plants with generating units whose computed FPHA cannot be fitted | accepted | Accepted 2026-11-18. Options (a)+(c): `compute_fpha` returns `PlantsNotFitted`, the system is re-validated after injection, and plants without units are not fitted. [Brief](../auditoria/fpha-decisions/ADR-026-skipped-plant-policy.md). |
| ADR-027 | Policy for missing tailrace, efficiency or hydraulic-loss data in computed FPHA | accepted | Accepted 2026-11-18. Option (a): a typed error names the plant and every missing input; `Perda.m = 0` is valid; EngMax is unchanged. [Brief](../auditoria/fpha-decisions/ADR-027-missing-physical-data-policy.md). |
| ADR-028 | Semantics of CAD_FPHA_CONFIG.csv, fitting defaults and fit volume domain | accepted | Accepted 2026-11-19. Option (a): every field takes effect, with the MM defaults, a window relative to the `CAD_UHE.csv` interval and an angle tolerance in degrees. [Brief](../auditoria/fpha-decisions/ADR-028-fpha-config-semantics.md). |
| ADR-029 | Plane-count rule, reduction criterion and MILP plane selection | accepted | Accepted 2026-11-19. Option (a): `max_planes` is enforced by tolerance doubling with the angle criterion; no MILP plane selection; every plane stays in every solve stage. [Brief](../auditoria/fpha-decisions/ADR-029-plane-count-and-milp-selection.md). |
| ADR-030 | Volume used in the FPHA rows | accepted | Accepted 2026-11-19. Option (b): the rows read the period-mean volume, with `γ_V` split over two slots. Implemented. [Brief](../auditoria/fpha-decisions/ADR-030-fpha-volume-reference.md). |
| ADR-031 | Turbining axis bound and Pmax cap of the fit cloud | accepted | Accepted 2026-11-19. Option (a): keep the axis `[0, Q_max]` and the Pmax cap; the divergence from MM §9.17.1 is recorded. No code change. [Brief](../auditoria/fpha-decisions/ADR-031-turbining-axis-and-pmax-cap.md). |
| ADR-032 | Tailrace modelling for computed FPHA | accepted | Accepted 2026-11-19. Option (a): one degree-4 polynomial per plant; families, piecewise ranges and drowning stay unmodelled; `TailraceSource::Families` is removed. [Brief](../auditoria/fpha-decisions/ADR-032-tailrace-family-modelling.md). |
| ADR-033 | Points and stage at which alpha is computed | accepted | Accepted 2026-11-19. Option (b): `alpha` on 30 evaluation points of the final reduced planes. Implemented. [Brief](../auditoria/fpha-decisions/ADR-033-alpha-evaluation-points.md). |

## Known Limitations

- The plane limit `max_planes` is a labdessem extension. DESSEM limits the plane count only in the MILP, through the `NFHAMILP` selection (MM §27.2), and the `FP` record has no maximum-plane field in the sources read (ADR-029).
- `NFHAMILP` plane selection is not implemented. Every fitted plane stays in every solve stage: LP, MILP, LP with fixed commitment and LP-CALC-CMO (ADR-029).
- A row of `OPER_FPHA.csv` whose plant code is absent from `CAD_UHE.csv` is ignored (`build_hydro_plants`, [`hydro.rs`](../../crates/labdessem-io/src/readers/hydro.rs)).
- The library API accepts a fit without a tailrace. `FitPlantParams.tailrace = None` gives a tailrace level of 0 m, and a missing efficiency or loss gives 1.0 and 0 m (`ProductionFunction::new`). The `fpha_fit` example reaches this path for a plant without a `CAD_TAILRACE.csv` row. The computed CLI path rejects a missing tailrace row under ADR-027. Decision of 2026-11-19: documented, not changed. Rework trigger: a library caller other than the CLI fits without a tailrace.
- Each plant has one tailrace polynomial. Backwater families (MM §9.16.4), piecewise polynomials by flow range (MM eq. 9.24) and explicit drowning (MM §9.16.5, Libs §15.3.3) are not modelled, in computed FPHA and in EngMax (ADR-032).
- The fit samples turbining on `[0, Q_max]`, the sum of the units' maximum turbining, and caps the FPH at `Pmax`. MM §9.17.1 builds the FPHA with turbining from 0 to the maximum swallowing (eq. 9.34). The divergence is recorded, not removed (ADR-031). On `examples/24Barras` the brief measured the maximum-swallowing axis at the head of the window top as identical to the current axis.
- Every plant with a volume span takes the daily defaults. The weekly and monthly defaults of DESSEM need a regulation-type input that labdessem does not have (ADR-028).
- The reduction runs on unscaled planes, and the angle test is not scale-free. This is recorded, not changed (ADR-029, ADR-033).
- `alpha` depends on `max_planes` and on the 30-point layout. For CAMARGOS the ADR-033 brief records `alpha` 0.955708, 0.913740 and 0.797232 for plane limits 5, 4 and 3.
- A single-volume plant can get `alpha` above 1, so the scaled envelope lies above the FPH at the fit-grid nodes: ITUTINGA 1.014733 and FUNIL-GRANDE 1.011552 in `examples/24Barras` (CHANGELOG).
- `compute_alpha_fpha` ignores `n_volume_points` and `n_flow_points` of its bounds and expects the final planes (CHANGELOG).

### Open points `[to confirm]`

These points are open in the ADR-028..033 briefs. This document does not settle them.

- Secant target (ADR-033): whether `γ_S` is fitted against the FPH or against FPH / `alpha`. The code keeps the unscaled FPH. Whether the MM coefficients of eq. 9.33 already carry `alpha` is not stated.
- Alpha points (ADR-033): the distribution of the 30 evaluation points; where the Libs place the reduction relative to the `alpha` fit.
- Alpha default (ADR-033): whether DESSEM fits `alpha` by default (MU `FP` record field 7 defaults to no adjustment; the `Fcorrec` of `AVL_FPHA1` in the 032026 case reads 1.00000 in the one row quoted), and whether the Libs FPHA ignores that flag.
- Tailrace (ADR-032): the conflict between MM §9.16.4, eq. 9.24 and §9.16.5 and Libs §15.3.4 on which mechanism is off by default with piecewise polynomials; the families of the four `examples/24Barras` plants in an official deck (`polinjus.csv`); the downstream level at fit time. The effect of modelling families is `[not measured]`.
- Volume reference (ADR-030): the meaning of `V_i^t` in MM eq. 9.33; the row form that DESSEM 22.0.2 builds under the Libs FPHA; the volume at which `AVL_DESVFPHA` evaluates `Ghid (FPHA`; useful versus total volume in the `Varm` of `AVL_FPHA1`. Rework trigger: if the 032026 full-case check of `AVL_FPHA1` shows that DESSEM 22.0.2 evaluates the FPHA at the end-of-period volume, the deck wins and the end-of-period row is reopened.
- Turbining axis (ADR-031): whether the Libs cloud stops at the unit sum (Libs §15.6.2) or at the maximum swallowing; whether DESSEM caps the FPH at `Pmax` in the cloud; the head at which the maximum swallowing is evaluated. Rework trigger: the cut extent of `AVL_FPHA1` against `QturMax` and `Engolim` of `PDO_AVAL_QMAXUSIH_000`.
- Configuration (ADR-028): how DESSEM clips the `FP` field 8 window around the initial volume at the volume limits; how DESSEM maps a run-of-river plant to the regulation flag; the weekly and monthly default with its regulation-type input.
- Reduction (ADR-028, ADR-029): whether DESSEM accepts the Libs angle card (Libs §15.7), its default angle, and any rule for reaching a plane count by changing the tolerance.
- MILP selection (ADR-029): the centroid definition of MM §27.2; the plane set of `PL.Int.Fix`; whether "Utilizado 5 pontos para o volume" in the DESSEM log is `NFHAMILP`; the per-plant cut count of `AVL_FPHA1` in the 032026 case. The effect of a binding plane limit on MILP solve time is `[not measured]`; rework trigger: if MILP solve time on official decks becomes binding, MILP plane selection is reopened.
- Not measured (ADR-033): the cause of the CAMARGOS plane-count changes recorded in the baseline report, the effect of reducing unscaled planes, and which plant drives the computed-objective change.
- Plants without units (ADR-026): whether the MLT is needed for them outside the FPHA fit.
- Zero values (ADR-027): how DESSEM treats a zero efficiency, loss or tailrace, and which component owns range checks such as efficiency in (0, 1].

## Key Files

Fit crate `labdessem-fpha`:

- [`pipeline.rs`](../../crates/labdessem-fpha/src/pipeline.rs): `fit_plant`, `fit_context`, `reduce_within_plane_limit`, `FitPlantParams`, `FphaPlantResult`.
- [`config.rs`](../../crates/labdessem-fpha/src/config.rs): defaults, `FphaPlantConfig`, `VolumeWindow`.
- [`error.rs`](../../crates/labdessem-fpha/src/error.rs): `FphaFittingError`.
- [`fitting/production.rs`](../../crates/labdessem-fpha/src/fitting/production.rs): `ProductionFunction`, `TailraceSource`.
- [`fitting/grid.rs`](../../crates/labdessem-fpha/src/fitting/grid.rs): the sampling grid and the single-volume synthesis.
- [`fitting/hull_fit.rs`](../../crates/labdessem-fpha/src/fitting/hull_fit.rs): the cloud and the hull planes.
- [`fitting/secant.rs`](../../crates/labdessem-fpha/src/fitting/secant.rs): the `γ_S` secant.
- [`fitting/reduction.rs`](../../crates/labdessem-fpha/src/fitting/reduction.rs): the plane reduction.
- [`fitting/alpha.rs`](../../crates/labdessem-fpha/src/fitting/alpha.rs): `compute_alpha_fpha`.
- [`fitting/deviation.rs`](../../crates/labdessem-fpha/src/fitting/deviation.rs): the deviation columns.
- [`fitting/selection.rs`](../../crates/labdessem-fpha/src/fitting/selection.rs): `validate_fitted_planes`.
- [`examples/fpha_fit.rs`](../../crates/labdessem-fpha/examples/fpha_fit.rs): the stand-alone fit example.

CLI, model and I/O:

- [`fpha_integration.rs`](../../crates/labdessem-cli/src/fpha_integration.rs): `compute_fpha`, `finalize_fpha_injection`, `FphaComputeError`, `convert_to_fpha_segments`.
- [`hydro.rs`](../../crates/labdessem-model/src/constraints/hydro.rs): `build_hydro_fpha_constraints` and the coupling rows.
- [`fpha_geometry.rs`](../../crates/labdessem-io/src/readers/fpha_geometry.rs): the readers of the geometry, tailrace and configuration files.
- [`readers/hydro.rs`](../../crates/labdessem-io/src/readers/hydro.rs): the `OPER_FPHA.csv` reader and its plant check.
- [`fpha_computed.rs`](../../crates/labdessem-io/src/output/fpha_computed.rs): the writer of `resultado_fpha_computed.csv`.
- [`fpha_e2e.rs`](../../crates/labdessem-cli/tests/fpha_e2e.rs): the end-to-end tests, including the copy-rule round trip.

## References

- MM 22.3.0: §9.16.1 to §9.16.12 (FPHA construction, defaults and the `alpha` step), §9.16.4, eq. 9.24 and §9.16.5 (tailrace), eq. 9.33 (FPHA row), §9.17.1 and eq. 9.34 (maximum swallowing), §9.4 and eq. 9.2 (volume interval), §27.2 (`NFHAMILP`).
- Libs 11.0.0: §15.3.3 and §15.3.4 (tailrace), §15.6.1 (angle reduction and the `alpha` bracket), §15.6.2, §15.7.
- MU 22.3.2: §III.5.4.5 (`FP` record), §III.6.3.1 (`REGRANPTV`), §V.11.4 (`AVL_FPHA1`).
- [`docs/auditoria/fpha-decisions/`](../auditoria/fpha-decisions/): the nine decision briefs of the Decisions table.
- [`docs/auditoria/fpha-baseline-reproduction.md`](../auditoria/fpha-baseline-reproduction.md): measurements of the FPHA corrections. Its sections 8 and 9 predate the ADR-030 and ADR-033 implementations; the `[Unreleased]` section of [`CHANGELOG.md`](../../CHANGELOG.md) holds the later before/after values.
- [`docs/design/engmax-formulation.md`](engmax-formulation.md): EngMax, which shares the production-function geometry and the head volume of ADR-006.
