---
requirements:
  - id: R1
    text: "In computed mode the LP applies the correction factor alpha exactly once to all four FPHA plane coefficients (gamma_0, gamma_V, gamma_Q, gamma_S)."
    kind: functional
    source: spec
  - id: R2
    text: "The fit-deviation diagnostic is computed on the same planes and correction factor that the LP uses."
    kind: functional
    source: spec
  - id: R3
    text: "No hydro plant with generating units reaches the solver with FPHA enabled and zero FPHA segments; the run fails (or applies the ADR-026 policy) with an explicit message naming the plant and the reason."
    kind: functional
    source: spec
  - id: R4
    text: "Computed FPHA never substitutes a 0 m tailrace, an efficiency of 1 or a zero hydraulic loss for missing input data without the explicit outcome defined by ADR-027."
    kind: functional
    source: spec
  - id: R5
    text: "CAD_FPHA_CONFIG.csv fields take effect or the file is removed from the documented inputs; fitting defaults are explicit and documented."
    kind: functional
    source: spec
  - id: R6
    text: "max_planes is enforced or removed, and the MILP plane-selection question (NFHAMILP) is decided."
    kind: functional
    source: spec
  - id: R7
    text: "Methodology divergences B1-B5 are confirmed against the DESSEM methodology with citations and recorded as decisions, without judging ONS operational choices."
    kind: constraint
    source: spec
  - id: R8
    text: "A1-A3 are reproduced quantitatively before any fix, on derived copies of examples/24Barras, leaving the official example files and defaults unchanged."
    kind: nonfunctional
    source: spec
  - id: R9
    text: "Any change to the resultado_fpha_computed.csv contract is documented, and OPER_FPHA.csv stays compatible: the documented copy of the computed planes reproduces the computed run."
    kind: constraint
    source: spec
  - id: R10
    text: "Project conventions hold: typed errors, no unwrap/expect/panic shortcuts in production code, no removed features or weakened tests, workspace clippy lints, English Conventional Commits, work on fix/fpha-corrections from develop without merge or push."
    kind: constraint
    source: spec
  - id: R11
    text: "FPHA documentation describes the implemented behavior and the accepted decisions (audit FP-11)."
    kind: nonfunctional
    source: spec
  - id: R12
    text: "FPHA input readers reject duplicate plant rows and non-finite values, and precomputed plants without units generate no FPHA rows (audit FP-R01, EM-04, FP-07)."
    kind: functional
    source: inferred
decisions:
  - id: ADR-025
    title: "Apply the FPHA correction factor alpha exactly once in computed mode (A1, audit FP-02/FP-08/D2)"
    status: proposed
    requirements: [R1, R2, R9]
    options:
      - name: "(a1) correction_factor = 1.0 in convert_to_fpha_segments; planes stay alpha-scaled"
        complexity: low
        durability: "gamma_S stays without alpha (FP-08 persists); CSV planes are pre-multiplied, so an OPER_FPHA copy needs Fcorrec = 1, unlike the Libs/OPER_FPHA convention"
      - name: "(a2) correction_factor = 1.0 and scale gamma_S by alpha after the secant fit"
        complexity: low
        durability: "alpha reaches every coefficient once, but CSV planes stay pre-multiplied and the alpha column becomes informational; an OPER_FPHA copy needs Fcorrec = 1"
      - name: "(b) remove scale_planes from fit_plant and keep correction_factor = alpha"
        complexity: medium
        durability: "matches the Libs form and the OPER_FPHA Fcorrec convention; changes resultado_fpha_computed.csv semantics (BREAKING) and can change the plane-reduction outcome"
    recommended: "(b) remove scale_planes from fit_plant and keep correction_factor = alpha"
    reversibility: two-way
    rework_trigger: "if ADR-033 moves the alpha computation after plane reduction, the fit_plant stage order is revisited by the ADR-033 follow-up ticket"
  - id: ADR-026
    title: "Policy for plants with generating units whose computed FPHA cannot be fitted (A2, audit HI-03/FP-01/NU-04)"
    status: proposed
    requirements: [R3, R10]
    options:
      - name: "(a) re-validate the System after FPHA injection only"
        complexity: low
        durability: "the run fails with the generic 'must contain at least one FPHA segment' message; the skip reason is lost"
      - name: "(c) compute_fpha returns a typed error listing every plant with units it could not fit"
        complexity: medium
        durability: "operator-facing failure with reasons; no structural guard against future injection paths"
      - name: "(a)+(c) typed error in compute_fpha plus post-injection re-validation"
        complexity: medium
        durability: "two checks to maintain; covers both the reported reason and future injection paths"
      - name: "(b) per-plant constant-productivity fallback with a warning"
        complexity: high
        durability: "needs a per-plant ConstraintSet switch and mixes physics per plant; the audit does not recommend it"
      - name: "(d) study_config.json policy key (fail | fallback)"
        complexity: high
        durability: "adds configuration surface and keeps a fallback path alive"
    recommended: "(a)+(c) typed error in compute_fpha plus post-injection re-validation; plants without units are excluded from fitting"
    reversibility: two-way
    rework_trigger: "none known"
  - id: ADR-027
    title: "Policy for missing tailrace, efficiency or hydraulic-loss data in computed FPHA (A3, audit FP-R02)"
    status: proposed
    requirements: [R4, R10]
    options:
      - name: "(a) fail-fast typed error naming the plant and every missing input"
        complexity: low
        durability: "decks without a tailrace row or the Eficiencia/Perda.m columns stop running in computed mode (behavior change)"
      - name: "(b) keep the defaults and emit a warning"
        complexity: low
        durability: "generation stays physically inflated (FURNAS FPH about +9 %, audit FP-R02); only visibility improves"
      - name: "(c) explicit configurable defaults per plant"
        complexity: medium
        durability: "new configuration surface; defaults become auditable but remain synthetic"
      - name: "(d) error for a missing tailrace, warning for efficiency/loss defaults"
        complexity: low
        durability: "mixed policy; efficiency = 1 still inflates generation"
    recommended: "(a) fail-fast typed error, scoped to computed FPHA fitting (EngMax untouched)"
    reversibility: two-way
    rework_trigger: "if EngMax later adopts the same policy (out of scope here), ProductionFunction::new defaults are revisited"
  - id: ADR-028
    title: "CAD_FPHA_CONFIG.csv semantics, fitting defaults and fit volume domain (A4 + B2, audit FP-05/D13)"
    status: proposed
    requirements: [R5, R7]
    options:
      - name: "(a) implement every field with DESSEM MM defaults (window 0-100 % of Vmin..Vmax, NPTV = NPTQ = 5, explicit even NPTV + 1)"
        complexity: medium
        durability: "fitted planes change for every computed run; regression baselines are re-recorded"
      - name: "(b) implement every field and keep the current defaults (full VHA range, 6 x 6 grid)"
        complexity: low
        durability: "config becomes effective; the defaults keep diverging from the MM"
      - name: "(c) remove CAD_FPHA_CONFIG.csv from inputs and documentation; keep hard-coded defaults"
        complexity: low
        durability: "no per-plant control; the fit domain divergence (B2) is documented as a limitation"
    recommended: "(a), preliminary; the ticket-018 brief confirms or replaces it with citations"
    reversibility: two-way
    rework_trigger: "a later default change re-records the ticket-017 regression baselines"
  - id: ADR-029
    title: "max_planes enforcement, plane-reduction criterion and MILP plane selection (A5 + angle-only reduction)"
    status: proposed
    requirements: [R6, R7]
    options:
      - name: "(a) enforce max_planes by iterative reduction and keep every plane in every solve stage"
        complexity: medium
        durability: "MILP stage keeps the full plane set; no NFHAMILP selection"
      - name: "(b) option (a) plus centroid selection of NFHAMILP planes for the MILP stage (MM 27.2)"
        complexity: high
        durability: "touches model assembly per solve stage; needs stage-aware constraint sets"
      - name: "(c) remove max_planes and num_planes; document the current angle reduction"
        complexity: low
        durability: "no plane-count control; the reduction divergence stays documented"
    recommended: "(a), preliminary; (b) only if the ticket-020 brief shows MILP runtime evidence"
    reversibility: two-way
    rework_trigger: "if MILP solve time on official decks becomes binding, option (b) is reopened"
  - id: ADR-030
    title: "Volume used in the FPHA rows: end-of-period V[t+1] or period mean (B1, audit FP-06/D9)"
    status: proposed
    requirements: [R7]
    options:
      - name: "(a) keep the end-of-period volume V[t+1] (MM eq. 9.33)"
        complexity: low
        durability: "diverges from the Libs form and from the EngMax head volume (ADR-006)"
      - name: "(b) use the period-mean volume (V[t] + V[t+1]) / 2 (Libs form)"
        complexity: medium
        durability: "changes the FPHA rows, GeracaoMaxPontoMW and the precomputed semantics"
    recommended: "(b) per the audit reviewer, pending the ticket-025 brief"
    reversibility: two-way
    rework_trigger: "an implementation ticket is added at Epic 4 refinement when (b) is accepted"
  - id: ADR-031
    title: "Turbining axis bound and Pmax cap of the fit cloud (B3)"
    status: proposed
    requirements: [R7]
    options:
      - name: "(a) keep the sum of unit maximum turbining and the Pmax-capped cloud"
        complexity: low
        durability: "diverges from MM 9.17.1 (swallowing from 0 to the maximum)"
      - name: "(b) bound the Q axis by the maximum swallowing (labdessem_fpha::calculate_max_swallowing)"
        complexity: medium
        durability: "couples the fit to the EngMax inputs (h_ef, Q_ef, turbine type)"
      - name: "(c) option (b) and drop the Pmax cap from the cloud, leaving the cap to unit bounds"
        complexity: medium
        durability: "changes the envelope near Pmax for every plant"
    recommended: "(b), preliminary, pending the ticket-026 brief"
    reversibility: two-way
    rework_trigger: "an implementation ticket is added at Epic 4 refinement when (b) or (c) is accepted"
  - id: ADR-032
    title: "Tailrace modelling: single degree-4 polynomial or polynomial families (B4)"
    status: proposed
    requirements: [R7]
    options:
      - name: "(a) keep one degree-4 polynomial per plant and document the limitation"
        complexity: low
        durability: "backwater and piecewise ranges stay unmodelled"
      - name: "(b) polynomial families selected by the downstream level (backwater, MM 9.16.4)"
        complexity: high
        durability: "new input schema and TailraceSource::Families implementation"
      - name: "(c) piecewise polynomials by flow range without backwater"
        complexity: medium
        durability: "new input schema; backwater still unmodelled"
    recommended: "(a), preliminary, pending the ticket-027 brief"
    reversibility: two-way
    rework_trigger: "decks with backwater-dependent plants would reopen option (b)"
  - id: ADR-033
    title: "Points used to compute alpha: fit-grid nodes or separate evaluation points (B5)"
    status: proposed
    requirements: [R7, R1]
    options:
      - name: "(a) keep alpha on the fit-grid nodes, computed before plane reduction"
        complexity: low
        durability: "diverges from the MM default of 30 evaluation points"
      - name: "(b) alpha on separate evaluation points (MM default 30), computed on the final reduced planes"
        complexity: medium
        durability: "changes alpha for every plant; the fit_plant stage order changes"
    recommended: "(b), preliminary, pending the ticket-028 brief"
    reversibility: two-way
    rework_trigger: "an implementation ticket is added at Epic 4 refinement when (b) is accepted"
---

# Master Plan: FPHA Corrections

## Executive Summary

The computed FPHA path of labdessem has three confirmed correctness defects (alpha applied twice,
plants without planes generating without water, silent physical defaults) plus two configuration
defects and five methodology divergences against DESSEM. This plan reproduces A1-A3 quantitatively on
derived copies of `examples/24Barras`, gates every open choice behind an explicit decision record
(ADR-025 to ADR-033), fixes A1-A3 with regression tests, and then decides and implements the
configuration, selection, validation and documentation items.

**Implementation mode: Rigoroso** (English code, comments, documents and commit messages). Output
strings written to CSV preambles and CLI messages follow the existing project convention (ASCII
Portuguese in CSV preambles, English in error messages).

## Goals & Non-Goals

### Goals

1. Reproduce A1 (double alpha), A2 (generation without water) and A3 (silent 0 m tailrace,
   efficiency 1, loss 0) before any fix, with tests that anyone can rerun (R8).
2. Fix A1-A3 under accepted decisions ADR-025, ADR-026, ADR-027 with regression tests (R1-R4).
3. Keep `OPER_FPHA.csv` compatible and document the `resultado_fpha_computed.csv` contract (R9).
4. Decide and implement `CAD_FPHA_CONFIG.csv` semantics (A4) and `max_planes` / MILP selection (A5)
   (R5, R6).
5. Confirm B1-B5 against the DESSEM methodology with citations and record the decisions (R7).
6. Bring the FPHA documentation in line with the behavior (FP-11, R11).

### Non-Goals

- Judging ONS operational choices (deck parameters, official run settings).
- EngMax behavior: EM-01 (computed + UCT + EngMax infeasibility), EM-02, EM-03 and the EngMax use of
  `ProductionFunction` defaults stay as they are; FP-09 (`GeracaoMaxPontoMW` ignores EngMax) is not
  reopened.
- HI-05/FP-03 and HI-04/FP-04 (audit decision D3): referenced, not reopened.
- FP-10 (tracked under NU-10) and the FP-12 MLT day-weighting (effect below 0.5 % in the example).
- Implementing accepted B1-B5 changes: Epic 4 records the decisions; code changes for accepted
  options are added at Epic 4 refinement ([ADD]) or in a follow-up plan.
- Changing `examples/24Barras` files or its `study_config.json` defaults.

## Architecture Overview

### Current State

- `crates/labdessem-fpha/src/pipeline.rs` `fit_plant`: hull fit, then `compute_alpha_fpha`, then
  `scale_planes(&raw_planes, alpha)` (first alpha), gamma_S secant fit on the scaled planes (gamma_S
  without alpha), angle reduction, deviation on the scaled planes, validation.
- `crates/labdessem-cli/src/fpha_integration.rs` `convert_to_fpha_segments` sets
  `correction_factor: result.alpha`; `crates/labdessem-model/src/constraints/hydro.rs`
  `build_hydro_fpha_constraints` multiplies every coefficient and the RHS by `correction_factor`
  (second alpha). The LP envelope is alpha squared times FPHA_0 for gamma_0..gamma_Q.
- `compute_fpha` skips a plant with units when its geometry is missing, its parameters fail or its fit
  fails, prints `Warning:` with `eprintln!`, and returns `Ok`. `main.rs` sets `fpha_enabled = true`
  without re-validating; the plant keeps zero segments, zero FPHA rows and zero coupling rows.
- `build_fit_params` passes `tailrace: None`, `efficiency: None`, `hydraulic_loss_m: None` through;
  `ProductionFunction::new` turns them into 0 m, 1.0 and 0 m.
- `build_fpha_config` hard-codes `FphaPlantConfig::computed(10, 6, 6, Angle 1.0)` and copies only
  `num_planes` into `max_planes`, which the pipeline never reads.
- The qhull submodule `crates/labdessem-fpha/vendor/qhull` is not initialized; `build.rs` panics.

### Target State

- One alpha in the LP for every coefficient, a deviation diagnostic on that same envelope, and a
  `resultado_fpha_computed.csv` whose documented copy into `OPER_FPHA.csv` reproduces the computed run.
- A computed run never reaches the solver with a generating plant lacking FPHA rows; the failure names
  every affected plant and the reason.
- Missing tailrace, efficiency or loss data in computed fitting produce the ADR-027 outcome.
- Configuration, plane count, MILP selection and methodology choices recorded in ADR-028 to ADR-033
  and reflected in the design and user documentation.

### Key Design Decisions

The frontmatter is the source of truth. Every decision is `proposed`; a decision ticket executes the
gate (see "Decision Gate Protocol").

- **ADR-025 (A1)**: recommended (b) — drop `scale_planes` from `fit_plant`, keep
  `correction_factor = alpha`. Alpha then reaches gamma_S once (FP-08), the deviation is computed on
  `scale_planes(&planes, alpha)`, and the CSV planes become the unscaled FPHA_0 planes.
- **ADR-026 (A2)**: recommended (a)+(c) — `compute_fpha` returns a typed error listing every plant
  with units it could not fit, and the System is re-validated after injection. Plants without units
  are excluded from fitting (resolves the FP-12 `MissingHydro` case for them and the computed-mode
  analogue of FP-07).
- **ADR-027 (A3)**: recommended (a) — fail fast in `build_fit_params` when the tailrace row,
  `Eficiencia` or `Perda.m` is missing; an explicit 0 m loss stays valid; EngMax is out of scope.
- **ADR-028 (A4 + B2)**: preliminary (a) — honor every field with MM defaults; the fit volume window
  and the B2 fit-domain question are the same parameter, so B2 is decided here.
- **ADR-029 (A5 + reduction)**: preliminary (a) — enforce `max_planes`; the angle-only reduction
  criterion is part of the same choice.
- **ADR-030 to ADR-033 (B1, B3, B4, B5)**: study tickets with dessem-specialist citations.

Planning choices (not product decisions; listed so the reviewer can veto them):

- **P1 Evidence first.** Epic 1 commits characterization tests named `baseline_*` that pass on the
  current code and assert the defect signature exactly; the Epic 2 fix ticket for each finding
  replaces them with the correct-behavior assertion. The branch is green at every ticket.
- **P2 Decision tickets.** Each gate is a ticket whose agent writes a brief and ends with a
  `CLARIFICATION REQUIRED:` payload; dependent tickets list it in Blocked By.
- **P3 [ASSUMPTION] Brief location.** Briefs are committed under `docs/auditoria/fpha-decisions/`
  (next to the audit that raised the findings), because `plans/` is gitignored. Alternative not
  chosen: plan-directory-only briefs, matching ADR-001 to ADR-024, which have no committed files.
- **P4 Derived cases.** Tests copy `examples/24Barras` into a `tempfile::TempDir` and patch the copy;
  the official example is never written.
- **P5 Test placement.** `labdessem-cli` is a bin-only crate, so in-process tests that need
  `compute_fpha` live in `#[cfg(test)]` child modules of `fpha_integration`
  (`crates/labdessem-cli/src/fpha_integration/*.rs`); end-to-end tests live in one new integration
  test binary, `crates/labdessem-cli/tests/fpha_e2e.rs`.

## Technical Approach

### Tech Stack

Rust workspace (edition per `Cargo.toml`), qhull (vendored C, FFI in `labdessem-fpha`), HiGHS through
`labdessem-solver`, `tempfile` 3 for test temp directories, CSV inputs under `CAD/` and `OPER/`.

### Specialist Chain

```
dessem-specialist (methodology briefs with citations: ADR-025..ADR-033)
  -> hpc-rust-developer (pipeline, CLI integration, readers, model)
  -> monorepo-test-developer (probe, characterization, E2E, regression evidence)
  -> open-source-documentation-writer (FP-11 documentation)
```

No solver-integration link: LP rows change only through `labdessem-model` constraint builders.
`monorepo-test-developer` also opens the chain in Epic 1, where the evidence precedes any fix.

### Component/Module Breakdown

| Component | Files | Epics |
| --- | --- | --- |
| Fit pipeline | `crates/labdessem-fpha/src/pipeline.rs`, `src/fitting/*.rs`, `src/config.rs` | 2, 3 |
| CLI integration | `crates/labdessem-cli/src/fpha_integration.rs`, `src/main.rs` | 2, 3 |
| Test harness | `crates/labdessem-cli/src/fpha_integration/*.rs`, `crates/labdessem-cli/tests/fpha_e2e.rs` | 1, 2 |
| Readers | `crates/labdessem-io/src/readers/fpha_geometry.rs`, `src/readers/hydro.rs` | 3 |
| Output contract | `crates/labdessem-io/src/output/fpha_computed.rs` | 2 |
| Model | `crates/labdessem-model/src/constraints/hydro.rs`, `constraint_set.rs` | 3 |
| Evidence and decisions | `docs/auditoria/fpha-baseline-reproduction.md`, `docs/auditoria/fpha-decisions/*.md` | 1-4 |
| Documentation | `docs/design/fpha-formulation.md`, `crates/labdessem-fpha/README.md`, `docs/*.html`, `README.md`, `ARCHITECTURE.md` | 4 |

### Data Flow

```
study_config.json (FPHA = computed)
  -> labdessem_io::read_study_from_path -> System (fpha_enabled = false)
  -> fpha_integration::compute_fpha (CAD_GEOMETRIA_UHE, CAD_TAILRACE, CAD_FPHA_CONFIG, MLT)
       -> labdessem_fpha::fit_plant per plant -> FphaPlantResult {planes, alpha, deviation}
  -> inject_fpha_segments (correction_factor = alpha) -> fpha_enabled = true
  -> write_computed_results -> RESULTADOS/resultado_fpha_computed.csv
  -> labdessem_model::Model (hydro_fpha rows: g - cf*gq/fc*q - cf*gv*V[t+1] - cf*gs/fc*S <= cf*g0)
  -> solver
```

### Output Contracts

- **`resultado_fpha_computed.csv`** (written to `<case>/RESULTADOS/`). Column names, order and number
  formats do not change. Under ADR-025 (b) the meaning changes (BREAKING, recorded in `CHANGELOG.md`
  by ticket-012):
  - `gamma_0`, `gamma_v`, `gamma_q`, `gamma_s` hold the unscaled fitted planes (FPHA_0);
  - `alpha` is the factor the LP applies once to all four coefficients;
  - the deviation columns describe alpha x FPHA_0 against the FPH on the fit grid.
- **`OPER_FPHA.csv`**: header, reader and semantics do not change. Documented copy: `USIH` =
  `codigo_usina`, `SegFPHA` = `plane_index` + 1, `Fcorrec` = `alpha`, `Rhs.mw` = `gamma_0`,
  `Varm.mw/hm3` = `gamma_v`, `Qtur.mw/m3s` = `gamma_q`, `Qlat.mw/m3s` = `gamma_s`. Ticket-013 proves
  the copy reproduces the computed run.
- If ADR-025 accepts (a1) or (a2) instead, the planes stay pre-multiplied, the copy needs
  `Fcorrec = 1`, and tickets 011-013 are re-refined before dispatch.

### Testing Strategy

- **Unit (labdessem-fpha)**: stage-order and deviation tests in `src/pipeline.rs`; bit-determinism
  tests in `tests/regression.rs` keep their assertions.
- **In-process (labdessem-cli, `#[cfg(test)]`)**: the probe builds derived cases, runs
  `compute_fpha` and `inject_fpha_segments`, builds `labdessem_model::Model`, reads the
  `hydro_fpha[...]` rows and evaluates the implied envelope at the fit-grid nodes and at an independent
  11 x 11 grid. No solver call.
- **End-to-end**: `crates/labdessem-cli/tests/fpha_e2e.rs` runs `CARGO_BIN_EXE_labdessem` on derived
  cases with the fast configuration (FPHA computed or precomputed, `rede` 0, `UCT` 0, `UCH` 0,
  `EngMax` 0, `FCF` 1, `opcao_execucao` 2). Tests run by default; no `#[ignore]`. If one debug run
  exceeds 120 s, the implementing agent stops and escalates instead of gating tests.
- **Tolerances**: LP envelope identities 1e-9 relative to max(1 MW, |value|); CSV round trip 5e-7
  absolute for 6-decimal columns and 5e-10 for 9-decimal columns; objective round trip 1e-5 relative
  (the A1 effect measured by the audit is about 7e-4 relative).
- **Gates at every ticket**: `cargo test --workspace` exits 0; `cargo clippy --workspace
  --all-targets` reports no more warnings than the baseline recorded by ticket-001.

## Decision Gate Protocol

1. A decision ticket's agent reads the evidence (baseline report sections, audit sections, code),
   writes `docs/auditoria/fpha-decisions/ADR-0NN-<slug>.md` with the sections Status, Context,
   Methodology reference, Options, Recommendation, Decision (`PENDING`), Consequences, and returns a
   `CLARIFICATION REQUIRED:` payload.
2. The orchestrator asks the user. After the choice it sets the decision to `status: accepted` with
   `chosen` in this frontmatter, fills the brief's Decision section, and commits that edit as
   `docs(audit): record ADR-0NN decision`.
3. Every implementation ticket that depends on the decision lists the decision ticket in Blocked By
   and carries a Pending Decisions note. If the accepted option differs from the option the ticket
   assumes, the ticket is re-refined before dispatch; the implementing agent never improvises.

## Git Workflow

- Work starts from `develop` (4ac43e0 at planning time) on `fix/fpha-corrections`, created by
  ticket-001. The plan never merges, pushes or opens a pull request.
- One commit per ticket, Conventional Commits with English descriptions, header at most 72
  characters, explicit `git add <path>` of the ticket's declared files only.
- The four unstaged deletions under `plans/dynamic-max-swallowing/` are left alone: never restored,
  staged or committed.

## Audit Reconciliation

| Audit ID | Finding | Treatment in this plan |
| --- | --- | --- |
| FP-02, D2 | alpha applied twice | A1: ticket-003, ADR-025 (ticket-008), ticket-011, ticket-012, ticket-013 |
| FP-08 | gamma_S fitted without alpha | A1 option (b): ticket-011 |
| FP-R03 | `convert_single_plane` locks FP-02 | replaced in ticket-012 |
| HI-03 = FP-01 = NU-04 | skipped plant generates without water | A2: ticket-005, ADR-026 (ticket-009), ticket-014, ticket-015 |
| FP-12 (`MissingHydro` for plants without units) | fatal error for non-generating plants | ticket-014 excludes plants without units |
| FP-12 (MLT simple mean) | day-weighting | non-goal (effect below 0.5 %) |
| FP-R02 | efficiency 1 and loss 0 silently | A3: ticket-006, ADR-027 (ticket-010), ticket-016 |
| (A3, not in audit) | missing tailrace becomes 0 m | same tickets as FP-R02 |
| FP-05, D13 | `CAD_FPHA_CONFIG.csv` ignored | A4: ADR-028 (ticket-018), ticket-019 |
| (A5, not in audit) | `max_planes` unused, no MILP selection | ADR-029 (ticket-020), ticket-021, ticket-022 |
| FP-07 | precomputed plant without units makes the LP infeasible | ticket-023 |
| FP-R01 | duplicate plant rows, last wins | ticket-024 |
| EM-04 | NaN accepted in geometry and tailrace | ticket-024 (reader validation, shared with EngMax inputs) |
| FP-06, D9 | FPHA volume V[t+1] | B1: ADR-030 (ticket-025) |
| FP-11 | stale documentation | ticket-029, ticket-030 |
| MX-06 | precomputed path never exercised | ticket-004, ticket-013 |
| FP-09, FP-10 | `GeracaoMaxPontoMW` vs EngMax; offsets | out of scope (NU-10 tracks FP-10) |
| FP-03 = HI-05, FP-04 = HI-04 | D3 | reference only |
| EM-01, EM-02, EM-03 | EngMax findings | reference; EM-01 motivates `EngMax` 0 in the fast configuration |

## Phases & Milestones

| Phase | Epic | Milestone | Points |
| --- | --- | --- | --- |
| 1 | epic-01-reproducible-baseline | Baseline evidence committed; ADR-025, ADR-026, ADR-027 accepted | 18 |
| 2 | epic-02-correctness-fixes | A1-A3 fixed with regression tests; post-fix evidence committed | 15 |
| 3 | epic-03-fitting-configuration | ADR-028, ADR-029 accepted and implemented; input validation hardened | 20 (outline) |
| 4 | epic-04-methodology-and-docs | ADR-030 to ADR-033 decided; FPHA documentation current | 12 (outline) |

## Risk Analysis

| Risk | Impact | Mitigation |
| --- | --- | --- |
| Debug-profile E2E solves are slow | E2E tests exceed a usable budget | ticket-001 measures the debug run; above 120 s the agent escalates (options: release-profile test runs, a `slow-tests` feature) |
| Removing `scale_planes` changes the angle-reduction outcome (plane normals are not scale-invariant) | plane counts differ after the fix | ticket-007 records pre-fix plane counts; ticket-017 compares them |
| A gate accepts a non-recommended option | Epic 2 tickets assume the recommendation | Pending Decisions notes; re-refinement before dispatch |
| CSV contract change breaks external consumers of `resultado_fpha_computed.csv` | wrong downstream reuse | BREAKING changelog entries, new preamble text, round-trip test |
| Fail-fast policies (ADR-026, ADR-027) stop decks that run today | operational interruption | policies are user decisions; messages name file, plant and input |
| qhull submodule fetch needs network access | ticket-001 blocked | ticket-001 escalates with the git error |
| Audit cache `~/.cache/labdessem-audit` is missing | audit tests cannot be reused | Epic 1 rebuilds the evidence from scratch |
| Methodology citations rely on the dessem skill reference | secondary source only | briefs tag such claims `[to confirm]` |

## Success Metrics

- A1: at every fit-grid node and at s in {0, Q_max/2, Q_max}, the LP envelope equals alpha x min_k
  P_k within 1e-9 relative, with P_k the unscaled fitted planes; the reported deviation equals the
  deviation of that envelope from the FPH.
- A2: no plant with units reaches the solver with FPHA enabled and zero segments; the derived run
  without FUNIL-GRANDE geometry exits 1 and its stderr names plant 2, FUNIL-GRANDE and
  `CAD/CAD_GEOMETRIA_UHE.csv`.
- A3: zero silent defaults: a missing tailrace row, `Eficiencia` or `Perda.m` in computed mode yields
  the ADR-027 outcome naming the plant and the input.
- Round trip: the precomputed run built from the documented copy matches the computed objective
  within 1e-5 relative.
- Every ticket ends with `cargo test --workspace` exiting 0 and no increase in clippy warnings.
