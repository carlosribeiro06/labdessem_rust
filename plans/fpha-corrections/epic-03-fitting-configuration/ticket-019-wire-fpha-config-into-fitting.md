# ticket-019 Wire CAD_FPHA_CONFIG into Computed Fitting

> **[OUTLINE]** This ticket requires refinement before execution.
> It will be refined with learnings from earlier epics.

## Objective

Implement the accepted ADR-028 option (audit FP-05): `build_fpha_config` honors every accepted field
of `CAD_FPHA_CONFIG.csv` and the fitting defaults become explicit constants, documented and tested;
per-plant overrides change the fitted planes as specified.

## Anticipated Scope

- **Files likely to be modified**:
  - `crates/labdessem-cli/src/fpha_integration.rs`
  - `crates/labdessem-fpha/src/config.rs`
  - `crates/labdessem-io/src/readers/fpha_geometry.rs`
  - `crates/labdessem-cli/src/fpha_integration/config_tests.rs`
- **Key decisions needed**:
  - Mapping of each accepted field to `FphaPlantConfig` / `FittingBounds` (depends on ADR-028)
  - Whether changed defaults re-record the post-fix evidence of ticket-017 (ADR-028 rework trigger)
- **Open questions**:
  - Does the hard-coded `FphaPlantConfig::computed(10, 6, 6, Angle 1.0)` survive as the default or
    move to MM defaults?
  - How is a per-plant window validated against `ForebayTable::v_min/v_max`?
  - Which E2E derived case proves that a per-plant override changes the planes (audit FP-05
    reproduced three files with identical output)?

## Dependencies

- **Blocked By**: ticket-018-decide-fpha-config-semantics.md
- **Blocks**: ticket-021-enforce-max-planes-in-fit-pipeline.md,
  ticket-028-decide-alpha-evaluation-points.md, ticket-029-update-fpha-design-documentation.md

## Effort Estimate

**Points**: 3
**Confidence**: Low (will be re-estimated during refinement)
