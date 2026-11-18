---
filesToBeCreated:
  - crates/labdessem-cli/src/fpha_integration/physical_data_tests.rs
filesToBeModified:
  - crates/labdessem-cli/src/fpha_integration.rs
filesToBeDeleted: []
implements: [R8, R4]
agent: monorepo-test-developer
tier: standard
---

# ticket-006 Characterize Silent FPHA Physical Defaults

## Context

### Background

Finding A3 (audit FP-R02 for efficiency and loss; the missing tailrace is not in the audit) states that
computed FPHA silently replaces missing physical data: `build_fit_params`
(`crates/labdessem-cli/src/fpha_integration.rs` lines 204-206, 246-247) passes `tailrace: None` when
`CAD/CAD_TAILRACE.csv` has no row for the plant and passes `plant.efficiency` /
`plant.hydraulic_loss_m` as `None` when `CAD/CAD_UHE.csv` lacks the `Eficiencia` / `Perda.m` columns
(serde `default` in `crates/labdessem-io/src/readers/hydro.rs` lines 453-456). `ProductionFunction::new`
(`crates/labdessem-fpha/src/fitting/production.rs` lines 47-48) turns them into loss 0 m and efficiency
1.0, and `tailrace_level` returns 0.0 m for `TailraceSource::Entity(None)` (line 67). No warning is
printed. The audit measured FURNAS FPH about +9 % with efficiency 1 and loss 0.

### Relation to Epic

Fifth evidence ticket of Epic 01 (finding A3). ticket-016 flips these tests once ADR-027 is accepted.

### Current State

- The probe (`crates/labdessem-cli/src/fpha_integration/probe.rs`, ticket-002) provides
  `DerivedCase::{remove_plant_rows, drop_columns}`, `computed_system`, `fit_contexts`,
  `fit_grid_nodes`, `lp_fpha_rows` and `envelope_mw`.
- `FitContext.params` is the `FitPlantParams` built by `build_fit_params`; `FitContext.pf` exposes
  `evaluate(v, q, s)` (uncapped FPH, MW) and `net_head(v, outflow)`.
- `CAD/CAD_TAILRACE.csv` header: `codigo_usina;a0.m;a1.m/m3s;a2.m/m3s2;a3.m/m3s3;a4.m/m3s4`, one row
  per plant code 0-3; `CAD/CAD_UHE.csv` ends with the columns `Perda.m;Eficiencia`.

## Specification

### Requirements

1. Create `crates/labdessem-cli/src/fpha_integration/physical_data_tests.rs`, declared in
   `fpha_integration.rs` with the same attributes as `mod probe;`.
2. `baseline_a3_missing_tailrace_row_uses_zero_level`: remove plant 3 rows from
   `CAD/CAD_TAILRACE.csv`; `computed_system` returns `Ok` with `plants_skipped == 0`; the plant 3
   context has `params.tailrace.is_none()`; at every fit-grid node `pf.net_head(v, q)` equals
   `(params.forebay.height(v) - loss).max(0.0)` within 1e-9 m, with `loss` the plant 3
   `params.hydraulic_loss_m` value (tailrace level 0 m); the mean over nodes with
   q > 0 of `pf_missing.evaluate(v, q, 0) / pf_control.evaluate(v, q, 0)` is above 1.01, where the
   control context comes from an unmodified case.
3. `baseline_a3_missing_efficiency_and_loss_default_silently`: drop `Perda.m` and `Eficiencia` from
   `CAD/CAD_UHE.csv`; every hydro plant in the `System` has `efficiency == None` and
   `hydraulic_loss_m == None`; `computed_system` returns `Ok` with `plants_skipped == 0`; for every
   plant, the mean FPH ratio (as in requirement 2) is above 1.01.
4. `control_a3_official_inputs_carry_physical_data`: unmodified case; every plant with groups has
   `Some` efficiency, `Some` hydraulic loss and `params.tailrace.is_some()`.
5. Both `baseline_a3_*` tests print one line per affected plant: code, name, mean and maximum FPH
   ratio, and the mean ratio of the LP envelope (`envelope_mw` at s = 0, derived over control).
6. Each `baseline_a3_*` test carries the doc comment `Characterizes audit FP-R02 (and the missing
   tailrace) on the pre-fix code. Replaced by ticket-016.`

### Inputs/Props

Computed `DerivedCase` variants; plant code 3 (FURNAS) for the tailrace case; all four plants for the
efficiency/loss case; an unmodified case as the control.

### Outputs/Behavior

Three passing tests plus printed ratios for section 7 of the evidence report.

### Error Handling

- Test-only code; helper results are unwrapped with `expect` messages naming the plant code.
- If the pre-fix code skips plant 3 (fit failure) instead of fitting it with a 0 m tailrace, stop and
  return the `CLARIFICATION REQUIRED:` payload: the A3 premise differs from the audit and the gate
  (ADR-027) needs the new evidence.

### Out of Scope

- EngMax use of the same defaults (`ProductionFunction` in `labdessem-fpha/src/calculator/`), audit
  EM-01..EM-03.
- Explicit zero values (`Perda.m = 0`) present in the CSV; ADR-027 decides whether they stay valid.
- Any production code change; end-to-end runs (the in-process evidence is sufficient for A3).

## Acceptance Criteria

- [ ] Given a computed derived case without plant 3 tailrace rows, when `cargo test -p labdessem-cli
  baseline_a3_missing_tailrace_row_uses_zero_level` runs, then it passes, asserting `Ok`, zero skipped
  plants, `params.tailrace.is_none()`, `net_head == (height - loss).max(0)` within 1e-9 m at every
  fit-grid node, and a mean FPH ratio above 1.01.
- [ ] Given a computed derived case without `Perda.m` and `Eficiencia`, when `cargo test -p
  labdessem-cli baseline_a3_missing_efficiency_and_loss_default_silently` runs, then it passes,
  asserting `None` efficiency and loss on all 4 plants, `Ok` with zero skipped plants, and a mean FPH
  ratio above 1.01 for every plant.
- [ ] Given the unmodified computed case, when `cargo test -p labdessem-cli
  control_a3_official_inputs_carry_physical_data` runs, then it passes, asserting `Some` efficiency,
  `Some` loss and a tailrace model for every plant with groups.
- [ ] Given `cargo test -p labdessem-cli baseline_a3 -- --nocapture`, when it runs, then stderr holds
  one line per affected plant with mean and maximum FPH ratio and mean LP-envelope ratio, and
  `grep -c "Replaced by ticket-016" crates/labdessem-cli/src/fpha_integration/physical_data_tests.rs`
  prints 2.
- [ ] Given the ticket commit, when `git show --stat HEAD` runs, then it lists exactly the two
  frontmatter files, `cargo test --workspace` exits 0, and `cargo clippy --workspace --all-targets`
  reports no more warnings than section 2 of the evidence report.

## Implementation Guide

### Suggested Approach

1. Add the module declaration beside `mod probe;`.
2. Build a control `DerivedCase` and its `fit_contexts` once per test; build the derived case, edit
   it, and compute its contexts and `System`.
3. Write a private helper `fph_ratio(missing: &FitContext, control: &FitContext) -> (f64, f64)`
   returning the mean and maximum ratio over `fit_grid_nodes(control)` with q > 0 and a control FPH
   of at least 1 MW.
4. Commit `test(fpha): characterize silent physical defaults in computed FPHA`.

### Key Files to Modify

- `crates/labdessem-cli/src/fpha_integration/physical_data_tests.rs` (create): A3 tests.
- `crates/labdessem-cli/src/fpha_integration.rs`: the module declaration only.

### Patterns to Follow

- The probe helpers and `super::probe::{...}` imports used by ticket-003 and ticket-005.
- `baseline_` / `control_` naming.

### Pitfalls to Avoid

- Use the control grid nodes for both contexts: the volume bounds are equal (same VHA rows), but the
  derived context must not choose its own nodes.
- `evaluate_capped` hides the inflation at Pmax; measure with the uncapped `evaluate`.
- `drop_columns` must keep `;` alignment of every data line; the hydro reader maps by header name.

## Testing Requirements

### Unit Tests

The three tests above; each `baseline_a3_*` test fails if `build_fit_params` rejected the missing
input (the `Ok` assertion), which is how ticket-016 flips them.

### Integration Tests

`cargo test --workspace` exits 0; tickets 002-005 tests keep passing.

### E2E Tests (if applicable)

None.

## Dependencies

- **Blocked By**: ticket-002-add-fpha-lp-envelope-probe.md
- **Blocks**: ticket-007-compile-fpha-baseline-evidence.md, ticket-016-reject-missing-fpha-physical-data.md
- **Notes**: independent of ticket-003, ticket-004 and ticket-005; it can run in parallel with them.

## Effort Estimate

**Points**: 2
**Confidence**: High
**Rationale**: the probe exposes the fit contexts; the work is two derived cases and ratio statistics.
