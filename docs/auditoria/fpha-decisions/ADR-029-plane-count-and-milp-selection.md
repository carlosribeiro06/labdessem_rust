# ADR-029: Plane-count rule, reduction criterion and MILP plane selection for computed FPHA

Decision brief for finding A5 (the plane limit `max_planes` is configured but never enforced; the angle
test is the only reduction criterion used; DESSEM's MILP-only centroid selection of NFHAMILP planes is
not implemented). Requirements R6 and R7 of `plans/fpha-corrections/00-master-plan.md`. Option names
follow the ADR-029 entry of the master-plan frontmatter. Context only, not options here: ADR-025
(accepted) applies alpha once; ADR-026 and ADR-027 (accepted) make unfittable plants and missing
physical data typed errors; ADR-028 (accepted 2026-11-19) chose MM defaults, mapped `MaxPlanes` of
`CAD/CAD_FPHA_CONFIG.csv` to `max_planes` and fixed `ReductionTolerance` as the plane-angle tolerance
in degrees (default 1.0) in its S3, which is linked to S2 below.

## Status

Accepted (2026-11-19)

## Context

**Defect.** Code at HEAD `d90ff13` (branch `fix/fpha-corrections`); line numbers re-verified at that
commit (the ticket quoted `27a608b`).

1. `crates/labdessem-fpha/src/config.rs:85-96`: `to_fitting_bounds` copies `max_planes` into
   `FittingBounds::max_planes_per_hydro` (line 93). The field is declared in
   `crates/labdessem-fpha/src/fitting/geometry.rs:104-113` (line 111; `Default` sets 15 at line 123).
   `/usr/bin/grep -rn max_planes_per_hydro crates --include=*.rs` prints only the field, its `Default`,
   the copy at `config.rs:93` and eleven test fixtures (`deviation.rs:175`, `reduction.rs:222`,
   `alpha.rs:109`, `secant.rs:179`, `hull_fit.rs:147`, `:219`, `grid.rs:158`, `:176`, `:194`, `:214`,
   `tests/regression.rs:59`). No production code reads it.
2. Default limits: `DEFAULT_MAX_PLANES = 10` (`config.rs:15`, added by ticket-019) and the CLI's
   `FphaPlantConfig::computed(10, 6, 6, Angle { tolerance_deg: 1.0 })`
   (`crates/labdessem-cli/src/fpha_integration.rs:377-391`, only `num_planes` copied, line 385;
   ticket-031 rewires it to the ADR-028 defaults).
3. `crates/labdessem-fpha/src/pipeline.rs` `fit_plant` (47-92), stage order: `fit_context` (48; body
   100-156), hull (51-60), alpha on the raw hull planes (63), gamma_S secant (67), `reduce_planes`
   (70-77), deviation of `scale_planes(planes, alpha)` (80), `validate_fitted_planes` (83). **Alpha is
   computed before the reduction and the deviation after it**: any limit enforced at stage 4 changes
   the planes and the reported deviation but not alpha.
4. `crates/labdessem-fpha/src/fitting/reduction.rs`: `PlaneReductionConfig { Angle { tolerance_deg },
   Distance { tolerance_pct, n_samples } }` (13-16), default `Angle 1.0` (18-22). The angle predicate
   (84-88) compares normals `[gamma_v, gamma_q, -1]` (`plane_normal`, 24-26); the normals are taken on
   the unscaled planes, and scaling both planes by alpha changes the angle (the `-1` component is not
   scaled), so the predicate is **not alpha-invariant**. `reduce_planes_with` (52-82) is a greedy tail
   merge in hull output order: a candidate is averaged into the last kept plane (`mean_plane`, 38-45)
   when the predicate holds; the origin plane is never merged (`is_origin_plane`, 48-50). `Distance`
   is seeded per pair by `pair_seed` (103-113); `reduce_planes` dispatches at 168-184. The CLI always
   builds `Angle`.
5. `crates/labdessem-model/src/model.rs:9-13`: `SolveMode::{LinearProgramming,
   MixedIntegerLinearProgramming, LinearProgrammingWithFixedCommitment}`; `Model::from_system` (27)
   takes the mode. `crates/labdessem-model/src/constraints/constraint_set.rs:41-42` adds
   `build_hydro_fpha_constraints` (`constraints/hydro.rs:198`, one row per plant, period and
   `plant.fpha_segments` entry) whenever `system.fpha_enabled`, independently of the solve mode: **every
   mode builds the same FPHA rows today**. The plane set is per plant, not per period.
6. Stage sequencing (`crates/labdessem-simulation/src/stages/orchestration.rs`; models built by
   `stage_model`, `stages/solve.rs:66-74`): `opcao_execucao` 1 (`ExecutionOption::Iterative`,
   `crates/labdessem-simulation/src/workflow.rs:29`, `:38`) runs LP (network cut loop at 95, or 372
   without network), MILP (109 / 384) and LP with fixed commitment (126 / 396);
   `opcao_execucao` 2 (`SingleMilp`, `workflow.rs:31`, `:39`; `run_single_milp_simulation`, 248) runs
   the MILP first, with **no LP before it**, then the fixed-commitment LP. The official example's
   `study_config.json` uses `opcao_execucao` 2.

**Post-fix plane counts** (`docs/auditoria/fpha-baseline-reproduction.md`, section 8, commit `d6a223d`,
fast configuration of section 3; unchanged at HEAD, see the timing below): CAMARGOS 7 (alpha 0.969607),
ITUTINGA 3 (1.000000), FUNIL-GRANDE 4 (1.000000), FURNAS 6 (0.991295); 20 planes, 960 `hydro_fpha` rows
over 48 periods. The default limit 10 binds no plant. A limit of 5 (the NFHAMILP default) binds CAMARGOS
and FURNAS only (3 planes, 144 rows, 15 % of the FPHA rows). CAMARGOS and FURNAS are the two `RV`
plants of `CAD/CAD_UHE.csv`; ITUTINGA and FUNIL-GRANDE are `FD` (ADR-028 table).

**MILP timing** (one run each, release build of HEAD `d90ff13`, `cargo build --release -p labdessem-cli`;
derived copies under the session scratchpad, outside the repository; `examples/24Barras` not edited
and not executed). Configuration: the fast configuration of section 3 with `"UCT": 1`, `"EngMax": 0`,
`"FPHA": "computed"`. The official example ships `OPER/OPER_FPHA.csv` with the header only, so the
"official planes" are the computed post-fix set (the run prints 7 / 3 / 4 / 6 planes and the alphas
above).

```bash
d=$(mktemp -d); cp -r examples/24Barras/CAD examples/24Barras/OPER "$d"/
# study_config.json: {"opcao_execucao": 1|2, "rede": 0, "UCT": 1, "UCH": 0, "TON_Residual": 0,
#                     "FPHA": "computed", "FCF": 1, "EngMax": 0}
target/release/labdessem run "$d" > run.stdout 2> run.stderr
```

| `opcao_execucao` | Stages (stderr) | MILP time (s) | Wall time (s) | Objective (R$) |
|---|---|---|---|---|
| 2 (MILP unico) | MILP 4.479; LP-CALC-CMO 0.200 | 4.479 | 4.71 | 4499200.0374 |
| 1 (LP -> MILP -> LP-FIXED -> LP-CALC-CMO, no network) | LP 0.107; MILP 4.654; LP-CALC-CMO 0.269 | 4.654 | 5.07 | 4499200.0374 |

- The LP of option 1 prints 2383481.5598 R$, the computed objective of section 8 (UCT off), so the plane
  set is the section 8 one. The MILP is 91.8 % of the wall time of option 1 (4.654 of 5.07 s) and 95.1 % of
  option 2 (4.479 of 4.71 s) on this case.
- The effect of removing 144 of 960 FPHA rows from the MILP (NFHAMILP 5) on MILP time is
  `[not measured]`; so is the effect of any limit on the objective and on the deviation.

## Methodology reference

Sources: `~/.claude/skills/dessem/references/47-metodologia-solucao-solver-cmo.md` line 30 (table 1.1,
MM 22.3.0 §27.2, p. 221-227 / PDF 247-253; same text in MM 22.4.1 cap. 27) and lines 34-38 (MU 22.3.2
§III.6.3.1 `REGRANPTV`); `~/.claude/skills/dessem/references/42-metodologia-hidreletricas.md` section 16
(MM 22.3.0 §9.16.11-9.16.12, p. 71-76 / PDF 97-102; Libs §15.6.1); `23-saidas-diagnostico-b.md` lines
105-113 and 542-545 (MU §V.11.4 AVL_FPHA1; Libs §15.6); `12-entradas-hidraulicas-b.md` divergence 9 (Libs
§15.7). The manual PDFs are not on this machine and no ONS deck is available here, so the claims come
from those skill references; no deck evidence is quoted.

MILP selection (`47-metodologia-solucao-solver-cmo.md` line 30, MM §27.2): the selection of FPHA cuts
applies **only to plants with a reservoir and only to the MILP**: (1) take `V*(PL)` from the last LP
before the MILP; (2) compute the centroid `Vc_k` of each FPHA cut `k`; (3) per plant and period keep the
`NFHAMILP` cuts whose centroid is closest to `V*(PL)` (default 5). In the final LP all cuts return. The
selection uses volume because it is the variable that changes least between solves (UCT moves the
turbined flow). How `Vc_k` is computed (grid points where cut `k` is active, or the facet geometry) is
not stated in the reference `[to confirm]`.

Stages (`47-metodologia-solucao-solver-cmo.md` table 2, MM §27.4, Tab. 27.2): DESSEM distinguishes
`PL`, `PL UCT[0,1]`, `MILP`, `PL.Int.Fix` (binaries fixed at the MILP solution) and `PL.CalcCMO` (final
LP, duals for the CMO). The reference says the cuts return "no PL final"; whether `PL.Int.Fix` already
has the full set is `[to confirm]`. labdessem uses `LinearProgrammingWithFixedCommitment` for both its
LP-FIXED and LP-CALC-CMO stages, so the distinction does not change the labdessem mapping.

Configuration (`47-metodologia-solucao-solver-cmo.md` lines 34-38 and `42-metodologia-hidreletricas.md`
section 16 "Entradas", MU 22.3.2 §III.6.3.1): `REGRANPTV` field 3 (columns 15-16) switches the MILP
point selection on (1, default) or off (0 or blank); field 4 (columns 19-20) is the number of points,
default 5. The 2026-06-15 ONS deck (DESSEM 22.0.2) has `REGRANPTV     1` and LOG_RELATO echoes
"Acionado a selecao de pontos para a FPHA" and "Utilizado  5 pontos para o volume". That the LOG's
"5 pontos para o volume" is NFHAMILP of §27.2 is the skill's reading `[to confirm]`.

No count cap in the fit (`42-metodologia-hidreletricas.md` section 16, MM §9.16.11-9.16.12; `FP` record,
MU 22.3.2 §III.5.4.5): the number of FPHA planes follows from the NPTV x NPTQ grid and the convex hull;
the `FP` record has no maximum-plane field and no such field appears in the sources read. In DESSEM the
only count of 5 is the MILP selection; a fit-time plane limit is a labdessem extension
(`CAD_FPHA_CONFIG.csv` `MaxPlanes`, ADR-028), not a DESSEM methodology item. AVL_FPHA1 of the 032026
full case lists 1 128 cuts (`23-saidas-diagnostico-b.md` line 108); the per-plant distribution was not
counted here `[to confirm]`.

Reduction criteria (`42-metodologia-hidreletricas.md` section 16 and `23-saidas-diagnostico-b.md` lines
542-545, Libs 11.0.0 §15.6.1 / §15.6): similar hyperplanes are merged either by angle
(`HIDRELETRICA-FPHA-METODO-REDUCAO-CORTES-ANGULO-PADRAO`, tolerance in degrees between normals) or by
distance (`...-DISTANCIA-PADRAO`, mean squared error of the projections over gh^2 max, in %, example
0.005); one method at a time; the plane through the origin is never replaced; it reduces constraints at
the cost of accuracy (AVL_FPHA1 and AVL_DESVFPHA change). The cards are not in the MM. Whether DESSEM
accepts the angle card is `[to confirm]` (`12-entradas-hidraulicas-b.md` divergence 9: the per-model
availability column of Libs §15.7 was lost in extraction). The sources give no default angle and no rule
for reaching a target count by changing the tolerance `[to confirm]`.

Contradiction check (`47-metodologia-solucao-solver-cmo.md` line 30, MM §27.2): the methodology agrees
with the audit's statement of A5 (NFHAMILP applies only to the MILP stage; the full set returns in the
final LP). It adds two restrictions the finding does not
state: the selection applies only to plants with a reservoir, and it needs `V*` from an LP solved before
the MILP, which labdessem's `opcao_execucao` 2 does not have (Context item 6). Neither contradicts the
audit.

## Options

### (a)

(a) enforce max_planes by iterative reduction and keep every plane in every solve stage

- Gains: `MaxPlanes` of `CAD_FPHA_CONFIG.csv` (ADR-028) takes effect as a deterministic user cap; with the
  default 10 no 24Barras plant binds, so the default run stays bit-identical (7 / 3 / 4 / 6 planes,
  objective 2383481.5598 R$); one change in one function of `labdessem-fpha`, no model or simulation
  change; every stage keeps the same envelope, so LP, MILP and CMO duals see one FPHA.
- Losses: a binding cap degrades the envelope in every stage, LP and CMO included, which DESSEM never does
  (its only count limit is MILP-only, MM §27.2); no MILP speed-up beyond what the cap removes; the cap
  remains a labdessem extension without an MM counterpart.
- Risks: alpha is computed before the reduction (`pipeline.rs:63` vs `70-77`), so a binding cap leaves
  alpha fitted to the unreduced hull while the reported deviation (line 80) grows `[not measured]`; the
  averaged planes (`mean_plane`) are not hull planes and may lie below or above the FPH; the greedy tail
  merge depends on hull output order; tolerance doubling can end above 180 degrees with planes left (the
  origin plane is never merged), which needs the typed error of S1; the doubled angle is not
  alpha-invariant.
- Fitted planes: unchanged at the default limit 10; with `MaxPlanes` 5, CAMARGOS at most 5 (from 7) and
  FURNAS at most 5 (from 6), the others unchanged; alpha unchanged bit for bit; deviation changes for the
  capped plants `[not measured]`.
- Solve stages: no change; LP, MILP and LP with fixed commitment build the same rows (`constraint_set.rs:41-42`).
- Tickets affected: 021 implements it (as written); 022 proposed for [REMOVE]; 031 unchanged
  (`config_plane_limit_caps_its_plant` relies on it); 029 documents the cap as a labdessem extension and
  the absence of MILP selection.

### (b)

(b) option (a) plus centroid selection of NFHAMILP planes for the MILP stage (MM 27.2)

- Gains: everything of (a), plus the MILP stage follows MM §27.2 (fewer FPHA rows in the MILP, full set
  in the final LP, so CMO duals keep the full envelope); the closest DESSEM match.
- Losses: a second plane-count concept (fit cap and MILP subset) to configure and document; on 24Barras
  only 144 of 960 FPHA rows leave the MILP, against a MILP of 4.5 s whose sensitivity to these rows is
  `[not measured]`.
- Risks: the subset is per plant **and per period** (MM §27.2), but `fpha_segments` is per plant and
  `build_hydro_fpha_constraints` (`hydro.rs:198`) iterates it for every period, so `Model::from_system`
  needs a per-stage, per-period plane mask threaded from `labdessem-simulation` (three `SolveMode`s; the
  stage paths of `orchestration.rs` reach one production model build, `stage_model` in
  `stages/solve.rs:66-74`, which must receive the LP solution) into `labdessem-model`; `V*` exists only
  when an LP precedes the MILP (`opcao_execucao` 2, the example default, has none); the centroid
  definition is `[to confirm]`; the reservoir test needs the `RV` / `FD` `Tipo` that is read and
  discarded today (ADR-028 Context); a MILP solution with the subset can violate the full set, which the
  fixed-commitment LP then absorbs `[not measured]`; high complexity (master plan).
- Fitted planes: as (a); the fit itself is unchanged by the selection.
- Solve stages: MILP gets at most NFHAMILP planes per reservoir plant and period (CAMARGOS and FURNAS
  on 24Barras); LP and LP with fixed commitment keep the full set.
- Tickets affected: 021 as written; 022 refined and executed (model + simulation, new configuration key);
  031 unchanged; 029 documents both the cap and the selection.

### (c)

(c) remove max_planes and num_planes; document the current angle reduction

- Gains: no unused knob; the plane count is governed only by the grid, the hull and the angle tolerance,
  which matches DESSEM's fit (no count cap in MM §9.16.11 or the `FP` record); least code.
- Losses: no direct control of the plane count (only indirectly through `ReductionTolerance`); no MILP
  selection; the `MaxPlanes` column accepted by ADR-028 disappears.
- Risks: reopens ADR-028 (its mapping `num_planes -> max_planes`) and invalidates ticket-031 Requirement 4
  `config_plane_limit_caps_its_plant` and the reader rejection of `num_planes = 0`; existing
  `CAD_FPHA_CONFIG.csv` files with a `MaxPlanes` column need a reader decision (reject or ignore);
  `max_planes_per_hydro` and `DEFAULT_MAX_PLANES` (ticket-019) become dead code to remove.
- Fitted planes: unchanged (7 / 3 / 4 / 6); alpha and deviation unchanged.
- Solve stages: no change.
- Tickets affected: 021 replaced by a removal ticket; 022 proposed for [REMOVE]; 031 loses the
  `num_planes` mapping and its test; 029 documents the angle reduction and the absence of a cap.

## Recommendation

(a). It makes the accepted ADR-028 mapping of `MaxPlanes` real at the lowest risk, is bit-identical at
the default limit, and keeps a single envelope in every stage. The measured case does not justify (b)
now: the MILP takes 4.5 s and only 15 % of its FPHA rows would leave it, so the master plan's condition
for (b) ("only if the ticket-020 brief shows MILP runtime evidence") is not met; its rework trigger
(MILP solve time binding on larger decks) stays open. The brief records that the cap is a labdessem
extension: DESSEM limits the count only in the MILP (MM §27.2), so ticket-029 must not present the cap
as DESSEM methodology.

S1: Meet the limit by doubling the configured tolerance (`tolerance_deg` or `tolerance_pct`) and
re-reducing the ORIGINAL plane set (never the previous result), at most 32 doublings, then return the
typed error `PlaneLimitUnreachable` (also for a limit of 0); a first pass within the limit returns the
current output bit for bit, and alpha is not recomputed (it stays fitted to the hull, as today).
Alternative: rank planes by contribution (drop, one at a time, the plane whose removal increases the
grid deviation of `alpha x planes` least) — more accurate for a given count and order-independent, but
O(n^2) grid evaluations and no source in the MM or Libs `[to confirm]`; not recommended.

S2: Keep the angle criterion (Libs §15.6.1 `...-ANGULO-PADRAO`), consistent with ADR-028 S3
(`ReductionTolerance` = angle in degrees, default 1.0); `Distance` stays available in the library but is
not wired. Choosing distance instead would reopen ADR-028 S3 (the column would change meaning to % of
gh^2 max). The angle predicate is not alpha-invariant (normals of the unscaled planes); that is recorded,
not changed.

S3: For (b) only: the subset applies to `SolveMode::MixedIntegerLinearProgramming` only;
`LinearProgramming` and `LinearProgrammingWithFixedCommitment` keep the full set; only reservoir plants
(`Tipo` `RV`) are selected. NFHAMILP is configured in `study_config.json` as the analog of `REGRANPTV`
fields 3-4 (switch, default on; count, default 5), not per plant in `CAD_FPHA_CONFIG.csv`. `V*` is the
per-period volume of the last LP before the MILP; under `opcao_execucao` 2 (no LP) either an extra LP is
solved or the selection is off (to be decided in ticket-022). Precomputed decks (`OPER/OPER_FPHA.csv`)
declare no subset and get no new column: the model computes each plane's centroid from the plane set and
the plant's registry volume range, so computed and precomputed planes are selected by the same rule.

## Decision

Accepted: option (a) — `max_planes` (`MaxPlanes` of `CAD_FPHA_CONFIG.csv`, ADR-028) is enforced in the
fit by iterative reduction, and every fitted plane is kept in every solve stage (LP, MILP, LP with
fixed commitment and LP-CALC-CMO build the same FPHA rows). No MILP plane selection is implemented; the
cap is a labdessem extension, since DESSEM limits the plane count only in the MILP (MM §27.2). At the
default limit 10 no 24Barras plant binds and the run stays bit-identical; a binding cap changes the
fitted planes and the deviation but not alpha (computed before the reduction).

Sub-question S1 (how the limit is met): accepted as recommended — double the configured angle tolerance
and re-reduce the ORIGINAL plane set (never the previous result), at most 32 doublings, then return the
typed error `PlaneLimitUnreachable` (also for a limit of 0); a first pass within the limit returns the
current output bit for bit; alpha is not recomputed; the deviation changes for capped plants.

Sub-question S2 (reduction criterion): accepted as recommended — keep the angle criterion (Libs §15.6.1),
consistent with ADR-028 S3 (`ReductionTolerance` = angle in degrees, default 1.0); `Distance` stays in the
library, not wired; choosing distance would reopen ADR-028 S3. The angle predicate is not
alpha-invariant; recorded, not changed.

Sub-question S3 (MILP selection scope): accepted as recommended, but moot under (a) — it would apply only
if (b) were chosen later: subset in `SolveMode::MixedIntegerLinearProgramming` only, reservoir plants
(`Tipo` `RV`) only, NFHAMILP configured in `study_config.json`, `OPER/OPER_FPHA.csv` does not declare it.

Chosen by the user on 2026-11-19 at the decision gate, matching the recommendation. Ticket-021 proceeds
as written; ticket-022 (MILP plane selection, which existed only under (b)) is removed from the plan;
tickets 031 and 032 are unchanged; ADR-028 stays accepted. The open points stay `[to confirm]`: the
centroid definition of MM §27.2, the plane set of `PL.Int.Fix`, NFHAMILP vs the LOG's "pontos para o
volume", the availability of the angle card in DESSEM (Libs §15.7), the per-plant cut distribution of
the 032026 AVL_FPHA1, and the effect of a binding cap on deviation, objective and MILP time
(`[not measured]`, recorded by ticket-032 after ticket-021). The rework trigger of (b) (MILP solve time
binding on larger decks) stays open.

## Consequences

Option (a) was chosen (see Decision); the other branches are kept for the record.

- Under (a): ticket-021 runs as written; ticket-022 was removed from the plan (its file stays on disk as
  the record of option (b)); ticket-029
  documents the cap as a labdessem extension and the MILP selection as not implemented (MM §27.2);
  ticket-032 records, after ticket-021, the deviation and objective of a binding cap (`[not measured]`
  here).
- Under (b): ticket-022 is refined with the S3 answers and a sized change across `labdessem-model` and
  `labdessem-simulation`; the timing above becomes its baseline.
- Under (c): ADR-028 is reopened for `MaxPlanes`; ticket-021 and ticket-031 are rewritten.
- Under any option: if S2 changes to distance, ADR-028 S3 is reopened. The `[to confirm]` items (centroid
  definition, `PL.Int.Fix` plane set, NFHAMILP vs the LOG's "pontos para o volume", angle card
  availability in DESSEM) need the MM/MU PDFs or a CEPEL answer before any MILP plane selection is
  specified (ticket-022 would be added back under (b)).
