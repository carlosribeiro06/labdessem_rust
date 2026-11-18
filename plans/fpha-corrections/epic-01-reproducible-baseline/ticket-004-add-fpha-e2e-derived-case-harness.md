---
filesToBeCreated:
  - crates/labdessem-cli/tests/fpha_e2e.rs
filesToBeModified: []
filesToBeDeleted: []
implements: [R8, R9]
agent: monorepo-test-developer
tier: standard
---

# ticket-004 Add FPHA End-to-End Derived-Case Harness

## Context

### Background

The audit measured A1 end to end: the computed run's objective sat +3,125.65 R$ above a precomputed
run built from the same printed planes with `Fcorrec = 1` (audit section 12.4), and it found that the
precomputed path had never been exercised on the example (MX-06: "missing FPHA data for hydro plant
0", because `examples/24Barras/OPER/OPER_FPHA.csv` is header-only). Those runs lived in a cache that
no longer exists. This ticket rebuilds them as an automated test binary that drives the real CLI on
derived copies of `examples/24Barras`, in computed and precomputed modes.

### Relation to Epic

Third evidence ticket of Epic 01. ticket-005 adds the A2 run to this file; ticket-013 turns the A1
test into the contract round-trip regression; ticket-014 flips the A2 run.

### Current State

- `crates/labdessem-cli/tests/integration.rs` shows the pattern: `Command::new(env!("CARGO_BIN_EXE_
  labdessem")).args(["run", <dir>])`; it is the only CLI integration test and is `#[ignore]`d.
- The CLI prints `  Objective:   {:.4}` on stderr; computed mode writes
  `<case>/RESULTADOS/resultado_fpha_computed.csv` (preamble lines, then the header
  `codigo_usina;nome_usina;plane_index;gamma_0;gamma_v;gamma_q;gamma_s;alpha;max_deviation_mw;
  mean_deviation_mw;rms_deviation_mw`, numbers formatted `{:.6}` for gamma_0, gamma_q, alpha and
  `{:.9}` for gamma_v, gamma_s).
- `OPER_FPHA.csv` header: `USIH;Nome;SegFPHA;Fcorrec;Rhs.mw;Varm.mw/hm3;Qtur.mw/m3s;Qlat.mw/m3s`;
  `SegFPHA` must be at least 1 (`HydroFphaSegment::validate`).
- The current preamble describes `alpha` as "fator de correcao aplicado aos coeficientes", that is,
  the gamma columns already include alpha, so the documented copy uses `Fcorrec = 1`.
- ticket-002 added `tempfile = "3"` to the crate's dev-dependencies; section 4 of
  `docs/auditoria/fpha-baseline-reproduction.md` holds the debug run time of the fast configuration.

## Specification

### Requirements

1. Create `crates/labdessem-cli/tests/fpha_e2e.rs` with `#![allow(clippy::unwrap_used,
   clippy::expect_used, clippy::float_cmp)]` and private helpers:
   - `derived_case(fpha_mode: &str) -> TempDir`: copy `examples/24Barras/CAD` and `OPER` and write
     the fast configuration JSON of ticket-001 with `"FPHA": fpha_mode`;
   - `run_cli(dir: &Path) -> RunOutcome { success: bool, code: Option<i32>, stderr: String,
     objective: Option<f64>, elapsed_s: f64 }` parsing the `  Objective:` line;
   - `read_computed_planes(dir: &Path) -> Vec<ComputedPlane>` (skip lines until the header starting
     with `codigo_usina;`);
   - `write_oper_fpha(dir: &Path, planes: &[ComputedPlane], fcorrec: Fcorrec)` with `enum Fcorrec {
     One, Alpha }`, writing `SegFPHA = plane_index + 1` and the CSV values verbatim.
2. `harness_derived_case_matches_official_inputs` (no solve): the copied files equal the official
   bytes and the official tree is untouched.
3. `baseline_a1_computed_vs_precomputed_copies` runs three solves:
   - computed case: exit 0, objective `O_c`, CSV rows for plant codes 0, 1, 2, 3;
   - precomputed case with `Fcorrec = Alpha`: exit 0, `|O_alpha - O_c| <= 1e-5 * |O_c|`;
   - precomputed case with `Fcorrec = One` (the copy the current preamble documents): exit 0,
     `|O_one - O_c| > 1e-5 * |O_c|`.
   It prints the three objectives, both relative gaps and each run's elapsed seconds with `eprintln!`.
4. Doc comment on the A1 test: `Characterizes audit FP-02 and MX-06 on the pre-fix code. Rewritten by
   ticket-013 into the OPER_FPHA round-trip regression.`
5. No `#[ignore]` in the file.

### Inputs/Props

Fast configuration of ticket-001; `examples/24Barras` read-only; the computed run's
`resultado_fpha_computed.csv`.

### Outputs/Behavior

One new test binary with two passing tests; precomputed runs exercise `read_fpha_table` and the
precomputed segment path for the first time.

### Error Handling

- Every helper failure aborts the test with an `expect` message naming the file or the run.
- A run that exits nonzero fails the test with its full stderr in the panic message.
- If the debug run time recorded in section 4 of the evidence report is above 120 s, stop before
  writing the tests and return the `CLARIFICATION REQUIRED:` payload (options: run this binary only
  with `--release`, add a `slow-tests` feature, reduce the horizon of the derived case).

### Out of Scope

- The A2 missing-geometry run (ticket-005) and any assertion about A3.
- Changing `crates/labdessem-cli/tests/integration.rs` or the `#[ignore]` reference test it holds.
- Any production code or output format change.

## Acceptance Criteria

- [ ] Given the new test binary, when `cargo test -p labdessem-cli --test fpha_e2e
  harness_derived_case_matches_official_inputs` runs, then it passes, asserting byte equality of every
  copied `CAD/` and `OPER/` file with `examples/24Barras`.
- [ ] Given the pre-fix code, when `cargo test -p labdessem-cli --test fpha_e2e
  baseline_a1_computed_vs_precomputed_copies` runs, then it passes, asserting the computed run exits 0
  with a parsed objective and CSV rows for codes 0-3, the `Fcorrec = Alpha` copy exits 0 within 1e-5
  relative of `O_c`, and the `Fcorrec = One` copy exits 0 more than 1e-5 relative away from `O_c`.
- [ ] Given each precomputed run, when its stderr is inspected by the test, then it contains
  `FPHA=precomputed` (printed by `banner::print_execution_info`) and does not contain
  `Computing FPHA hyperplanes`.
- [ ] Given `cargo test -p labdessem-cli --test fpha_e2e -- --nocapture`, when it runs, then stderr
  shows the three objectives, both relative gaps and the elapsed seconds of each run, and
  `grep -c "#\[ignore" crates/labdessem-cli/tests/fpha_e2e.rs` prints 0.
- [ ] Given the ticket commit, when `git show --stat HEAD` runs, then it lists only
  `crates/labdessem-cli/tests/fpha_e2e.rs`, `cargo test --workspace` exits 0, and `cargo clippy
  --workspace --all-targets` reports no more warnings than section 2 of the evidence report.

## Implementation Guide

### Suggested Approach

1. Read section 4 of `docs/auditoria/fpha-baseline-reproduction.md`; apply the 120 s rule.
2. Write the helpers with `std::fs` and `std::process::Command`; measure time with
   `std::time::Instant`.
3. Write the two tests; run them with `--nocapture` and check the printed objectives against
   section 4 (the computed objective must match the release-binary value to 4 decimals).
4. Commit `test(fpha): add end-to-end FPHA derived-case harness`.

### Key Files to Modify

- `crates/labdessem-cli/tests/fpha_e2e.rs` (create): helpers and tests for every FPHA end-to-end
  case of this plan.

### Patterns to Follow

- `crates/labdessem-cli/tests/integration.rs` for invoking the binary.
- One test binary for all FPHA end-to-end cases (rules: a new test binary needs a reason; this file
  is the single home of every later FPHA E2E case).

### Pitfalls to Avoid

- The helpers here duplicate a few lines of the bin-crate probe; an integration test cannot import
  bin modules, so the duplication is intentional and limited to file copying.
- The `Objective:` value is printed with 4 decimals; compare with the relative tolerance only.
- `OPER_FPHA.csv` lives at `OPER/OPER_FPHA.csv`; overwrite the header-only copy, keep the header
  spelling exactly.

## Testing Requirements

### Unit Tests

None outside the test binary.

### Integration Tests

`cargo test --workspace` exits 0 with the new binary included.

### E2E Tests (if applicable)

The two tests above; their stderr output feeds section 5 of the evidence report (ticket-007).

## Dependencies

- **Blocked By**: ticket-002-add-fpha-lp-envelope-probe.md
- **Blocks**: ticket-005-characterize-skipped-plant-generation.md, ticket-007-compile-fpha-baseline-evidence.md, ticket-013-add-precomputed-round-trip-regression.md
- **Notes**: needs the `tempfile` dev-dependency and the debug run time recorded upstream.

## Effort Estimate

**Points**: 3
**Confidence**: Medium
**Rationale**: the precomputed path has never run on this example (MX-06) and may expose reader
issues; solve time in the debug profile is not yet known.
