---
filesToBeCreated: []
filesToBeModified:
  - crates/labdessem-cli/tests/fpha_e2e.rs
  - crates/labdessem-cli/src/fpha_integration/lp_envelope_tests.rs
filesToBeDeleted: []
implements: [R9, R8]
agent: monorepo-test-developer
tier: standard
---

# ticket-013 Add Computed-to-Precomputed Round-Trip Regression

> **Pending Decisions**: this ticket assumes ADR-025 option (b) (copy rule `Fcorrec = alpha`).
> Dispatch only when ADR-025 is `accepted` with option (b) as `chosen`.

## Context

### Background

R9 requires that the documented copy of `resultado_fpha_computed.csv` into `OPER_FPHA.csv` reproduces
the computed run. ticket-012 wrote the copy rule (`Fcorrec = alpha`, gamma columns verbatim,
`SegFPHA = plane_index + 1`). The precomputed path was never exercised on the example before this plan
(audit MX-06), and the ticket-004 E2E test still carries its pre-fix name and doc comment
(`baseline_a1_computed_vs_precomputed_copies`, "Rewritten by ticket-013"). This ticket turns it into
the contract regression and adds an in-process round trip that checks the segments, not only the
objective.

### Relation to Epic

Third A1 ticket of Epic 02; it closes R9.

### Current State

- `crates/labdessem-cli/tests/fpha_e2e.rs` has `derived_case`, `run_cli`, `read_computed_planes`,
  `write_oper_fpha(dir, planes, Fcorrec::{One, Alpha})` and the A1 test with three solves.
- `crates/labdessem-cli/src/fpha_integration/lp_envelope_tests.rs` (after ticket-011) has the two
  envelope tests and the `lp_deviation` helper; the probe provides `DerivedCase`, `computed_system`,
  `segment_envelope_mw`, `fit_contexts`, `fit_grid_nodes`.
- CSV formats: `gamma_0`, `gamma_q`, `alpha` with 6 decimals; `gamma_v`, `gamma_s` with 9 decimals.
- `labdessem_io::read_study_from_path` reads `OPER/OPER_FPHA.csv` into `HydroPlant.fpha_segments`.

## Specification

### Requirements

1. In `fpha_e2e.rs`, rename `baseline_a1_computed_vs_precomputed_copies` to
   `computed_csv_round_trip_reproduces_objective` and set its doc comment to `Regression for audit
   FP-02 and MX-06: the documented OPER_FPHA copy (Fcorrec = alpha) reproduces the computed run.`
   Assertions:
   - computed run exits 0 with objective `O_c`;
   - precomputed run with `Fcorrec::Alpha`: exit 0 and `|O_alpha - O_c| <= 1e-5 * |O_c|`;
   - sensitivity control, precomputed run with `Fcorrec::One`: exit 0 and
     `|O_one - O_c| > 1e-5 * |O_c|` (proves the comparison detects a missing alpha).
2. In `lp_envelope_tests.rs`, add `computed_csv_round_trip_reproduces_segments`:
   - computed `DerivedCase` -> `computed_system` -> `super::write_computed_results` into that case;
   - read its `RESULTADOS/resultado_fpha_computed.csv` and write `OPER/OPER_FPHA.csv` of a second
     `DerivedCase::new("precomputed")` with the copy rule;
   - `read_study_from_path` on the second case; for every plant code 0-3, segment counts are equal and
     each field matches the computed segment within 5e-7 (`rhs`, `turbining_coefficient`,
     `correction_factor`) or 5e-10 (`volume_coefficient`, `lateral_flow_coefficient`);
   - `segment_envelope_mw` of both segment sets agree within 1e-6 x max(1, |value|) at every fit-grid
     node for s in {0, Q_max/2, Q_max}.
3. Print the three objectives, both gaps and the maximum segment-envelope difference per plant with
   `eprintln!`.

### Inputs/Props

Fast configuration; computed and precomputed derived cases; the copy rule of ticket-012.

### Outputs/Behavior

One renamed E2E test (three solves, unchanged count) and one new in-process test; no production code
changes.

### Error Handling

- A failing run fails the test with its stderr in the panic message.
- If the `Fcorrec::Alpha` round trip misses the 1e-5 tolerance, stop and return the
  `CLARIFICATION REQUIRED:` payload with both objectives; do not widen the tolerance (it is 70 times
  smaller than the A1 effect measured by the audit).

### Out of Scope

- Changing the `OPER_FPHA.csv` reader or the CSV writer.
- The A2 E2E test in the same file (ticket-014).
- Precomputed plants without units (ticket-023).

## Acceptance Criteria

- [ ] Given the fixed code, when `cargo test -p labdessem-cli --test fpha_e2e
  computed_csv_round_trip_reproduces_objective` runs, then it passes, asserting the `Fcorrec = alpha`
  copy within 1e-5 relative of `O_c` and the `Fcorrec = 1` control more than 1e-5 away.
- [ ] Given the fixed code, when `cargo test -p labdessem-cli
  computed_csv_round_trip_reproduces_segments` runs, then it passes, asserting equal segment counts,
  per-field tolerances of 5e-7 / 5e-10 and envelope agreement within 1e-6 relative for all 4 plants.
- [ ] Given `crates/labdessem-cli/tests/fpha_e2e.rs`, when `grep -c "baseline_a1"
  crates/labdessem-cli/tests/fpha_e2e.rs` runs, then it prints 0.
- [ ] Given `cargo test -p labdessem-cli round_trip -- --nocapture`, when it runs, then stderr shows
  three objectives, two gaps and one maximum envelope difference per plant.
- [ ] Given the ticket commit, when `git show --stat HEAD` runs, then it lists exactly the two
  frontmatter files, `cargo test --workspace` exits 0, and `cargo clippy --workspace --all-targets`
  reports no more warnings than section 2 of the evidence report.

## Implementation Guide

### Suggested Approach

[ASSUMPTION] ADR-025 option (b): the copy rule is `Fcorrec = alpha`. Under (a1)/(a2) the documented
copy would use `Fcorrec = 1` and the control would be inverted.

1. Rename and rewrite the E2E test; keep the helpers.
2. Write a private `write_oper_fpha_from_csv(src_case: &Path, dst_case: &Path)` in
   `lp_envelope_tests.rs` (the E2E helper cannot be imported from an integration test binary).
3. Write the in-process test; commit `test(fpha): add computed to precomputed FPHA round-trip test`.

### Key Files to Modify

- `crates/labdessem-cli/tests/fpha_e2e.rs`: rename and rewrite the A1 test.
- `crates/labdessem-cli/src/fpha_integration/lp_envelope_tests.rs`: one helper, one test.

### Patterns to Follow

- The ticket-004 helpers for CSV parsing (skip to the `codigo_usina;` header).
- The probe's `segment_envelope_mw` for envelope comparisons.

### Pitfalls to Avoid

- `SegFPHA` must start at 1 (`HydroFphaSegment::validate`); `plane_index` starts at 0.
- `OPER_FPHA.csv` plant names are informational; match plants by `USIH` code.
- The precomputed case must not contain `RESULTADOS/` from the computed case; use two `DerivedCase`s.

## Testing Requirements

### Unit Tests

`computed_csv_round_trip_reproduces_segments` (in-process).

### Integration Tests

`cargo test --workspace` exits 0; the ticket-011 envelope tests keep passing.

### E2E Tests (if applicable)

`computed_csv_round_trip_reproduces_objective` (three solves, as before).

## Dependencies

- **Blocked By**: ticket-012-align-computed-fpha-contract-with-lp.md, ticket-004-add-fpha-e2e-derived-case-harness.md
- **Blocks**: ticket-017-record-post-fix-fpha-evidence.md
- **Notes**: shares `fpha_e2e.rs` with ticket-014; run 013 before 014.

## Effort Estimate

**Points**: 2
**Confidence**: High
**Rationale**: helpers exist from ticket-004 and the probe; the work is one rename and one test.
