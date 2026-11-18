# FPHA Corrections Implementation Plan

**Implementation Mode**: Rigoroso (English identifiers, comments, documents and commit messages;
minimal comments; terse style). CSV preambles and CLI messages follow the existing project convention.

**Rigor tier**: full (`/plan` + `/implement-plan`). **Scaffolded**: no (established Rust workspace).

## Overview

The computed FPHA path of labdessem has three correctness defects confirmed by the model audit
(`docs/auditoria/2026-11-05-auditoria-modelo.md`): alpha applied twice (A1), plants without planes
generating without water (A2) and silent physical defaults (A3), plus two configuration defects (A4
`CAD_FPHA_CONFIG.csv` ignored, A5 `max_planes` unused) and five methodology divergences (B1-B5). This
plan reproduces A1-A3 on derived copies of `examples/24Barras` before any fix, puts every open choice
behind a decision gate (ADR-025 to ADR-033), fixes A1-A3 with regression tests, then decides and
implements configuration, selection, validation and documentation.

See `00-master-plan.md` for requirements (R1-R12), decisions, output contracts, testing strategy and
risks.

## Tech Stack

- **Language**: Rust (Cargo workspace: `labdessem-fpha`, `labdessem-cli`, `labdessem-io`,
  `labdessem-model`, `labdessem-core`, `labdessem-simulation`)
- **Native dependency**: qhull (vendored submodule `crates/labdessem-fpha/vendor/qhull`)
- **Solver**: HiGHS through `labdessem-solver`
- **Testing**: `cargo test` (unit, in-process `#[cfg(test)]` modules in `labdessem-cli`, one E2E test
  binary `crates/labdessem-cli/tests/fpha_e2e.rs`), `tempfile` for derived cases

## Specialist Chain

```
dessem-specialist (methodology briefs with citations: ADR-025..ADR-033)
  -> hpc-rust-developer (pipeline, CLI integration, readers, model)
  -> monorepo-test-developer (probe, characterization, E2E, regression evidence)
  -> open-source-documentation-writer (FP-11 documentation)
```

## Git Workflow

- Work starts from `develop` on the branch **`fix/fpha-corrections`** (convention `<type>/<kebab>`),
  created by ticket-001. The plan never merges, pushes or opens a pull request.
- One commit per ticket, Conventional Commits in English, explicit `git add <path>` of declared files.
- The unstaged deletions under `plans/dynamic-max-swallowing/` belong to another session: never
  restore, stage or commit them.

## Epics

| Epic | Name | Tickets | Points | Status |
| --- | --- | --- | --- | --- |
| 1 | epic-01-reproducible-baseline | 10 | 18 | Detailed |
| 2 | epic-02-correctness-fixes | 7 | 15 | Detailed |
| 3 | epic-03-fitting-configuration | 7 | 20 | Outline |
| 4 | epic-04-methodology-and-docs | 6 | 12 | Outline |

## Decision Gates

Every decision is `proposed`. A gate ticket writes a brief under `docs/auditoria/fpha-decisions/` and
returns `CLARIFICATION REQUIRED:`; the orchestrator asks the user, records `status: accepted` and
`chosen` in the master-plan frontmatter, and only then dispatches the dependent tickets.

| ADR | Topic | Gate ticket | Recommended | Gates |
| --- | --- | --- | --- | --- |
| ADR-025 | Alpha applied once (A1) | ticket-008 | (b) drop `scale_planes`, keep `cf = alpha` | 011, 012, 013 |
| ADR-026 | Plants with units not fitted (A2) | ticket-009 | (a)+(c), plants without units excluded | 014, 015 |
| ADR-027 | Missing tailrace/efficiency/loss (A3) | ticket-010 | (a) fail fast, computed only | 016 |
| ADR-028 | `CAD_FPHA_CONFIG.csv` + fit domain (A4, B2) | ticket-018 | (a) preliminary | 019 |
| ADR-029 | `max_planes`, reduction, MILP selection (A5) | ticket-020 | (a) preliminary | 021, 022 |
| ADR-030 | FPHA volume reference (B1) | ticket-025 | (b) preliminary | [ADD] at refinement |
| ADR-031 | Turbining axis and Pmax cap (B3) | ticket-026 | (b) preliminary | [ADD] at refinement |
| ADR-032 | Tailrace families (B4) | ticket-027 | (a) preliminary | [ADD] at refinement |
| ADR-033 | Alpha evaluation points (B5) | ticket-028 | (b) preliminary | [ADD] at refinement |

## Output Contracts

- `resultado_fpha_computed.csv`: column names, order and formats unchanged. Under ADR-025 (b) the
  gamma columns become the unscaled planes and `alpha` is applied once by the model (BREAKING,
  `CHANGELOG.md`, ticket-011 commit footer and ticket-012 entries).
- `OPER_FPHA.csv`: unchanged. Documented copy: `USIH = codigo_usina`, `SegFPHA = plane_index + 1`,
  `Fcorrec = alpha`, `Rhs.mw = gamma_0`, `Varm.mw/hm3 = gamma_v`, `Qtur.mw/m3s = gamma_q`,
  `Qlat.mw/m3s = gamma_s`; ticket-013 proves the round trip.

## Progress

<!-- progress:start -->
| Ticket | Status | Detail | Est. (min) | Self | Readiness | Quality | Badge |
| ------ | ------ | ------ | ---------- | ---- | --------- | ------- | ----- |
| ticket-001-record-workspace-baseline.md | completed | detailed | -- | 0.97 | 0.98 | 0.93 | EXCELLENT |
| ticket-002-add-fpha-lp-envelope-probe.md | completed | detailed | -- | 1.00 | 1.00 | 0.90 | EXCELLENT |
| ticket-003-characterize-double-alpha-envelope.md | completed | detailed | -- | 1.00 | 1.00 | 0.90 | EXCELLENT |
| ticket-004-add-fpha-e2e-derived-case-harness.md | completed | detailed | -- | 1.00 | 0.98 | 1.00 | EXCELLENT |
| ticket-005-characterize-skipped-plant-generation.md | completed | detailed | -- | 1.00 | 0.97 | 1.00 | EXCELLENT |
| ticket-006-characterize-silent-physical-defaults.md | completed | detailed | -- | 1.00 | 0.94 | 0.90 | EXCELLENT |
| ticket-007-compile-fpha-baseline-evidence.md | in_progress | detailed | -- | 0.97 | 0.94 | -- | EXCELLENT |
| ticket-008-decide-alpha-application.md | pending | detailed | -- | 0.97 | -- | -- | -- |
| ticket-009-decide-skipped-plant-policy.md | pending | detailed | -- | 0.97 | -- | -- | -- |
| ticket-010-decide-missing-physical-data-policy.md | pending | detailed | -- | 0.97 | -- | -- | -- |
| ticket-011-apply-alpha-once-in-fit-pipeline.md | pending | detailed | -- | 1.00 | -- | -- | -- |
| ticket-012-align-computed-fpha-contract-with-lp.md | pending | detailed | -- | 1.00 | -- | -- | -- |
| ticket-013-add-precomputed-round-trip-regression.md | pending | detailed | -- | 1.00 | -- | -- | -- |
| ticket-014-fail-on-skipped-fpha-plants.md | pending | detailed | -- | 1.00 | -- | -- | -- |
| ticket-015-revalidate-system-after-fpha-injection.md | pending | detailed | -- | 1.00 | -- | -- | -- |
| ticket-016-reject-missing-fpha-physical-data.md | pending | detailed | -- | 1.00 | -- | -- | -- |
| ticket-017-record-post-fix-fpha-evidence.md | pending | detailed | -- | 0.97 | -- | -- | -- |
| ticket-018-decide-fpha-config-semantics.md | pending | outline | -- | -- | -- | -- | -- |
| ticket-019-wire-fpha-config-into-fitting.md | pending | outline | -- | -- | -- | -- | -- |
| ticket-020-decide-plane-count-and-milp-selection.md | pending | outline | -- | -- | -- | -- | -- |
| ticket-021-enforce-max-planes-in-fit-pipeline.md | pending | outline | -- | -- | -- | -- | -- |
| ticket-022-select-fpha-planes-for-milp-stage.md | pending | outline | -- | -- | -- | -- | -- |
| ticket-023-reject-fpha-rows-for-plants-without-units.md | pending | outline | -- | -- | -- | -- | -- |
| ticket-024-validate-fpha-geometry-inputs.md | pending | outline | -- | -- | -- | -- | -- |
| ticket-025-decide-fpha-volume-reference.md | pending | outline | -- | -- | -- | -- | -- |
| ticket-026-decide-turbining-axis-and-pmax-cap.md | pending | outline | -- | -- | -- | -- | -- |
| ticket-027-decide-tailrace-family-modelling.md | pending | outline | -- | -- | -- | -- | -- |
| ticket-028-decide-alpha-evaluation-points.md | pending | outline | -- | -- | -- | -- | -- |
| ticket-029-update-fpha-design-documentation.md | pending | outline | -- | -- | -- | -- | -- |
| ticket-030-update-fpha-user-documentation.md | pending | outline | -- | -- | -- | -- | -- |
<!-- progress:end -->

## Dependency Graph

```
Epic 01
001 -> 002 -+-> 003 ------------------+
            +-> 004 -+----------------+
            |        +-> 005 (+002) --+-> 007 -+-> 008 (ADR-025)
            +-> 006 ------------------+        +-> 009 (ADR-026)
                                               +-> 010 (ADR-027)

Epic 02
008 + 003 -> 011 -> 012 -> 013 (+004) ---------+
009 + 005 -> 014 -+-> 015 ---------------------+-> 017
                  +-> 016 (+010, +006) --------+

Epic 03 (outline)
017 -> 018 (ADR-028) -> 019 -+
017 -> 020 (ADR-029) --------+-> 021 -> 022
017 -> 023
017 -> 024

Epic 04 (outline)
017 -> 025 (ADR-030), 026 (ADR-031), 027 (ADR-032)
017 + 019 -> 028 (ADR-033)
019, 021, 022, 023, 024, 025, 026, 027, 028 -> 029 -> 030
```

## Execution Order

1. ticket-001-record-workspace-baseline.md
2. ticket-002-add-fpha-lp-envelope-probe.md
3. ticket-003-characterize-double-alpha-envelope.md
4. ticket-004-add-fpha-e2e-derived-case-harness.md
5. ticket-005-characterize-skipped-plant-generation.md
6. ticket-006-characterize-silent-physical-defaults.md (parallel-safe with 003-005)
7. ticket-007-compile-fpha-baseline-evidence.md
8. ticket-008-decide-alpha-application.md (gate: ADR-025)
9. ticket-009-decide-skipped-plant-policy.md (gate: ADR-026)
10. ticket-010-decide-missing-physical-data-policy.md (gate: ADR-027)
11. ticket-011-apply-alpha-once-in-fit-pipeline.md
12. ticket-012-align-computed-fpha-contract-with-lp.md
13. ticket-013-add-precomputed-round-trip-regression.md
14. ticket-014-fail-on-skipped-fpha-plants.md
15. ticket-015-revalidate-system-after-fpha-injection.md
16. ticket-016-reject-missing-fpha-physical-data.md
17. ticket-017-record-post-fix-fpha-evidence.md
18. ticket-018-decide-fpha-config-semantics.md (outline; gate: ADR-028)
19. ticket-020-decide-plane-count-and-milp-selection.md (outline; gate: ADR-029)
20. ticket-019-wire-fpha-config-into-fitting.md (outline)
21. ticket-021-enforce-max-planes-in-fit-pipeline.md (outline)
22. ticket-022-select-fpha-planes-for-milp-stage.md (outline; only if ADR-029 (b))
23. ticket-023-reject-fpha-rows-for-plants-without-units.md (outline)
24. ticket-024-validate-fpha-geometry-inputs.md (outline)
25. ticket-025-decide-fpha-volume-reference.md (outline; gate: ADR-030)
26. ticket-026-decide-turbining-axis-and-pmax-cap.md (outline; gate: ADR-031)
27. ticket-027-decide-tailrace-family-modelling.md (outline; gate: ADR-032)
28. ticket-028-decide-alpha-evaluation-points.md (outline; gate: ADR-033)
29. ticket-029-update-fpha-design-documentation.md (outline)
30. ticket-030-update-fpha-user-documentation.md (outline)

Tickets that share a file run in this order: `fpha_integration.rs` (002, 003, 005, 006, 012, 014,
015, 016), `fpha_e2e.rs` (004, 005, 013, 014), `lp_envelope_tests.rs` (003, 011, 013),
`plant_coverage_tests.rs` (005, 014, 015), `physical_data_tests.rs` (006, 016), the evidence report
(001, 007, 017).

## Phases & Estimates

Execution strategy: **phased** (approval stop at each epic boundary; decision gates pause
regardless). Recorded in `.implementation-state.json` as `execution_strategy` and per-epic `estimate`.

| Phase | Epic | Tickets | Points | Est. agent time | Confidence | User gates | Exit |
| --- | --- | --- | --- | --- | --- | --- | --- |
| 1 | epic-01-reproducible-baseline | 10 | 18 | ~2.5–4 h | High | ADR-025, ADR-026, ADR-027 | evidence sections 1-7a committed; three gates accepted |
| 2 | epic-02-correctness-fixes | 7 | 15 | ~2–3.5 h | High | none | A1-A3 fixed; section 8 committed |
| 3 | epic-03-fitting-configuration | 7 | 20 (outline) | ≈2.5–4.5 h | Low (outline) | ADR-028, ADR-029 | config and plane count implemented; inputs validated |
| 4 | epic-04-methodology-and-docs | 6 | 12 (outline) | ≈1.5–3 h | Low (outline) | ADR-030 to ADR-033 | decisions recorded; documentation current |

Total: ≈8.5–15 h of agent working time (approximate; excludes decision pauses and review cycles).

## Evidence and Decision Artifacts

- `docs/auditoria/fpha-baseline-reproduction.md`: baseline (sections 1-7a) and post-fix (section 8).
- `docs/auditoria/fpha-decisions/ADR-0NN-<slug>.md`: one brief per gate (planning assumption P3; the
  alternative was plan-directory-only briefs, because `plans/` is gitignored).

## Success Criteria

- A1: the LP envelope equals alpha times the returned (unscaled) planes; the reported deviation equals
  the LP-envelope deviation within 1e-9 MW; the `OPER_FPHA.csv` copy reproduces the computed
  objective within 1e-5 relative.
- A2: a computed run with a plant with units lacking geometry exits 1 naming the plant and
  `CAD/CAD_GEOMETRIA_UHE.csv`; no plant with units reaches the solver without FPHA rows.
- A3: a missing tailrace row, `Eficiencia` or `Perda.m` yields the ADR-027 outcome naming plant and
  input.
- Every ticket ends with `cargo test --workspace` exiting 0 and no increase in clippy warnings.
