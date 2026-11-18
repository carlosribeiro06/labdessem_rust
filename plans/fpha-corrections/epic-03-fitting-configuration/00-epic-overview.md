# Epic 03: Fitting Configuration and Input Validation

> Outline epic: its tickets are refined with the Epic 01 and Epic 02 learnings before execution.

## Goal

Make the computed FPHA fit configurable and bounded as decided (A4 `CAD_FPHA_CONFIG.csv`, A5
`max_planes` and MILP plane selection), and harden the FPHA inputs (FP-07, FP-R01, EM-04), so that no
FPHA input is read and then ignored, silently overwritten or accepted as non-finite.

## Scope

- ADR-028 (A4 + B2 fit domain) and ADR-029 (A5 + reduction criterion) decision briefs and their
  implementation.
- Reader validation of `CAD_GEOMETRIA_UHE.csv`, `CAD_TAILRACE.csv`, `CAD_FPHA_CONFIG.csv`.
- Precomputed `OPER_FPHA.csv` rows for plants without units (FP-07).

Out of scope: methodology choices B1, B3, B4, B5 (Epic 04), EngMax behavior beyond input validation.

## Tickets

| Ticket | Title | Agent (planned) | Points |
| --- | --- | --- | --- |
| ticket-018-decide-fpha-config-semantics.md | Decide CAD_FPHA_CONFIG Semantics and Fit Domain (ADR-028) | dessem-specialist | 2 |
| ticket-019-wire-fpha-config-into-fitting.md | Wire CAD_FPHA_CONFIG into Computed Fitting | hpc-rust-developer | 3 |
| ticket-020-decide-plane-count-and-milp-selection.md | Decide Plane Count, Reduction Criterion and MILP Selection (ADR-029) | dessem-specialist | 2 |
| ticket-021-enforce-max-planes-in-fit-pipeline.md | Enforce max_planes in the Fit Pipeline | hpc-rust-developer | 3 |
| ticket-022-select-fpha-planes-for-milp-stage.md | Select FPHA Planes for the MILP Stage | hpc-rust-developer | 5 |
| ticket-023-reject-fpha-rows-for-plants-without-units.md | Reject FPHA Rows for Plants Without Units | hpc-rust-developer | 2 |
| ticket-024-validate-fpha-geometry-inputs.md | Validate FPHA Geometry and Tailrace Inputs | hpc-rust-developer | 3 |

## Dependency Order

```
017 -> 018 -> 019 --\
017 -> 020 ---------+-> 021 -> 022
017 -> 023
017 -> 024
```

## Refinement Notes

- ticket-022 exists only if ADR-029 accepts option (b); otherwise refinement proposes [REMOVE].
- ticket-022 is a [SPLIT] candidate (5 points).
- A changed fitting default re-records the ticket-017 evidence (ADR-028 rework trigger).

## Exit Criteria

- ADR-028 and ADR-029 `accepted`; every accepted `CAD_FPHA_CONFIG.csv` field changes the fitted planes
  in a test; `max_planes` is enforced or removed.
- Duplicate plant rows and non-finite values in FPHA inputs fail with file, plant and column named.
- `cargo test --workspace` exits 0 and clippy warnings do not exceed the ticket-001 baseline.
