# Epic 04: Methodology Decisions and Documentation

> Outline epic: its tickets are refined with the learnings of Epics 01-03 before execution.

## Goal

Confirm the methodology divergences B1 (volume reference), B3 (turbining axis and Pmax cap), B4
(tailrace families) and B5 (alpha evaluation points) against the DESSEM methodology with citations,
record the user's decisions (ADR-030 to ADR-033) without judging ONS operational choices, and bring
the FPHA design and user documentation in line with the implemented behavior (audit FP-11).

## Scope

- Four decision briefs under `docs/auditoria/fpha-decisions/` (dessem-specialist).
- `docs/design/fpha-formulation.md`, `crates/labdessem-fpha/README.md` (design docs).
- `docs/methodology.html`, `docs/configuration.html`, `docs/docs.html`, `README.md`,
  `ARCHITECTURE.md` (user docs).

Out of scope: implementing accepted B-options. When a brief's accepted option changes code, the
refinement of this epic adds an implementation ticket ([ADD]) or defers it to a follow-up plan, at
the user's choice.

## Tickets

| Ticket | Title | Agent (planned) | Points |
| --- | --- | --- | --- |
| ticket-025-decide-fpha-volume-reference.md | Decide the FPHA Volume Reference (ADR-030) | dessem-specialist | 2 |
| ticket-026-decide-turbining-axis-and-pmax-cap.md | Decide the Turbining Axis Bound and Pmax Cap (ADR-031) | dessem-specialist | 2 |
| ticket-027-decide-tailrace-family-modelling.md | Decide Tailrace Family Modelling (ADR-032) | dessem-specialist | 2 |
| ticket-028-decide-alpha-evaluation-points.md | Decide Alpha Evaluation Points (ADR-033) | dessem-specialist | 2 |
| ticket-029-update-fpha-design-documentation.md | Update FPHA Design Documentation | open-source-documentation-writer | 2 |
| ticket-030-update-fpha-user-documentation.md | Update FPHA User Documentation | open-source-documentation-writer | 2 |

## Dependency Order

```
017 -> 025, 026, 027 ------------------------\
017 + 019 -> 028 ----------------------------+-> 029 -> 030
019, 021, 022, 023, 024 (Epic 03) ------------/
```

The four decision tickets can start after ticket-017, in parallel with Epic 03; ticket-028 waits for
ticket-019 because the alpha evaluation points depend on the grid defaults of ADR-028.

## Exit Criteria

- ADR-030 to ADR-033 `accepted`, each with a committed brief.
- No stale reference remains in the FPHA documentation (`FPHA = 0/1`, `readers/fpha.rs`, old column
  names); the computed mode, the alpha contract and the failure policies are documented.
