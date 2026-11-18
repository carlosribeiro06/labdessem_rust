---
filesToBeCreated: []
filesToBeModified:
  - crates/labdessem-cli/src/fpha_integration.rs
  - crates/labdessem-io/src/output/fpha_computed.rs
  - CHANGELOG.md
filesToBeDeleted: []
implements: [ADR-025, R9]
agent: hpc-rust-developer
tier: standard
---

# ticket-012 Align the Computed FPHA Contract with the LP

> **Pending Decisions**: this ticket assumes ADR-025 option (b). Dispatch only when ADR-025 is
> `accepted` with option (b) as `chosen`; otherwise the ticket is re-refined first.

## Context

### Background

After ticket-011, `FphaPlantResult.planes` are the unscaled planes and the model applies alpha once.
Three artifacts still describe the old contract: the unit test `convert_single_plane`
(`crates/labdessem-cli/src/fpha_integration.rs` lines 394-424), which asserts only
`correction_factor == alpha` and never the effective product (audit FP-R03); the preamble of
`resultado_fpha_computed.csv` (`crates/labdessem-io/src/output/fpha_computed.rs` line 59), which says
alpha is "aplicado aos coeficientes" as if the printed planes already carried it; and `CHANGELOG.md`,
which has no entry for the fix or the BREAKING change of the CSV semantics.

### Relation to Epic

Second A1 ticket of Epic 02. ticket-013 relies on the documented copy rule written here.

### Current State

- `convert_to_fpha_segments` maps `rhs = gamma_0`, `volume_coefficient = gamma_v`,
  `turbining_coefficient = gamma_q`, `lateral_flow_coefficient = gamma_s`,
  `correction_factor = alpha`, `segment = index + 1`; it has no doc comment about scaling.
- The CSV preamble pairs every column with an ASCII Portuguese description (`csv_with_header`), the
  project convention for output files.
- `CHANGELOG.md` follows Keep a Changelog with an empty `## [Unreleased]` section.

## Specification

### Requirements

1. Replace the test `convert_single_plane` by `convert_single_plane_lp_product_is_alpha_times_plane`:
   for the same fixture, assert `segment == 1` and, for each of the four coefficients,
   `correction_factor * coefficient == alpha * plane_coefficient` (exact for this fixture) and
   `correction_factor == alpha`.
2. Add a doc comment to `convert_to_fpha_segments`: segments carry the unscaled planes; the model
   multiplies all four coefficients and the RHS by `correction_factor` once.
3. In `fpha_computed.rs`, change only the description strings (ASCII Portuguese, no accents):
   - `gamma_0`, `gamma_v`, `gamma_q`, `gamma_s`: keep the current text and append
     `, sem o fator alpha`;
   - `alpha`: `fator de correcao que o modelo aplica uma vez aos quatro coeficientes (Fcorrec do
     OPER_FPHA)`;
   - the three deviation columns: keep the text and append `de alpha x plano em relacao a FPH`.
4. In `CHANGELOG.md` under `## [Unreleased]`:
   - `### Fixed`: computed FPHA applied alpha twice to gamma_0..gamma_Q and gamma_S without alpha
     (audit FP-02, FP-08); the LP now applies alpha once to all four coefficients;
   - `### Changed`: a `**BREAKING**:` entry for `resultado_fpha_computed.csv`: gamma columns are now
     unscaled, `alpha` is applied once by the model, the deviation columns describe alpha x planes,
     column names/order/format unchanged, and the `OPER_FPHA.csv` copy rule
     (`USIH = codigo_usina`, `SegFPHA = plane_index + 1`, `Fcorrec = alpha`, `Rhs.mw = gamma_0`,
     `Varm.mw/hm3 = gamma_v`, `Qtur.mw/m3s = gamma_q`, `Qlat.mw/m3s = gamma_s`).

### Inputs/Props

The ticket-011 pipeline; `ComputedFphaRecord` (unchanged); the copy rule in the master plan's
"Output Contracts".

### Outputs/Behavior

Same CSV columns and numbers as after ticket-011; new description strings; one replaced unit test; two
changelog entries.

### Error Handling

No runtime behavior changes. If an existing test in `fpha_computed.rs` asserts the old description
text, update that assertion to the new text in this ticket and name it in the commit body.

### Out of Scope

- Column names, column order or number formats of `resultado_fpha_computed.csv`.
- The `OPER_FPHA.csv` reader or header (`crates/labdessem-io/src/readers/`).
- User documentation (`docs/*.html`, `README.md`: ticket-030) and design docs (ticket-029).

## Acceptance Criteria

- [ ] Given the replaced test, when `cargo test -p labdessem-cli
  convert_single_plane_lp_product_is_alpha_times_plane` runs, then it passes, and `grep -c "fn
  convert_single_plane()" crates/labdessem-cli/src/fpha_integration.rs` prints 0.
- [ ] Given a computed derived case run with the release binary, when the preamble of
  `RESULTADOS/resultado_fpha_computed.csv` is read, then the `alpha` line contains `aplica uma vez` and
  `Fcorrec`, and each gamma line contains `sem o fator alpha`.
- [ ] Given `CHANGELOG.md`, when `sed -n '/## \[Unreleased\]/,/## \[0.12.0\]/p' CHANGELOG.md` runs, then
  the output contains a `### Fixed` entry citing FP-02 and FP-08 and a `**BREAKING**` entry naming
  `resultado_fpha_computed.csv` and `Fcorrec = alpha`.
- [ ] Given `cargo test -p labdessem-io fpha_computed`, when it runs, then every test passes and the
  header-columns test is unchanged.
- [ ] Given the ticket commit, when `git show --stat HEAD` runs, then it lists exactly the three
  frontmatter files, `cargo test --workspace` exits 0, and `cargo clippy --workspace --all-targets`
  reports no more warnings than section 2 of the evidence report.

## Implementation Guide

### Suggested Approach

[ASSUMPTION] ADR-025 option (b): planes unscaled, `Fcorrec = alpha`. Under (a1)/(a2) the copy rule
would be `Fcorrec = 1` and the preamble text differs; that is why the ticket is gated.

1. Replace the test and add the doc comment in `fpha_integration.rs`.
2. Edit the description strings in `fpha_computed.rs`.
3. Write the changelog entries.
4. Commit `docs(fpha): document the computed FPHA plane contract`; the body cites FP-R03 and
   ADR-025.

### Key Files to Modify

- `crates/labdessem-cli/src/fpha_integration.rs`: one test replaced, one doc comment.
- `crates/labdessem-io/src/output/fpha_computed.rs`: description strings only.
- `CHANGELOG.md`: `[Unreleased]` entries.

### Patterns to Follow

- The 0.12.0 changelog entries (`**BREAKING**:` with nested bullets for before/now).
- ASCII Portuguese descriptions as in the existing `csv_with_header` call.

### Pitfalls to Avoid

- Do not add accents to CSV preamble strings (the existing outputs are ASCII).
- Floating products in the replaced test: choose the existing fixture values; if any product is not
  exact, compare with `f64::EPSILON * |value|`, never relax to a looser tolerance.
- The semantics changed in ticket-011's commit, which carries the `BREAKING CHANGE:` footer; this
  commit documents the change and must not alter numbers written to the CSV.

## Testing Requirements

### Unit Tests

`convert_single_plane_lp_product_is_alpha_times_plane`; existing `convert_multiple_planes` and
`convert_empty_planes` keep passing.

### Integration Tests

`cargo test -p labdessem-io fpha_computed` and `cargo test --workspace` exit 0.

### E2E Tests (if applicable)

One release CLI run on a computed derived case to read the preamble (acceptance criterion 2).

## Dependencies

- **Blocked By**: ticket-011-apply-alpha-once-in-fit-pipeline.md
- **Blocks**: ticket-013-add-precomputed-round-trip-regression.md
- **Notes**: ADR-025 must be accepted with option (b); ticket-030 repeats the copy rule in user docs.

## Effort Estimate

**Points**: 2
**Confidence**: High
**Rationale**: one test, description strings and changelog text; no logic change.
