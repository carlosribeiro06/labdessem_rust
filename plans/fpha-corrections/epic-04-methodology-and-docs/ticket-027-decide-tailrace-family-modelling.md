# ticket-027 Decide Tailrace Family Modelling (ADR-032)

> **[OUTLINE]** This ticket requires refinement before execution.
> It will be refined with learnings from earlier epics.

## Objective

Write the ADR-032 decision brief (dessem-specialist) for B4: whether one degree-4 tailrace
polynomial per plant stays (documented limitation) or polynomial families selected by downstream
level (backwater, MM 9.16.4) or by flow range are modelled, including the input-schema consequences;
it ends with a `CLARIFICATION REQUIRED:` payload.

## Anticipated Scope

- **Files likely to be modified**:
  - `docs/auditoria/fpha-decisions/ADR-032-tailrace-family-modelling.md`
- **Key decisions needed**:
  - ADR-032 option (a) single polynomial, (b) families by downstream level, (c) piecewise by flow
    range (preliminary recommendation (a))
- **Open questions**:
  - Which official plants depend on backwater, per the dessem skill references?
  - What would the CAD_TAILRACE.csv schema look like for families, and does
    `TailraceSource::Families` (placeholder returning 0.0) need removal under (a)?
  - Is a follow-up plan more appropriate than an [ADD] ticket if (b) is accepted?

## Dependencies

- **Blocked By**: ticket-017-record-post-fix-fpha-evidence.md
- **Blocks**: ticket-029-update-fpha-design-documentation.md

## Effort Estimate

**Points**: 2
**Confidence**: Low (will be re-estimated during refinement)
