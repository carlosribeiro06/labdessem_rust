# ticket-028 Decide Alpha Evaluation Points (ADR-033)

> **[OUTLINE]** This ticket requires refinement before execution.
> It will be refined with learnings from earlier epics.

## Objective

Write the ADR-033 decision brief (dessem-specialist) for B5: whether alpha stays computed on the
fit-grid nodes before plane reduction or moves to separate evaluation points (MM default 30) on the
final reduced planes, with the effect on alpha per plant; it ends with a `CLARIFICATION REQUIRED:`
payload.

## Anticipated Scope

- **Files likely to be modified**:
  - `docs/auditoria/fpha-decisions/ADR-033-alpha-evaluation-points.md`
- **Key decisions needed**:
  - ADR-033 option (a) keep fit-grid alpha before reduction or (b) separate evaluation points after
    reduction (preliminary recommendation (b))
- **Open questions**:
  - How are the MM evaluation points distributed (citation), and how does that interact with the
    ADR-028 grid defaults implemented in ticket-019?
  - Does option (b) trigger the ADR-025 rework (stage order of `fit_plant`)?
  - What alpha change per plant does (b) produce on the derived case?

## Dependencies

- **Blocked By**: ticket-017-record-post-fix-fpha-evidence.md,
  ticket-019-wire-fpha-config-into-fitting.md
- **Blocks**: ticket-029-update-fpha-design-documentation.md

## Effort Estimate

**Points**: 2
**Confidence**: Low (will be re-estimated during refinement)
