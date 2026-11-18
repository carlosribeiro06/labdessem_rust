---
filesToBeCreated:
  - crates/labdessem-cli/src/fpha_integration/probe.rs
filesToBeModified:
  - crates/labdessem-cli/src/fpha_integration.rs
  - crates/labdessem-cli/Cargo.toml
  - Cargo.lock
filesToBeDeleted: []
implements: [R8]
agent: monorepo-test-developer
tier: standard
---

# ticket-002 Add FPHA LP Envelope Probe

## Context

### Background

A1 is a statement about the LP: the `hydro_fpha[...]` rows built by
`crates/labdessem-model/src/constraints/hydro.rs` (`build_hydro_fpha_constraints`) multiply every
coefficient and the RHS by `correction_factor`, and `crates/labdessem-cli/src/fpha_integration.rs`
(`convert_to_fpha_segments`) sets that factor to an alpha that `fit_plant` already applied. To prove
the defect and later its fix, the tests must read the rows the model actually builds and evaluate the
generation cap they imply, independently of the segment structs. No such harness exists, and the
audit's test cache (`~/.cache/labdessem-audit`) is gone.

### Relation to Epic

Foundation of Epic 01: ticket-003, ticket-005 and ticket-006 build their characterization tests on
this probe, and Epic 02 reuses it for the regression tests.

### Current State

- `labdessem-cli` is a bin-only crate (`[[bin]] name = "labdessem"`); its modules are private, so
  in-process tests that call `compute_fpha` must be `#[cfg(test)]` modules inside `src/`.
- `labdessem_model::Model::from_system(&system, SolveMode::LinearProgramming)` exposes
  `constraints.hydro_fpha()` returning `Vec<&LinearConstraint>`; each `LinearConstraint` has `name`,
  `terms: Vec<LinearTerm { variable: String, coefficient: f64 }>`, `sense` and `rhs`.
- Row names follow `hydro_fpha[p=<plant name>,seg=<k>,t=<period + 1>]`; variable names start with
  `hydro_generation[`, `hydro_turbining[`, `hydro_volume[` and `hydro_spillage[`.
- In the rows, turbining and spillage variables are in hm3 per period; the model converts with
  `flow_conversion = system.horizon.period_duration_hours * 0.0036`.
- `crates/labdessem-cli/Cargo.toml` `[dev-dependencies]` holds only `chrono = "0.4"`; `tempfile = "3"`
  is already in the lockfile through `labdessem-fpha` and `labdessem-io`.

## Specification

### Requirements

1. Create `crates/labdessem-cli/src/fpha_integration/probe.rs` and declare it in
   `fpha_integration.rs` as `#[cfg(test)] #[allow(clippy::unwrap_used, clippy::expect_used,
   clippy::float_cmp)] mod probe;`. It is test-only and never compiled into the binary.
2. Provide the items below with `pub(super)` visibility (sibling test modules use them):
   - `struct DerivedCase` (owns a `tempfile::TempDir`): `new(fpha_mode: &str) -> io::Result<Self>`
     copies `examples/24Barras/CAD` and `examples/24Barras/OPER` recursively from
     `concat!(env!("CARGO_MANIFEST_DIR"), "/../../examples/24Barras")` and writes the fast
     configuration JSON of ticket-001 with `"FPHA": <fpha_mode>`; `path() -> &Path`;
     `remove_plant_rows(relative: &str, plant_code: u32) -> io::Result<usize>` (drops data lines whose
     first `;` field, trimmed, equals the code; keeps the header; returns the count);
     `drop_columns(relative: &str, columns: &[&str]) -> io::Result<()>`;
     `remove_file(relative: &str) -> io::Result<()>`.
   - `fn computed_system(case: &Path) -> Result<(System, FphaComputeResult), Box<dyn Error>>`:
     `read_study_from_path`, `super::compute_fpha`, `super::inject_fpha_segments`, then
     `system.fpha_enabled = true` (the sequence of `main.rs` lines 131-135).
   - `struct EnvelopeRow { rhs_mw, volume_mw_per_hm3, turbining_mw_per_m3s, spillage_mw_per_m3s }`
     and `fn lp_fpha_rows(model: &Model, plant_name: &str, flow_conversion: f64) -> Vec<EnvelopeRow>`
     reading the period-1 rows of one plant: `rhs_mw = rhs`, `volume_mw_per_hm3 = -c_volume`,
     `turbining_mw_per_m3s = -c_turbining * flow_conversion`,
     `spillage_mw_per_m3s = -c_spillage * flow_conversion`; it asserts that every unit turbining
     term of a row has the same coefficient.
   - `fn envelope_mw(rows: &[EnvelopeRow], v_hm3: f64, q_m3s: f64, s_m3s: f64) -> f64` (minimum over
     rows, `f64::INFINITY` when empty) and `fn segment_envelope_mw(segments: &[HydroFphaSegment], v_hm3,
     q_m3s, s_m3s) -> f64` (minimum of `cf * (rhs + vc*v + tc*q + lc*s)`).
   - `struct FitContext { params: FitPlantParams, pf: ProductionFunction, bounds: FittingBounds }` and
     `fn fit_contexts(case: &Path, system: &System) -> Result<BTreeMap<i32, FitContext>,
     FphaComputeError>`: load VHA, tailrace, config and the MLT table as `compute_fpha` does, call
     `super::build_fit_params`, then build `pf` and `bounds` with the exact statements of
     `labdessem_fpha::pipeline::fit_plant` (lines 41-67); plants without VHA rows are omitted.
   - `fn fit_grid_nodes(ctx: &FitContext) -> Vec<(f64, f64)>` (`labdessem_fpha::build_grid`
     `v_points` x `q_points`) and `fn evaluation_grid_nodes(ctx: &FitContext, n: usize) ->
     Vec<(f64, f64)>` (n x n uniform nodes over `[bounds.v_min, bounds.v_max]` x
     `[0, pf.max_turbined_m3s()]`).
3. Add four control tests (they pass now and after every Epic 02 fix): see Acceptance Criteria.
4. Add `tempfile = "3"` to `[dev-dependencies]` of `crates/labdessem-cli/Cargo.toml`.

### Inputs/Props

`examples/24Barras` (read-only), the fast configuration JSON of ticket-001, plant codes 0 CAMARGOS,
1 ITUTINGA, 2 FUNIL-GRANDE, 3 FURNAS (names are read from the `System`, never hard-coded in helpers).

### Outputs/Behavior

Test-only helpers plus four passing tests; the release binary and every production module are
unchanged except the one `mod probe;` declaration.

### Error Handling

Helpers return `io::Result` or `Result<_, Box<dyn Error>>`; tests unwrap them with `expect("...")`
messages naming the file or plant. No helper panics on a missing row: `envelope_mw` returns
`f64::INFINITY` for a plant without rows, which ticket-005 relies on.

### Out of Scope

- Any assertion about the A1, A2 or A3 defects (tickets 003, 005, 006).
- End-to-end CLI runs (ticket-004) and solver calls.
- Refactoring `compute_fpha` or `build_fit_params` to share the data loading with the probe.

## Acceptance Criteria

- [ ] Given the probe, when `cargo test -p labdessem-cli probe_derived_case_copies_official_example`
  runs, then it passes, asserting that every file under `examples/24Barras/CAD` and
  `examples/24Barras/OPER` exists in the copy with identical bytes and that
  `labdessem_io::read_study_config` reads the copy's `study_config.json` with `fpha.is_computed()`.
- [ ] Given a derived case, when `cargo test -p labdessem-cli probe_derived_case_edits_stay_local`
  runs, then it passes, asserting that `remove_plant_rows("CAD/CAD_GEOMETRIA_UHE.csv", 2)` returns a
  count above 0, the copy keeps no data line whose first field is `2`, `drop_columns("CAD/CAD_UHE.csv",
  &["Perda.m", "Eficiencia"])` leaves a header without those names, and the bytes of
  `examples/24Barras/CAD/CAD_GEOMETRIA_UHE.csv` and `CAD_UHE.csv` equal their pre-test snapshot.
- [ ] Given the computed derived case, when `cargo test -p labdessem-cli
  probe_lp_rows_match_injected_segments` runs, then it passes, asserting for each of the 4 plants that
  the period-1 row count equals the segment count and that `envelope_mw` equals `segment_envelope_mw`
  within 1e-9 x max(1, |value|) at every fit-grid node for s in {0, Q_max/2, Q_max}.
- [ ] Given the computed derived case, when `cargo test -p labdessem-cli
  probe_fit_context_reproduces_pipeline_alpha` runs, then it passes, asserting for each plant that
  `compute_alpha_fpha(&fit_hull_planes(&ctx.pf, &ctx.bounds)?, &ctx.pf, &ctx.bounds).to_bits()` equals
  the alpha bits returned by `compute_fpha`.
- [ ] Given the ticket commit, when `git show --stat HEAD` runs, then it lists exactly the four
  frontmatter files, `cargo test --workspace` exits 0, and `cargo clippy --workspace --all-targets`
  reports no more warnings than section 2 of `docs/auditoria/fpha-baseline-reproduction.md`.

## Implementation Guide

### Suggested Approach

1. Add the dev-dependency and the `mod probe;` declaration next to the existing `mod tests`.
2. Write `DerivedCase` with `std::fs` only (recursive copy of two directories, line filtering on `;`).
3. Write `computed_system`, `lp_fpha_rows`, `envelope_mw`, `segment_envelope_mw`.
4. Write `fit_contexts` by copying the data-loading statements of `compute_fpha` and the `pf`/`bounds`
   statements of `fit_plant`; add a comment `// Mirrors labdessem_fpha::pipeline::fit_plant lines
   41-67; keep in sync.`
5. Write the four tests; commit `test(fpha): add LP envelope probe for computed FPHA`.

### Key Files to Modify

- `crates/labdessem-cli/src/fpha_integration/probe.rs` (create): helpers and control tests.
- `crates/labdessem-cli/src/fpha_integration.rs`: the `#[cfg(test)] mod probe;` declaration only.
- `crates/labdessem-cli/Cargo.toml`: `tempfile = "3"` under `[dev-dependencies]`.
- `Cargo.lock`: the dependency edge added by cargo (no version changes).

### Patterns to Follow

- Test-module attributes of `fpha_integration.rs` (`#[allow(clippy::unwrap_used,
  clippy::expect_used)]`); import specific items instead of `use super::*` in the new module.
- `crates/labdessem-model/src/tests.rs` (lines 1170-1260) shows how rows are found by name.

### Pitfalls to Avoid

- A child module file of `fpha_integration.rs` lives at `src/fpha_integration/probe.rs`; do not
  convert `fpha_integration.rs` into `fpha_integration/mod.rs`.
- Plant names in `CAD_UHE.csv` carry padding (` CAMARGOS     `); build row prefixes from
  `plant.name` as stored in the `System`.
- The volume term references `V[t+1]`; evaluate it with the same `v_hm3` as the grid node (the probe
  measures the row's algebra, not a period trajectory).
- Do not call `write_computed_results` on `examples/24Barras`; only on a `DerivedCase` path.

## Testing Requirements

### Unit Tests

The four `probe_*` tests above; each must fail if its helper is broken (for example, a sign error in
`lp_fpha_rows` breaks `probe_lp_rows_match_injected_segments`).

### Integration Tests

`cargo test --workspace` exits 0; no existing test is changed.

### E2E Tests (if applicable)

None in this ticket (ticket-004).

## Dependencies

- **Blocked By**: ticket-001-record-workspace-baseline.md
- **Blocks**: ticket-003-characterize-double-alpha-envelope.md, ticket-004-add-fpha-e2e-derived-case-harness.md, ticket-005-characterize-skipped-plant-generation.md, ticket-006-characterize-silent-physical-defaults.md
- **Notes**: ticket-004 needs the `tempfile` dev-dependency added here.

## Effort Estimate

**Points**: 3
**Confidence**: High
**Rationale**: all APIs are known and public; the work is careful unit conversion plus file helpers.
