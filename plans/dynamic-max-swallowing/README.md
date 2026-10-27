# Dynamic Maximum Swallowing (EngMax) Implementation Plan

**Implementation Mode**: Rigoroso (English identifiers, comments, docs; minimal comments; terse style)

## Overview

Implement dynamic maximum turbining (engolimento máximo) for hydro units based on net head calculation. The constraint limits turbining capacity when net head is below nominal head.

**Equation**:
$$Q_{\max,eng} = Q_{ef} \times \left( \frac{h_l(V, q_{def})}{h_{ef}} \right)^\alpha$$

## Tech Stack

- **Language**: Rust
- **Workspace**: Cargo multi-crate
- **Solver**: HiGHS via good_lp
- **Testing**: cargo test

## Specialist Chain

```
hpc-rust-developer (core entities, data structures)
  → hpc-rust-developer + solver-integration skill (constraint formulation)
  → hpc-rust-developer (simulation orchestration, I/O)
```

## Epics

| Epic | Name | Tickets | Status |
|------|------|---------|--------|
| 1 | Domain Model & I/O Foundation | 3 | Ready |
| 2 | Calculator & Constraint Implementation | 4 | Ready |
| 3 | Simulation Orchestration | 3 | Outline |
| 4 | Output & Validation | 2 | Outline |

## Progress

<!-- progress:start -->
| Ticket | Status | Detail | Est. (min) | Self | Readiness | Quality | Badge |
| ------ | ------ | ------ | ---------- | ---- | --------- | ------- | ----- |
| ticket-001-add-turbine-type-enum.md | completed | detailed | -- | 0.94 | 0.92 | 0.83 | ACCEPTABLE |
| ticket-002-promote-hef-field.md | completed | detailed | -- | 0.92 | 0.93 | 0.85 | ACCEPTABLE |
| ticket-003-add-engmax-config.md | completed | detailed | -- | 0.93 | 0.87 | 1.00 | ACCEPTABLE |
| ticket-004-implement-max-swallowing-calculator.md | completed | detailed | -- | 0.95 | 0.94 | 1.00 | EXCELLENT |
| ticket-005-add-engmax-bounds-struct.md | completed | detailed | -- | 0.91 | 0.93 | 1.00 | EXCELLENT |
| ticket-006-add-engmax-constraint.md | completed | detailed | -- | 0.88 | 0.97 | -- | EXCELLENT |
| ticket-007-orchestrate-inter-stage-updates.md | completed | detailed | -- | 0.86 | -- | -- | -- |
| ticket-008-initial-engmax-computation.md | superseded | outline | -- | -- | -- | -- | -- |
| ticket-009-execution-mode-integration.md | superseded | outline | -- | -- | -- | -- | -- |
| ticket-010-logging-and-diagnostics.md | superseded | outline | -- | -- | -- | -- | -- |
| ticket-011-write-engmax-output-csv.md | completed | refined | -- | -- | -- | -- | -- |
| ticket-012-validate-end-to-end.md | completed | refined | -- | -- | -- | -- | -- |
<!-- progress:end -->

## Dependency Graph

```
ticket-001 ──┬──► ticket-002 ──► ticket-004 ──┬──► ticket-006 ──► ticket-007 ──► ticket-008 ──► ticket-009 ──► ticket-010 ──► ticket-011 ──► ticket-012
             │                                │
ticket-003 ──┼────────────────────────────────┼──► ticket-007
             │                                │
             └──► ticket-005 ─────────────────┘
```

## Execution Order

1. ticket-001-add-turbine-type-enum.md
2. ticket-003-add-engmax-config.md (parallel with 001)
3. ticket-002-promote-hef-field.md (after 001)
4. ticket-005-add-engmax-bounds-struct.md (parallel with 002-004)
5. ticket-004-implement-max-swallowing-calculator.md (after 002)
6. ticket-006-add-engmax-constraint.md (after 004, 005)
7. ticket-007-orchestrate-inter-stage-updates.md (after 003, 006)
8. ticket-008-initial-engmax-computation.md [OUTLINE]
9. ticket-009-execution-mode-integration.md [OUTLINE]
10. ticket-010-logging-and-diagnostics.md [OUTLINE]
11. ticket-011-write-engmax-output-csv.md [OUTLINE]
12. ticket-012-validate-end-to-end.md [OUTLINE]

## Success Criteria

1. EngMax constraint correctly limits turbining based on head ratio
2. Inter-stage recalculation updates bounds correctly
3. Fallback behavior handles missing geometry gracefully
4. Output files contain EngMax values for analysis
5. All edge cases handled per specification
