---
filesToBeCreated: []
filesToBeModified:
  - crates/labdessem-cli/src/fpha_integration.rs
  - crates/labdessem-cli/src/main.rs
  - crates/labdessem-cli/src/fpha_integration/plant_coverage_tests.rs
  - crates/labdessem-cli/src/fpha_integration/probe.rs
filesToBeDeleted: []
implements: [ADR-026, R3]
agent: hpc-rust-developer
tier: standard
---

# ticket-015 Re-validate the System After FPHA Injection

> **Pending Decisions**: this ticket assumes ADR-026 option (a)+(c). Dispatch only when ADR-026 is
> `accepted` with that option as `chosen`; under option (c) alone this ticket is removed.

## Context

### Background

`read_study_from_path` validates the `System` while `fpha_enabled` is false; `main.rs` (lines 131-135)
then injects the computed segments and sets `fpha_enabled = true` without validating again, so the
rule in `HydroPlant::validate` ("must contain at least one FPHA segment" for a plant with groups,
`crates/labdessem-core/src/entities/hydro.rs` line 311) never runs in computed mode. ticket-014 makes
`compute_fpha` fail for plants it cannot fit; ADR-026 (a) adds this structural guard so that any
future injection path that leaves a plant with units without segments is still rejected before the
solver.

### Relation to Epic

Second A2 ticket of Epic 02; it completes ADR-026.

### Current State

- `main.rs`: `compute_fpha` -> `inject_fpha_segments(&mut system, &results)` ->
  `system.fpha_enabled = true` -> `write_computed_results` -> print counts.
- `System::validate(&self) -> Result<(), CoreError>` (`crates/labdessem-core/src/system/system.rs`
  line 38) validates every entity with the current `fpha_enabled`.
- The probe's `computed_system` (`probe.rs`) mirrors the `main.rs` sequence by hand.

## Specification

### Requirements

1. Add `FphaComputeError::InvalidSystem(String)` with `Display`
   `system validation failed after FPHA injection: <msg>`.
2. Add `pub fn finalize_fpha_injection(system: &mut System, plant_results: &[FphaPlantResult]) ->
   Result<(), FphaComputeError>`: `inject_fpha_segments`, `system.fpha_enabled = true`, then
   `system.validate()` mapped to `InvalidSystem(e.to_string())`.
3. In `main.rs`, replace the `inject_fpha_segments` call and the `fpha_enabled` assignment with
   `fpha_integration::finalize_fpha_injection(&mut system, &fpha_result.plant_results)?;`; the order
   of the remaining statements is unchanged.
4. In `probe.rs`, make `computed_system` call `super::finalize_fpha_injection` so the probe mirrors
   production.
5. Add to `plant_coverage_tests.rs`:
   - `finalize_rejects_plant_with_units_without_segments`: on the computed case, take the `Ok` result of
     `compute_fpha`, drop the plant 2 result, call `finalize_fpha_injection`; it returns
     `InvalidSystem` whose message contains `at least one FPHA segment`;
   - `finalize_accepts_complete_results`: with all results, it returns `Ok` and `fpha_enabled` is true.

### Inputs/Props

The `System` after `read_study_from_path`; the `FphaComputeResult` of `compute_fpha`.

### Outputs/Behavior

Computed runs validate the full `System` with FPHA enabled before the solver; the official example and
every existing computed test keep passing.

### Error Handling

- `InvalidSystem` carries the `CoreError` text; no `unwrap`/`expect` in production code.
- If `System::validate` rejects the unmodified computed example (a rule unrelated to A2 fails), stop
  and return the `CLARIFICATION REQUIRED:` payload with the message; do not relax any rule.

### Out of Scope

- Changing `HydroPlant::validate` or any rule of `labdessem-core`.
- `CHANGELOG.md` (no user-visible change once ticket-014 is in place).
- Validation for the precomputed path (`read_study_from_path` already runs it with segments present).

## Acceptance Criteria

- [ ] Given partial results without plant 2, when `cargo test -p labdessem-cli
  finalize_rejects_plant_with_units_without_segments` runs, then it passes, asserting
  `Err(FphaComputeError::InvalidSystem(msg))` with `msg` containing `at least one FPHA segment`.
- [ ] Given complete results, when `cargo test -p labdessem-cli finalize_accepts_complete_results`
  runs, then it passes, asserting `Ok(())` and `system.fpha_enabled == true`.
- [ ] Given `crates/labdessem-cli/src/main.rs`, when `grep -c "fpha_enabled = true"
  crates/labdessem-cli/src/main.rs` runs, then it prints 0, and `grep -c "finalize_fpha_injection"`
  on the same file prints 1.
- [ ] Given the computed E2E tests, when `cargo test -p labdessem-cli --test fpha_e2e` runs, then every
  test passes, including `computed_csv_round_trip_reproduces_objective` and
  `plant_without_geometry_fails_run`.
- [ ] Given the ticket commit, when `git show --stat HEAD` runs, then it lists exactly the four
  frontmatter files, `cargo test --workspace` exits 0, and clippy warnings do not exceed section 2 of
  the evidence report.

## Implementation Guide

### Suggested Approach

[ASSUMPTION] ADR-026 option (a)+(c): this ticket is the (a) half. Alternative not chosen: (c) alone,
which would drop this ticket.

1. Add the variant and `finalize_fpha_injection` next to `inject_fpha_segments`.
2. Switch `main.rs` and the probe to it.
3. Add the two tests; commit `fix(fpha): validate the system after FPHA injection`.

### Key Files to Modify

- `crates/labdessem-cli/src/fpha_integration.rs`: variant, `Display` arm, function.
- `crates/labdessem-cli/src/main.rs`: two lines replaced by one call.
- `crates/labdessem-cli/src/fpha_integration/plant_coverage_tests.rs`: two tests.
- `crates/labdessem-cli/src/fpha_integration/probe.rs`: `computed_system` body.

### Patterns to Follow

- `?` propagation into `Box<dyn Error>` in `run_and_print`, as for `compute_fpha`.
- The `Display` style of the existing `FphaComputeError` arms.

### Pitfalls to Avoid

- Set `fpha_enabled` before `validate()`; the segment rule only runs with FPHA enabled.
- Keep `write_computed_results` after the finalize call so a rejected system writes no CSV.
- Do not make `inject_fpha_segments` private if other tests call it; only stop calling it from `main`.

## Testing Requirements

### Unit Tests

The two `finalize_*` tests.

### Integration Tests

All probe-based tests (tickets 002-014) keep passing through the new `computed_system`.

### E2E Tests (if applicable)

`cargo test -p labdessem-cli --test fpha_e2e` passes unchanged.

## Dependencies

- **Blocked By**: ticket-014-fail-on-skipped-fpha-plants.md
- **Blocks**: ticket-017-record-post-fix-fpha-evidence.md
- **Notes**: ADR-026 must be accepted with option (a)+(c).

## Effort Estimate

**Points**: 2
**Confidence**: High
**Rationale**: one small function, a two-line call-site change and two tests; the risk is only an
unrelated validation rule failing.
