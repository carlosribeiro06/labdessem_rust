# ADR-031: Turbining axis bound and Pmax cap of the computed FPHA fit cloud

Decision brief for methodology divergence B3: the computed fit samples turbining from 0 to the sum of the
units' maximum turbining and caps the exact production function at the plant's installed capacity (Pmax)
wherever the fit evaluates it, while MM §9.17.1 builds the FPHA with turbining from 0 to the maximum
swallowing (engolimento maximo, MM eq. 9.34). Requirement R7 of `plans/fpha-corrections/00-master-plan.md`.
Option names follow the ADR-031 entry of the master-plan frontmatter. Context only, not options here:
ADR-025 (accepted) applies alpha once; ADR-027 (accepted) makes missing physical inputs of computed FPHA a
typed error and leaves EngMax untouched; ADR-028 (accepted) fixes the fit volume window; ADR-029 (accepted)
caps the plane count; ADR-030 (accepted) moves the FPHA rows to the period-mean volume. The tailrace
families and the alpha points are ADR-032 and ADR-033.

## Status

Proposed

## Context

**Current behaviour.** Code at HEAD `089beba` (branch `fix/fpha-corrections`); the fitting code is unchanged
since `8b8afaa`, the commit the ticket verified.

1. `build_fit_params` (`crates/labdessem-cli/src/fpha_integration.rs`) sets `max_turbined_m3s` to the sum of
   the units' `max_turbining_hm3` converted to m3/s with the period duration (`* 1e6 / (period_duration_hours
   * 3600)`), and `max_generation_mw` to the sum of the units' `max_generation_mw`. It reads neither `Hef.m`
   nor `Turbina`.
2. `ProductionFunction` (`crates/labdessem-fpha/src/fitting/production.rs`): `evaluate` clamps the turbining
   to `[0, max_turbined_m3s]`; `evaluate_capped` returns `evaluate(..).min(max_generation_mw)`.
3. The Pmax cap reaches three stages of `fit_plant` (`crates/labdessem-fpha/src/pipeline.rs`) through
   `evaluate_capped`: the hull cloud (`fit_hull_planes`, `fitting/hull_fit.rs`), alpha (`compute_alpha_fpha`,
   `fitting/alpha.rs`) and the deviation (`compute_fit_deviation`, `fitting/deviation.rs`). The gamma_S secant
   (`fit_gamma_s_for_planes`, `fitting/secant.rs`) and the reduction (`reduce_planes`, `fitting/reduction.rs`)
   call the uncapped `evaluate`.
4. `build_grid` (`fitting/grid.rs`) spans the flow axis `[0, pf.max_turbined_m3s()]` and also uses
   `max_turbined_m3s` as the upper end of the spillage axis of the secant. Any change of `max_turbined_m3s`
   therefore moves the flow axis, the clamp of `evaluate` and the secant's spillage axis together.
5. EngMax: `calculate_max_swallowing(q_ef, effective_head_m, net_head_m, alpha)`
   (`crates/labdessem-fpha/src/calculator/mod.rs`, MM eq. 9.34) is called by `plant_bound_hm3`
   (`crates/labdessem-simulation/src/engmax.rs`) with `PlantReference::from_plant` (Q_ef = sum of unit
   maximum turbining, h_ef = unit-count-weighted mean of `Hef.m`, exponent 0.5 Francis/Pelton, 0.2 Kaplan).
   The bound is raw (never clamped to Q_ef), sits beside the static per-unit turbining rows (the effective
   limit is the tighter of the two, D1 of `docs/design/engmax-formulation.md`), is recomputed between solves
   from the period-mean volume (ADR-006) and has no slack (ADR-017). Computed FPHA never calls it.
6. In the fast configuration (`EngMax` 0) the LP turbining domain of a plant is `[0, Q_sum]`, the static
   per-unit rows; generation is bounded by the unit `max_generation_mw` bounds, so the LP never generates
   above Pmax whatever the cloud does.

**Baseline** (`docs/auditoria/fpha-baseline-reproduction.md`, section 9, commit `51751ba`, fast configuration
`opcao_execucao` 2, `rede` 0, `UCT` 0, `UCH` 0, `FCF` 1, `EngMax` 0): planes 6 / 3 / 3 / 5 (CAMARGOS, ITUTINGA,
FUNIL-GRANDE, FURNAS), alpha 0.972834 / 1.0 / 1.0 / 0.993203, fit-grid max deviation CAMARGOS 2.876295 MW and
FURNAS 23.284586 MW, computed objective 2420590.3195 R$.

**Static measurement.** A scratch Cargo crate in the session scratchpad, outside the repository, with its
own `[workspace]` table and path dependencies on `labdessem-fpha` and `labdessem-io` (HEAD `089beba`,
release build, `CARGO_TARGET_DIR` in the scratchpad). It reads a derived copy of `examples/24Barras` (`CAD`
and `OPER` copied, `study_config.json` set to the fast computed configuration above) with
`read_study_from_path`, `read_vha_geometry` and `read_tailrace`, rebuilds `FitPlantParams` exactly as
`build_fit_params` does (MLT from `inflow_history.parquet`, default `FphaPlantConfig` with the `CAD_UHE.csv`
`[Vmin, Vmax]` reference), and calls `fit_plant`. `examples/24Barras` was neither edited nor executed; no
repository file other than this brief was changed.

```bash
cp -r examples/24Barras/CAD examples/24Barras/OPER "$S/case/"   # + fast computed study_config.json
cd "$S/adr031" && CARGO_TARGET_DIR="$S/target" cargo build --release
"$S/target/release/adr031" "$S/case"
```

Reproduction check, run (i) with the current inputs: CAMARGOS 6 planes, alpha 0.972834, max deviation
2.876295 MW; ITUTINGA 3, 1.000000, 0.000000 MW; FUNIL-GRANDE 3, 1.000000, 0.000000 MW; FURNAS 5, 0.993203,
23.284586 MW. This equals section 9 for all four plants, so the comparison below is valid.

Plant inputs (`CAD_UHE.csv`, `CAD_CONJ_UHE.csv`): CAMARGOS Q_sum 214 m3/s (2 x 107), Pmax 46 MW, h_ef 24.6 m,
`Turbina` 2 (Kaplan, 0.2); FURNAS Q_sum 1506 m3/s (6 x 188 + 2 x 189), Pmax 1216 MW, h_ef 89.825 m,
`Turbina` 1 (Francis, 0.5). Maximum swallowing (eq. 9.34) with the tailrace at Q_sum and zero spillage
(the conditions of MM §9.17.1), at four candidate head volumes:

| Plant | Volume | V (hm3) | h_l (m) | QmaxEng (m3/s) | QmaxEng / Q_sum |
|---|---|---|---|---|---|
| CAMARGOS | `VolInic.hm3` | 786.09 | 26.8779 | 217.824038 | 1.017869 |
| CAMARGOS | window min | 120.000 | 12.9775 | 188.306427 | 0.879937 |
| CAMARGOS | window mid | 456.000 | 21.6975 | 208.693442 | 0.975203 |
| CAMARGOS | window max | 792.000 | 26.9475 | 217.936784 | 1.018396 |
| FURNAS | `VolInic.hm3` | 5833.84 | 76.3054 | 1388.046458 | 0.921678 |
| FURNAS | window min | 5733.000 | 76.1199 | 1386.357802 | 0.920556 |
| FURNAS | window mid | 14341.500 | 87.1699 | 1483.575037 | 0.985110 |
| FURNAS | window max | 22950.000 | 94.1199 | 1541.583241 | 1.023628 |

The FURNAS value at `VolInic.hm3` (1388 m3/s) is the EngMax bound of the first solve (`initial_bounds`) and
equals the hand computation of audit item EM-01 (`docs/auditoria/2026-11-05-auditoria-modelo.md`). ITUTINGA
and FUNIL-GRANDE are one-point plants (`Vmin = Vmax`, constant head): QmaxEng 244.929447 m3/s (Q_sum 241) and
579.724545 m3/s (Q_sum 573) at every volume. The uncapped FPH at (window max, Q_sum) exceeds Pmax for every
plant: CAMARGOS 50.556080 MW (Pmax 46), ITUTINGA 56.501977 MW (52), FUNIL-GRANDE 191.008780 MW (180), FURNAS
1275.077221 MW (1216), so the cap is active in every current cloud.

Fit results. "Raw" puts QmaxEng itself on the axis; "clipped" uses `min(QmaxEng, Q_sum)`. "Reported" is
`deviation.max_abs_mw` of `fit_plant` (its reference is the capped FPH in (i) and (ii), the uncapped FPH in
(iii)). "LP-effective" is `max |min(alpha * envelope, Pmax) - min(FPH, Pmax)|` on the same fit grid, the
envelope the LP can reach once the unit generation bounds cut it at Pmax; it is comparable across runs.

| Plant | Run | Q axis max (m3/s) | Pmax cap | Planes | Alpha | Reported max (MW) | LP-effective max (MW) |
|---|---|---|---|---|---|---|---|
| CAMARGOS | (i) current | 214.000000 | yes | 6 | 0.972834 | 2.876295 | 2.876295 |
| CAMARGOS | (ii) (b) raw, `VolInic` head | 217.824038 | yes | 7 | 0.974697 | 2.834441 | 2.834441 |
| CAMARGOS | (ii) (b) raw, window-max head | 217.936784 | yes | 7 | 0.974752 | 2.833201 | 2.833201 |
| CAMARGOS | (ii) (b) clipped, window-max head | 214.000000 | yes | 6 | 0.972834 | 2.876295 | 2.876295 |
| CAMARGOS | (ii) (b), window-min head | 188.306427 | yes | 5 | 0.966375 | 3.272638 | 3.272638 |
| CAMARGOS | (iii) (c) raw, `VolInic` head | 217.824038 | no | 5 | 0.966958 | 3.714006 | 3.714006 |
| CAMARGOS | (iii) (c) clipped, window-max head | 214.000000 | no | 5 | 0.966882 | 3.658006 | 3.658006 |
| FURNAS | (i) current | 1506.000000 | yes | 5 | 0.993203 | 23.284586 | 23.284586 |
| FURNAS | (ii) (b), `VolInic` head | 1388.046458 | yes | 3 | 0.990111 | 38.872802 | 38.872802 |
| FURNAS | (ii) (b), window-mid head | 1483.575037 | yes | 6 | 0.991921 | 24.896522 | 24.896522 |
| FURNAS | (ii) (b) raw, window-max head | 1541.583241 | yes | 6 | 0.993585 | 22.156326 | 22.156326 |
| FURNAS | (ii) (b) clipped, window-max head | 1506.000000 | yes | 5 | 0.993203 | 23.284586 | 23.284586 |
| FURNAS | (iii) (c), `VolInic` head | 1388.046458 | no | 3 | 0.990111 | 38.872802 | 38.872802 |
| FURNAS | (iii) (c) raw, window-max head | 1541.583241 | no | 4 | 0.989997 | 35.628881 | 35.628881 |
| FURNAS | (iii) (c) clipped, window-max head | 1506.000000 | no | 4 | 0.990019 | 34.728022 | 34.728022 |

Measured effect: (b) with the clipped window-max head (S1 below) leaves every plant bit-identical to (i): CAMARGOS 6 planes, alpha 0.972834, 2.876295 MW; FURNAS 5 planes, alpha 0.993203, 23.284586 MW; ITUTINGA and FUNIL-GRANDE unchanged, because QmaxEng at the top of the window exceeds Q_sum for all four plants.

Measured effect: (b) with the `VolInic` head (the EngMax first-solve bound) changes FURNAS from 5 to 3 planes, alpha 0.993203 to 0.990111 and max deviation 23.284586 to 38.872802 MW; CAMARGOS goes from 6 to 7 planes and 2.876295 to 2.834441 MW on a cloud that reaches 217.82 m3/s, above the 214 m3/s the static unit rows allow.

Measured effect: (c) (cap dropped, clipped window-max head, so the Q axis equals (i)) raises the LP-effective max deviation of CAMARGOS from 2.876295 to 3.658006 MW (6 to 5 planes, alpha 0.966882) and of FURNAS from 23.284586 to 34.728022 MW (5 to 4 planes, alpha 0.990019); ITUTINGA goes from 3 to 2 planes (LP-effective 0.113502 MW), FUNIL-GRANDE stays at 3 planes and 0.000000 MW.

Measured effect: under (c) the reported deviation against the uncapped FPH understates what the cloud spends above Pmax: against the capped FPH the unclipped envelope of FURNAS deviates by up to 75.824625 MW (raw window-max head) and 46.351056 MW (clipped), the excess lying entirely above Pmax where the unit bounds already cut generation.

Measured effect: the re-optimized objective under (b) or (c) is `[not measured]`; it needs a code change, which this ticket does not make. Under (b) with the clipped window-max head the fit is bit-identical on 24Barras, so the 2420590.3195 R$ objective would not move there.

## Methodology reference

No manual PDF and no ONS deck are on this machine (`~/projects/dessem/data/files/` and
`~/modelos/dessem/references/` are absent), so every claim below comes from the `dessem` skill references
(`~/.claude/skills/dessem/references/`), which cite MM 22.3.0 (chapter 9 formulation and equation numbers
unchanged in MM 22.4.1, `42-metodologia-hidreletricas.md` section 20) and the Libs manual of 30/09/2026.

- FPHA construction, `42-metodologia-hidreletricas.md` section 16 step 1 (MM §9.16.11, p. 71-76 / PDF 97-102):
  the V x Q grid has NPTV x NPTQ points and "o turbinamento vai de 0 ao engolimento maximo". MM defaults:
  5 x 5 for daily regulation, 2 x 5 for weekly/monthly; one turbining point when the tailrace polynomial is
  constant; operative constraints (e.g. Qmin) may narrow the window; record `FP` overrides all.
- Maximum swallowing in the FPHA construction, `42-metodologia-hidreletricas.md` section 17 (MM §9.17.1,
  p. 76-79 / PDF 102-105): turbining "de 0 ao maximo", with spillage and lateral flows null (except gauge
  stations) and without maintenance. The quantity is eq. 9.34,
  `QmaxEng = (h_l(q_def) / h_ef)^alpha * Q_ef`, h_ef the unit-count-weighted mean of the sets' effective
  heads, Q_ef the sum of effective flows, alpha 0.5 Francis/Pelton, 0.2 Kaplan. labdessem's
  `calculate_max_swallowing` implements exactly this form.
- Maximum swallowing as an LP/MILP limit, `42-metodologia-hidreletricas.md` section 17 (MM §9.17.2): part of
  the iterative process (MM chapter 27), recomputed between solves, with maintenance. It is computed
  "a priori" because h_l depends on V and Q_def (MM note 13: an implicit version is in development).
- Libs first-iteration rule, `42-metodologia-hidreletricas.md` section 17 (Libs §15.6.2): "no DESSEM, a 1a
  iteracao nao usa o engolimento: usa a soma das vazoes maximas de cadastro". The reference places this
  under the LP limit, not under the construction bullet. The Libs build the FPHA planes by default in
  official DESSEM (MU 22.3.2 §III.6.3.5, section 16 "Abordagens e opcoes"), and the planes are built before
  the first iteration, so whether the Libs cloud stops at the unit sum (this rule) or at the maximum
  swallowing (MM §9.17.1) is not stated `[to confirm]`.
- Physical limits, `42-metodologia-hidreletricas.md` section 9 (MM §9.11, p. 48-49 / PDF 74-75; Libs
  §15.5.2): (9.10) `0 <= Q_i^t <= Qbar_i = sum_j qbar_ij` and (9.12) `0 <= GH_i^t <= GHbar_i = sum_j ghbar_ij`,
  maxima from the available units; "o limite efetivo de Q e o engolimento maximo". (9.12) is a separate LP
  bound on generation, which is what labdessem's unit `max_generation_mw` bounds implement.
- Pmax cap of the FPH when building the cloud: neither section 15 (FPH, MM §9.16.1-9.16.10) nor section 16
  (FPHA, MM §9.16.11-9.16.12) of `42-metodologia-hidreletricas.md` says that the FPH is capped at
  `GHbar_i` at the grid points; step 2 only says "FPH exata em cada ponto, com S = 0". Whether DESSEM caps
  the cloud is `[to confirm]`.
- Head at which the maximum swallowing is evaluated during the FPHA construction: `42-metodologia-hidreletricas.md`
  section 17 (MM §9.17.1) states the flow conditions (spillage and laterals null) but not the volume or the
  tailrace flow of `h_l(q_def)`, nor whether the bound is one scalar per plant or one value per volume grid
  point `[to confirm]`.
- Deck evidence for the LP limit (not the cloud), `23-saidas-diagnostico-b.md` section 2.6 (MU §V.11.1;
  MM §9.17.2), PDO_AVAL_QMAXUSIH of the 032026 full case, DESSEM 22.0.2:
  `1 ; 001 ; CAMARGOS ; 99.20 ; 214.00; 0.00; 912.934; 214.00; 886.077; 26.76; 214.00; 217.64 ; 46.00 ;`
  (`Vutil Med`, `Qtur`, `Qver`, `Hmon`, `Qjus`, `Hjus`, `Hliq`, `QturMax`, `Engolim`, `Ghmax`). With the
  24Barras CAMARGOS reference (Q_ef 214, h_ef 24.6, Kaplan 0.2), eq. 9.34 at `Hliq` 26.76 gives 217.633
  m3/s, equal to `Engolim` 217.64 within the 2-decimal rounding of `Hliq`. So DESSEM 22.0.2 reports a raw
  swallowing above `QturMax` (as labdessem's D1 does), evaluated at the period-mean volume (`Vutil Med`),
  and `Ghmax` 46.00 MW equals the 46 MW installed capacity of 24Barras (2 x 23 MW). These are LP-limit
  quantities after the N-th solve; they say nothing about the cloud `[to confirm]`.
- A numeric check would settle both open points, `23-saidas-diagnostico-b.md` sections 2.2 and 2.6
  (MU §V.11.4, §V.11.1): compare the turbining extent implied by the AVL_FPHA1 cuts of the 032026 full case
  with `QturMax` and with `Engolim` of PDO_AVAL_QMAXUSIH_000 (`N = 0`, before the first LP), and check whether
  the cuts' maximum at full turbining exceeds the installed capacity. The check needs the deck, which is not
  on this machine `[to confirm]`.

## Options

### (a)

(a) keep the sum of unit maximum turbining and the Pmax-capped cloud

- Gains: no code, test or output change; section 9 stays the baseline. The cloud spans exactly the LP
  turbining domain of the fast configuration (`EngMax` 0, static unit rows `[0, Q_sum]`) and, with
  `EngMax` 1, every Q the LP can reach at volumes whose swallowing exceeds Q_sum. Of the measured runs on
  the same Q axis, it has the lowest LP-effective deviation (CAMARGOS 2.876295 MW against 3.658006 MW
  uncapped, FURNAS 23.284586 MW against 34.728022 MW). Computed FPHA keeps needing no EngMax input (ADR-027
  S2 untouched).
- Losses: the construction diverges from the literal MM §9.17.1 ("0 ao engolimento maximo"); with `EngMax`
  1 and a low reservoir (FURNAS at `VolInic`, EngMax 1388 m3/s), the cloud spends planes on the
  1388-1506 m3/s band the EngMax row forbids in that solve.
- Risks: if the `[to confirm]` check shows that DESSEM's cloud stops at the maximum swallowing, labdessem's
  planes differ from DESSEM's for plants whose swallowing at the operating head is below Q_sum. If it shows
  DESSEM does not cap the cloud at Pmax, the cap is a second divergence; both are documented, not hidden.
- Fit cloud: Q axis `[0, Q_sum]` (`build_fit_params`, `build_grid`), FPH capped at Pmax in the cloud, alpha
  and deviation; secant and reduction uncapped. Unchanged.
- EngMax coupling: none. Computed FPHA reads no `Hef.m` or `Turbina`; EngMax keeps its own
  `PlantReference` and per-solve bounds (ADR-006, ADR-017).
- Tickets affected: no implementation ticket. Ticket-029 documents the Q axis `[0, Q_sum]`, the Pmax cap in
  three stages, the divergence from MM §9.17.1 and the `[to confirm]` items. Ticket-030 is unaffected.

### (b)

(b) bound the Q axis by the maximum swallowing (labdessem_fpha::calculate_max_swallowing)

- Gains: the construction follows the literal MM §9.17.1 and reuses the eq. 9.34 function EngMax already
  uses (one definition of the swallowing in the codebase). With a low-head plant whose swallowing stays
  below Q_sum over the whole window, the planes concentrate on the reachable turbining range.
- Losses: computed FPHA must read `Hef.m` and `Turbina` even with `EngMax` 0 (S2). On 24Barras, with the
  recommended S1 head (clipped, top of the window) nothing changes (all four plants bit-identical), so the
  change buys no measured accuracy here. With the `VolInic` head, FURNAS degrades from 5 planes and
  23.284586 MW to 3 planes and 38.872802 MW, and with `EngMax` 0 the LP still turbines up to 1506 m3/s,
  118 m3/s beyond the cloud, on extrapolated planes.
- Risks: the head is not stated by the sources `[to confirm]`; any scalar head is a labdessem choice, and a
  head below the operating range shortens the cloud under the LP domain. A raw (unclipped) bound samples Q
  above Q_sum that the static unit rows forbid (CAMARGOS 217.82, FURNAS 1541.58 m3/s). Moving
  `max_turbined_m3s` also moves the `evaluate` clamp and the secant's spillage axis (`build_grid`), so the
  gamma_S fit changes with it.
- Fit cloud: Q axis `[0, min(QmaxEng(h_S1), Q_sum)]` per S1; Pmax cap unchanged in the three stages.
- EngMax coupling: computed FPHA calls `calculate_max_swallowing` with the EngMax plant reference (Q_ef,
  h_ef, turbine exponent) even when `EngMax` is 0, so EngMax input errors (partial `Hef.m`, Unknown
  `Turbina`) become computed-FPHA input errors too (S2). The per-solve EngMax bounds and their rows are
  unchanged (ADR-006, ADR-017); with `EngMax` 1 the cloud's bound is fixed at fit time while the row moves
  between solves.
- Tickets affected: an implementation ticket ([ADD], S4) changes `build_fit_params` (axis bound, new
  `FphaInput` variants), its tests in `fpha_integration/physical_data_tests.rs`, and re-records section 9
  if any plant moves. Ticket-029 documents the swallowing-bounded axis and S1. Ticket-030 lists `Hef.m` and
  `Turbina` as inputs of computed FPHA.

### (c)

(c) option (b) and drop the Pmax cap from the cloud, leaving the cap to unit bounds

- Gains: everything of (b), plus the cloud is the plain FPH, as step 2 of the MM construction literally
  reads ("FPH exata em cada ponto"); the generation limit (9.12) is left to the unit bounds that already
  implement it in the LP.
- Losses: the hull spends planes above Pmax, where the unit bounds already cut generation: the LP-effective
  max deviation rises from 2.876295 to 3.658006 MW (CAMARGOS) and from 23.284586 to 34.728022 MW (FURNAS),
  with 5 instead of 6 and 4 instead of 5 planes. All of (b)'s losses apply.
- Risks: the reported deviation changes meaning (it would be measured against the uncapped FPH and count
  the region above Pmax, up to 75.824625 MW for FURNAS) unless S3 keeps a capped reference; alpha is fitted
  to a region the LP never reaches, so it moves away from 1 (CAMARGOS 0.972834 to 0.966882). Whether DESSEM
  caps is `[to confirm]`, so (c) may introduce a divergence instead of removing one.
- Fit cloud: Q axis as (b); FPH uncapped at the stages S3 names; secant and reduction unchanged (already
  uncapped).
- EngMax coupling: as (b).
- Tickets affected: as (b), plus `ProductionFunction` (`evaluate_capped` call sites per S3), the
  labdessem-fpha unit and regression tests that pin capped values (`tests/regression.rs`), and a section 9
  re-record (planes and alpha move on 24Barras). Ticket-029 documents the uncapped cloud and the deviation
  reference; ticket-030 as (b).

## Recommendation

(a). The sources do not settle the turbining axis: MM §9.17.1 says "0 ao engolimento maximo", while the
Libs, the default FPHA builder of official DESSEM, say that DESSEM's first iteration uses the sum of the
registered maximum flows, and neither states the head of the swallowing during the construction
`[to confirm]`. The measurement then decides on accuracy and coupling:

- with the only head that keeps the cloud over every Q the LP can reach in the window (top of the window,
  clipped at Q_sum), (b) is bit-identical to (a) on all four 24Barras plants, so it adds the EngMax input
  coupling (S2) for no measured gain;
- with the head of EngMax's first solve (`VolInic`), (b) shortens the FURNAS cloud below the fast
  configuration's LP domain and degrades its fit from 23.284586 to 38.872802 MW;
- (c) degrades the LP-effective fit of CAMARGOS and FURNAS (to 3.658006 and 34.728022 MW) because the planes
  are spent above Pmax, where the unit bounds (9.12) already cut generation.

(a) keeps the cloud equal to the LP's turbining domain of the fast configuration, keeps computed FPHA free of
EngMax inputs (ADR-027 S2) and is a two-way door. The divergence from MM §9.17.1 is confirmed and recorded,
not removed. Rework trigger: if the 032026 full-case check (AVL_FPHA1 cut extent against `QturMax` and
`Engolim` of PDO_AVAL_QMAXUSIH_000) shows that DESSEM 22.0.2 builds its cloud to the maximum swallowing, the
deck wins and (b) is reopened with the S1 below; if it shows an uncapped cloud, (c)'s cap question is
reopened with the S3 below.

S1: For (b)/(c), one scalar per plant: eq. 9.34 at the head of the top of the fit volume window
(`FittingBounds.v_max` from `fit_context`, so the ADR-028 window decides it), with the tailrace at Q_sum and
zero spillage (the flow conditions of MM §9.17.1), and the axis bound `min(QmaxEng, Q_sum)`. The forebay
level rises with V, so this is the largest swallowing in the window: the cloud covers every turbining the LP
can reach at any window volume, and the clip keeps it inside the static unit rows (D1), which the LP never
leaves. One scalar keeps `build_grid` rectangular, so alpha, the secant and the deviation code do not
change. Measured on 24Barras: bit-identical to (a) (QmaxEng / Q_sum 1.018396 CAMARGOS, 1.023628 FURNAS,
1.016305 ITUTINGA, 1.011736 FUNIL-GRANDE). Alternatives: the `VolInic` head (EngMax `initial_bounds`
symmetry; measured above, FURNAS 3 planes, 38.872802 MW) and one bound per volume grid point (closest to
"h_l depends on V", but a non-rectangular grid touching `build_grid`, alpha and the deviation;
`[not measured]`).

S2: For (b)/(c), computed FPHA then requires Q_ef (sum of `MaxTurb.m3/s` x `Unidades`, already read),
h_ef (`Hef.m` of `CAD_CONJ_UHE.csv`, unit-count-weighted as `PlantReference::from_plant`) and the turbine
exponent (`Turbina` of `CAD_UHE.csv`), even when `EngMax` is 0. Policy, as ADR-027: a typed
`MissingInputs` naming the plant for a partial `Hef.m` (some units `<= 0`, the case EngMax rejects with
`PartialEffectiveHead`) and for an Unknown `Turbina` (no silent 0.5 exponent); a plant with `Hef.m <= 0` on
every unit keeps the unit sum, because `calculate_max_swallowing` returns Q_ef there, the same inert rule as
EngMax. No fallback to the unit sum for the other cases. This does not conflict with ADR-017: the EngMax rows,
their bounds, their missing slack and the infeasibility diagnosis are untouched; the fit bound is a separate,
fit-time quantity. It does extend ADR-027 S2 ("EngMax scope untouched") to two new computed-FPHA inputs,
which the decision must accept explicitly.

S3: For (c), the cloud (`fit_hull_planes`) and alpha (`compute_alpha_fpha`) drop the cap, since both define
the fit; the deviation (`compute_fit_deviation`) keeps `evaluate_capped` as its reference and compares it
with `min(alpha * envelope, Pmax)`, the envelope the LP can reach under the unit bounds. Dropping the cap in
the deviation as well would report the region above Pmax (FURNAS up to 75.824625 MW) as fit error that never
reaches the LP. `evaluate_capped` then has one call site.

S4: Under (a) no implementation ticket: ticket-029 documents the Q axis `[0, Q_sum]`, the Pmax cap in three
stages, the confirmed divergence from MM §9.17.1, the rework trigger and the `[to confirm]` items; ticket-030
is unaffected. An accepted (b) or (c) is delivered by an [ADD] implementation ticket in this plan, added by
the orchestrator through the Scope Change Approval protocol and run before ticket-029 (scope: `build_fit_params`
and its tests for (b); plus `ProductionFunction` call sites, `labdessem-fpha` tests and the section 9
re-record for (c)); ticket-029 then documents the new axis (and the cap for (c)) as current behaviour, and
ticket-030 lists `Hef.m` and `Turbina` as computed-FPHA inputs. A follow-up plan is the alternative, in which
case ticket-029 documents (a) as current behaviour and the accepted option as planned.

## Decision

PENDING

## Consequences

Pending the decision. Under (a): no code change; ticket-029 records the divergence, the rework trigger and the
`[to confirm]` items. Under (b) or (c): an [ADD] implementation ticket per S4 (or a follow-up plan), the S2
input policy, and for (c) the S3 split of `evaluate_capped`.
