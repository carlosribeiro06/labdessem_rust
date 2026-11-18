# ticket-023 Reject FPHA Rows for Plants Without Units

> **[OUTLINE]** This ticket requires refinement before execution.
> It will be refined with learnings from earlier epics.

## Objective

Close audit FP-07: in precomputed mode, a plant without generating units that has rows in
`OPER_FPHA.csv` gets `-gamma_V*V - gamma_S*S <= gamma_0` rows that can make the LP infeasible; such
rows are rejected at read time or skipped with an explicit outcome, consistently with the ticket-014
exclusion of plants without units in computed mode.

## Anticipated Scope

- **Files likely to be modified**:
  - `crates/labdessem-io/src/readers/hydro.rs`
  - `crates/labdessem-core/src/entities/hydro.rs`
  - `crates/labdessem-model/src/constraints/hydro.rs`
  - `crates/labdessem-cli/tests/fpha_e2e.rs`
- **Key decisions needed**:
  - Error vs skip-with-warning for OPER_FPHA.csv rows of plants without units (user decision at
    refinement if not covered by ADR-026)
  - Which layer owns the check (reader, `HydroPlant::validate`, or the model builder)
- **Open questions**:
  - Does any official deck carry FPHA rows for plants without units (operational impact)?
  - Is the audit's 48-row reproduction (section 12.3) reusable as a derived precomputed case?
  - Should the rule mirror ticket-014's `plants_skipped` semantics?

## Dependencies

- **Blocked By**: ticket-017-record-post-fix-fpha-evidence.md
- **Blocks**: ticket-029-update-fpha-design-documentation.md

## Effort Estimate

**Points**: 2
**Confidence**: Low (will be re-estimated during refinement)
