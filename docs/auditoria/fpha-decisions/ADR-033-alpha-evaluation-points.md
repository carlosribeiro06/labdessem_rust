# ADR-033: Points and stage at which the FPHA correction factor alpha is computed

Decision brief for methodology divergence B5 (labdessem fits the correction factor alpha on the fit
grid, on the raw hull planes, before the gamma_S secant and the plane reduction; the MM fits alpha
against the FPH on its own evaluation points, default 30, and the MU `FP` record makes the
least-squares adjustment optional with default "no"). Requirements R7 and R1 of
`plans/fpha-corrections/00-master-plan.md`. Option names follow the ADR-033 entry of the master-plan
frontmatter. Context only, not options here: ADR-025 (accepted) applies alpha once, in the consumer,
and its rework trigger names this ADR ("if ADR-033 moves the alpha computation after plane reduction,
the fit_plant stage order is revisited by the ADR-033 follow-up ticket"); ADR-028 (accepted) set the
5 x 5 grid; ADR-029 (accepted) enforces `max_planes` by iterative reduction and states that alpha is
not recomputed after the reduction. Three alpha-related questions left without an owner by Epics 01-03
are attached (S3, S4): the gamma_S secant target after alpha, the alpha-dependence of the angle
reduction, and the CAMARGOS plane-count changes.

## Status

Accepted (2026-11-19)

## Context

**Current state.** Code at HEAD `66a0a48` (branch `fix/fpha-corrections`); `git diff --stat
8b8afaa..HEAD -- crates examples` is empty, so the ticket's commit `8b8afaa` describes the same code.

1. `crates/labdessem-fpha/src/pipeline.rs`, `fit_plant`: `fit_context` builds the production function
   and the bounds; stage 1 `fit_hull_planes`; stage 2 `compute_alpha_fpha(&raw_planes, &pf, &bounds)`;
   stage 3 `fit_gamma_s_for_planes` on the unscaled planes; stage 4 `reduce_within_plane_limit`
   (ADR-029); stage 5 `compute_fit_deviation` of `scale_planes(&planes, alpha)`; stage 6
   `validate_fitted_planes`. **Alpha is computed on the raw hull, before the secant and the reduction**;
   the deviation is computed on the reduced planes scaled by that alpha.
2. `crates/labdessem-fpha/src/fitting/alpha.rs`, `compute_alpha_fpha`:
   `alpha = sum(FPHA_0 * FPH) / sum(FPHA_0^2)` over `build_grid(pf, bounds)`, the fit grid (ADR-028:
   5 volume x 5 flow points, uniform, over the configured window; for single-volume plants the volume
   axis is the 2 synthesized points of `resolve_fitting_bounds`, whatever `n_volume_points` says).
   `FPHA_0` is the minimum over the planes at spill 0; `FPH` is `evaluate_capped`; a zero denominator
   returns 1.0.
3. `crates/labdessem-fpha/src/fitting/secant.rs`, `fit_gamma_s_for_planes`: fits gamma_S on the
   unscaled planes against the uncapped `pf.evaluate`, while the LP applies `alpha * gamma_S`
   (ADR-025). Whether the secant target should account for alpha is `[to confirm]` since Epic 01.
4. `crates/labdessem-fpha/src/fitting/reduction.rs`, `plane_normal` = `[gamma_v, gamma_q, -1.0]`: the
   angle predicate is not invariant to a scaling of the planes (the `-1` is not scaled); ADR-029 S2
   recorded this without changing it. Under both options below the reduction runs on unscaled planes,
   so alpha never decides which planes survive.
5. Tests that pin the current order bit for bit: `fit_plant_returns_unscaled_planes` and
   `fit_plant_deviation_matches_alpha_scaled_envelope` (`pipeline.rs`), `fit_plant_enforces_plane_limit`
   (`pipeline.rs`, asserts that alpha with a binding limit equals alpha without it) and
   `plane_limit_is_bit_deterministic` (`crates/labdessem-fpha/tests/regression.rs`).
6. Section 9 of `docs/auditoria/fpha-baseline-reproduction.md` (commit `51751ba`): alpha CAMARGOS
   0.972834, ITUTINGA 1.0, FUNIL-GRANDE 1.0, FURNAS 0.993203; planes 6 / 3 / 3 / 5; computed objective
   2420590.3195 R$. Alpha moved from section 8 (0.969607 and 0.991295, 6 x 6 grid) to section 9 because
   of the ADR-028 grid, not because of the ADR-029 plane limit (the default limit 10 binds no plant).
   CAMARGOS plane count: 6 before Epic 02, 7 in section 8, 6 in section 9; the causes were not isolated.

**Measurement** (2026-11-19; no repository change). A scratch Cargo crate in the session scratchpad,
outside the repository (own `[workspace]` table, path dependencies on `crates/labdessem-fpha` and
`crates/labdessem-io`, `CARGO_TARGET_DIR` in the scratch directory), reads a fresh copy of
`examples/24Barras/CAD` and `OPER` (the example was neither edited nor executed) and builds
`FitPlantParams` as the CLI does (`crates/labdessem-cli/src/fpha_integration.rs`: unit sums for
Q_max and P_max, inflow-history mean as MLT, `FphaPlantConfig::computed(DEFAULT_MAX_PLANES,
DEFAULT_VOLUME_POINTS, DEFAULT_FLOW_POINTS, Angle { DEFAULT_ANGLE_TOLERANCE_DEG })` with the registry
volume range as `window_reference_hm3`). It calls the public `fit_context`, `fit_plant`,
`fit_hull_planes`, `compute_alpha_fpha`, `compute_fit_deviation` and `scale_planes`. Option (b) is
emulated by `compute_alpha_fpha(&result.planes, &pf, &bounds')`, with `bounds'` the fit bounds with
`n_volume_points` / `n_flow_points` replaced. The 30-point distribution is not defined in the sources
read `[to confirm]`; two uniform layouts over the same window were measured, 6 V x 5 Q and 5 V x 6 Q,
plus 2 V x 15 Q for the single-volume plants (on which `build_grid` keeps 2 volume points, so 6 x 5
and 5 x 6 give 10 and 12 points there, not 30).

```bash
cargo run --release -- "$SCRATCH/adr033_case"   # in "$SCRATCH/adr033", CARGO_TARGET_DIR="$SCRATCH/adr033/target"
```

Reproduction check: the run prints CAMARGOS alpha 0.972834, 6 planes, max deviation 2.8763 MW;
ITUTINGA 1.000000, 3; FUNIL-GRANDE 1.000000, 3; FURNAS 0.993203, 5; and alpha recomputed on the raw
hull over the fit grid equals the `fit_plant` alpha bit for bit for all four plants. Section 9 is
reproduced.

Alpha per plant and variant, default limit 10 (deviation = `compute_fit_deviation` of alpha x final
planes on the fit grid: max / mean / rms, MW):

| Plant | Planes | (a) fit grid, raw hull | (b) fit grid, final planes | (b) 30 pts 6V x 5Q | (b) 30 pts 5V x 6Q | (b) 2V x 15Q | Deviation (a) | Deviation (b), fit grid |
|---|---|---|---|---|---|---|---|---|
| CAMARGOS | 6 | 0.972834 | 0.969920 | 0.968663 | 0.969524 | n/a | 2.8763 / 0.8808 / 1.2306 | 2.8412 / 0.8868 / 1.2284 |
| ITUTINGA | 3 | 1.000000 | 1.000000 | 1.000000 (10 pts) | 1.005761 (12 pts) | 1.014733 | 0 / 0 / 0 | 0 / 0 / 0 |
| FUNIL-GRANDE | 3 | 1.000000 | 1.000000 | 1.000000 (10 pts) | 1.004185 (12 pts) | 1.011552 | 0 / 0 / 0 | 0 / 0 / 0 |
| FURNAS | 5 | 0.993203 | 0.989526 | 0.989872 | 0.989602 | n/a | 23.2846 / 7.7466 / 10.6830 | 22.1382 / 7.7442 / 10.3484 |

Alpha under a binding `max_planes` (ADR-029 iterative reduction; (a) is unchanged by construction):

| Plant, `max_planes` | Planes | (a) alpha | (b) alpha, fit grid | (b) alpha, 6V x 5Q | Max deviation (a) / (b) fit grid, MW |
|---|---|---|---|---|---|
| CAMARGOS, 5 | 5 | 0.972834 | 0.955708 | 0.954769 | 3.1571 / 2.6729 |
| CAMARGOS, 4 | 4 | 0.972834 | 0.913740 | 0.914871 | 8.6373 / 6.6337 |
| CAMARGOS, 3 | 2 | 0.972834 | 0.797232 | 0.799471 | 20.7012 / 12.5697 |
| FURNAS, 4 | 4 | 0.993203 | 0.975640 | 0.976090 | 69.1746 / 49.7155 |
| FURNAS, 3 | 3 | 0.993203 | 0.939935 | 0.940944 | 176.5107 / 111.7362 |

(FURNAS at limit 5 keeps 5 planes and the default values; ITUTINGA and FUNIL-GRANDE never bind.)

Measured effect: at the default limit, option (b) on the fit grid moves alpha by -0.30 % for CAMARGOS (0.972834 to 0.969920) and -0.37 % for FURNAS (0.993203 to 0.989526), leaves ITUTINGA and FUNIL-GRANDE at alpha 1.0, and lowers the max and rms deviation of both reservoir plants (mean slightly up for CAMARGOS).
Measured effect: the evaluation points move alpha by at most 0.0013 on the reservoir plants (CAMARGOS 0.969920 fit grid vs 0.968663 with 6V x 5Q), less than the stage does (0.0029); on the single-volume plants off-grid points lift alpha above 1 (ITUTINGA 1.014733, FUNIL-GRANDE 1.011552 with 2V x 15Q), so the LP envelope then exceeds the FPH at the grid nodes (max deviation 0.77 and 2.08 MW instead of 0).
Measured effect: under (b) alpha depends on `max_planes` (CAMARGOS alpha 0.955708 / 0.913740 / 0.797232 for limits 5 / 4 / 3), and a binding limit loses less accuracy (FURNAS limit 3: max deviation 111.74 MW instead of 176.51 MW).
Measured effect: 30 points on the raw hull (a further variant, not an option here) give alpha 0.971832 (CAMARGOS, 6V x 5Q) and 0.993576 (FURNAS).
Measured effect: objective of the computed run under (b) `[not measured]` (needs a code change in `fit_plant`); the section 9 baseline is 2420590.3195 R$.

## Methodology reference

Sources: the `dessem` skill references named below (they cite MM 22.3.0, Libs 30/09/2026 and MU
22.3.2); the manual PDFs and the ONS decks are not on this machine, so no page was re-read here and
the only deck evidence is what the references quote.

- `42-metodologia-hidreletricas.md` section 16, step 4 (MM 22.3.0 §9.16.11-9.16.12, p. 71-76 / PDF
  97-102; Libs §15.6.1): correction factor `FPHA = alpha_FPHA * FPHA_0` (eq. 9.32), minimizing the
  mean squared error against the FPH, "default de 30 pontos"; the Libs gives the closed form
  `alpha = sum sum FPHA_0 FPH / sum sum FPHA_0^2`, the formula `compute_alpha_fpha` implements. The MM
  step order is grid (1), FPH (2), convex hull (3), alpha (4), lateral-flow secant (5): alpha follows
  the hull and precedes the secant.
- `11-entdados-registros-a2.md` record `FP` (MU 22.3.2 §III.5.4.5): field 7, column 25, I1, "1 = ajuste
  por minimos quadrados da funcao", 0 or blank = no (default no). The only active `FP` line in the
  2026-06-15 ONS deck is `FP 287 2   10    5           100.00` (Santo Antonio), with column 25 blank;
  the other plants take the defaults.
- `42-metodologia-hidreletricas.md` "Divergencias e duvidas" item 2: in the MM the alpha fit is always a
  step, in the MU `FP` field 7 defaults to "nao ajusta"; whether the Libs FPHA (the DESSEM default)
  ignores that flag is not stated `[to confirm]`.
- `23-saidas-diagnostico-b.md` §2.2 (MU §V.11.4, AVL_FPHA1, column `Fcorrec`): the one real row quoted
  (full case 032026, DESSEM 22.0.2) is CAMARGOS cut 1 with `Fcorrec` 1.00000; whether every CAMARGOS cut
  and every plant of that case has 1.0, which would mean no alpha fit by default, is `[to confirm]`.
- Position of the reduction relative to alpha in the Libs `[to confirm]`: `42-metodologia-hidreletricas.md`
  section 16 and `23-saidas-diagnostico-b.md` lines 542-545 (Libs 11.0.0 §15.6.1 / §15.6) describe the
  similar-hyperplane reduction (angle or distance cards, origin plane never replaced, it "reduz
  restricoes a custa de precisao") but not whether alpha is fitted before or after it; the MM has no
  reduction step at all. The methodology therefore places alpha neither before nor after the reduction.
- Distribution of the 30 points `[to confirm]`: the references give only the count (MM §9.16.11 step 4);
  whether the points are a uniform V x Q grid, random points, or points of the operating domain, and
  how they apply to single-volume plants, is not stated.
- Secant target after alpha (`42-metodologia-hidreletricas.md` section 16, step 5 and "Restricao no
  PL/MILP"; Libs §15.6.1): the secant minimizes the mean squared error against the FPH on
  `[Qlat_hat, Qlat_max]`, and the Libs constraint is `GH <= alpha [gamma_0 + gamma_V V + gamma_Q Q +
  gamma_S (Qlat - Qlat_hat)]`, with gamma_S inside the bracket that alpha multiplies; whether gamma_S is
  fitted against FPH or FPH / alpha is not stated `[to confirm]`.

## Options

### (a)

(a) keep alpha on the fit-grid nodes, computed before plane reduction

- Gains: no code change; alpha 0.972834 / 1.0 / 1.0 / 0.993203 and the 2420590.3195 R$ objective of section 9 stay bit-identical; alpha is independent of `max_planes` (ADR-029 statement holds); the four pinned tests stay as they are.
- Losses: alpha is calibrated on an envelope (11 raw hull planes for CAMARGOS and FURNAS) that the LP never uses; it diverges from the MM default of 30 evaluation points; with a binding `max_planes` the reduced envelope keeps the raw-hull alpha and its deviation grows (FURNAS limit 3: 176.51 MW max).
- Risks: low; the divergence from MM §9.16.11 step 4 stays documented in ticket-029; the MU `FP` field 7 default (no adjustment) and the AVL_FPHA1 `Fcorrec` 1.00000 row stay unexplained under either option.
- Alpha per plant: CAMARGOS 0.972834 (6 planes), ITUTINGA 1.000000 (3), FUNIL-GRANDE 1.000000 (3), FURNAS 0.993203 (5); unchanged by any `max_planes`.
- Pinned tests: `fit_plant_returns_unscaled_planes`, `fit_plant_deviation_matches_alpha_scaled_envelope`, `fit_plant_enforces_plane_limit` and `plane_limit_is_bit_deterministic` unchanged.
- Tickets affected: none added; ticket-029 documents alpha on the fit grid before the reduction as a labdessem divergence from MM §9.16.11 step 4.

### (b)

(b) alpha on separate evaluation points (MM default 30), computed on the final reduced planes

- Gains: alpha is fitted to the envelope the LP uses (after secant and reduction), which is the least-squares optimum for those planes on its points; the point count follows the MM default of 30; lower max and rms deviation at the default limit (CAMARGOS 2.8412 vs 2.8763 MW max, FURNAS 22.1382 vs 23.2846 MW) and much lower under a binding limit (FURNAS limit 3: 111.74 vs 176.51 MW).
- Losses: alpha changes for the reservoir plants (CAMARGOS 0.968663, FURNAS 0.989872 with 6V x 5Q) and the objective changes `[not measured]`; the fit_plant stage order changes (alpha moves after `reduce_within_plane_limit`), which reopens the ADR-025 rework trigger and the ADR-029 statement that alpha is not recomputed after the reduction.
- Risks: alpha depends on `max_planes`, so a binding `MaxPlanes` of `CAD_FPHA_CONFIG.csv` changes alpha (CAMARGOS 0.955708 / 0.913740 / 0.797232 for limits 5 / 4 / 3); the 30-point distribution is not in the sources `[to confirm]`, and off-grid points lift the alpha of single-volume plants above 1 (ITUTINGA 1.014733, FUNIL-GRANDE 1.011552 with 2V x 15Q), so their envelope exceeds the FPH at the grid nodes; the MU `FP` field 7 default and the AVL_FPHA1 `Fcorrec` 1.00000 row suggest DESSEM may not fit alpha by default at all `[to confirm]`.
- Alpha per plant: CAMARGOS 0.968663 (6 planes), ITUTINGA 1.000000 (3), FUNIL-GRANDE 1.000000 (3), FURNAS 0.989872 (5) with 30 points 6V x 5Q on the fit window (the single-volume plants then get 2 x 5 = 10 points; with 2V x 15Q they get 1.014733 and 1.011552); on the fit grid instead: 0.969920 / 1.0 / 1.0 / 0.989526.
- Pinned tests: `fit_plant_enforces_plane_limit` is inverted (alpha with a binding limit must differ from, not equal, the unlimited alpha); `fit_plant_deviation_matches_alpha_scaled_envelope` and `plane_limit_is_bit_deterministic` are re-pinned to the post-reduction alpha (they still pass formally, but their expected values come from a new alpha source); `fit_plant_returns_unscaled_planes` is re-pinned only if S3 changes the secant target (the planes are otherwise bit-identical).
- Tickets affected: a new implementation ticket (stage order of `fit_plant`, `compute_alpha_fpha` evaluation points, the four tests, a regression value for CAMARGOS and FURNAS, CHANGELOG entry for the alpha change); ticket-029 documents the new order and the point set; ticket-032's evidence report gets a new section for the objective.

## Recommendation

(b), the master plan's preliminary recommendation, now with measurements: alpha is a least-squares fit
of the envelope the LP actually applies, so it belongs after the secant and the reduction; at the
default limit the change is small (-0.30 % and -0.37 %), and under a binding limit it recovers a large
part of the lost accuracy. The stage matters more than the points (0.0029 vs at most 0.0013 on
CAMARGOS). Two source gaps stay explicit and are part of the decision: the 30-point distribution and
whether DESSEM fits alpha by default at all (MU `FP` field 7, AVL_FPHA1 `Fcorrec` 1.00000).

S1: A separate evaluation set of 30 points, as the MM default count: a uniform 6 V x 5 Q grid over the same fit window and `[0, Q_max]` for plants with a volume axis, and 2 V x 15 Q for single-volume plants (their volume axis has only the 2 synthesized points); the distribution is a labdessem choice `[to confirm]` against the MM/Libs text. Alternative inside (b): keep the fit grid on the final planes (CAMARGOS 0.969920, FURNAS 0.989526, single-volume plants exactly 1.0), which needs no uncited distribution; it is a request to amend the option set, not recommended over the MM count, but it is the fallback if the distribution cannot be confirmed and the user prefers no envelope overshoot on the single-volume plants.
S2: On the final reduced planes, after `fit_gamma_s_for_planes` and `reduce_within_plane_limit` (spill 0, so gamma_S does not enter alpha); alpha therefore depends on `max_planes`, which reopens the ADR-029 statement that alpha is not recomputed after the reduction (the statement is replaced, not the ADR-029 decision: the cap and its doubling rule stay); the deviation keeps describing `alpha x planes`.
S3: Keep the unscaled FPH target of the secant in this change. The methodology fits the secant against the FPH (MM step 5) and the Libs form puts gamma_S inside the alpha bracket, which suggests a target of FPH / alpha, but no source read states the target `[to confirm]`; changing it would re-pin `fit_plant_returns_unscaled_planes` and scale every gamma_S by 1 / alpha (about +3 % for CAMARGOS, +1 % for FURNAS, not measured on the LP), so it waits for the Libs/MM text and, if confirmed, gets its own follow-up item.
S4: Record, do not change: the reduction runs on unscaled planes under both options, so alpha does not change which planes survive; the angle predicate (`plane_normal` = `[gamma_v, gamma_q, -1.0]`) is still not scale-free. The CAMARGOS plane-count changes (Epic 02: 6 to 7; Epic 03: 7 to 6, coinciding with the ADR-028 grid change) are `[not measured]` here; recommend a follow-up investigation outside this plan (follow-up plan backlog), not in this ADR.
S5: Deliver (b) by an [ADD] implementation ticket in Epic 04 of this plan, as the master-plan rework trigger anticipates ("an implementation ticket is added at Epic 4 refinement when (b) is accepted"); that ticket also performs the ADR-025 rework of the fit_plant stage order, re-pins the four tests, records the CAMARGOS and FURNAS alpha as regression values and appends the objective to the evidence report. Ticket-029 documents the new stage order, the 30-point set (marked `[to confirm]` until the MM/Libs text is read) and the open `FP` field 7 question. Under (a), ticket-029 documents the divergence and nothing is added.

## Decision

Accepted: option (b) — the correction factor alpha is computed on separate evaluation points (MM
default count of 30) on the final reduced planes, after the gamma_S secant and the plane reduction.
Chosen by the user on 2026-11-19 at the decision gate, matching the recommendation.

Sub-question S1 (points): accepted as recommended — 30 points, a uniform 6 V x 5 Q grid over the fit
window and `[0, Q_max]` for plants with a volume axis, and 2 V x 15 Q for single-volume plants; the
distribution is a labdessem choice `[to confirm]` against the MM/Libs text.

Sub-question S2 (stage): accepted as recommended — alpha is computed on the final reduced planes, after
`fit_gamma_s_for_planes` and `reduce_within_plane_limit`, so alpha depends on `max_planes`; this
supersedes the ADR-029 statement that alpha is not recomputed after the reduction (the ADR-029 plane-cap
rule itself, iterative reduction with tolerance doubling, is unchanged); the deviation keeps describing
`alpha x planes`.

Sub-question S3 (secant target): accepted as recommended — keep the unscaled FPH target of the secant;
the FPH / alpha target suggested by the Libs form stays `[to confirm]` and, if confirmed, gets its own
follow-up item.

Sub-question S4 (reduction and CAMARGOS plane counts): accepted as recommended — record only: the
reduction keeps running on unscaled planes and the angle predicate is not scale-free; the cause of the
CAMARGOS plane-count changes (Epic 02: 6 to 7; Epic 03: 7 to 6) and the effect of reducing unscaled
planes are `[not measured]` and go to a follow-up investigation outside this plan.

Sub-question S5 (delivery): accepted as recommended — an [ADD] implementation ticket is to be added to
Epic 04 of this plan, before ticket-029, by the orchestrator; it performs the ADR-025 rework of the
fit_plant stage order and re-pins the four tests named in option (b). Ticket-029 documents the new stage
order, the 30-point set and the open `FP` field 7 question.

Amendment requests considered and not chosen: (b') alpha on the fit grid on the final reduced planes
(no uncited distribution, no envelope overshoot on single-volume plants) and (c') alpha = 1 by default
per MU `FP` field 7. The open points stay `[to confirm]`: the 30-point distribution, the `FP` field 7
default versus the Libs FPHA, the AVL_FPHA1 `Fcorrec` 1.00000 of the 032026 case, and the secant target;
the objective change is `[not measured]` until the [ADD] ticket runs. Rework trigger: if the MM/Libs text
defines a different 30-point distribution, the S1 point set is revised (fallback (b'), fit grid on the
final planes, if no distribution can be confirmed); if it confirms the FPH / alpha secant target, S3 is
reopened as a follow-up item; if it confirms that DESSEM does not fit alpha by default (`FP` field 7,
`Fcorrec` 1.00000), this ADR is reopened for (c').

## Consequences

Option (b) was chosen (see Decision); the other branch is kept for the record.

- Under (a): no code change; ticket-029 would have documented alpha on the fit grid before the reduction
  as a divergence from MM §9.16.11 step 4; ADR-025 and ADR-029 would have stood unchanged.
- Under (b): an implementation ticket is to be added to Epic 04 by the orchestrator ([ADD]), before
  ticket-029; it changes `fit_plant` and `compute_alpha_fpha`, re-pins the four tests named above and
  records the new alpha values; ADR-025's rework trigger is exercised; ADR-029's statement that alpha is
  not recomputed after the reduction is superseded by this ADR (the plane-cap rule stays); ticket-029
  documents the outcome.
- In all cases: the `[to confirm]` items (30-point distribution, reduction position in the Libs, the
  `FP` field 7 default versus the Libs FPHA, the AVL_FPHA1 `Fcorrec` values of the 032026 case, the
  secant target) need the MM/MU/Libs PDFs, a full-case AVL_FPHA1 or a CEPEL answer; the CAMARGOS
  plane-count question and the reduction on unscaled planes (`[not measured]`) go to a follow-up
  investigation outside this plan.
