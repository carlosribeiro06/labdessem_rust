# ticket-018 Decide CAD_FPHA_CONFIG Semantics and Fit Domain (ADR-028)

> **[OUTLINE]** This ticket requires refinement before execution.
> It will be refined with learnings from earlier epics.

## Objective

Write the ADR-028 decision brief (dessem-specialist) for audit FP-05/D13 and the B2 fit-domain
divergence: which `CAD_FPHA_CONFIG.csv` fields take effect, with which defaults (volume window,
NPTV/NPTQ grid, plane count, reduction tolerance), and whether the file stays a documented input.
The brief cites the DESSEM methodology and ends with a `CLARIFICATION REQUIRED:` payload, as tickets
008-010 do.

## Anticipated Scope

- **Files likely to be modified**:
  - `docs/auditoria/fpha-decisions/ADR-028-fpha-config-semantics.md`
- **Key decisions needed**:
  - ADR-028 option (a) implement every field with MM defaults, (b) implement every field with
    current defaults, or (c) remove the file (preliminary recommendation (a))
  - Whether the B2 fit volume window and the `min_volume_pct`/`max_volume_pct` fields are the same
    parameter (planned: yes)
- **Open questions**:
  - What are the MM default window and grid sizes, with section citations (MM 9.16.11-9.16.12)?
  - Does the MM require an even NPTV + 1 grid, and how does that interact with the run-of-river
    synthesis in `resolve_fitting_bounds`?
  - Does `tolerance` map to the angle tolerance in degrees or to a distance criterion (ties to
    ADR-029)?
  - How much do the fitted planes and the objective move on the fast derived case for each option
    (section 8 of the evidence report as the base)?

## Dependencies

- **Blocked By**: ticket-017-record-post-fix-fpha-evidence.md
- **Blocks**: ticket-019-wire-fpha-config-into-fitting.md

## Effort Estimate

**Points**: 2
**Confidence**: Low (will be re-estimated during refinement)
