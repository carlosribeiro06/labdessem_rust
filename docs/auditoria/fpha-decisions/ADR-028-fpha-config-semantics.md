# ADR-028: Semantics of CAD_FPHA_CONFIG.csv, fitting defaults and fit volume domain

Decision brief for defects A4 and B2 (audit FP-05, decision D13; `docs/auditoria/2026-11-05-auditoria-modelo.md`
lines 905-920, section 12.3 at lines 1634-1675, D13 at line 1906). Requirements R5 and R7 of
`plans/fpha-corrections/00-master-plan.md`. Option names follow the ADR-028 entry of the master-plan
frontmatter. Context only, not options here: ADR-025 (accepted) applies alpha once; ADR-026 (accepted)
makes `compute_fpha` fail for every plant with units it cannot fit; ADR-027 (accepted) makes a missing
tailrace row, `Eficiencia` or `Perda.m` a typed error. The plane-count rule and the MILP selection are
ADR-029 (ticket-020); the volume variable of the FPHA rows is ADR-030.

## Status

Accepted (2026-11-19)

## Context

**Defect.** `CAD/CAD_FPHA_CONFIG.csv` is read, but no field of it reaches the fit (code at commit
27a608b):

1. `crates/labdessem-io/src/readers/fpha_geometry.rs:195-204`: `FphaPlantConfig` holds `num_planes`,
   `tolerance`, `min_volume_pct` and `max_volume_pct`, each under `#[allow(dead_code)]` (four
   attributes, lines 196, 198, 200 and 202).
2. `crates/labdessem-io/src/readers/fpha_geometry.rs:225-295`: `read_fpha_config` accepts the aliases
   `MaxPlanes`/`max_planes`, `ReductionTolerance`/`Tolerance`, `MinVolumePct` and `MaxVolumePct`. It
   rejects a negative tolerance and a percentage outside 0-1 (the percentages are fractions in the file,
   for example `0.5`), but not `min >= max`, and a repeated plant row overwrites the earlier one
   (`result.insert`, line 283).
3. `crates/labdessem-cli/src/fpha_integration.rs:377-391`: `build_fpha_config` starts from
   `FphaPlantConfig::computed(10, 6, 6, PlaneReductionConfig::Angle { tolerance_deg: 1.0 })`
   (10 planes, 6 volume points, 6 flow points, 1 degree angle reduction) and copies only `num_planes`
   into `max_planes`; the tolerance and the two percentages are dropped (comment "tolerance applies to
   the reduction config, not a direct field").
4. `crates/labdessem-fpha/src/config.rs:55-65`: `to_fitting_bounds` copies `max_planes` into
   `FittingBounds::max_planes_per_hydro`, which no production code reads (only test fixtures set it;
   ticket-020 / ADR-029). So no field of the file changes the fit today.

**Fit domain (B2).** `fit_plant` (`crates/labdessem-fpha/src/pipeline.rs:58-73`) takes `v_min`/`v_max`
from the forebay (VHA) table of `CAD/CAD_GEOMETRIA_UHE.csv`, not from `CAD/CAD_UHE.csv`, and passes the
forebay span as the useful storage. `resolve_fitting_bounds` (`crates/labdessem-fpha/src/fitting/grid.rs:84-123`)
ignores the config's volume fields and, for a single-volume plant (forebay span <= 1e-9 or one volume
point), synthesizes a 2-point window of half-width max(0.5 % of useful storage, 1 hm3)
(`V_SYNTH_FRAC = 0.005`, `V_SYNTH_ABS = 1.0`, lines 10 and 13). Because the useful storage passed is the
forebay span, a one-point VHA table gives a span of 0 and therefore a half-width of exactly 1 hm3.

**24Barras data** (`examples/24Barras/CAD/`):

| Code | Plant | `Tipo` in CAD_UHE | VHA rows | VHA volume range (hm3) | CAD_UHE Vmin / Vmax (hm3) |
|---|---|---|---|---|---|
| 0 | CAMARGOS | RV | 100 | 120 .. 792 | 120 / 792 |
| 1 | ITUTINGA | FD | 1 | 10.64 | 11 / 11 |
| 2 | FUNIL-GRANDE | FD | 1 | 265.86 | 304 / 304 |
| 3 | FURNAS | RV | 100 | 5733 .. 22950 | 5733 / 22950 |

- For CAMARGOS and FURNAS the two intervals coincide; for ITUTINGA CAD_UHE has Vmin = Vmax (11) and
  differs from the VHA point (10.64); for FUNIL-GRANDE the VHA point (265.86) differs from the CAD_UHE
  volume (304).
- `Tipo` (RV / FD) is read and discarded (`crates/labdessem-io/src/readers/hydro.rs:437-438`, field
  `_tipo`). CAD_UHE has no regulation-period (daily / weekly / monthly) field.
- The official example has no `CAD/CAD_FPHA_CONFIG.csv`. The file is announced in `README.md:140`,
  `CHANGELOG.md:104` and `docs/docs.html:174`.

**Audit reproduction** (FP-05, pre-fix code): a row `3;2;5.0;0.5;0.6` (FURNAS: 2 planes, window
50 %-60 % of the useful volume) gives FURNAS 6 planes identical byte by byte to the run without the
file, and three different files give the same objective 4.502.332,7276 (audit configuration, not the
fast configuration below).

**Post-fix values** (`docs/auditoria/fpha-baseline-reproduction.md`, section 8, commit d6a223d, fast
configuration of section 3, file absent):

| Code | Plant | Planes | alpha |
|---|---|---|---|
| 0 | CAMARGOS | 7 | 0.969607 |
| 1 | ITUTINGA | 3 | 1.000000 |
| 2 | FUNIL-GRANDE | 4 | 1.000000 |
| 3 | FURNAS | 6 | 0.991295 |

Objective of the computed run: R$ 2383481.5598 (release CLI and `O_c` of the end-to-end test agree to
the 4 decimals printed). No new measurement is made for this brief; every effect of an option below that
is not in section 8 is tagged `[not measured]` and is recorded by ticket-032 after the change.

## Methodology reference

Sources: `~/.claude/skills/dessem/references/42-metodologia-hidreletricas.md` section 16 (MM 22.3.0
§9.16.11-9.16.12, p. 71-76 / PDF 97-102; Libs §15.6.1), section 3 (MM 22.3.0 §9.4, eq. 9.2) and
`~/.claude/skills/dessem/references/11-entdados-registros-a2.md`, record `FP` (MU 22.3.2 §III.5.4.5),
with `REGRANPTV` from `42-metodologia-hidreletricas.md` section 16 (MU 22.3.2 §III.6.3.1) and the
regulation flag of `hidr.dat` from `12-entradas-hidraulicas-a.md` §3 (MU 22.3.2 §III.11, field 42). The
manual PDFs were not available on this machine; the claims below come from those skill references.

Grid and window (`42-metodologia-hidreletricas.md` §16, MM §9.16.11): the FPHA is fitted on an
NPTV x NPTQ grid over a volume window `(V0 +/- DeltaV_FPHA)`, with turbined flow from 0 to the maximum
turbined flow. The MM defaults depend on the plant's regulation type: daily regulation, window 0-100 %,
NPTV 5, NPTQ 5; weekly or monthly regulation, window +/-1 % of the useful volume around V0, NPTV 2,
NPTQ 5. With a constant tailrace polynomial one flow point is used; operative constraints such as
`Qmin` can narrow the window; everything can be changed by the `FP` record.

Percentage reference (`42-metodologia-hidreletricas.md` §3, MM §9.4 eq. 9.2): a volume in % of the
useful volume maps to `V = Vmin + V%/100 * (Vmax - Vmin)`, with `Vmin`/`Vmax` the registry minimum and
maximum volumes of the plant (the `hidr.dat` record, MU 22.3.2 §III.11). The window percentages of the
MM are therefore taken over the registry `[Vmin, Vmax]`, whose labdessem counterpart is CAD_UHE
`Vmin.hm3`/`Vmax.hm3`; the forebay polynomial is a separate registry item, not the reference interval.

FP record (`11-entdados-registros-a2.md`, MU 22.3.2 §III.5.4.5): field 3 volume treatment for weekly /
monthly plants (1 linear approximation in V, default; 2 3D convex envelope), field 4 NPTQ (2..ZPTQFPHA,
default 5), field 5 NPTV (1..ZPTVFPHA, default 5 daily and 2 weekly / monthly; an even value informed
explicitly receives +1), field 6 concavity check, field 7 least-squares fit, field 8 window length in %
of the useful volume (default 100 % daily, 1 % weekly / monthly) and field 9 tolerance for the deviation
of the production function in % (default 2.0). Field 8 is one length around V0, not a min / max pair as
in `CAD_FPHA_CONFIG.csv`; a 0-100 % window is expressible as the pair (0, 1), while +/-1 % around V0
needs V0 and so is case-dependent `[to confirm]` how DESSEM clips a window around V0 at the volume
limits.

Tolerance (`11-entdados-registros-a2.md` FP field 9; `42-metodologia-hidreletricas.md` §16 "Acurácia e
saídas", MU §V.11.5.1): FP field 9 is a deviation tolerance, the threshold above which LOG_DESVFPHA
reports deviations between FPH and FPHA (the 2026-06-15 deck reports "HOUVE 468 DESVIOS SUPERIORES AOS
VALORES TOLERADOS"); it does not reduce planes. It is not the meaning of labdessem's
`ReductionTolerance`, which is consumed (if wired) by `PlaneReductionConfig::Angle { tolerance_deg }`.

Plane reduction (`42-metodologia-hidreletricas.md` §16 and `23-saidas-diagnostico-b.md` §8, Libs 11.0.0
§15.6.1 / §15.6): merging similar hyperplanes by angle or by distance comes from the Libs cards
`HIDRELETRICA-FPHA-METODO-REDUCAO-CORTES-ANGULO-PADRAO` (tolerance in degrees between normals) and
`...-DISTANCIA-PADRAO` (mean squared error of the projections over gh^2 max, in %, example 0.005),
created in Libs 11.0.0 (Jul/2025) for NEWAVE and DESSEM, one method at a time; it is not in the MM.
Whether DESSEM accepts the angle card is `[to confirm]` (`12-entradas-hidraulicas-b.md` divergence 9:
the "available per model" column of Libs §15.7 was lost in extraction). No default angle value is given
in the sources read `[to confirm]`.

Regulation type and its rule (`42-metodologia-hidreletricas.md` §3 and §16; `12-entradas-hidraulicas-a.md`
§3, MU 22.3.2 §III.11 field 42): the regulation flag M / S / D (monthly, weekly, daily) is a `hidr.dat`
field and determines the FPHA defaults. `REGRANPTV` field 2 (MU 22.3.2 §III.6.3.1) selects the default
rule: 0 = 2 points weekly / monthly and 5 daily, 1 = 2 points for all, 5 = 5 points for all. In the
2026-06-15 ONS deck the rule is 0 (LOG_RELATO "Utilizada a regra: 0 para a FPHA") and the only active
`FP` is plant 287 with a 100 % window, NPTQ 10 and NPTV 5. labdessem has no counterpart of the
regulation flag (the `Tipo` RV / FD of CAD_UHE is reservoir versus run-of-river, not a regulation
period, and is not read); how DESSEM maps a run-of-river plant to the regulation flag is `[to confirm]`.

Contradiction check: no contradiction was found between the methodology and the audit's description of
`CAD_FPHA_CONFIG.csv` (FP-05 describes the window as "50 % a 60 % do volume útil", the reading of eq. 9.2
above). Two divergences are recorded instead: the file's window is a min / max pair while FP field 8 is
a length around V0, and the file's tolerance is a reduction tolerance while FP field 9 is a deviation
tolerance (`11-entdados-registros-a2.md` FP fields 8 and 9, MU 22.3.2 §III.5.4.5; `42-metodologia-hidreletricas.md` §16, Libs §15.6.1).

## Options

### (a)

(a) implement every field with DESSEM MM defaults (window 0-100 % of Vmin..Vmax, NPTV = NPTQ = 5,
explicit even NPTV + 1)

- Gains: the defaults become the MM daily defaults (MM §9.16.11), so a computed run without the file
  fits the grid DESSEM uses for daily plants and `REGRANPTV` rule 5; every field of the file reaches the
  fit (`MaxPlanes` through ADR-029, `ReductionTolerance` as the angle tolerance, the two fractions as the
  window over CAD_UHE `[Vmin, Vmax]`), so FP-05 is closed in substance; B2 is decided with the MM
  reference interval; the defaults are named constants (ticket-019) instead of literals in
  `build_fpha_config`.
- Losses: BREAKING change of every computed run: the 6 x 6 grid becomes 5 x 5 for plants with a volume
  span and the 6 flow points become 5 for single-volume plants (their volume points stay 2 by the
  run-of-river synthesis), so the fitted planes, alpha and objective change and the ticket-017
  regression baselines (section 8) are re-recorded; medium complexity (two tickets, 019 and 031).
- Risks: the weekly / monthly default (+/-1 % around V0, 2 points) is not applied, because labdessem
  has no regulation flag (S4), so reservoir plants that DESSEM would treat as weekly or monthly get the
  daily window; the "even NPTV + 1" rule has no field to act on unless S1 adds `NPTV`; a narrower grid
  can change the plane count in either direction `[not measured]`.
- Fitted planes: change for CAMARGOS and FURNAS (5 x 5 instead of 6 x 6 over the same interval, since
  CAD_UHE equals the VHA span) and for ITUTINGA and FUNIL-GRANDE (5 instead of 6 flow points); counts,
  alpha and objective relative to 7 / 3 / 4 / 6, 0.969607 / 1.0 / 1.0 / 0.991295 and R$ 2383481.5598
  `[not measured]` (ticket-032).
- CAD_FPHA_CONFIG.csv: kept, optional, columns `Codigo;MaxPlanes;ReductionTolerance;MinVolumePct;MaxVolumePct`
  unchanged; empty cell = default; fractions 0-1 over CAD_UHE `[Vmin, Vmax]`; `ReductionTolerance` in
  angle degrees; new rejections for `min >= max` and a non-finite value (ticket-031), duplicate rows
  (ticket-024).
- Tickets affected: 019 proceeds as written (constants 5 / 5, `VolumeWindow`, `window_reference_hm3`);
  031 proceeds as written (behavior change, CHANGELOG `### Changed`); 024 keeps its
  `CAD_FPHA_CONFIG.csv` duplicate-row part; 032 records the new planes, alpha and objective and the
  per-field effect of one config row.

### (b)

(b) implement every field and keep the current defaults (full VHA range, 6 x 6 grid)

- Gains: every field of the file reaches the fit as in (a), closing FP-05; a computed run without the
  file keeps its fitted planes, alpha and objective exactly, so no baseline moves; low complexity (019
  with 6 / 6 constants, 031 without a default change).
- Losses: the defaults keep diverging from the MM (6 x 6 instead of 5 x 5; window over the VHA table
  instead of the registry interval unless S2 picks CAD_UHE, in which case only the empty-file default
  stays on the VHA span); the 6 x 6 grid has no source in the MM or the Libs references read (an even
  NPTV of 6 informed in DESSEM would become 7).
- Risks: users take 6 x 6 as the DESSEM default; a later move to the MM defaults is a second breaking
  change and a second baseline re-recording (master-plan rework trigger).
- Fitted planes: unchanged without the file (7 / 3 / 4 / 6, alpha as in section 8, R$ 2383481.5598),
  bit for bit by construction of ticket-019 (full window, no reference); with a config row, change for
  that plant only `[not measured]`.
- CAD_FPHA_CONFIG.csv: kept, same columns and semantics as (a); only the defaults differ.
- Tickets affected: 019 with `DEFAULT_VOLUME_POINTS = DEFAULT_FLOW_POINTS = 6`; 031 wires the fields
  (CHANGELOG `### Fixed` for the file, no default change); 024 unchanged; 032 shows the default run
  unchanged and records the per-field effect.

### (c)

(c) remove CAD_FPHA_CONFIG.csv from inputs and documentation; keep hard-coded defaults

- Gains: no announced input without effect; smallest surface; no baseline moves; low complexity.
- Losses: no per-plant control of the grid, window or plane count (the `FP` record of DESSEM, which ONS
  uses for plant 287, has no counterpart); the B2 fit-domain divergence (VHA span instead of the
  registry interval) stays and is documented as a limitation; the 6 x 6 grid stays without a source.
- Risks: a case that ships the file today silently loses nothing (the file already has no effect) but
  now gets no "Lendo arquivo" line, so users may not notice the removal unless the reader rejects or
  warns on the file (which of the two is `[to confirm]`); re-adding the file later repeats the design of (a) or (b).
- Fitted planes: unchanged (7 / 3 / 4 / 6, alpha and objective as in section 8).
- CAD_FPHA_CONFIG.csv: removed from the reader (`read_fpha_config`, `FphaPlantConfig` of `labdessem-io`),
  from `build_fpha_config`, from `README.md:140`, `CHANGELOG.md` (`### Removed`) and `docs/docs.html:174`.
- Tickets affected: 019 and 031 proposed for removal and re-refined as one removal ticket; 024 drops
  its `CAD_FPHA_CONFIG.csv` part; 032 records unchanged planes and objective.

## Recommendation

(a). The MM gives explicit FPHA defaults (MM §9.16.11, FP field 5 and 8 of MU 22.3.2 §III.5.4.5) and
eq. 9.2 fixes the reference interval of a volume percentage, so (a) is the only option whose defaults
and domain have a source; (b) closes FP-05 but keeps an unsourced 6 x 6 grid and would need a second
breaking change to align later; (c) removes the per-plant control that DESSEM's `FP` record provides
and leaves B2 as a limitation. The cost of (a) is one breaking change of every computed run with
re-recorded baselines (ticket-032), which this plan already budgets; the decision is two-way (master
plan), since the defaults are named constants. The weekly / monthly default is left out by S4 because
the case data cannot tell it apart.

S1: No `NPTV` / `NPTQ` columns in `CAD_FPHA_CONFIG.csv` for now; the defaults 5 / 5 are constants and
the "even NPTV + 1" rule only matters for an explicitly informed value, so it is moot without the
column; adding the columns later is additive (two-way).

S2: The window fractions refer to CAD_UHE `[Vmin, Vmax]` (MM §9.4 eq. 9.2, the registry interval), not
to the VHA span; for 24Barras the two coincide for CAMARGOS and FURNAS. For a run-of-river plant with a
one-point VHA table (ITUTINGA, FUNIL-GRANDE) the forebay level is constant, so the volume window has no
effect on the FPH; the window and the reference are ignored and the current 2-point synthesis around
the VHA point (half-width 1 hm3) is kept, which also covers ITUTINGA (CAD_UHE Vmin = Vmax, a zero-width
reference) and FUNIL-GRANDE (CAD_UHE volume 304 outside the VHA point 265.86). A config window on such a
plant is accepted and has no effect `[to confirm]` whether it should warn (ticket-031).

S3: `ReductionTolerance` is the angle tolerance in degrees between plane normals of the Libs card
`HIDRELETRICA-FPHA-METODO-REDUCAO-CORTES-ANGULO-PADRAO` (Libs 11.0.0 §15.6.1), default 1.0 (current
literal), consumed by `PlaneReductionConfig::Angle { tolerance_deg }`; it is not FP field 9 (a deviation
tolerance in %, which labdessem does not model). Linked to ADR-029 S2: if ADR-029 chooses the distance
criterion, this column changes meaning to % and the brief is reopened.

S4: Since neither CAD_UHE nor any labdessem input carries the `hidr.dat` regulation flag M / S / D,
the daily default (window 0-100 %, NPTV 5, NPTQ 5) applies to every plant with a volume span, which
equals DESSEM's `REGRANPTV` rule 5 (5 points for all); the weekly / monthly default (+/-1 % around V0,
2 points) is reachable per plant through the file's window fractions only as a fixed interval, not
centred on V0. Single-volume plants keep the 2-point synthesis (S2). The `Tipo` RV / FD column is not
used as a proxy for the regulation period `[to confirm]` with the user if a regulation field should be
added to CAD_UHE later (new input, out of this plan).

## Decision

Accepted: option (a) — every field of `CAD_FPHA_CONFIG.csv` reaches the computed FPHA fit with the
DESSEM MM defaults (MM §9.16.11): volume window 0-100 % of CAD_UHE `[Vmin, Vmax]`, NPTV = NPTQ = 5,
`ReductionTolerance` as the plane-angle tolerance, `MaxPlanes` enforced as ADR-029 decides; the file
stays optional with columns `Codigo;MaxPlanes;ReductionTolerance;MinVolumePct;MaxVolumePct`, an empty
cell takes the default, and the defaults become named constants. BREAKING: the fitted planes, alpha and
objective of every computed run change (6 x 6 grid becomes 5 x 5; 6 flow points become 5 for
single-volume plants).

Sub-question S1 (NPTV / NPTQ columns): accepted as recommended — no `NPTV` / `NPTQ` columns in the
file; 5 / 5 are constants and the "even NPTV + 1" rule is moot without the column.

Sub-question S2 (window reference): accepted as recommended — the fractions (0-1) refer to CAD_UHE
`[Vmin, Vmax]` (MM §9.4 eq. 9.2); run-of-river plants with a one-point VHA table ignore the window and
keep the 2-point synthesis around the VHA point (half-width 1 hm3).

Sub-question S3 (`ReductionTolerance`): accepted as recommended — plane-angle tolerance in degrees
(Libs 11.0.0 §15.6.1), default 1.0; linked to ADR-029 S2, which reopens this point if it chooses the
distance criterion.

Sub-question S4 (regulation type): accepted as recommended — labdessem has no regulation-type input,
so the daily default (window 0-100 %, 5 x 5) applies to every plant with a volume span.

Chosen by the user on 2026-11-19 at the decision gate, matching the recommendation. Ticket-019,
ticket-031 and ticket-024 proceed as written; ticket-032 re-records the baselines of section 8 (planes,
alpha, objective) and the per-field effect of one config row. The open points stay `[to confirm]`:
whether a window on a one-point-VHA plant should warn (ticket-031), and the weekly / monthly regulation
default with its regulation-type input (out of this plan).

## Consequences

Option (a) was chosen with S1-S4 as recommended; the other branches record the rejected
alternatives.

- If (a) is chosen:
  - ticket-019: proceeds as written (constants 10 / 5 / 5 / 1.0, `VolumeWindow::FULL`,
    `window_reference_hm3`, `InvalidFitWindow`).
  - ticket-031: proceeds as written; every computed run changes; CHANGELOG behaviour-change entry.
  - ticket-024: keeps the duplicate-row rejection for `CAD_FPHA_CONFIG.csv`.
  - ticket-032: re-records the planes, alpha and objective of section 8 and the effect of one config
    row; the `[not measured]` effects above get numbers.
- If (b) is chosen:
  - ticket-019: constants 6 / 6; otherwise as written.
  - ticket-031: wires the fields with no default change; the default run is bit-identical.
  - ticket-032: shows the default run unchanged and records the per-field effect.
  - The divergence of the 6 x 6 grid from the MM is documented by ticket-029 / ticket-030.
- If (c) is chosen:
  - ticket-019 and ticket-031: proposed for removal and re-refined as one removal ticket (reader,
    `build_fpha_config`, README, CHANGELOG `### Removed`, `docs/docs.html`).
  - ticket-024: drops the `CAD_FPHA_CONFIG.csv` part.
  - B2 is recorded as a documented limitation.
- Sub-questions:
  - S1 yes: ticket-019 / ticket-031 gain `NPTV` / `NPTQ` columns, the even + 1 rule and range checks
    (NPTQ >= 2, NPTV >= 1).
  - S2 VHA span: ticket-019 drops `window_reference_hm3`; no change for 24Barras, a change for cases
    whose VHA table and CAD_UHE interval differ.
  - S3 distance: reopened together with ADR-029 S2.
  - S4 a regulation field: a new input ticket outside this plan.
- In all cases: the master-plan ADR-028 entry is set by the orchestrator; ADR-029 decides how
  `MaxPlanes` is enforced.
