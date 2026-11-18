# ADR-026: Policy for plants with generating units whose computed FPHA cannot be fitted

Decision brief for defect A2 (audit HI-03, also FP-01 and NU-04; FP-12 for the plants without
units). Requirements R3 and R10 of `plans/fpha-corrections/00-master-plan.md`. Option names follow
the ADR-026 entry of the master-plan frontmatter.

## Status

Proposed

## Context

**Defect.** In `FPHA = "computed"` mode a hydro plant with generating units whose hyperplanes cannot
be computed is skipped with a `Warning:` and reaches the solver with no FPHA segment. With
`fpha_enabled = true` the model builds neither its `hydro_fpha` rows nor its
generation-turbining coupling rows, and the constant-productivity row is not built either
(`crates/labdessem-model/src/constraints/constraint_set.rs:41-50` switches the whole system on
`system.fpha_enabled`). Only `0 <= g <= Pmax` and `0 <= q <= Qmax` remain, with no link between them.

Code path (line numbers on the current branch, commit 0bd9178):

1. `crates/labdessem-io/src/study.rs:146` builds the `System` with
   `fpha_enabled = config.fpha.is_precomputed()`, which is `false` in computed mode, and
   `crates/labdessem-io/src/study.rs:244` calls `system.validate()` at load time, before any FPHA
   exists. `System::validate` (`crates/labdessem-core/src/system/system.rs:38`, plant loop at line
   180) calls `HydroPlant::validate(horizon, self.fpha_enabled)`.
2. `HydroPlant::validate` (`crates/labdessem-core/src/entities/hydro.rs:267`) already holds the
   right rule at lines 311-315: with `fpha_enabled`, a plant with non-empty `groups` and empty
   `fpha_segments` is rejected with `"hydro plant {:?} must contain at least one FPHA segment"`. At
   load time `fpha_enabled` is `false`, so the rule is not reached; the branch evaluated instead is
   lines 317-322 (positive specific productivity when FPHA is disabled).
3. `compute_fpha` (`crates/labdessem-cli/src/fpha_integration.rs:62`, loop at lines 95-151) has
   three skip branches and one fatal branch:
   - the plant has no VHA geometry row: `Warning: Plant {} ({}) has no VHA geometry data, skipping`
     (lines 98-104, message at line 100);
   - `build_fit_params` fails with an error other than `MissingHydro`:
     `Warning: Plant {} ({}): failed to build fit params: {}` (lines 116-127);
   - `fit_plant` fails: `Warning: Plant {} ({}): fitting failed: {}` (lines 143-148);
   - `MissingHydro` is returned immediately (lines 118-121). It is raised in `build_fit_params`
     (lines 231-236) for any plant missing from `inflow_history.parquet` when that file is present,
     plants without units included (audit FP-12).
   - Before the loop, an empty geometry file returns `Ok` with every plant counted as skipped
     (lines 82-89, `Warning: No VHA geometry data found. No planes will be computed.` at line 83).
4. `crates/labdessem-cli/src/main.rs:131-144` calls `compute_fpha` (line 133),
   `inject_fpha_segments` (line 134) and sets `system.fpha_enabled = true` (line 135). Nothing
   validates the `System` again: `crates/labdessem-io/src/study.rs:244` is the only production
   `System::validate` call on this path, and no non-test call to `.validate(` exists under
   `crates/labdessem-cli/src` (one test hit,
   `crates/labdessem-cli/src/fpha_integration/plant_coverage_tests.rs`).

The precomputed path is asymmetric: `crates/labdessem-io/src/readers/hydro.rs:298-306` returns
`missing FPHA data for hydro plant {}` for a plant with unit rows and no `OPER_FPHA.csv` rows, and
accepts an empty segment list for a plant without unit rows.

**Reproduced numbers** (`docs/auditoria/fpha-baseline-reproduction.md`, commit 70cbbba, section 6;
tests of tickets 004 and 005, all commands exit 0):

| Case | `plants_processed` | `plants_skipped` | FPHA segments of the affected plants | `hydro_fpha` rows | Coupling rows |
|---|---|---|---|---|---|
| Geometry row of plant 2 (FUNIL-GRANDE) removed | 3 | 1 | 0 (plant 2) | 0 (plant 2) | 0 (plant 2) |
| `CAD_GEOMETRIA_UHE.csv` absent | 0 | 4 (all) | 0 (all) | 0 | 0 |
| Control, full geometry | 4 | 0 | 6 / 3 / 4 / 6 | 288 / 144 / 192 / 288 | 48 per unit |

- End to end (plant 2 geometry removed, `crates/labdessem-cli/tests/fpha_e2e.rs`): FUNIL-GRANDE has
  generation above 1 MW with turbining at most 1e-6 m3/s in 48 of 48 periods, maximum `GeracaoMW`
  180.000000; the other three plants have 0 such periods; the run exits 0 with
  `objective=2430458.1703`.
- The two skip cases print different warnings: the per-plant one (line 100) when one row is missing,
  only the global one (line 83) when the file is absent.
- `baseline_a2_plant_without_geometry_has_no_fpha_rows` asserts that `System::validate` with FPHA
  enabled rejects the skipped plant with a message containing `at least one FPHA segment`
  (`plant_coverage_tests.rs` lines 155-161). The full rendered text was not captured.

## Methodology reference

Sources: `~/.claude/skills/dessem/references/42-metodologia-hidreletricas.md` sections 15 (MM
22.3.0 §9.16.1-9.16.10, p. 58-71 / PDF 84-97; Libs §15.3.4, §15.6.1) and 16 (MM §9.16.11-9.16.12,
p. 71-76 / PDF 97-102; Libs §15.6.1), and its `Divergências e dúvidas` list.

Neither section states what DESSEM does when a plant lacks the data needed to build its FPHA (no
geometry, invalid parameters, a failed fit). No automatic per-plant fallback to another production
model is described in MM §9.16 or Libs §15.6.1 as summarised there; whether DESSEM aborts, warns or
substitutes a model in that situation is `[to confirm]` against the manuals or a DESSEM run with a
deliberately incomplete deck.

The production function is a property of every plant with units: MM §9.16.10 (eq. 9.30) defines one
FPH per plant once the limits implied by the connected units are known, and MM §9.16.11 step 1
builds the FPHA grid with turbining from 0 to the maximum turbined flow. The Libs §15.6.1 form
`GH_i <= alpha_i [gamma_0^k + gamma_V^k V_i + gamma_Q^k Q_i + gamma_S^k (Q_lat,i - Q_lat_hat,i)]` is
recorded in section 16 as a physical constraint, with a slack `f_FPHA` and an infeasibility
diagnostic. A plant with units and no production-function constraint therefore has no counterpart
in that formulation; this is consistent with failing rather than skipping, but it is an inference
from the formulation, not a stated DESSEM behaviour.

DESSEM does let the user choose the production model per plant: field 11 of the `UH` record
(0 = production function, the default; 1 = constant productivity), listed in the `Entradas` table of
section 15 (MM §9.16.7 for the constant specific productivity); the MU section of the `UH` record is
not given there `[to confirm]`. This is an input declared by the user, plant by plant, not a fallback
applied when the FPHA cannot be built. The global flag `FPHADESSEM` (MU §III.6.3.5) switches every
plant back to DESSEM's own FPHA calculation, which per Libs §15.6.1 (note 10) has no piecewise
tailrace polynomials, lateral flow or grids; it is a global switch between two FPHA builders, not a
per-plant fallback to constant productivity.

At the point level, section 15 records repairs, not skips: per LOG_ALTQUEDA (MU §V.11.3, cited in the
section 15 outputs), `HLIQ<0` uses a head of 0 and `HJUS<0` uses the tailrace level of the largest
"well-behaved" flow. These act on grid points of a plant that has data (MM §9.16.11 step 2); they do
not cover a plant without geometry.

For plants without units, MM §9.16.10 ties the FPH to the units connected to the plant, and the FPHA
grid upper bound of MM §9.16.11 step 1 is the plant's maximum turbined flow; with no units there is
no generation to bound. Whether DESSEM skips the FPHA of such plants (for example plants modelled
only as reservoirs) is not stated in sections 15-16 `[to confirm]`.

## Options

### (a)

Re-validate the System after FPHA injection only: call `System::validate` after
`crates/labdessem-cli/src/main.rs:135` (`fpha_enabled = true`), which reaches
`crates/labdessem-core/src/entities/hydro.rs:311-315` for every plant with units.

- Gains: smallest change (one call and an error mapping in `main.rs`); it reuses a rule that already
  exists and is already tested (`plant_coverage_tests.rs` lines 155-161); it is a structural guard:
  any future injection path that leaves a plant with units without segments fails, whatever the
  reason; it also runs `HydroFphaSegment::validate` (`hydro.rs:216-229`, segment index 0 and
  negative correction factor) on the injected segments, which load-time validation never sees in
  computed mode.
- Losses: the skip reason is lost (missing geometry, invalid parameters or fit failure); the
  `Warning:` lines still print before the error; `System::validate` returns on the first failing
  plant, so only one plant is named even when the whole geometry file is absent (4 plants in the
  section 6 case); the whole `System` is validated a second time.
- Risks: the plant is named by its `Debug` id, not by its name or input file, so the operator must
  map `HydroPlantId(2)` to FUNIL-GRANDE; a complete deck that today passes could fail if an injected
  segment violates `HydroFphaSegment::validate` (not observed in the control case of section 6, 4 of
  4 plants fitted).
- Operator message: the existing warning, then (expected rendering from the format string at
  `hydro.rs:313` and `#[derive(Debug)]` on `HydroPlantId`, not captured)
  `Error: hydro plant HydroPlantId(2) must contain at least one FPHA segment`; exit code non-zero.
- Tickets affected: ticket-015 proceeds as written; ticket-014 is dropped or reduced (no
  `PlantsNotFitted`, no typed reasons); ticket-016 loses the failure collection it reuses
  (`PlantsNotFitted`) and must be re-specified; ticket-017 A2 rows record the first error line of
  the re-validation instead of the typed error.

### (c)

`compute_fpha` returns a typed error listing every plant with units it could not fit.

- Gains: the operator sees every failing plant at once, with its code, name and reason (missing
  geometry, invalid parameters, fit failure); the three `Warning:` skip branches and the empty-file
  early return (lines 82-89) become one error path; symmetric with the precomputed reader, which
  already fails on a plant with units and no segments (`readers/hydro.rs:298-306`); ticket-016 can
  reuse the failure collection for A3.
- Losses: medium change (a reason enum, a failure struct, an error variant, three tests flipped in
  `plant_coverage_tests.rs`); no guard after injection.
- Risks: the guard lives inside one function; a later injection path, or a mismatch between
  `FphaPlantResult::codigo` and the plant id in `inject_fpha_segments`
  (`fpha_integration.rs:325-348`), would again leave a plant with units without segments and the run
  would still exit 0.
- Operator message: as specified by ticket-014 requirement 2, `computed FPHA could not be fitted for
  <n> hydro plant(s) with generating units:` followed by one line per plant, for example
  `  plant 2 (FUNIL-GRANDE): no rows in CAD/CAD_GEOMETRIA_UHE.csv`; exit code non-zero.
- Tickets affected: ticket-014 proceeds as written; ticket-015 is dropped; ticket-016 proceeds as
  written; ticket-017 A2 rows record the typed error.

### (a)+(c)

Typed error in `compute_fpha` plus post-injection re-validation.

- Gains: the reported failure carries every plant and its reason (from (c)), and the structural guard
  covers any other way of reaching the solver with a plant with units and no segments (from (a));
  `HydroFphaSegment::validate` runs on the injected segments; R3 is met by both the reported reason
  and the invariant.
- Losses: two checks to maintain; in the normal failure case only the typed error is seen, the
  re-validation fires only on an injection defect.
- Risks: as in (a), a complete deck could newly fail on `HydroFphaSegment::validate` (not observed in
  the section 6 control); the two messages must stay consistent in their naming of plants.
- Operator message: the (c) message for fitting failures; the (a) message, wrapped as
  `system validation failed after FPHA injection: <msg>` (ticket-015 requirement 1), only if an
  injection defect leaves a plant without segments; exit code non-zero in both.
- Tickets affected: ticket-014, ticket-015, ticket-016 and ticket-017 proceed as written (they assume
  this option).

### (b)

Per-plant constant-productivity fallback with a warning.

- Gains: the run completes; the skipped plant is linked to water through `g = rho * q`, so the free
  generation of section 6 (180 MW with no turbining in 48 of 48 periods) disappears; the
  specific-productivity data it needs is already required positive for plants with units at load
  time in computed mode (`hydro.rs:317-322`, reached because `fpha_enabled` is `false` at
  `study.rs:244`).
- Losses: high cost: the `ConstraintSet` switch at `constraint_set.rs:41-50` is global on
  `system.fpha_enabled` and must become per plant (FPHA and coupling rows for fitted plants,
  productivity rows for the others), and the EngMax, output and `resultado_fpha_computed.csv` paths
  must learn which model each plant used; the study mixes two production models without the user
  having declared it, unlike `UH` field 11 in DESSEM (methodology section above); the fallback plant
  inherits the per-unit gap of constant productivity (audit HI-04, out of scope here).
- Risks: a data error (the whole geometry file absent) turns into a run that completes with every
  plant on constant productivity, a different physics from the one requested, signalled only by
  warnings; the audit does not recommend it (HI-03, "o revisor não recomenda"); no DESSEM behaviour
  of this kind is found in sections 15-16 `[to confirm]`.
- Operator message: a warning per plant, for example `Warning: Plant 2 (FUNIL-GRANDE): computed FPHA
  not available (<reason>); using constant productivity` (text to be specified), then a normal run
  with exit code 0.
- Tickets affected: ticket-014 and ticket-015 rewritten (per-plant model selection in
  `labdessem-model`, a new constraint-set test matrix, output tagging of the model per plant); new
  work in `crates/labdessem-model/src/constraints/constraint_set.rs` and
  `crates/labdessem-model/src/constraints/hydro.rs`, outside the current Epic 02 scope; ticket-016
  re-specified (the A3 policy must say whether missing physical data also falls back); ticket-017 A2
  rows change from `exit code 1` to a completed run with a recorded objective.

### (d)

`study_config.json` policy key (`fail | fallback`).

- Gains: the user chooses per study between the strict behaviour (fail, as in (a)+(c)) and the
  fallback of (b); the default can be `fail`, so current decks are protected unless the user opts in.
- Losses: all the cost of (b) (per-plant `ConstraintSet` switch, output tagging) plus the cost of
  (a)+(c) for the `fail` branch; a new configuration key with its schema
  (`generate_study_config_schema`, `crates/labdessem-io/src/study.rs`), documentation and tests for
  both values; the key name is not fixed by the plan.
- Risks: keeps a fallback path alive that must be tested on every later FPHA change; the key becomes
  part of the `study_config.json` contract, which is costly to remove once studies use it; with
  `fallback` the risks of (b) apply.
- Operator message: with `fail`, the (a)+(c) messages; with `fallback`, the (b) warnings and a
  completed run; a configuration error if the key holds another value.
- Tickets affected: ticket-014 and ticket-015 proceed for the `fail` branch; new tickets for the key
  (config schema, reader, documentation) and for the per-plant model selection of (b); ticket-016
  re-specified as in (b); ticket-017 records both branches.

### Sub-question: plants without units

Are hydro plants with no generating units (empty `groups`) excluded from fitting, and so from the
VHA lookup and from the `MissingHydro` lookup of `build_fit_params`
(`fpha_integration.rs:231-236`)? Today such a plant follows the same path as any other: with no
geometry row it is counted as skipped with a warning (line 100); with geometry but absent from a
present `inflow_history.parquet` it stops the run with `MissingHydro` (audit FP-12); with both, a
fit is attempted with a maximum turbined flow of 0 (`fpha_integration.rs:208-218`), with an outcome
not measured in section 6.

- Gains: no run fails for a plant that needs no FPHA (FP-12); consistent with
  `HydroPlant::validate` (`hydro.rs:311` only requires segments when `groups` is non-empty) and with
  the precomputed reader, which accepts an empty segment list for a plant without unit rows
  (`readers/hydro.rs:298-300`); `plants_skipped` gets one meaning ("plants without generating units,
  no FPHA needed"), which makes (a), (c) and (a)+(c) unambiguous; consistent with MM §9.16.10, which
  ties the FPH to the connected units.
- Risks: a plant whose units were omitted by mistake from `CAD/CAD_CONJ_UHE.csv` passes silently (it
  then cannot generate, so the error is conservative, not free energy); `MissingHydro` no longer
  reports a parquet gap for such plants, so a gap in `inflow_history.parquet` that matters elsewhere
  would surface later or not at all `[to confirm]` whether the MLT is used outside the FPHA fit; FPHA
  rows supplied for such plants in precomputed mode stay out of scope (FP-07, ticket-023).

## Recommendation

(a)+(c), with the sub-question answered yes (plants without units are excluded from fitting and
from the `MissingHydro` lookup).

(c) gives the operator the full list of failing plants with their names and reasons in one run,
symmetric with the precomputed reader, and provides the failure collection that ticket-016 reuses for
A3. (a) adds the invariant at the point where it matters (after `fpha_enabled = true`), reusing the
existing rule of `hydro.rs:311-315`, so a future injection path cannot reintroduce free generation
without failing; it also validates the injected segments, which load-time validation never sees in
computed mode. (b) and (d) keep the run alive by changing the physics of a plant without the user
having declared it, which has no counterpart found in the DESSEM sources read (per-plant constant
productivity in DESSEM is a user input, `UH` field 11), cost a per-plant `ConstraintSet` switch, and,
for (d), add a configuration key that is costly to remove. The master plan classifies the decision
as two-way: (a)+(c) can later be extended with an opt-in fallback if a requirement appears, while
removing a published fallback key would not be cheap.

The sub-question answer yes follows the existing domain rule (`hydro.rs:311` and
`readers/hydro.rs:298-300` already treat plants without units as needing no segments) and removes the
FP-12 failure mode for plants that cannot generate; the open point is whether the MLT is needed for
those plants anywhere else `[to confirm]`.

## Decision

PENDING

## Consequences

- If (a) is chosen:
  - ticket-015: proceeds as written.
  - ticket-014: dropped or reduced to the plants-without-units exclusion (if the sub-question is
    answered yes); the skip `Warning:` lines stay.
  - ticket-016: re-specified without `PlantsNotFitted`.
  - ticket-017: A2 rows record the re-validation message.
- If (c) is chosen:
  - ticket-014 and ticket-016: proceed as written.
  - ticket-015: dropped.
  - ticket-017: A2 rows record the typed error.
- If (a)+(c) is chosen:
  - ticket-014, ticket-015, ticket-016, ticket-017: proceed as written.
- If (b) is chosen:
  - ticket-014 and ticket-015 rewritten around a per-plant production-model switch in
    `labdessem-model`; Epic 02 scope widened to `constraint_set.rs` and `constraints/hydro.rs`;
    ticket-016 and ticket-017 re-specified.
- If (d) is chosen:
  - as (a)+(c) for the `fail` branch, plus the (b) work for `fallback`, plus new tickets for the
    `study_config.json` key, schema and documentation.
- Sub-question:
  - yes: ticket-014 requirement 3 (exclusion before the VHA lookup and `build_fit_params`) and its
    test `plant_without_units_is_excluded_from_fitting` proceed; the `plants_skipped` doc comment
    changes; ticket-023 (Epic 03) aligns with this meaning.
  - no: ticket-014 requirement 3 and that test are removed; plants without units stay subject to the
    VHA lookup and to `MissingHydro`, and under (c) or (a)+(c) a plant without units that fails the
    fit would be reported as a failure only if the failure collection is extended to them (to be
    specified).
- In all cases: the `baseline_a2_*` characterization tests are flipped or replaced by the ticket that
  implements the chosen option; the master-plan ADR-026 entry is set to `accepted` by the
  orchestrator.
