# ticket-030 Update FPHA User Documentation

> **[OUTLINE]** This ticket requires refinement before execution.
> It will be refined with learnings from earlier epics.

## Objective

Bring the user-facing documentation in line with the corrected FPHA behavior:
`docs/methodology.html`, `docs/configuration.html`, `docs/docs.html`, `README.md` and
`ARCHITECTURE.md` describe the three FPHA modes, the inputs computed mode requires, the failure
messages, the `CAD_FPHA_CONFIG.csv` fields and the `OPER_FPHA.csv` copy rule
(open-source-documentation-writer).

## Anticipated Scope

- **Files likely to be modified**:
  - `docs/methodology.html`
  - `docs/configuration.html`
  - `docs/docs.html`
  - `README.md`
  - `ARCHITECTURE.md`
- **Key decisions needed**:
  - Language of the HTML docs (the project's existing convention vs Rigoroso English) if they are
    currently Portuguese
- **Open questions**:
  - Do the HTML pages share generated sections that must stay in sync (sitemap, navigation)?
  - Is the scope above 3 days with five files (candidate [SPLIT]: HTML pages vs
    README/ARCHITECTURE)?
  - Which CLI messages from tickets 014-016 should be quoted verbatim?

## Dependencies

- **Blocked By**: ticket-029-update-fpha-design-documentation.md
- **Blocks**: None

## Effort Estimate

**Points**: 2
**Confidence**: Low (will be re-estimated during refinement)
