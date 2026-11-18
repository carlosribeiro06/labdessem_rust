---
filesToBeCreated:
  - crates/labdessem-cli/src/fpha_integration/plant_coverage_tests.rs
filesToBeModified:
  - crates/labdessem-cli/src/fpha_integration.rs
  - crates/labdessem-cli/tests/fpha_e2e.rs
filesToBeDeleted: []
implements: [R8, R3]
agent: monorepo-test-developer
tier: standard
---

# ticket-005 Characterize Generation of Skipped FPHA Plants

## Context

### Background

Audit finding HI-03 (also FP-01 and NU-04) states that, in computed mode, a plant with units whose
planes cannot be computed is skipped with a `Warning:` and keeps no link between generation and water:
`compute_fpha` (`crates/labdessem-cli/src/fpha_integration.rs` lines 98-104, 117-128, 143-149) skips
it, `main.rs` lines 131-135 enables FPHA without re-validating, the model builds no `hydro_fpha` row
for the plant, and `build_hydro_generation_turbining_coupling_constraints` skips the unit coupling
because the maximum productivity is 0. The audit saw FUNIL-GRANDE generate 180 MW with zero
turbining in 48 of 48 periods, and every plant generate for free when `CAD_GEOMETRIA_UHE.csv` is
absent. This ticket reproduces both cases in-process and end to end.

### Relation to Epic

Fourth evidence ticket of Epic 01 (finding A2). ticket-014 flips its `baseline_a2_*` tests (typed
error) once ADR-026 is accepted; ticket-015 adds the post-injection validation tests to the same file.

### Current State

- `HydroPlant::validate(horizon, fpha_enabled)` in `crates/labdessem-core/src/entities/hydro.rs`
  (about line 311) rejects a plant with groups and no segments when FPHA is enabled ("must contain at
  least one FPHA segment"), but `System::validate` runs in `read_study_from_path` while
  `fpha_enabled` is still false.
- The probe (`probe.rs`) provides `DerivedCase::remove_plant_rows`, `DerivedCase::remove_file`,
  `computed_system` and `lp_fpha_rows`; `crates/labdessem-cli/tests/fpha_e2e.rs` provides
  `derived_case` and `run_cli`.
- `resultado_hidreletricas.csv` has a preamble, then the header starting `Usina;Conjunto;Unidade;
  Periodo;...;GeracaoMW;...;TurbinamentoM3s;...`; plant aggregate rows use `Conjunto` = 99 and
  `Unidade` = 99, with the plant name in `Usina`.

## Specification

### Requirements

1. Create `crates/labdessem-cli/src/fpha_integration/plant_coverage_tests.rs`, declared in
   `fpha_integration.rs` with the same `#[cfg(test)] #[allow(...)]` attributes as `mod probe;`.
2. `baseline_a2_plant_without_geometry_has_no_fpha_rows`: remove plant 2 rows from
   `CAD/CAD_GEOMETRIA_UHE.csv` in a computed `DerivedCase`; `computed_system` returns `Ok` with
   `plants_skipped == 1`; plant 2 has zero `fpha_segments`; the model holds zero rows named
   `hydro_fpha[p=<plant 2 name>,...` and zero rows named
   `hydro_generation_turbining_coupling[p=<plant 2 name>,...`; `system.validate()` returns `Err` whose
   message contains `at least one FPHA segment`.
3. `baseline_a2_missing_geometry_file_skips_every_plant`: remove `CAD/CAD_GEOMETRIA_UHE.csv`;
   `computed_system` returns `Ok`, `plant_results` is empty and `plants_skipped` equals the number of
   hydro plants (4).
4. `control_a2_full_geometry_links_every_plant`: unmodified computed case; `plants_skipped == 0`, and
   every plant with groups has at least one `hydro_fpha` row and one coupling row per unit.
5. In `crates/labdessem-cli/tests/fpha_e2e.rs`, add `remove_plant_rows` (same contract as the probe
   helper) and `baseline_a2_plant_without_geometry_generates_without_water`: computed case without
   plant 2 geometry; the run exits 0; stderr contains `has no VHA geometry data, skipping`; among the
   plant 2 aggregate rows of `RESULTADOS/resultado_hidreletricas.csv`, at least one period has
   `GeracaoMW > 1.0` with `TurbinamentoM3s <= 1e-6`. Print the count of such periods and the maximum
   `GeracaoMW`.
6. Every `baseline_*` test carries the doc comment `Characterizes audit HI-03 (FP-01, NU-04) on the
   pre-fix code. Replaced by ticket-014.`

### Inputs/Props

Computed `DerivedCase` variants; plant code 2 (FUNIL-GRANDE, 3 units in the audit); the fast
configuration.

### Outputs/Behavior

Three in-process tests and one end-to-end test, all passing on the pre-fix code, plus printed counts
for section 6 of the evidence report.

### Error Handling

Test-only code; a missing CSV column or plant row fails the test with an `expect` message naming the
file and the column.

### Out of Scope

- Plants without units and the FP-12 `MissingHydro` behavior (ticket-014 adds those tests with the
  fix).
- Changing `compute_fpha`, `main.rs` or any validation rule.
- A3 inputs (tailrace, efficiency, loss): ticket-006.

## Acceptance Criteria

- [ ] Given a computed derived case without plant 2 geometry, when `cargo test -p labdessem-cli
  baseline_a2_plant_without_geometry_has_no_fpha_rows` runs, then it passes, asserting
  `plants_skipped == 1`, zero segments, zero `hydro_fpha` rows and zero coupling rows for plant 2, and
  a `system.validate()` error containing `at least one FPHA segment`.
- [ ] Given a computed derived case without `CAD/CAD_GEOMETRIA_UHE.csv`, when `cargo test -p
  labdessem-cli baseline_a2_missing_geometry_file_skips_every_plant` runs, then it passes, asserting
  `Ok`, zero plant results and `plants_skipped == 4`.
- [ ] Given the unmodified computed derived case, when `cargo test -p labdessem-cli
  control_a2_full_geometry_links_every_plant` runs, then it passes, asserting zero skipped plants and,
  for every plant with groups, at least one `hydro_fpha` row and one coupling row per unit.
- [ ] Given the end-to-end case without plant 2 geometry, when `cargo test -p labdessem-cli --test
  fpha_e2e baseline_a2_plant_without_geometry_generates_without_water -- --nocapture` runs, then it
  passes, asserting exit code 0, the skip warning on stderr and at least one plant 2 aggregate period
  with `GeracaoMW > 1.0` and `TurbinamentoM3s <= 1e-6`, and it prints the period count and the maximum
  generation.
- [ ] Given the ticket commit, when `git show --stat HEAD` runs, then it lists exactly the three
  frontmatter files, `cargo test --workspace` exits 0, and `cargo clippy --workspace --all-targets`
  reports no more warnings than section 2 of the evidence report.

## Implementation Guide

### Suggested Approach

1. Add the module declaration beside `mod probe;`.
2. Write the three in-process tests with the probe helpers; find rows by the name prefixes above.
3. Add the end-to-end helper and test; parse the hydro CSV by header names, not by position.
4. Commit `test(fpha): characterize generation of plants skipped by computed FPHA`.

### Key Files to Modify

- `crates/labdessem-cli/src/fpha_integration/plant_coverage_tests.rs` (create): in-process A2 tests.
- `crates/labdessem-cli/src/fpha_integration.rs`: the module declaration only.
- `crates/labdessem-cli/tests/fpha_e2e.rs`: one helper and one end-to-end test.

### Patterns to Follow

- Row lookups by name prefix as in `crates/labdessem-model/src/tests.rs` (about line 1225).
- The `baseline_` / `control_` naming of the audit's proof tests (audit section 12.1).

### Pitfalls to Avoid

- Take plant 2's name from the `System` (the CSV value carries padding).
- `system.validate()` here is called by the test, not by production code; do not add it to `main.rs`
  (ticket-015).
- The hydro CSV numbers are formatted `{:.6}`; compare with the stated thresholds, never with
  equality.

## Testing Requirements

### Unit Tests

The three in-process tests listed above.

### Integration Tests

`cargo test --workspace` exits 0; tests from tickets 002-004 keep passing.

### E2E Tests (if applicable)

`baseline_a2_plant_without_geometry_generates_without_water` in `fpha_e2e.rs` (one extra solve).

## Dependencies

- **Blocked By**: ticket-002-add-fpha-lp-envelope-probe.md, ticket-004-add-fpha-e2e-derived-case-harness.md
- **Blocks**: ticket-007-compile-fpha-baseline-evidence.md, ticket-014-fail-on-skipped-fpha-plants.md
- **Notes**: ticket-014 flips every `baseline_a2_*` test created here.

## Effort Estimate

**Points**: 2
**Confidence**: High
**Rationale**: the audit already established the expected behavior; the probe and E2E helpers exist.
