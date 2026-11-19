# ADR-032: Tailrace modelling for computed FPHA (single polynomial or families)

Decision brief for methodology divergence B4 (labdessem models the tailrace of each plant with one degree-4
polynomial in the plant outflow; DESSEM supports families of polynomials selected by the downstream reservoir
level, piecewise polynomials by flow range and explicit drowning). Requirement R7 of
`plans/fpha-corrections/00-master-plan.md`. Option names follow the ADR-032 entry of the master-plan
frontmatter. Context only, not options here: ADR-027 (accepted) makes a missing tailrace row a typed error on
the computed path and forbids silent physical defaults; ADR-030 (volume reference), ADR-031 (turbining axis)
and ADR-033 (alpha points) are separate decisions.

## Status

Accepted (2026-11-19)

## Context

**Code facts.** Verified at HEAD `9942497` (branch `fix/fpha-corrections`); the tailrace code is unchanged
since `8b8afaa`, the commit quoted by the ticket.

1. Input: `CAD/CAD_TAILRACE.csv` has one row per plant with the header
   `codigo_usina;a0.m;a1.m/m3s;a2.m/m3s2;a3.m/m3s3;a4.m/m3s4` (`examples/24Barras/CAD/CAD_TAILRACE.csv`, four
   rows, codes 0 to 3). `read_tailrace` (`crates/labdessem-io/src/readers/fpha_geometry.rs`) rejects a
   non-finite coefficient (`check_finite`) and a second row for the same plant
   (`plant <id> has duplicate rows at lines <a> and <b> in CAD_TAILRACE.csv`, ticket-024), and returns
   `TailraceData { plant_code, coefficients: [f64; 5] }`.
2. Two consumers of the same coefficients:
   - computed FPHA: `build_fit_params` (`crates/labdessem-cli/src/fpha_integration.rs`) pushes
     `FphaInput::Tailrace` into `MissingFphaInputs` when the row is absent (ADR-027) and otherwise builds
     `TailraceModel::Polynomial { coefficients }`; `fit_context` (`crates/labdessem-fpha/src/pipeline.rs`)
     wraps it as `TailraceSource::Entity(params.tailrace.clone())`;
   - EngMax: `read_hydro_plants` (`crates/labdessem-io/src/readers/hydro.rs`) calls `read_tailrace` again and
     stores the row in `HydroPlant::tailrace_coefficients: Option<[f64; 5]>`
     (`crates/labdessem-core/src/entities/hydro.rs`); `plant_geometry`
     (`crates/labdessem-simulation/src/engmax.rs`) builds
     `TailraceSource::Entity(Some(TailraceModel::Polynomial { .. }))` from it, or returns `None` without it.
     `/usr/bin/grep -rn "tailrace_coefficients" crates --include=*.rs | wc -l` prints 24 (field, reader,
     EngMax and test fixtures in `labdessem-core`, `labdessem-model` and `labdessem-simulation`): a type
     change of the field ripples through all of them.
3. `TailraceModel` (`crates/labdessem-fpha/src/types.rs`) has `Constant { level_m }` and
   `Polynomial { coefficients }`; `evaluate` sums `coef[i] * q^i` over the whole outflow range, with no
   flow-range bounds and no downstream-level argument.
4. `TailraceSource` (`crates/labdessem-fpha/src/fitting/production.rs`) has `Entity(Option<TailraceModel>)`
   and `Families`, documented as "placeholder for backwater coupling". `ProductionFunction::tailrace_level`
   returns `0.0` for `Families` (and for `Entity(None)`). `TailraceSource` is re-exported by
   `crates/labdessem-fpha/src/lib.rs` (via `fitting/mod.rs`); the crate is used inside the workspace only
   (`labdessem-cli`, `labdessem-simulation`). The Current State grep, re-run at `9942497`:

   ```text
   $ /usr/bin/grep -rn "TailraceSource::Families" crates --include=*.rs
   crates/labdessem-fpha/src/fitting/production.rs:68:            TailraceSource::Families => 0.0, // Placeholder
   ```

   Only the match arm: `Families` is never constructed. A 0 m tailrace is the silent default ADR-027
   forbids on the computed path.
5. Outflow range evaluated by the fit: `build_grid` (`crates/labdessem-fpha/src/fitting/grid.rs`) samples
   turbining on `[0, max_turbined_m3s]` and the secant spillage axis on `[0, max_turbined_m3s]`, so the
   tailrace polynomial is evaluated at outflows up to `2 x max_turbined_m3s`. For CAMARGOS
   (`CAD_CONJ_UHE.csv`: 2 units x 107 m3/s) that is 214 m3/s turbined and 428 m3/s total.

**24Barras topology** (`examples/24Barras/CAD/CAD_UHE.csv`, columns `Tipo`, `Jusante`, `Vmin.hm3`,
`Vmax.hm3`): CAMARGOS (`RV`) -> ITUTINGA (`FD`, 11 = 11 hm3) -> FUNIL-GRANDE (`FD`, 304 = 304 hm3) -> FURNAS
(`RV`, 5733 to 22950 hm3, `Jusante` `-`). A family selection by downstream level would therefore see a fixed
downstream level for CAMARGOS and ITUTINGA (their downstream plants have Vmin = Vmax), a varying one only for
FUNIL-GRANDE (downstream FURNAS), and no downstream plant for FURNAS (its downstream reservoir is outside the
case, so its level would be an exogenous input).

**Relation to the official data.** Row 0 (CAMARGOS) of `CAD_TAILRACE.csv`, `885.3052;-3.15E-18;1.97E-05;
-2.55E-08;0.00E+00`, equals, rounded to three significant digits in a1-a3, segment 1 of curve 001 of plant
0001 in the 2026-06-15 `polinjus.csv` quoted by the `dessem` skill (`0.88530520000000E+03;
-.31521360000000E-17;0.19686530000000E-04;-.25518040000000E-07;0.00000000000000E+01`, valid for
0 <= Qjus <= 408.649 m3/s, in a curve with `nPartes` 002, Href 885.3052 m). That plant 0001 is CAMARGOS is
the skill's reading of the ONS code `[to confirm]`. The origin of rows 1-3 was not traced.

**Baseline** (`docs/auditoria/fpha-baseline-reproduction.md`, section 9, commit `51751ba`): planes 6 / 3 / 3 / 5,
alpha 0.972834 / 1.000000 / 1.000000 / 0.993203, reported maximum fit-grid deviation 2.876295 / 0.000000 /
0.000000 / 23.284586 MW (CAMARGOS / ITUTINGA / FUNIL-GRANDE / FURNAS), computed objective 2420590.3195 R$.

Measured effect: option (a) changes no code, so it keeps the section 9 values (objective 2420590.3195 R$; CAMARGOS 2.876295 MW and FURNAS 23.284586 MW maximum deviation).
Measured effect: (b) on 24Barras [not measured]: needs the official tailrace families (Href and coefficients per curve) of the four plants and a rule for the downstream level used at fit time; `examples/24Barras` has one row per plant and no ONS deck is available on this machine.
Measured effect: (c) on 24Barras [not measured]: needs the official segments 2 and beyond of each plant (for CAMARGOS, segment 2 of curve 001 above 408.649 m3/s, which the fit reaches only on the spillage axis, outflow up to 428 m3/s); they are not in the repository.

## Methodology reference

- Sources: the `dessem` skill references `42-metodologia-hidreletricas.md` (section 15, MM 22.3.0
  §9.16.1-9.16.10, p. 58-71 / PDF 84-97, Libs §15.3.4, §15.6.1; section 16; "Divergencias" item 1),
  `12-entradas-hidraulicas-b.md` (`polinjus.csv`, MU 22.3.2 §III.31.3; divergences 6 and 7),
  `12-entradas-hidraulicas-a.md` (`hidr.dat` fields 26-28, MU 22.3.2 §III.11), `23-saidas-diagnostico-b.md`
  §4.4 and `61-evolucao-decks-oficiais.md`. The manual PDFs and the ONS decks are not on this machine, so no
  claim below is re-read from a PDF or a deck.
- Base model (MM 22.3.0 eq. 9.21-9.23, `42-metodologia-hidreletricas.md` section 15): gross head
  `h_mon(V) - h_jus(Q, S)`, with the tailrace rating curve `h_jus = sum_{k=0..4} a_k (Q + S)^k`; S enters only
  when the spillage flag is on. labdessem implements this single-polynomial form.
- Piecewise polynomial (MM 22.3.0 eq. 9.24, `42-metodologia-hidreletricas.md` section 15): from GTDP/ONS, one
  polynomial k per flow range `[Qdef_min^(k), Qdef_max^(k)]`, with continuity, generalised to the downstream
  flow `Q_jus` of the lateral-flow equation 9.25 (§9.16.6).
- Backwater, remanso (MM 22.3.0 §9.16.4, `42-metodologia-hidreletricas.md` section 15): N polynomials, each
  tied to a reference level of the **downstream** reservoir; with simple polynomials (`hidr.dat`) the first
  one with reference level >= `h_mon,j(V_j)` is used; with piecewise polynomials DESSEM interpolates between
  the two neighbouring families. `hidr.dat` carries up to 6 curves and one downstream reference level per
  curve (fields 26-28, `12-entradas-hidraulicas-a.md`, MU 22.3.2 §III.11).
- Drowning, afogamento (MM 22.3.0 §9.16.5, `42-metodologia-hidreletricas.md` section 15): if
  `h_jus,i < h_mon,j`, `h_mon,j` is used; optional; the MM says it is **not** enabled with piecewise
  polynomials because they are already calibrated. Libs v7.0.0 (Apr/2023) added the cards
  `HIDRELETRICA-CURVAJUSANTE-AFOGAMENTO-EXPLICITO-PADRAO/-USINA`, default "do not consider" (Libs §15.3.3);
  the card is absent from MU 22.3.2 §III.31.3 (`12-entradas-hidraulicas-b.md` divergence 7).
- MM/Libs conflict on backwater with piecewise polynomials (Libs §15.3.4 vs MM 22.3.0 §9.16.4-9.16.5,
  `42-metodologia-hidreletricas.md` "Divergencias" item 1): the Libs says the **backwater** (remanso) is
  "desconsiderado, por padrao" with piecewise polynomials, while the MM treats backwater by interpolating the
  families and says it is the **drowning** that is not enabled. The skill reference records the MM 22.3.0 as
  outdated on this point and the exact meaning of "remanso" in the Libs as a question for CEPEL. This brief
  does not choose between the two readings `[to confirm]`.
- Libs card `HIDRELETRICA-CURVAJUSANTE` (`12-entradas-hidraulicas-b.md`, `polinjus.csv`, MU 22.3.2
  §III.31.3, Libs Tabelas 15.32-15.33): `HIDRELETRICA-CURVAJUSANTE;usina;curva;Href`, where Href (m) is the
  reference height of curve k = the upstream (forebay) level of the downstream plant;
  `-POLINOMIOPORPARTES;usina;curva;nPartes`; `-POLINOMIOPORPARTES-SEGMENTO;usina;curva;parte;Qmin;Qmax;a0..a4`
  (segment valid for `Qmin <= Qjus <= Qmax`, degree 4). Rules: up to 5 curves per plant, up to 5 segments
  per curve, curves in increasing Href, adjacent segment ranges, linear interpolation between curves at an
  intermediate downstream level; when the functionality is on, every plant of the configuration must be in
  the file and these polynomials prevail over `hidr.dat`; applied only to the FPHA. The Libs table calls
  the segment limits downstream reservoir levels (m) while the MU, the deck header (`QjusMin/QjusMax`) and
  the values (0; 408.649; 1e12) show flows (divergence 6).
- FPHA builder (Libs §15.6.1 note 10, `42-metodologia-hidreletricas.md` section 16): the FPHA by the Libs is
  the default; the `FPHADESSEM` flag (MU §III.6.3.5) returns to DESSEM's own computation, which has no
  piecewise polynomials, lateral flows or grids. The FPHA row of MM 22.3.0 eq. 9.33 has terms in V, Q and
  Q_lat only, with no downstream-volume term; which downstream level selects or interpolates the family when
  the planes are fitted is not stated in the sources read `[to confirm]`.
- Official deck counts (2026-06-15, DESSEM 22.0.2; `42-metodologia-hidreletricas.md` section 15 and
  `12-entradas-hidraulicas-b.md`): `indice.csv` activates `HIDRELETRICA-CADASTRO-RESERVATORIO-CURVAJUSANTE;
  pol;polinjus.csv`; `polinjus.csv` covers 211 plants (169 of the case + 42 outside it), 425 curves, 976
  segments; curves per plant 1 (141), 2 (12), 3 (12), 4 (6), 5 (40), so 70 plants have 2 to 5 families; 1 to
  4 segments per curve; 54 plants with `AFOGAMENTO-EXPLICITO-USINA;...;sim` (all `sim`). The 2024-12-31
  deck had 424 curves, 976 segments and 54 drowning cards; `polinjus.csv` first appears in that deck
  (`61-evolucao-decks-oficiais.md`).
- Families of the four 24Barras plants in an official deck `[to confirm]`: the skill references quote only
  curve 001 of plant 0001 (2 segments, Href 885.3052) and give no per-plant curve count; the 032026 full case
  (DESSEM 22.0.2) `PDO_ECO_USIH_POLIN` shows for CAMARGOS only curve 1 non-zero with `Hjus1 = 0.00` on the
  `hidr.dat`/`AC` side (`23-saidas-diagnostico-b.md` §4.4), which says nothing about the `polinjus.csv`
  curves that prevail. Whether CAMARGOS, ITUTINGA, FUNIL-GRANDE or FURNAS has more than one family, and which
  of them has an explicit drowning card, needs the `polinjus.csv` of an ONS deck.

## Options

### (a)

(a) keep one degree-4 polynomial per plant and document the limitation

- Gains: no input, reader, model or output change; section 9 stays bit-identical (objective 2420590.3195 R$);
  computed FPHA and EngMax keep reading one row through one reader, so the two consumers cannot disagree;
  no one-way-door schema decision is taken while the MM/Libs conflict is open; on 24Barras the turbined axis
  of CAMARGOS (0-214 m3/s) lies inside segment 1 of its official curve 001 (0-408.649 m3/s).
- Losses: backwater (§9.16.4), piecewise ranges (9.24) and explicit drowning (§9.16.5) stay unmodelled; a
  plant with several segments or families must be reduced by hand to one polynomial (the CAMARGOS row is
  segment 1 only, so the polynomial is extrapolated above 408.649 m3/s, which the secant spillage axis
  reaches); the divergence size for the 70 official plants with 2 to 5 families is unknown.
- Risks: if `TailraceSource::Families` is kept, a public variant that returns a 0 m tailrace stays reachable
  by any future caller (ADR-027 forbids that default); removing it (S1) is a public-API change of
  `labdessem-fpha` (re-exported from `lib.rs`), small in practice: one variant and one match arm, no
  constructor, consumers inside the workspace only (`labdessem-simulation` constructs `Entity` only).
- Input schema: unchanged, `codigo_usina;a0.m;a1.m/m3s;a2.m/m3s2;a3.m/m3s3;a4.m/m3s4`, one row per plant,
  the ticket-024 duplicate-row rule unchanged.
- EngMax coupling: unchanged; `plant_geometry` and `build_fit_params` keep the same `[f64; 5]`
  (`HydroPlant::tailrace_coefficients` and `TailraceData`).
- Tickets affected: ticket-029 documents the single polynomial, the limitation, the MM/Libs conflict and the
  `[to confirm]` items; the removal of `TailraceSource::Families` (S1) becomes an implementation ticket
  through the orchestrator; no other ticket changes.

### (b)

(b) polynomial families selected by the downstream level (backwater, MM 9.16.4)

- Gains: follows MM 22.3.0 §9.16.4 (family per downstream reference level, interpolation between
  neighbouring families) and gives `TailraceSource::Families` a real implementation; can carry the
  `polinjus.csv` families of the 70 official plants with 2 to 5 curves.
- Losses: new input schema and a new physical input, the downstream level at fit time; on 24Barras the
  downstream level is fixed for CAMARGOS and ITUTINGA (FD downstream plants with Vmin = Vmax), exogenous for
  FURNAS (no downstream plant in the case) and varying only for FUNIL-GRANDE, so the gain on the example is
  confined to one plant; high complexity (master plan).
- Risks: the MM/Libs conflict (Libs §15.3.4: backwater "desconsiderado, por padrao" with piecewise
  polynomials, the form the official decks have used since 2024-12-31) means (b) may model a mechanism
  official DESSEM does not apply by default `[to confirm]`; the FPHA row (9.33) has no downstream-volume
  term, so the family must be fixed or interpolated at one downstream level per fit, a rule the sources do
  not state `[to confirm]` (initial volume of the downstream plant, its window midpoint, or a per-period
  refit); the result depends on the fit order of the cascade; a family selected for FPHA but not for EngMax
  gives two tailrace levels for the same plant.
- Input schema: `CAD_TAILRACE.csv` keyed by `codigo_usina;curva` with a `Href.m` column (downstream reference
  level, strictly increasing per plant) before `a0..a4` (and the segment columns of (c) if piecewise families
  are wanted); FURNAS-like plants need an exogenous downstream level column or file; the ticket-024 rule
  becomes "duplicate (plant, curve)" plus an Href-order check and a curve-count limit (5 in `polinjus.csv`,
  6 in `hidr.dat`).
- EngMax coupling: `HydroPlant::tailrace_coefficients: Option<[f64; 5]>` cannot hold families, so the field
  type changes (24 occurrences, `labdessem-core`, `labdessem-io`, `labdessem-model`, `labdessem-simulation`);
  EngMax's `plant_geometry` must adopt the same family selection, or a family schema read only by computed
  FPHA creates a new asymmetry between the two consumers.
- Tickets affected: one or more implementation tickets across `labdessem-io` (reader), `labdessem-core`
  (entity field), `labdessem-fpha` (`TailraceSource::Families`, `TailraceModel`), `labdessem-cli`
  (`build_fit_params`) and `labdessem-simulation` (EngMax), with new example data; ticket-029 documents the
  families and the downstream-level rule.

### (c)

(c) piecewise polynomials by flow range without backwater

- Gains: follows MM 22.3.0 eq. 9.24 and the single-curve structure of 141 of the 211 official plants (1 to 4
  segments per curve), and agrees with the Libs §15.3.4 reading that backwater is off by default with
  piecewise polynomials; CAMARGOS could carry its whole official curve 001 (2 segments) instead of segment 1;
  one shared `TailraceModel` variant serves both consumers; medium complexity (master plan).
- Losses: backwater stays unmodelled; for the 70 official plants with 2 to 5 families one curve must still be
  chosen by hand; explicit drowning stays unmodelled; on 24Barras the effect is limited to outflows above
  segment 1 (CAMARGOS: only the spillage axis above 408.649 m3/s) `[not measured]`.
- Risks: the continuity of 9.24 must be checked or enforced at the breakpoints (a jump changes the net head
  discontinuously and the hull planes near it); the open last segment (`Qmax` 1e12 in the deck) needs a
  sentinel rule; a schema keyed by segment without a curve column must be extended again if (b) is adopted
  later; under the MM reading (§9.16.4) the deck's families are interpolated by DESSEM, so (c) still diverges
  for those plants.
- Input schema: `CAD_TAILRACE.csv` keyed by `codigo_usina;segmento` with `QjusMin.m3s;QjusMax.m3s` before
  `a0..a4`; the ticket-024 rule becomes "duplicate (plant, segment)" plus first `QjusMin` = 0, adjacent ranges
  (`QjusMax` of k = `QjusMin` of k+1), at most 5 segments, last segment open; a one-row-per-plant file stays
  valid as one segment on `[0, inf)`.
- EngMax coupling: a `TailraceModel::Piecewise` variant in `labdessem-fpha` evaluated by both
  `ProductionFunction` users; `HydroPlant::tailrace_coefficients` changes type (same 24 occurrences as (b));
  EngMax adopts the segments in the same ticket, so both consumers see one tailrace.
- Tickets affected: one implementation ticket across `labdessem-io`, `labdessem-core`, `labdessem-fpha`,
  `labdessem-cli` and `labdessem-simulation`; `TailraceSource::Families` is still removed (S1) because (c) has
  no families; ticket-029 documents the segments and the absence of backwater.

## Recommendation

(a), preliminary as in the master plan, with the limitation recorded and the Families placeholder removed.
No effect of (b) or (c) can be measured without the official family data of the four plants, which neither
the repository nor this machine has; the only official curve quoted by the sources (CAMARGOS curve 001) shows
the 24Barras row is its segment 1 and that the turbined axis stays inside it. (b) depends on a downstream-level
rule the sources do not state and on the open MM/Libs conflict; (c) is the better-founded extension if one is
wanted (it matches eq. 9.24, 141 of 211 official plants and the Libs default reading) but its schema is a
one-way door best decided once the conflict is answered. Rework trigger: a deck whose `polinjus.csv` gives the
four plants more than one segment in the fitted range, or more than one family, or a CEPEL answer on Libs
§15.3.4, reopens (c) first and then (b).

S1: Yes. The Current State grep, re-run at `9942497`, prints only the match arm in `production.rs`, so
`TailraceSource::Families` is never constructed and its 0 m tailrace is the silent default ADR-027 forbids.
Recommended: an implementation ticket removes the variant and its match arm (public-API change of
`labdessem-fpha`, re-exported from `lib.rs`; workspace-internal consumers only; no constructor to migrate;
no behaviour change for any run). Alternative: keep it and turn its arm into a typed error, which keeps an
unused public variant. The `Entity(None)` arm also returns 0 m; whether that implementation ticket makes the
tailrace non-optional is a scope question for the caller, not part of this decision.

S2: Under (b): `CAD_TAILRACE.csv` keyed by `codigo_usina;curva`, `Href.m` (downstream reference level,
strictly increasing per plant), then `a0..a4` (plus the segment columns of (c) for piecewise families), an
exogenous downstream-level input for plants whose downstream reservoir is outside the case, at most 5 curves;
the ticket-024 rule becomes "duplicate (plant, curve)". Under (c): keyed by `codigo_usina;segmento`,
`QjusMin.m3s;QjusMax.m3s`, then `a0..a4`, at most 5 segments, first minimum 0, adjacent ranges, open last
segment; the ticket-024 rule becomes "duplicate (plant, segment)"; a single-row plant stays valid. Under both,
EngMax adopts the new model in the same ticket through a shared `labdessem-fpha` type and a retyped
`HydroPlant::tailrace_coefficients`, so computed FPHA and EngMax never read different tailraces.

S3: Under (a), the S1 removal is an [ADD] implementation ticket in this plan, before ticket-029 (one file
in `labdessem-fpha`, no behaviour change); ticket-029 documents the single polynomial, the unmodelled
backwater, piecewise ranges and drowning, the MM/Libs conflict, the `[to confirm]` items and the rework
trigger. An accepted (b) or (c) goes to a follow-up plan (multi-crate input-schema change, one-way door, needs
official family data for its tests), and ticket-029 then documents the decision and points to that plan.

## Decision

Accepted: option (a) — computed FPHA and EngMax keep one degree-4 tailrace polynomial per plant in the plant
outflow (`CAD_TAILRACE.csv`, one row per plant, `read_tailrace` and its ticket-024 duplicate-row rule,
`TailraceModel::Polynomial` built by `build_fit_params`, `plant.tailrace_coefficients` in EngMax). The
divergence from DESSEM (backwater families, MM §9.16.4; piecewise polynomials by flow range, MM eq. 9.24;
explicit drowning, MM §9.16.5 and Libs §15.3.3) is confirmed and recorded as a limitation, not removed. On
24Barras the fit and the 2420590.3195 R$ objective of section 9 do not change, since this decision changes
no tailrace input or model.

Sub-question S1 (the `TailraceSource::Families` placeholder): accepted as recommended. An implementation
ticket removes the variant and its match arm in `crates/labdessem-fpha/src/fitting/production.rs`; this is a
public-API change of `labdessem-fpha` (re-exported from `lib.rs`) with no constructor to migrate and no
behaviour change for any run, and it ends the silent 0 m tailrace path that ADR-027 forbids. Whether that
ticket also covers the `Entity(None)` arm (which also returns 0 m), i.e. makes the tailrace non-optional, is
decided in that ticket's refinement, not here.

Sub-question S2 (schema and EngMax under (b)/(c)): moot under (a); no schema change. Recorded only as the
conditions of a later reopening of (b) or (c): under (b) `CAD_TAILRACE.csv` keyed by `codigo_usina;curva`
with `Href.m` strictly increasing per plant, an exogenous downstream-level input for plants whose downstream
reservoir is outside the case, at most 5 curves, and the ticket-024 rule becoming "duplicate (plant, curve)";
under (c) keyed by `codigo_usina;segmento` with `QjusMin.m3s;QjusMax.m3s`, at most 5 segments, first minimum
0, adjacent ranges, open last segment, a single-row plant still valid, and the ticket-024 rule becoming
"duplicate (plant, segment)"; under both, EngMax adopts the new model in the same ticket through a shared
`labdessem-fpha` type, so computed FPHA and EngMax never read different tailraces.

Sub-question S3 (delivery): accepted as recommended. The S1 removal is an [ADD] implementation ticket in
this plan, to be added before ticket-029 (by the orchestrator, not by this ticket). Ticket-029 documents the
single polynomial, the unmodelled mechanisms (backwater families, piecewise ranges, drowning), the MM/Libs
conflict on which mechanism is off by default with piecewise polynomials, the `[to confirm]` items and the
rework trigger.

Chosen by the user on 2026-11-19 at the decision gate, matching the recommendation. The rework trigger stays
recorded: a deck whose `polinjus.csv` gives the four 24Barras plants more than one segment in the fitted
range or more than one family, or a CEPEL answer on Libs §15.3.4, reopens (c) first and then (b), with the
S2 conditions above. The families of the four plants in an official deck, the MM/Libs conflict and the
downstream level at fit time stay `[to confirm]`; any effect of (b) or (c) stays `[not measured]` for lack of
official family data.

## Consequences

Option (a) was chosen (see Decision); the other branches are kept for the record.

- Under (a) (chosen): no change to `CAD_TAILRACE.csv`, `read_tailrace`, `TailraceModel` or EngMax, and the
  section 9 baseline stays current. S1 accepted: an [ADD] implementation ticket, to be added by the
  orchestrator before ticket-029, removes `TailraceSource::Families` and its match arm (public-API change of
  `labdessem-fpha`, no behaviour change); its refinement decides whether `Entity(None)` is covered too.
  Ticket-029 documents the single polynomial, the unmodelled mechanisms, the MM/Libs conflict, the
  `[to confirm]` items and the rework trigger.
- Under (b) (not chosen): a follow-up plan would specify the family schema, the downstream-level rule and the
  EngMax adoption per the S2 conditions; the `[to confirm]` items (MM/Libs conflict, downstream level at fit
  time, families of the four plants) must be answered first. Live again only through the rework trigger.
- Under (c) (not chosen): a follow-up plan would specify the segment schema and the EngMax adoption per the S2
  conditions; S1 still removes `TailraceSource::Families`. Live again only through the rework trigger.
- Under any option: the `[to confirm]` items need the `polinjus.csv` of an ONS deck, the MM/Libs PDFs or a
  CEPEL answer.
