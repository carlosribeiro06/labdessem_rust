# ticket-021 Enforce max_planes in the Fit Pipeline

> **[OUTLINE]** This ticket requires refinement before execution.
> It will be refined with learnings from earlier epics.

## Objective

Implement the accepted ADR-029 plane-count rule in `fit_plant`: the reduced plane set never exceeds
`max_planes`, with a deterministic selection or iterative tolerance rule, and the result is covered
by unit and regression tests; if ADR-029 chooses (c), this ticket instead removes the unused fields.

## Anticipated Scope

- **Files likely to be modified**:
  - `crates/labdessem-fpha/src/pipeline.rs`
  - `crates/labdessem-fpha/src/fitting/reduction.rs`
  - `crates/labdessem-fpha/src/config.rs`
  - `crates/labdessem-fpha/tests/regression.rs`
- **Key decisions needed**:
  - Selection rule when the reduction leaves more than `max_planes` planes (tolerance escalation vs
    contribution ranking)
  - Whether the rule runs before or after the gamma_S fit (interaction with ADR-025 stage order and
    ADR-033)
- **Open questions**:
  - Is the reduction outcome bit-deterministic under the chosen rule (existing determinism tests)?
  - How does the deviation diagnostic change when planes are capped, and is that reported?
  - Does `crates/labdessem-fpha/src/pipeline.rs` need the hpcFencedPaths fence and `tier: complex` again (it did in
    ticket-011)?

## Dependencies

- **Blocked By**: ticket-020-decide-plane-count-and-milp-selection.md,
  ticket-019-wire-fpha-config-into-fitting.md
- **Blocks**: ticket-022-select-fpha-planes-for-milp-stage.md,
  ticket-029-update-fpha-design-documentation.md

## Effort Estimate

**Points**: 3
**Confidence**: Low (will be re-estimated during refinement)
