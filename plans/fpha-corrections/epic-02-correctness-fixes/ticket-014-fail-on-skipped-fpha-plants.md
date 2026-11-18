---
filesToBeCreated: []
filesToBeModified:
  - crates/labdessem-cli/src/fpha_integration.rs
  - crates/labdessem-cli/src/fpha_integration/plant_coverage_tests.rs
  - crates/labdessem-cli/tests/fpha_e2e.rs
  - CHANGELOG.md
filesToBeDeleted: []
implements: [ADR-026, R3, R10]
agent: hpc-rust-developer
tier: standard
---

# ticket-014 Fail on Plants with Units Lacking Computed FPHA

> **Pending Decisions**: this ticket assumes ADR-026 option (a)+(c) with the sub-question "plants
> without units are excluded from fitting" answered yes. Dispatch only when ADR-026 is `accepted`
> with that option as `chosen`; otherwise the ticket is re-refined first.

## Context

### Background

Audit HI-03 (= FP-01 = NU-04): `compute_fpha` (`crates/labdessem-cli/src/fpha_integration.rs` lines
82-151) skips a plant with units when its VHA geometry is missing (line 98), when `build_fit_params`
fails with anything but `MissingHydro` (line 117) or when `fit_plant` fails (line 143), prints a
`Warning:` and returns `Ok`; with no VHA file at all it returns `Ok` with every plant skipped (line 82).
The skipped plant then generates without water. ticket-005 reproduced it (section 6 of the evidence
report). Under ADR-026 (c), `compute_fpha` collects every plant with units it could not fit and returns
one typed error; plants without units need no FPHA and are excluded before any lookup.

### Relation to Epic

First A2 ticket of Epic 02. ticket-015 adds the post-injection validation (ADR-026 (a)); ticket-016
adds a failure reason for missing physical data.

### Current State

- `FphaComputeError { Io(String), Fitting(String) /* #[allow(dead_code)] */, MissingHydro { hydro_id,
  hydro_name } }` with a manual `Display`; `MissingHydro` is fatal and returned immediately.
- `FphaComputeResult { plant_results, plants_processed, plants_skipped }`; `main.rs` prints
  `  Processed {} plants, {} skipped`.
- `HydroPlant.groups: Vec<HydroGroup>`; `HydroPlant::validate` treats `groups.is_empty()` as "no
  units" for the FPHA-segment rule.
- `plant_coverage_tests.rs` (ticket-005) holds `baseline_a2_plant_without_geometry_has_no_fpha_rows`,
  `baseline_a2_missing_geometry_file_skips_every_plant` and `control_a2_full_geometry_links_every_plant`;
  `fpha_e2e.rs` holds `baseline_a2_plant_without_geometry_generates_without_water`.

## Specification

### Requirements

1. Add to `fpha_integration.rs`:
   - `pub enum FitFailureReason { MissingGeometry, InvalidParameters(String), FittingFailed(String) }`;
   - `pub struct PlantFitFailure { pub hydro_id: i32, pub hydro_name: String, pub reason:
     FitFailureReason }`;
   - variant `FphaComputeError::PlantsNotFitted { failures: Vec<PlantFitFailure> }`.
2. `Display` of `PlantsNotFitted`: first line `computed FPHA could not be fitted for <n> hydro plant(s)
   with generating units:`, then one line per failure `  plant <id> (<trimmed name>): <reason>`, with
   reasons `no rows in CAD/CAD_GEOMETRIA_UHE.csv`, `invalid fit parameters: <msg>`,
   `fitting failed: <msg>`.
3. `compute_fpha`:
   - a plant with `groups.is_empty()` is excluded before the VHA lookup and before `build_fit_params`
     (no MLT lookup, so no `MissingHydro` for it) and counted in `plants_skipped`, whose doc comment
     becomes "plants without generating units (no FPHA needed)";
   - every plant with units that fails (missing VHA rows, a non-`MissingHydro` parameter error, a fit
     error) is pushed to `failures`; the loop continues to collect all of them;
   - `MissingHydro` stays fatal and immediate;
   - after the loop, a non-empty `failures` returns `Err(PlantsNotFitted { failures })` in plant order;
   - the empty-VHA early return is removed (every plant with units then fails with
     `MissingGeometry`); the three skip `Warning:` lines are removed.
4. Flip the tests in `plant_coverage_tests.rs`:
   - `baseline_a2_plant_without_geometry_has_no_fpha_rows` -> `plant_without_geometry_fails_with_typed_error`
     (one failure: hydro_id 2, `MissingGeometry`; `Display` contains the trimmed plant 2 name and
     `CAD/CAD_GEOMETRIA_UHE.csv`);
   - `baseline_a2_missing_geometry_file_skips_every_plant` ->
     `missing_geometry_file_lists_every_plant_with_units` (four `MissingGeometry` failures, codes 0-3);
   - new `plant_without_units_is_excluded_from_fitting`: remove plant 2 rows from
     `CAD/CAD_CONJ_UHE.csv` and `CAD/CAD_GEOMETRIA_UHE.csv`; `compute_fpha` returns `Ok` with 3 plant
     results, none for code 2, and `plants_skipped == 1`.
5. Flip the E2E test: `baseline_a2_plant_without_geometry_generates_without_water` ->
   `plant_without_geometry_fails_run`: exit code 1; stderr contains `computed FPHA could not be
   fitted`, the plant 2 name and `CAD/CAD_GEOMETRIA_UHE.csv`; `RESULTADOS/resultado_hidreletricas.csv`
   does not exist.
6. `CHANGELOG.md` `[Unreleased]`: `### Fixed` (HI-03/FP-01/NU-04) and a `**BREAKING**:` entry under
   `### Changed` (computed runs now fail, naming every plant with units that could not be fitted;
   plants without units are no longer fitted).

### Inputs/Props

`compute_fpha(case_path, &System)` (signature unchanged); derived cases of the probe.

### Outputs/Behavior

`Ok(FphaComputeResult)` only when every plant with units has fitted planes; otherwise
`Err(PlantsNotFitted)`; the CLI prints `Error: <Display>` and exits 1 (existing `main` handling).

### Error Handling

- No `unwrap`/`expect`/`panic` in production code; reasons carry the source error text.
- If removing plant 2 from `CAD/CAD_CONJ_UHE.csv` makes `read_study_from_path` fail (another input
  references its units), stop and return the `CLARIFICATION REQUIRED:` payload with the error.

### Out of Scope

- Post-injection validation and `main.rs` (ticket-015); missing tailrace/efficiency/loss (ticket-016).
- Precomputed plants without units (FP-07, ticket-023) and the MLT day-weighting (FP-12, non-goal).
- Renaming `plants_skipped` or changing the `Processed ... skipped` CLI line.

## Acceptance Criteria

- [ ] Given a computed derived case without plant 2 geometry, when `cargo test -p labdessem-cli
  plant_without_geometry_fails_with_typed_error` runs, then it passes, asserting one
  `PlantFitFailure { hydro_id: 2, reason: MissingGeometry, .. }` and a `Display` naming the plant and
  `CAD/CAD_GEOMETRIA_UHE.csv`.
- [ ] Given a computed derived case without `CAD/CAD_GEOMETRIA_UHE.csv`, when `cargo test -p
  labdessem-cli missing_geometry_file_lists_every_plant_with_units` runs, then it passes, asserting four
  `MissingGeometry` failures for codes 0, 1, 2, 3 in order.
- [ ] Given a derived case where plant 2 has no units and no geometry, when `cargo test -p labdessem-cli
  plant_without_units_is_excluded_from_fitting` runs, then it passes, asserting `Ok`, 3 plant results
  without code 2 and `plants_skipped == 1`.
- [ ] Given the E2E case without plant 2 geometry, when `cargo test -p labdessem-cli --test fpha_e2e
  plant_without_geometry_fails_run` runs, then it passes, asserting exit code 1, the three stderr
  fragments of requirement 5 and no `RESULTADOS/resultado_hidreletricas.csv`.
- [ ] Given the ticket commit, when `git show --stat HEAD` and `grep -r "baseline_a2"
  crates/labdessem-cli/ | wc -l` run, then the commit lists exactly the four frontmatter files, the
  pipeline prints 0, `cargo test --workspace` exits 0, and clippy warnings do not exceed section 2 of
  the evidence report.

## Implementation Guide

### Suggested Approach

[ASSUMPTION] ADR-026 option (a)+(c) and "plants without units excluded". Alternatives not chosen:
(a) validation only, (b) per-plant constant-productivity fallback, (d) a `study_config.json` policy key.

1. Add the types and the `Display` arm; drop the `#[allow(dead_code)]` on `Fitting` only if the
   variant becomes used, otherwise leave it.
2. Restructure the `compute_fpha` loop with a `failures: Vec<PlantFitFailure>` accumulator.
3. Flip the in-process tests, then the E2E test (add a helper that removes plant rows from
   `CAD/CAD_CONJ_UHE.csv` only if `DerivedCase::remove_plant_rows` does not already fit).
4. Write the changelog entries; commit `fix(fpha)!: fail when a plant with units has no FPHA planes`
   with a `BREAKING CHANGE:` footer.

### Key Files to Modify

- `crates/labdessem-cli/src/fpha_integration.rs`: types, `Display`, `compute_fpha`.
- `crates/labdessem-cli/src/fpha_integration/plant_coverage_tests.rs`: three tests.
- `crates/labdessem-cli/tests/fpha_e2e.rs`: one test.
- `CHANGELOG.md`: `[Unreleased]` entries.

### Patterns to Follow

- The manual `Display` of `FphaComputeError` (no `thiserror` in this crate).
- Plant names trimmed with `str::trim` in messages; codes as `plant.id.0`.

### Pitfalls to Avoid

- Do not return at the first failure: the operator must see every affected plant in one run.
- Exclude plants without units before the MLT lookup, or `MissingHydro` still fires for them.
- Keep `MissingHydro` fatal for plants with units; it is not part of ADR-026.

## Testing Requirements

### Unit Tests

The three in-process tests above plus a `Display` test for a two-failure `PlantsNotFitted` in the
existing `tests` module of `fpha_integration.rs` (exact expected string).

### Integration Tests

`control_a2_full_geometry_links_every_plant` keeps passing; `cargo test --workspace` exits 0.

### E2E Tests (if applicable)

`plant_without_geometry_fails_run` in `fpha_e2e.rs`.

## Dependencies

- **Blocked By**: ticket-009-decide-skipped-plant-policy.md, ticket-005-characterize-skipped-plant-generation.md
- **Blocks**: ticket-015-revalidate-system-after-fpha-injection.md, ticket-016-reject-missing-fpha-physical-data.md
- **Notes**: shares `fpha_e2e.rs` with ticket-013 and `fpha_integration.rs` with tickets 012, 015 and
  016; run in the epic order.

## Effort Estimate

**Points**: 3
**Confidence**: Medium
**Rationale**: a loop restructuring with a new error type, four test flips and one derived case whose
loadability is not yet proven.
