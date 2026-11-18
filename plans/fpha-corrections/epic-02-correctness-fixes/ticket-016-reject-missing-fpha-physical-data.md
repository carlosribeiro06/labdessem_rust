---
filesToBeCreated: []
filesToBeModified:
  - crates/labdessem-cli/src/fpha_integration.rs
  - crates/labdessem-cli/src/fpha_integration/physical_data_tests.rs
  - CHANGELOG.md
filesToBeDeleted: []
implements: [ADR-027, R4, R10]
agent: hpc-rust-developer
tier: standard
---

# ticket-016 Reject Missing Physical Data in Computed FPHA

> **Pending Decisions**: this ticket assumes ADR-027 option (a), with the sub-questions answered
> "an explicit `Perda.m = 0` stays valid" and "EngMax is out of scope". Dispatch only when ADR-027 is
> `accepted` with that option as `chosen`; otherwise the ticket is re-refined first.

## Context

### Background

Finding A3 (audit FP-R02, plus the missing tailrace): `build_fit_params`
(`crates/labdessem-cli/src/fpha_integration.rs` lines 180-253) passes `tailrace: None`,
`efficiency: None` and `hydraulic_loss_m: None` to `fit_plant`, and `ProductionFunction::new` turns
them into a 0 m tailrace, efficiency 1.0 and loss 0 m. ticket-006 measured the FPH inflation (section 7
of the evidence report). Under ADR-027 (a), computed fitting rejects these inputs with a typed failure
that names the plant and every missing input; ticket-014's `PlantsNotFitted` error carries it.

### Relation to Epic

A3 fix of Epic 02. It reuses the failure collection of ticket-014.

### Current State

- After ticket-014: `FitFailureReason { MissingGeometry, InvalidParameters(String),
  FittingFailed(String) }`, `PlantFitFailure`, `FphaComputeError::PlantsNotFitted`; `compute_fpha`
  maps non-`MissingHydro` errors of `build_fit_params` to `InvalidParameters`.
- `CAD/CAD_UHE.csv` columns `Perda.m` and `Eficiencia` are read with serde `default`, so an absent
  column and an empty cell are both `None`; `CAD/CAD_TAILRACE.csv` yields no entry for a plant without
  a row.
- `physical_data_tests.rs` (ticket-006) holds two `baseline_a3_*` tests and
  `control_a3_official_inputs_carry_physical_data`.

## Specification

### Requirements

1. Add `pub enum FphaInput { Tailrace, Efficiency, HydraulicLoss }` with `Display`:
   `row in CAD/CAD_TAILRACE.csv`, `Eficiencia in CAD/CAD_UHE.csv (column absent or empty)`,
   `Perda.m in CAD/CAD_UHE.csv (column absent or empty)`.
2. Add `FitFailureReason::MissingInputs(Vec<FphaInput>)`, displayed as `missing inputs: <a>; <b>`.
3. Add `FphaComputeError::MissingFphaInputs { hydro_id: i32, hydro_name: String, inputs:
   Vec<FphaInput> }`; `build_fit_params` checks the three inputs first (order Tailrace, Efficiency,
   HydraulicLoss), returns this error when any is missing, and passes `Some` values through otherwise.
   `compute_fpha` maps it to `PlantFitFailure { reason: MissingInputs(inputs), .. }`.
4. Flip the tests in `physical_data_tests.rs`:
   - `baseline_a3_missing_tailrace_row_uses_zero_level` -> `missing_tailrace_row_fails_fitting`: one
     failure for code 3 with `MissingInputs([Tailrace])`; `Display` contains `CAD/CAD_TAILRACE.csv`;
   - `baseline_a3_missing_efficiency_and_loss_default_silently` ->
     `missing_efficiency_and_loss_fail_fitting`: four failures, each `MissingInputs([Efficiency,
     HydraulicLoss])`; `Display` contains `Eficiencia` and `Perda.m`;
   - new `explicit_zero_loss_is_accepted`: set `Perda.m` of plant 0 to `0` in a derived case;
     `compute_fpha` returns `Ok` with 4 plant results.
5. `CHANGELOG.md` `[Unreleased]`: `### Fixed` (FP-R02 and the missing tailrace) and a `**BREAKING**:`
   entry under `### Changed` (computed FPHA requires a tailrace row, `Eficiencia` and `Perda.m` for
   every plant with units; EngMax unchanged).

### Inputs/Props

`HydroPlant { efficiency: Option<f64>, hydraulic_loss_m: Option<f64>, .. }`; `Option<&TailraceData>`.

### Outputs/Behavior

Computed runs with any of the three inputs missing for a plant with units fail with
`PlantsNotFitted`, listing every affected plant and input; complete decks behave as after ticket-015.

### Error Handling

- No `unwrap`/`expect`/`panic` in production code.
- `ProductionFunction::new` keeps its `unwrap_or` defaults (EngMax uses them; out of scope); the CLI
  simply never passes `None` in computed mode after this ticket.

### Out of Scope

- `crates/labdessem-fpha/src/fitting/production.rs` and the EngMax calculator.
- Range checks on the values (efficiency in (0, 1], loss >= 0) and NaN handling (ticket-024).
- The `ProductionFunction` defaults themselves (an ADR-027 rework trigger, not this ticket).

## Acceptance Criteria

- [ ] Given a computed derived case without plant 3 tailrace rows, when `cargo test -p labdessem-cli
  missing_tailrace_row_fails_fitting` runs, then it passes, asserting one failure for code 3 with
  `MissingInputs([Tailrace])` and a `Display` containing `CAD/CAD_TAILRACE.csv`.
- [ ] Given a computed derived case without `Perda.m` and `Eficiencia`, when `cargo test -p
  labdessem-cli missing_efficiency_and_loss_fail_fitting` runs, then it passes, asserting four failures
  with `[Efficiency, HydraulicLoss]` each and a `Display` containing `Eficiencia` and `Perda.m`.
- [ ] Given a derived case with `Perda.m = 0` for plant 0, when `cargo test -p labdessem-cli
  explicit_zero_loss_is_accepted` runs, then it passes, asserting `Ok` with 4 plant results.
- [ ] Given the ticket commit, when `grep -r "baseline_a3" crates/labdessem-cli/ | wc -l` runs, then it
  prints 0, and `control_a3_official_inputs_carry_physical_data` still passes.
- [ ] Given the ticket commit, when `git show --stat HEAD` runs, then it lists exactly the three
  frontmatter files, `cargo test --workspace` exits 0, and clippy warnings do not exceed section 2 of
  the evidence report.

## Implementation Guide

### Suggested Approach

[ASSUMPTION] ADR-027 option (a) with explicit zero loss valid and EngMax excluded. Alternatives not
chosen: (b) warning only, (c) configurable defaults, (d) error for tailrace and warning for the rest.

1. Add `FphaInput`, the new reason and the error variant with their `Display` arms.
2. Insert the check at the top of `build_fit_params`; map the error in `compute_fpha`.
3. Flip the tests; add a local helper that rewrites one field of one CSV row in a `DerivedCase`.
4. Write the changelog entries; commit `fix(fpha)!: reject missing physical data in computed FPHA`
   with a `BREAKING CHANGE:` footer.

### Key Files to Modify

- `crates/labdessem-cli/src/fpha_integration.rs`: types, check, mapping.
- `crates/labdessem-cli/src/fpha_integration/physical_data_tests.rs`: three tests.
- `CHANGELOG.md`: `[Unreleased]` entries.

### Patterns to Follow

- The `PlantFitFailure` collection of ticket-014 (all plants reported in one error).
- The probe `DerivedCase` editing helpers.

### Pitfalls to Avoid

- Report all missing inputs of a plant at once, in the fixed order of requirement 3.
- The probe's `fit_contexts` calls `build_fit_params`; on a derived case with missing inputs it now
  returns an error, so tests must not call it on those cases.
- `MissingHydro` stays fatal for a plant missing from `inflow_history.parquet`; because the input
  check runs first, a plant that also lacks inputs is reported as `MissingInputs`, which is intended.

## Testing Requirements

### Unit Tests

The three tests above, plus a `Display` test for `MissingInputs` with two inputs (exact string) in the
`tests` module of `fpha_integration.rs`.

### Integration Tests

All probe-based tests and `control_a3_official_inputs_carry_physical_data` keep passing.

### E2E Tests (if applicable)

`cargo test -p labdessem-cli --test fpha_e2e` passes unchanged (official inputs are complete).

## Dependencies

- **Blocked By**: ticket-010-decide-missing-physical-data-policy.md, ticket-006-characterize-silent-physical-defaults.md, ticket-014-fail-on-skipped-fpha-plants.md
- **Blocks**: ticket-017-record-post-fix-fpha-evidence.md
- **Notes**: ADR-027 must be accepted with option (a); run after ticket-015 (same file).

## Effort Estimate

**Points**: 2
**Confidence**: High
**Rationale**: the failure collection exists after ticket-014; the change is one check and three test
flips.
