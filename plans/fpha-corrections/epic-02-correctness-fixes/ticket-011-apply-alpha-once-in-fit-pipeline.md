---
filesToBeCreated: []
filesToBeModified:
  - crates/labdessem-fpha/src/pipeline.rs
  - crates/labdessem-fpha/tests/regression.rs
  - crates/labdessem-cli/src/fpha_integration/lp_envelope_tests.rs
filesToBeDeleted: []
implements: [ADR-025, R1, R2]
agent: hpc-rust-developer
tier: complex
hpcFencedPaths:
  - crates/labdessem-fpha/src/pipeline.rs
---

# ticket-011 Apply Alpha Once in the FPHA Fit Pipeline

> **Pending Decisions**: this ticket assumes ADR-025 option (b) ("remove scale_planes from fit_plant
> and keep correction_factor = alpha"). Dispatch only when the master-plan frontmatter shows ADR-025
> `status: accepted` with that option as `chosen`; otherwise the ticket is re-refined first.

## Context

### Background

Audit FP-02 / FP-08: `fit_plant` (`crates/labdessem-fpha/src/pipeline.rs` line 85) runs
`let mut planes = scale_planes(&raw_planes, alpha);` before the gamma_S secant fit, and the CLI passes
`correction_factor = alpha` to the model, which multiplies every coefficient again. The LP envelope is
therefore alpha squared times the hull envelope for gamma_0..gamma_Q, and gamma_S carries alpha once.
Under ADR-025 (b) the pipeline returns the unscaled planes and the model alone applies alpha, once, to
all four coefficients; the deviation diagnostic is computed on `scale_planes(&planes, alpha)`, which is
exactly the envelope the LP uses. Section 5 of `docs/auditoria/fpha-baseline-reproduction.md` holds
the pre-fix numbers.

### Relation to Epic

First fix ticket of Epic 02 (A1). ticket-012 documents the changed CSV contract and ticket-013 proves
the `OPER_FPHA.csv` round trip; both depend on this change.

### Current State

- `fit_plant` stages: hull fit (`fit_hull_planes`), `compute_alpha_fpha`, `scale_planes`,
  `fit_gamma_s_for_planes`, `reduce_planes`, `compute_fit_deviation(&planes, &pf, &bounds)`,
  `validate_fitted_planes(&planes, alpha, ..)`.
- `crates/labdessem-fpha/tests/regression.rs` `gamma_s_fitting_is_bit_deterministic` and
  `reduction_is_bit_deterministic` (lines 78-113) build their inputs with `scale_planes(&raw, alpha)`,
  mirroring the old stage order.
- `crates/labdessem-cli/src/fpha_integration/lp_envelope_tests.rs` (ticket-003) holds
  `baseline_a1_lp_envelope_applies_alpha_twice` and
  `baseline_a1_reported_deviation_differs_from_lp_envelope`, both of which fail once alpha is applied
  once; they carry `Replaced by ticket-011.`

## Specification

### Requirements

1. In `fit_plant`, delete the `scale_planes` stage: the gamma_S fit runs on the unscaled hull planes
   (moved into a mutable binding after `compute_alpha_fpha`), the reduction runs on the gamma_S-fitted unscaled planes, and the returned
   `FphaPlantResult.planes` are those unscaled reduced planes.
2. Compute `deviation` as `compute_fit_deviation(&scale_planes(&planes, alpha), &pf, &bounds)`.
3. Update the stage comments and the doc comment of `fit_plant` to state the contract: planes are
   unscaled; `alpha` is applied once, by the consumer, to all four coefficients; the deviation
   describes `alpha x planes`.
4. Add unit tests in the `tests` module of `pipeline.rs`:
   - `fit_plant_returns_unscaled_planes`: for the fixture, the returned planes equal, bit for bit, the
     planes obtained by running `fit_hull_planes`, `fit_gamma_s_for_planes` on a copy, and
     `reduce_planes` with the fixture's arguments (no scaling); the fixture has `|alpha - 1| > 1e-6`.
   - `fit_plant_deviation_matches_alpha_scaled_envelope`: `result.deviation` equals
     `compute_fit_deviation(&scale_planes(&result.planes, result.alpha), ..)` bit for bit in all five
     fields.
5. In `tests/regression.rs`, feed `gamma_s_fitting_is_bit_deterministic` and
   `reduction_is_bit_deterministic` with unscaled planes (mirroring the new order); keep every
   assertion.
6. In `lp_envelope_tests.rs`, delete both `baseline_a1_*` tests and the `unscaled_pre_fix` helper and
   add:
   - `lp_envelope_deviation_matches_reported_deviation`: for every plant, the LP-based deviation
     (`E_LP` from `envelope_mw` at s = 0 on the fit grid, formulas of `compute_fit_deviation`) equals
     `result.deviation` within 1e-9 MW in mean absolute, maximum absolute and RMS; at least one plant
     has `|alpha - 1| > 1e-3`.
   - `lp_envelope_matches_alpha_scaled_planes`: for every plant, at every fit-grid node and for
     s in {0, Q_max / 2, Q_max}, `E_LP == alpha * min_k P_k(v, q, s)` within
     1e-9 x max(1, |value|), with `P_k` the result planes (this pins gamma_S, which now carries alpha).

### Inputs/Props

`FitPlantParams` (unchanged); probe helpers of ticket-002; the computed `DerivedCase`.

### Outputs/Behavior

`FphaPlantResult { planes: unscaled reduced planes, alpha, deviation of alpha x planes }`; the LP built
from it applies alpha exactly once. Public signatures do not change.

### Error Handling

- `validate_fitted_planes(&planes, alpha, ..)` keeps its checks on the unscaled planes (sign checks
  are scale-invariant for alpha > 0).
- No new `unwrap`/`expect` in production code; existing error types are unchanged.
- If the fixture or the example yields `alpha == 1` for every plant, stop and return the
  `CLARIFICATION REQUIRED:` payload (the tests would be vacuous).

### Out of Scope

- `crates/labdessem-cli/src/fpha_integration.rs` (`convert_to_fpha_segments` stays as is; ticket-012
  changes its test and the CSV preamble) and `CHANGELOG.md` (ticket-012).
- Moving the alpha computation after the reduction or changing its points (ADR-033, Epic 04).
- The precomputed path and `fpha_e2e.rs` (ticket-013).

## Acceptance Criteria

- [ ] Given the fixed pipeline, when `cargo test -p labdessem-fpha fit_plant_returns_unscaled_planes`
  runs, then it passes, asserting bitwise equality with the unscaled stage chain for a fixture with
  `|alpha - 1| > 1e-6`.
- [ ] Given the fixed pipeline, when `cargo test -p labdessem-fpha
  fit_plant_deviation_matches_alpha_scaled_envelope` runs, then it passes, and `cargo test -p
  labdessem-fpha --test regression` passes with the same assertion count as before the ticket.
- [ ] Given the computed derived case, when `cargo test -p labdessem-cli
  lp_envelope_deviation_matches_reported_deviation` runs, then it passes, asserting LP-based and
  reported deviations agree within 1e-9 MW for all 4 plants (the same check failed by more than 1e-3
  MW before the fix, per section 5 of the evidence report).
- [ ] Given the computed derived case, when `cargo test -p labdessem-cli
  lp_envelope_matches_alpha_scaled_planes` runs, then it passes for s in {0, Q_max/2, Q_max}, and
  `grep -c "baseline_a1_l\|baseline_a1_r\|unscaled_pre_fix" crates/labdessem-cli/src/fpha_integration/lp_envelope_tests.rs`
  prints 0.
- [ ] Given the ticket commit, when `git show --stat HEAD` runs, then it lists exactly the three
  frontmatter files, `cargo test --workspace` exits 0, and `cargo clippy --workspace --all-targets`
  reports no more warnings than section 2 of the evidence report.

## Implementation Guide

### Suggested Approach

[ASSUMPTION] ADR-025 option (b). Alternatives not chosen here: (a1) `correction_factor = 1.0` in the
CLI, (a2) the same plus gamma_S scaled after the secant fit; either would leave `pipeline.rs` scaling
and change `convert_to_fpha_segments` instead.

1. Edit `fit_plant`: after `compute_alpha_fpha`, move the hull planes into the mutable binding
   (`let mut planes = raw_planes;`, no clone) before `fit_gamma_s_for_planes`; delete the
   `scale_planes` line; compute the deviation on `scale_planes(&planes, alpha)`.
2. Write the two pipeline unit tests; reuse `test_params()` and assert its alpha is not 1.
3. Update the two regression tests' inputs.
4. Replace the A1 tests in `lp_envelope_tests.rs`; run `cargo test -p labdessem-cli lp_envelope --
   --nocapture` and keep the printed table (ticket-017 reads it).
5. Commit `fix(fpha)!: apply the FPHA correction factor once` with a body citing FP-02/FP-08 and
   ADR-025 and the footer `BREAKING CHANGE: resultado_fpha_computed.csv gamma columns are unscaled;
   the model applies alpha once`.

### Key Files to Modify

- `crates/labdessem-fpha/src/pipeline.rs`: stage order, deviation input, doc comment, two tests.
- `crates/labdessem-fpha/tests/regression.rs`: inputs of two determinism tests.
- `crates/labdessem-cli/src/fpha_integration/lp_envelope_tests.rs`: replace the A1 tests.

### Patterns to Follow

- Bitwise comparisons with `to_bits()` as in `fit_plant_is_deterministic` (`pipeline.rs` tests).
- The LP-deviation helper of ticket-003 (`lp_deviation`) for the CLI test.

### Pitfalls to Avoid

- The angle reduction is not scale-invariant: plane counts may change. Do not tune the reduction
  tolerance to keep the old counts; ticket-017 records the change.
- `E_LP / min_k P_k = alpha` held before the fix too; the deviation identity and the unscaled
  stage-chain test are the checks that distinguish the fix, keep both.
- `fit_gamma_s_for_planes` reads a snapshot of all planes; give it the unscaled set, not a mix.

## Testing Requirements

### Unit Tests

`fit_plant_returns_unscaled_planes`, `fit_plant_deviation_matches_alpha_scaled_envelope`; existing
pipeline tests (`fit_plant_is_deterministic`, `planes_pass_validation`, run-of-river) keep passing.

### Integration Tests

`crates/labdessem-fpha/tests/regression.rs` (inputs updated, assertions kept); the two new CLI
in-process tests; `cargo test --workspace` exits 0, including `fpha_e2e` (its A1 test still holds,
because the precomputed copy with `Fcorrec = alpha` reproduces the computed run before and after).

### E2E Tests (if applicable)

`cargo test -p labdessem-cli --test fpha_e2e` runs unchanged and passes.

## Dependencies

- **Blocked By**: ticket-008-decide-alpha-application.md, ticket-003-characterize-double-alpha-envelope.md
- **Blocks**: ticket-012-align-computed-fpha-contract-with-lp.md
- **Notes**: ADR-025 must be accepted with option (b) before dispatch.

## Effort Estimate

**Points**: 3
**Confidence**: Medium
**Rationale**: a small production change in a fenced numerical file, with plane-count side effects and
four test changes across two crates.
