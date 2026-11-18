---
filesToBeCreated:
  - crates/labdessem-cli/src/fpha_integration/lp_envelope_tests.rs
filesToBeModified:
  - crates/labdessem-cli/src/fpha_integration.rs
filesToBeDeleted: []
implements: [R8, R1, R2]
agent: monorepo-test-developer
tier: standard
---

# ticket-003 Characterize the Double-Alpha LP Envelope

## Context

### Background

Audit finding FP-02 (with FP-08) states that computed FPHA applies alpha twice:
`crates/labdessem-fpha/src/pipeline.rs` line 85 runs `scale_planes(&raw_planes, alpha)` (all four
coefficients), the gamma_S secant fit then overwrites gamma_S without alpha, and
`convert_to_fpha_segments` hands the scaled planes to the model with `correction_factor = alpha`.
The audit measured `G / printed plane = alpha` (CAMARGOS 0.969607, FURNAS 0.991295) with a release
binary and a cache that no longer exists. This ticket reproduces the defect in-process, exactly, and
records the numbers the decision brief (ticket-008) needs.

### Relation to Epic

Second evidence ticket of Epic 01. ticket-011 replaces both tests once ADR-025 is accepted: the
pipeline fix changes the envelope and the deviation in the same commit.

### Current State

- The probe of ticket-002 (`crates/labdessem-cli/src/fpha_integration/probe.rs`) provides
  `DerivedCase`, `computed_system`, `lp_fpha_rows`, `envelope_mw`, `fit_contexts`, `fit_grid_nodes`
  and `evaluation_grid_nodes`.
- `FphaPlantResult { codigo, nome, planes: Vec<RawPlane>, alpha, deviation: FphaFitDeviation }`;
  `RawPlane::evaluate(v, q, s)`; `FphaFitDeviation { mean_abs_mw, max_abs_mw, rms_mw,
  mean_signed_mw, relative }` computed by `compute_fit_deviation` on the fit grid at s = 0.
- Before the fix, the unscaled fitted plane U_k is recoverable exactly from a result plane P_k:
  `U_k = (gamma_0/alpha, gamma_v/alpha, gamma_q/alpha, gamma_s)`, because `scale_planes` runs before
  the gamma_S fit replaces gamma_S.

## Specification

### Requirements

1. Create `crates/labdessem-cli/src/fpha_integration/lp_envelope_tests.rs`, declared in
   `fpha_integration.rs` as `#[cfg(test)] #[allow(clippy::unwrap_used, clippy::expect_used,
   clippy::float_cmp)] mod lp_envelope_tests;`.
2. `baseline_a1_lp_envelope_applies_alpha_twice`: on `DerivedCase::new("computed")`, for every plant,
   with `intended(v, q) = alpha * min_k U_k(v, q, 0)` and `E_LP` from `envelope_mw` at s = 0, assert
   `|E_LP / intended - alpha| <= 1e-9` at every fit-grid node with q > 0 and intended >= 1 MW. Guard
   against a vacuous pass: assert at least one plant has `|alpha - 1| > 1e-3`.
3. `baseline_a1_reported_deviation_differs_from_lp_envelope`: at the fit-grid nodes, s = 0, compute
   two deviations against `ctx.pf.evaluate_capped(v, q, 0.0)` with the formulas of
   `compute_fit_deviation` (mean absolute, maximum absolute, RMS): `dev_p` of `min_k P_k` (the result
   planes as returned) and `dev_lp` of `E_LP`. Assert, for every plant, that `result.deviation`
   equals `dev_p` within 1e-9 MW in all three statistics (the diagnostic measures the returned planes),
   and, for plants with `|alpha - 1| > 1e-3`, that `|dev_lp.max_abs - result.deviation.max_abs_mw| >
   1e-3` MW (the diagnostic misses the LP envelope).
4. Both tests print a measurement table with `eprintln!` (one line per plant): code, name, alpha (6
   decimals), plane count, minimum and maximum of `E_LP / intended`, reported and LP-based
   `max_abs/mean_abs/rms` deviation on the fit grid, and the LP-based deviation on
   `evaluation_grid_nodes(ctx, 11)`.
5. Each test carries the doc comment `Characterizes audit FP-02/FP-08 on the pre-fix code. Replaced by
   ticket-011.`

### Inputs/Props

The computed `DerivedCase` (all 4 plants fitted), `FphaComputeResult.plant_results`, the LP model
built with `labdessem_model::Model::from_system(&system, SolveMode::LinearProgramming)`.

### Outputs/Behavior

Two passing tests that pin the exact pre-fix signature and print the numbers ticket-007 copies into
section 5 of the evidence report.

### Error Handling

Test-only code: helper results are unwrapped with `expect` messages that name the plant code. A plant
missing from `fit_contexts` or from the LP rows fails the test with that plant named.

### Out of Scope

- Any change to production code (`pipeline.rs`, `convert_to_fpha_segments`, the model).
- Spillage nodes (s > 0): the pre-fix gamma_S mismatch is printed but not asserted here; ticket-011
  asserts the s > 0 identity after the fix.
- End-to-end objective comparisons (ticket-004).

## Acceptance Criteria

- [ ] Given the computed derived case on the pre-fix code, when `cargo test -p labdessem-cli
  baseline_a1_lp_envelope_applies_alpha_twice` runs, then it passes, asserting `|E_LP / intended -
  alpha| <= 1e-9` at every qualifying fit-grid node of every plant and that at least one plant has
  `|alpha - 1| > 1e-3`.
- [ ] Given the same case, when `cargo test -p labdessem-cli
  baseline_a1_reported_deviation_differs_from_lp_envelope` runs, then it passes, asserting that
  `result.deviation` equals the deviation of `min_k P_k` within 1e-9 MW for every plant, and that the
  LP-based maximum deviation differs from `result.deviation.max_abs_mw` by more than 1e-3 MW for every
  plant with `|alpha - 1| > 1e-3`.
- [ ] Given `cargo test -p labdessem-cli baseline_a1 -- --nocapture`, when it runs, then stderr holds
  one table line per plant code 0, 1, 2 and 3 with alpha, plane count, ratio range, reported and
  LP-based deviations on the fit grid, and the LP-based deviation on the 11 x 11 grid.
- [ ] Given `crates/labdessem-cli/src/fpha_integration/lp_envelope_tests.rs`, when
  `grep -c "Replaced by ticket-011" crates/labdessem-cli/src/fpha_integration/lp_envelope_tests.rs`
  runs, then it prints 2.
- [ ] Given the ticket commit, when `git show --stat HEAD` runs, then it lists exactly the two
  frontmatter files, `cargo test --workspace` exits 0, and `cargo clippy --workspace --all-targets`
  reports no more warnings than section 2 of the evidence report.

## Implementation Guide

### Suggested Approach

1. Add the module declaration beside `mod probe;`.
2. Write a private helper `unscaled_pre_fix(plane: &RawPlane, alpha: f64) -> RawPlane` with a doc
   comment stating it is valid only while `fit_plant` scales before the gamma_S fit.
3. Write a private helper `lp_deviation(rows, ctx, nodes) -> (f64, f64, f64)` returning
   `(mean_abs, max_abs, rms)`.
4. Write the two tests and the shared printing helper; commit
   `test(fpha): characterize double alpha in computed FPHA envelope`.

### Key Files to Modify

- `crates/labdessem-cli/src/fpha_integration/lp_envelope_tests.rs` (create): both tests and helpers.
- `crates/labdessem-cli/src/fpha_integration.rs`: the module declaration only.

### Patterns to Follow

- Use `super::probe::{...}` imports; no `use super::*`.
- Formulas of `crates/labdessem-fpha/src/fitting/deviation.rs` (`compute_fit_deviation`) for the
  LP-based deviation, so both numbers are comparable.

### Pitfalls to Avoid

- Do not compare against `min_k P_k` alone: before and after the fix `E_LP / min_k P_k = alpha`, so
  that ratio cannot tell the two apart; the reconstruction U_k carries the pre-fix contract.
- Skip nodes with q = 0 or intended below 1 MW; the ratio is undefined there.
- ITUTINGA and FUNIL-GRANDE have alpha = 1.000000 in the audit; they are the control plants.

## Testing Requirements

### Unit Tests

The two `baseline_a1_*` tests are the deliverable; each must fail if alpha were applied once (verify
by reasoning in the doc comment: `E_LP / intended` would be 1).

### Integration Tests

`cargo test --workspace` exits 0; probe tests of ticket-002 keep passing.

### E2E Tests (if applicable)

None (ticket-004).

## Dependencies

- **Blocked By**: ticket-002-add-fpha-lp-envelope-probe.md
- **Blocks**: ticket-007-compile-fpha-baseline-evidence.md, ticket-011-apply-alpha-once-in-fit-pipeline.md
- **Notes**: ticket-011 replaces the two tests created here.

## Effort Estimate

**Points**: 2
**Confidence**: High
**Rationale**: the probe supplies the data; the work is two exact assertions and a printed table.
