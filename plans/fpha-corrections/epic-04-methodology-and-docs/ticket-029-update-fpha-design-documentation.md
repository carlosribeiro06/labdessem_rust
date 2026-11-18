# ticket-029 Update FPHA Design Documentation

> **[OUTLINE]** This ticket requires refinement before execution.
> It will be refined with learnings from earlier epics.

## Objective

Close audit FP-11 for the design documents: rewrite `docs/design/fpha-formulation.md` and
`crates/labdessem-fpha/README.md` to describe the implemented
computed/precomputed/constant-productivity modes, the alpha contract, the fit pipeline stages, the
failure policies and every accepted ADR-025 to ADR-033 decision (open-source-documentation-writer,
Rigoroso, English).

## Anticipated Scope

- **Files likely to be modified**:
  - `docs/design/fpha-formulation.md`
  - `crates/labdessem-fpha/README.md`
- **Key decisions needed**:
  - Whether accepted-but-unimplemented B-options (follow-up plan) are documented as current behavior
    or as planned changes
- **Open questions**:
  - Which ADRs are implemented by the time Epic 4 is refined (depends on [ADD] tickets)?
  - Should the design doc carry the resultado_fpha_computed.csv to OPER_FPHA.csv copy rule or
    link to the user docs?
  - Which stale references (`FPHA = 0/1`, the nonexistent readers/fpha.rs, old column names) remain after Epic 03?

## Dependencies

- **Blocked By**: ticket-019-wire-fpha-config-into-fitting.md,
  ticket-021-enforce-max-planes-in-fit-pipeline.md, ticket-022-select-fpha-planes-for-milp-stage.md,
  ticket-023-reject-fpha-rows-for-plants-without-units.md,
  ticket-024-validate-fpha-geometry-inputs.md, ticket-025-decide-fpha-volume-reference.md,
  ticket-026-decide-turbining-axis-and-pmax-cap.md, ticket-027-decide-tailrace-family-modelling.md,
  ticket-028-decide-alpha-evaluation-points.md
- **Blocks**: ticket-030-update-fpha-user-documentation.md

## Effort Estimate

**Points**: 2
**Confidence**: Low (will be re-estimated during refinement)
