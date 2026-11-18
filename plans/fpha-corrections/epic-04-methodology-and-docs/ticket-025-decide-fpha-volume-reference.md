# ticket-025 Decide the FPHA Volume Reference (ADR-030)

> **[OUTLINE]** This ticket requires refinement before execution.
> It will be refined with learnings from earlier epics.

## Objective

Write the ADR-030 decision brief (dessem-specialist) for B1 (audit FP-06/D9): whether the FPHA rows
use the end-of-period volume V[t+1] (MM eq. 9.33) or the period-mean volume (Libs form, as EngMax
ADR-006 does), with citations and the measured effect; it ends with a `CLARIFICATION REQUIRED:`
payload.

## Anticipated Scope

- **Files likely to be modified**:
  - `docs/auditoria/fpha-decisions/ADR-030-fpha-volume-reference.md`
- **Key decisions needed**:
  - ADR-030 option (a) keep V[t+1] or (b) period-mean volume (preliminary recommendation (b), per
    the audit reviewer)
- **Open questions**:
  - Do MM eq. 9.33 and the Libs form actually differ for the official model version (citation
    check)?
  - What is the effect on the fast derived case (the audit measured at most 0.25 MW)?
  - If (b) is accepted, which implementation ticket is added ([ADD]) and does it change the
    precomputed semantics?

## Dependencies

- **Blocked By**: ticket-017-record-post-fix-fpha-evidence.md
- **Blocks**: ticket-029-update-fpha-design-documentation.md

## Effort Estimate

**Points**: 2
**Confidence**: Low (will be re-estimated during refinement)
