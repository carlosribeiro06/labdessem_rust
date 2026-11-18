# ticket-020 Decide Plane Count, Reduction Criterion and MILP Selection (ADR-029)

> **[OUTLINE]** This ticket requires refinement before execution.
> It will be refined with learnings from earlier epics.

## Objective

Write the ADR-029 decision brief (dessem-specialist) for finding A5: whether `max_planes` is
enforced or removed, whether the angle-only reduction stays or follows the MM distance criterion,
and whether the MILP stage uses a reduced NFHAMILP plane subset (MM 27.2); it ends with a
`CLARIFICATION REQUIRED:` payload.

## Anticipated Scope

- **Files likely to be modified**:
  - `docs/auditoria/fpha-decisions/ADR-029-plane-count-and-milp-selection.md`
- **Key decisions needed**:
  - ADR-029 option (a) enforce `max_planes` with every plane in every stage, (b) (a) plus centroid
    NFHAMILP selection for the MILP stage, (c) remove `max_planes`/`num_planes` (preliminary
    recommendation (a))
  - Reduction criterion: keep the angle criterion or adopt the MM distance criterion
- **Open questions**:
  - What MILP solve-time evidence on the derived case (and on any available larger deck) justifies
    option (b)?
  - How did the plane counts change after ticket-011 (section 8 of the evidence report)?
  - What does the MM prescribe for NFHAMILP defaults and plane choice (MM 27.2 citation)?

## Dependencies

- **Blocked By**: ticket-017-record-post-fix-fpha-evidence.md
- **Blocks**: ticket-021-enforce-max-planes-in-fit-pipeline.md

## Effort Estimate

**Points**: 2
**Confidence**: Low (will be re-estimated during refinement)
