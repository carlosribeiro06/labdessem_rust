# Epic 01: Reproducible Baseline

## Goal

Measure A1 (alpha applied twice), A2 (generation without water) and A3 (silent physical defaults)
on the current code, before any fix, with tests anyone can rerun, then put the three decisions that
gate the fixes (ADR-025, ADR-026, ADR-027) in front of the user with evidence.

## Scope

- Branch `fix/fpha-corrections` from `develop`, qhull submodule, build, test and clippy baseline.
- A test-only probe in `labdessem-cli` that reads the LP `hydro_fpha[...]` rows and evaluates the
  envelope they imply (no solver call).
- Characterization tests named `baseline_*` that pass on the current code and assert the exact defect
  signature; Epic 2 replaces them.
- One end-to-end test binary, `crates/labdessem-cli/tests/fpha_e2e.rs`, running the CLI on derived
  copies of `examples/24Barras` in computed and precomputed modes.
- The evidence report `docs/auditoria/fpha-baseline-reproduction.md`.
- Three decision briefs under `docs/auditoria/fpha-decisions/`.

Out of scope: any production code change, any edit of `examples/24Barras`, EngMax findings.

## Tickets

| Ticket | Title | Agent | Points |
| --- | --- | --- | --- |
| ticket-001-record-workspace-baseline.md | Record Workspace Baseline Before FPHA Changes | hpc-rust-developer | 1 |
| ticket-002-add-fpha-lp-envelope-probe.md | Add FPHA LP Envelope Probe | monorepo-test-developer | 3 |
| ticket-003-characterize-double-alpha-envelope.md | Characterize the Double-Alpha LP Envelope | monorepo-test-developer | 2 |
| ticket-004-add-fpha-e2e-derived-case-harness.md | Add FPHA End-to-End Derived-Case Harness | monorepo-test-developer | 3 |
| ticket-005-characterize-skipped-plant-generation.md | Characterize Generation of Skipped FPHA Plants | monorepo-test-developer | 2 |
| ticket-006-characterize-silent-physical-defaults.md | Characterize Silent FPHA Physical Defaults | monorepo-test-developer | 2 |
| ticket-007-compile-fpha-baseline-evidence.md | Compile FPHA Baseline Evidence Report | monorepo-test-developer | 1 |
| ticket-008-decide-alpha-application.md | Decide Alpha Application (ADR-025) | dessem-specialist | 2 |
| ticket-009-decide-skipped-plant-policy.md | Decide Skipped-Plant Policy (ADR-026) | dessem-specialist | 1 |
| ticket-010-decide-missing-physical-data-policy.md | Decide Missing Physical-Data Policy (ADR-027) | dessem-specialist | 1 |

## Dependency Order

```
001 -> 002 -> 003 --------------------\
        |  \-> 004 -> 005 ------------+-> 007 -> 008, 009, 010
        \-> 006 ----------------------/
(004 also feeds 007; 002 also feeds 005)
```

## Exit Criteria

- `cargo test --workspace` exits 0 with every `baseline_*` test passing.
- Sections 1-7 of the evidence report are filled.
- ADR-025, ADR-026 and ADR-027 are `accepted` in the master-plan frontmatter.
