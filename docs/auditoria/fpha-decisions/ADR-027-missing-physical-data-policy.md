# ADR-027: Policy for missing tailrace, efficiency or hydraulic-loss data in computed FPHA

Decision brief for defect A3 (audit FP-R02 for efficiency and loss; the missing tailrace is new).
Requirements R4 and R10 of `plans/fpha-corrections/00-master-plan.md`. Option names follow the
ADR-027 entry of the master-plan frontmatter. Context only, not options here: ADR-025 (accepted)
applies alpha once; ADR-026 (accepted) excludes plants without generating units from fitting and
makes `compute_fpha` return a typed error for every plant with units it cannot fit.

## Status

Proposed

## Context

**Defect.** In `FPHA = "computed"` mode, three physical inputs of the production function are
replaced by synthetic values when they are absent, without any message:

1. `crates/labdessem-fpha/src/fitting/production.rs:47`: `hydraulic_losses_m.unwrap_or(0.0)` (missing
   `Perda.m` becomes a 0 m loss).
2. `crates/labdessem-fpha/src/fitting/production.rs:48`: `efficiency.unwrap_or(1.0)` (missing
   `Eficiencia` becomes an efficiency of 1.0).
3. `crates/labdessem-fpha/src/fitting/production.rs:67`: `TailraceSource::Entity(None) => 0.0` (a plant
   without a `CAD/CAD_TAILRACE.csv` row gets a 0 m tailrace level, so the gross head becomes the
   forebay elevation above sea level).
4. `crates/labdessem-io/src/readers/hydro.rs:453-456`: `CAD/CAD_UHE.csv` reads `Perda.m` and
   `Eficiencia` as `Option<f64>` with serde `default`, so an absent column and an empty cell both
   become `None`; an explicit `0` cell becomes `Some(0.0)`.
5. `build_fit_params` (`crates/labdessem-cli/src/fpha_integration.rs:204` and `:245-247`) passes the
   optional tailrace model, `plant.hydraulic_loss_m` and `plant.efficiency` straight to the fit, so
   the defaults of `ProductionFunction::new` apply.

Adjacent, not decided here: `production.rs:68` (`TailraceSource::Families => 0.0`, a placeholder that
no caller constructs in the current tree; tailrace families belong to ticket-027) and the EngMax
geometry builder (`crates/labdessem-simulation/src/engmax.rs:96-137`), which returns no geometry for a
plant without a tailrace polynomial (EngMax then falls back to `h_ef`) but passes
`plant.hydraulic_loss_m` and `plant.efficiency` to the same `ProductionFunction::new` defaults.

**Reproduced numbers** (`docs/auditoria/fpha-baseline-reproduction.md`, section 7, tests of
ticket-006 in `crates/labdessem-cli/src/fpha_integration/physical_data_tests.rs`; fast configuration
of section 3). FPH ratio = derived FPH / control FPH, uncapped, at `s = 0`; LP-envelope ratio =
derived / control envelope at `s = 0`. The two ratios are informational and are not comparable with
each other (section 7).

| Case | Code | Plant | FPH ratio, mean | FPH ratio, max | LP-envelope ratio, mean |
|---|---|---|---|---|---|
| Tailrace row removed | 3 | FURNAS | 8.754581 | 9.842334 | 2.319804 |
| `Perda.m` and `Eficiencia` columns dropped | 0 | CAMARGOS | 1.124287 | 1.127187 | 1.123132 |
| same | 1 | ITUTINGA | 1.159956 | 1.160583 | 1.122542 |
| same | 2 | FUNIL-GRANDE | 1.099868 | 1.100352 | 1.079797 |
| same | 3 | FURNAS | 1.100635 | 1.102037 | 1.093203 |

- Plane counts and alpha also move: missing tailrace, FURNAS 2 planes and alpha 1.0000 (control 6,
  0.9913); missing efficiency and loss, CAMARGOS 6 / 0.9747, ITUTINGA 5 / 1.0000, FUNIL-GRANDE
  4 / 1.0000, FURNAS 9 / 0.9940 (control 6 / 0.9696, 3, 4, 6 / 0.9913).
- Neither case prints a warning; `computed_system` returns `Ok` and `plants_skipped = 0`.
- The official `examples/24Barras/CAD/CAD_UHE.csv` carries both columns for all four plants
  (`Eficiencia` 0.8937 / 0.8817 / 0.9185 / 0.9170, `Perda.m` 0.095 / 0.631 / 0.394 / 0.803 m), so the
  official example is unaffected by any option below.
- Audit comparison (section 7a): the audit's "about 9 %" for FURNAS (FP-R02, audit section 7, line
  937) matches in order of magnitude (+9.05 % from efficiency alone, +10.06 % mean FPH ratio). Audit
  section 13 lists decisions D1-D13 and has no entry for FP-R02; the audit's remark is "should at least
  warn" ("Deveria ao menos avisar").

## Methodology reference

Sources: `~/.claude/skills/dessem/references/42-metodologia-hidreletricas.md` sections 1 and 15
(MM 22.3.0 §9.1, §9.16.1-9.16.10; Libs §15.3.2, §15.3.4-15.3.5, §15.6.1), with the `hidr.dat` field
table of `12-entradas-hidraulicas-a.md` §3 (MU 22.3.2 §III.11) and the `AC` table of
`11-entdados-registros-a2.md` (MU 22.3.2 §III.5.4.7). MM section numbers are those of 22.3.0.

Registry data (MM §9.1, Libs §15.3.2): the DESSEM registry gives, per plant, the forebay polynomial,
the tailrace rating polynomial (`a_cf,0..4`), the specific productivity `rho_esp`
(MW/((m3/s)·m)) and the hydraulic losses, as `hperdas` in m or `kperdas` in p.u. All three inputs of
this ADR are therefore registry data of every hydro plant, not optional tuning.

Exact production function (MM §9.16.10, eq. 9.30): `FPH_i(V,Q,S) = rho_i Q [h_mon(V) - h_jus(Q_jus) -
hperdas_i]`, with the p.u. form 9.31. The tailrace level comes from the rating polynomial (MM §9.16.3,
eq. 9.23, piecewise form 9.24; Libs §15.3.4), with backwater families (MM §9.16.4) and lateral flows
(MM §9.16.6). Without a tailrace polynomial there is no `h_jus` term in DESSEM's FPH; no zero-level
substitute is described in the sources read `[to confirm]`.

Efficiency (MM §9.16.7, eq. 9.20 against 9.30): DESSEM's unit equation uses
`9.81e-3 * eta_t * eta_g * q * h_liq`, and the plant FPH uses `rho_esp`, constant from `hidr.dat` or
variable on a head x flow grid (Libs card `HIDRELETRICA-PRODUTIBILIDADE-ESPECIFICA-GRADE`, Libs
§15.3.4). labdessem's `Eficiencia` plays the role of `rho_esp / 9.81e-3`: the `hidr.dat` record of
FURNAS (deck 2026-06-15, DESSEM 22.0.2) has `rho_esp` 0.008996 and losses 0.803, and
0.008996 / 0.00981 = 0.9170 matches the 24Barras `Eficiencia` 0.91698 and `Perda.m` 0.803 of FURNAS.
That the 24Barras values were derived from `hidr.dat` this way is an inference `[to confirm]`.

Hydraulic loss (MM §9.16.8-9.16.9; Libs §15.3.5): three forms, constant in m (eq. 9.27), p.u. factor
of the gross head (eq. 9.28) and a loss x flow grid (eq. 9.29). labdessem models only the constant-m
form; `hidr.dat` field 39 (`tipo_perda`) selects m or %, but the sources do not give the meaning of
its numeric codes (FURNAS has code 2) `[to confirm]`.

Does DESSEM default any of these inputs? Not in the sources read. `hidr.dat` is a fixed-length binary
record (792 B per plant, MU 22.3.2 §III.11), so fields 24 (`rho_esp`), 25 (losses) and 26-28 (number of
tailrace curves, coefficients, reference levels) always exist and DESSEM reads them; "missing" is not
representable there, only a numeric value such as 0 `[to confirm]` how DESSEM treats `rho_esp = 0` or
zero tailrace curves. The `AC PERHID` and `AC PROESP` overrides accept values `>= 0` (MU 22.3.2
§III.5.4.7), so an explicit zero loss is an admissible DESSEM value. Libs `CURVAJUSANTE` cards, when
activated through `indice.csv`, prevail over the registry tailrace (Libs §15.3.4), again a replacement
by data, not by a default.

Run-time fallbacks that do exist concern bad evaluations, not missing data: LOG_ALTQUEDA (MU 22.3.2
§V.11.3) reports `HLIQ<0` (head set to 0) and `HJUS<0` (level of the largest "well-behaved" flow used),
and AVL_ALTQUEDA lists the problems; whether these are applied silently or with a message beyond
these logs is `[to confirm]`. labdessem's `net_head` clamps a negative net head to 0
(`production.rs:60`), the counterpart of the first case.

## Options

### (a)

(a) fail-fast typed error naming the plant and every missing input

- Gains: the FPH is fitted only from the physical data the methodology requires (MM §9.1, §9.16.10),
  so no synthetic generation enters the LP; every missing input of a plant is reported at once (code,
  name, and which of tailrace row, `Eficiencia`, `Perda.m`); reuses the failure collection of
  ticket-014 (ADR-026), so all failing plants appear in one error; scoped to `build_fit_params`, so
  EngMax and the precomputed path are untouched; low complexity.
- Losses: a case that runs today in computed mode without a tailrace row or without the
  `Eficiencia`/`Perda.m` columns stops with an error until the data is supplied (behaviour change for
  those cases; the official 24Barras example is not affected); `ProductionFunction::new` keeps its
  defaults for the other callers, so the policy lives in the caller, not in the type.
- Risks: an empty cell is indistinguishable from an absent column after reading (both `None`), so a
  user who left the cell empty meaning "zero loss" gets an error and must write `0`; a case with
  efficiency or loss genuinely unknown has no in-model fallback and must take a value from another
  source.
- Operator message: one `PlantsNotFitted` error listing, per plant, `missing inputs: row in
  CAD/CAD_TAILRACE.csv; Eficiencia in CAD/CAD_UHE.csv (column absent or empty); Perda.m in
  CAD/CAD_UHE.csv (column absent or empty)` (only the missing ones); run exit non-zero.
- Tickets affected: ticket-016 proceeds as written (it assumes (a), explicit `Perda.m = 0` valid,
  EngMax out of scope); ticket-017 records the new failing cases in the post-fix evidence; ticket-029
  and ticket-030 document the required inputs; `CHANGELOG.md` gets a behaviour-change entry
  (ticket-016).

### (b)

(b) keep the defaults and emit a warning

- Gains: no case that runs today stops; smallest change (one warning per plant and input in
  `build_fit_params`); answers the audit's minimum ("should at least warn").
- Losses: the generation stays physically inflated (efficiency and loss: +10 % to +16 % mean FPH
  ratio in the four plants; missing tailrace: FURNAS mean FPH ratio 8.75 and LP-envelope ratio 2.32,
  section 7); the cost and the dispatch of the run remain those of a non-physical plant; R4 is met
  only in its "explicit outcome" reading, not in substance.
- Risks: warnings are easy to miss in batch runs and in the matrix runs; results of such runs can be
  compared with physical runs without the reader knowing; a 0 m tailrace has no DESSEM counterpart in
  the sources read (`[to confirm]` above), so the warned result is not a DESSEM-like approximation.
- Operator message: one `Warning` per plant and missing input, for example `FURNAS: no row in
  CAD/CAD_TAILRACE.csv, tailrace level 0 m assumed`; run exit 0.
- Tickets affected: ticket-016 rewritten (warnings instead of `MissingInputs`; the `baseline_a3_*`
  tests become "defaults applied and warned" tests); ticket-017 unchanged in scope; ticket-029/030
  document the defaults; `CHANGELOG.md` `### Changed` (new warnings).

### (c)

(c) explicit configurable defaults per plant

- Gains: every substitution becomes a declared, auditable value per plant (for example a default
  efficiency and loss, and perhaps a fixed tailrace level); runs keep going when the user chooses to
  supply a stand-in; the choice is visible in the case files instead of in code.
- Losses: new configuration surface (file or columns, schema, reader, validation, documentation),
  medium complexity; the values remain synthetic, only now user-declared; overlaps
  `CAD_FPHA_CONFIG.csv`, whose semantics are still open (ADR-028, tickets 018-019).
- Risks: a configured stand-in for the tailrace (a constant level) changes the shape of the FPH, not
  only its scale, and has no counterpart in the sources read `[to confirm]`; a default in the new
  file can be copied across cases and outlive the reason for it; still needs a rule for plants with
  no configured default (error or code default), which reopens this ADR.
- Operator message: an error for a missing input with no configured default, and an `Info` or
  `Warning` line naming the configured value used per plant and input.
- Tickets affected: ticket-016 rewritten and enlarged (reader, config schema, error for the
  unconfigured case); a new ticket for the configuration file or columns; coordinated with ticket-018
  and ticket-019 (ADR-028); ticket-029/030 document the new inputs; `CHANGELOG.md` `### Added`.

### (d)

(d) error for a missing tailrace, warning for efficiency/loss defaults

- Gains: stops the largest distortion (missing tailrace: FURNAS mean FPH ratio 8.75) while keeping
  cases without `Eficiencia`/`Perda.m` running; low complexity.
- Losses: mixed policy, two behaviours to document and test; efficiency = 1 and loss = 0 still
  inflate generation (+10 % to +16 % mean FPH ratio in section 7), which is about the size of the
  FP-R02 finding itself.
- Risks: the split is by severity in one example, not by methodology: MM §9.1 lists efficiency
  (through `rho_esp`) and losses as registry data as much as the tailrace; warnings may be missed, as
  in (b).
- Operator message: the tailrace part of the (a) error (`missing inputs: row in
  CAD/CAD_TAILRACE.csv`, exit non-zero), plus one `Warning` per plant for an efficiency of 1.0 or a
  loss of 0 m assumed.
- Tickets affected: ticket-016 rewritten (error only for `Tailrace`; warnings for the other two; the
  efficiency/loss test asserts the warning and the default); ticket-017, ticket-029/030 and
  `CHANGELOG.md` as in (a) for the tailrace and as in (b) for the rest.

### Sub-question: explicit zero loss

Is `Perda.m = 0` present in `CAD/CAD_UHE.csv` (an explicit `0` cell) a valid value under the chosen
policy? The reader gives `Some(0.0)` for an explicit `0` and `None` for both an absent column and an
empty cell (`readers/hydro.rs:453-456`); the absent-column and empty-cell cases cannot be told apart
after reading, and the policy treats both as missing.

- Gains: consistent with DESSEM, where `AC PERHID` accepts values `>= 0` (MU 22.3.2 §III.5.4.7) and
  MM eq. 9.27 has no lower bound above 0; a plant with negligible losses can be described without a
  fake small value; the only change for the user is to write `0` instead of leaving the cell empty.
- Risks: a user who means "unknown" and types `0` passes silently with the same inflation as today
  (the policy can only check presence, not plausibility); `Eficiencia = 0` (zero FPH) and other
  out-of-range values are not covered by this ADR: ticket-016 lists range checks (efficiency in
  (0, 1], loss >= 0) and NaN handling as out of scope and points them to ticket-024, whose scope is
  stated as geometry inputs, so where the `CAD_UHE.csv` range check lands is `[to confirm]`.

### Sub-question: EngMax scope

Does the policy also apply to EngMax? The plan recommends no: EngMax is out of scope (master-plan
non-goals; EM-01..EM-03 stay as they are). Today EngMax already skips a plant without a tailrace
polynomial (`engmax.rs:96-137`, falls back to `h_ef`) but uses efficiency 1.0 and loss 0 m through
`ProductionFunction::new` when `Eficiencia`/`Perda.m` are missing.

- Gains: (answering no) ticket-016 stays inside `build_fit_params`; EngMax behaviour and its accepted
  ADR-017 hard error are not reopened; no EngMax regression baseline moves.
- Risks: (answering no) the same `CAD_UHE.csv` gap is an error in the FPHA fit and a silent default in
  EngMax, an asymmetry the user may not expect; the loss enters the EngMax net head, so a missing
  `Perda.m` raises the EngMax limit slightly; whether efficiency enters the EngMax limit at all (MM
  §9.17 uses the head ratio) is `[to confirm]`; per the master-plan rework trigger, if EngMax later
  adopts the policy, the defaults of `ProductionFunction::new` are revisited.

## Recommendation

(a), with "explicit `Perda.m = 0` is valid" and "EngMax out of scope". The three inputs are registry
data of every DESSEM hydro plant (MM §9.1, §9.16.10), always present in the fixed `hidr.dat` record;
the sources read show no DESSEM default that the current substitutions imitate, and the 0 m tailrace
in particular has no counterpart. The measured effect (section 7: +10 % to +16 % mean FPH from
efficiency and loss, a factor of about 8.75 on FURNAS without tailrace, plane counts and alpha
changed) is large enough that a warning (b) or a mixed policy (d) leaves a non-physical model in
place; (c) adds a configuration surface that still produces synthetic data and overlaps the open
ADR-028. (a) is low complexity, reuses the ADR-026 failure collection, does not touch the official
example, and is reversible (two-way in the master plan): a later need for stand-in values can add (c)
on top of the error without changing the error's contract. The cost is that cases without these
inputs stop in computed mode until the data is supplied, and an empty `Perda.m` cell must be written
as `0`.

## Decision

PENDING

## Consequences

- If (a) is chosen:
  - ticket-016: proceeds as written (`FphaInput`, `MissingInputs`, `MissingFphaInputs`, flipped
    `baseline_a3_*` tests, control test kept).
  - ticket-017: post-fix evidence shows the two derived A3 cases failing with the typed error.
  - ticket-029, ticket-030: document the three required inputs and the error.
  - `CHANGELOG.md`: behaviour-change entry (ticket-016).
- If (b) is chosen:
  - ticket-016: rewritten for warnings; tests assert the warning and the unchanged defaults.
  - ticket-029, ticket-030: document the defaults and the warning.
  - R4 is recorded as met by "explicit outcome = warning"; the FPH inflation stays a known limitation.
- If (c) is chosen:
  - ticket-016: rewritten and enlarged; a new configuration ticket is added and sequenced with
    ticket-018/019 (ADR-028).
  - ticket-029, ticket-030: document the new configuration.
  - `CHANGELOG.md`: `### Added` for the configuration.
- If (d) is chosen:
  - ticket-016: rewritten (error for `Tailrace`, warnings for `Efficiency`/`HydraulicLoss`).
  - ticket-029, ticket-030: document both behaviours.
- Sub-questions:
  - Explicit zero loss valid: ticket-016 as written (its `explicit_zero_loss_is_accepted` test); if
    invalid, ticket-016 adds a rejection of `0` and the brief's `AC PERHID` reference becomes a
    recorded divergence from DESSEM.
  - EngMax in scope: a new ticket outside this plan's non-goals, or a plan amendment; the
    master-plan rework trigger applies (`ProductionFunction::new` defaults revisited).
- In all cases: `ProductionFunction::new` keeps its signature in this plan; the master-plan ADR-027
  entry is set to `accepted` by the orchestrator.
