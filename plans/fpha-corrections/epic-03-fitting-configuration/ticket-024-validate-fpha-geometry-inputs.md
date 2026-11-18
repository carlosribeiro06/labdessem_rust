# ticket-024 Validate FPHA Geometry and Tailrace Inputs

> **[OUTLINE]** This ticket requires refinement before execution.
> It will be refined with learnings from earlier epics.

## Objective

Close audit FP-R01 and EM-04: `read_vha_geometry`, `read_tailrace` and `read_fpha_config` reject
duplicate plant rows and non-finite values with errors that name the file, the plant and the column,
so neither computed FPHA nor EngMax can consume NaN or a silently overwritten row.

## Anticipated Scope

- **Files likely to be modified**:
  - `crates/labdessem-io/src/readers/fpha_geometry.rs`
  - `crates/labdessem-io/src/error.rs`
  - `crates/labdessem-cli/src/fpha_integration/physical_data_tests.rs`
- **Key decisions needed**:
  - Duplicate rows: error vs last-wins with warning (FP-R01 recommends validation)
  - Whether monotonicity of the VHA curve is also validated here or left to `ForebayTable::new`
- **Open questions**:
  - Which `IoError` constructor carries file/plant/column context today?
  - Does the EngMax path (EM-04) need its own test, given EngMax is otherwise out of scope?
  - Are the existing reader unit tests in `crates/labdessem-io/src/readers/fpha_geometry.rs` enough, or is a derived-case test
    needed?

## Dependencies

- **Blocked By**: ticket-017-record-post-fix-fpha-evidence.md
- **Blocks**: ticket-029-update-fpha-design-documentation.md

## Effort Estimate

**Points**: 3
**Confidence**: Low (will be re-estimated during refinement)
