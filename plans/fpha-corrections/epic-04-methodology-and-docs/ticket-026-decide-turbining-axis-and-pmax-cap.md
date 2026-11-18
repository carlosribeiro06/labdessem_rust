# ticket-026 Decide the Turbining Axis Bound and Pmax Cap (ADR-031)

> **[OUTLINE]** This ticket requires refinement before execution.
> It will be refined with learnings from earlier epics.

## Objective

Write the ADR-031 decision brief (dessem-specialist) for B3: whether the fit cloud's Q axis stays
bounded by the sum of unit maximum turbining or uses the maximum swallowing (MM 9.17.1,
`labdessem_fpha::calculate_max_swallowing`), and whether the Pmax cap stays in the cloud; it ends
with a `CLARIFICATION REQUIRED:` payload.

## Anticipated Scope

- **Files likely to be modified**:
  - `docs/auditoria/fpha-decisions/ADR-031-turbining-axis-and-pmax-cap.md`
- **Key decisions needed**:
  - ADR-031 option (a) keep, (b) maximum swallowing, (c) (b) and no Pmax cap in the cloud
    (preliminary recommendation (b))
- **Open questions**:
  - Does option (b) couple computed FPHA to the EngMax inputs (h_ef, Q_ef, turbine type) in a way
    that conflicts with ADR-017?
  - How much do the planes near Pmax change on the derived case?
  - Which implementation ticket is added at refinement if (b) or (c) is accepted?

## Dependencies

- **Blocked By**: ticket-017-record-post-fix-fpha-evidence.md
- **Blocks**: ticket-029-update-fpha-design-documentation.md

## Effort Estimate

**Points**: 2
**Confidence**: Low (will be re-estimated during refinement)
