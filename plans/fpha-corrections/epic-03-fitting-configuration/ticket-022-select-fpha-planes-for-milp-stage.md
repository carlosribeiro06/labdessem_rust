# ticket-022 Select FPHA Planes for the MILP Stage

> **[OUTLINE]** This ticket requires refinement before execution.
> It will be refined with learnings from earlier epics.

## Objective

If ADR-029 accepts option (b), build the MILP-stage FPHA rows from an NFHAMILP subset of planes
chosen by the MM centroid rule while the LP stages keep the full set; if ADR-029 accepts (a) or (c),
this ticket is proposed for [REMOVE] at refinement.

## Anticipated Scope

- **Files likely to be modified**:
  - `crates/labdessem-model/src/constraints/hydro.rs`
  - `crates/labdessem-model/src/constraints/constraint_set.rs`
  - `crates/labdessem-simulation/src/stages/solve.rs`
  - `crates/labdessem-core/src/entities/hydro.rs`
- **Key decisions needed**:
  - Where the per-stage plane subset is stored (segment flag, separate list, or model-time
    selection)
  - Default NFHAMILP and its configuration key (study_config.json or CAD_FPHA_CONFIG.csv)
- **Open questions**:
  - Is the scope above 3 days (candidate [SPLIT]: subset selection in labdessem-fpha vs stage-aware
    model assembly)?
  - How do precomputed decks declare the MILP subset, if at all?
  - Which test proves the MILP stage uses the subset and the LP-CALC-CMO stage the full set?

## Dependencies

- **Blocked By**: ticket-021-enforce-max-planes-in-fit-pipeline.md
- **Blocks**: ticket-029-update-fpha-design-documentation.md
- **Notes**: 5 points is the outline estimate; refinement must split it if the refined scope exceeds
  3 days.

## Effort Estimate

**Points**: 5
**Confidence**: Low (will be re-estimated during refinement)
