---
filesToBeCreated:
  - docs/auditoria/fpha-decisions/ADR-026-skipped-plant-policy.md
filesToBeModified: []
filesToBeDeleted: []
implements: [ADR-026, R3]
agent: dessem-specialist
tier: standard
---

# ticket-009 Decide Skipped-Plant Policy (ADR-026)

## Context

### Background

Audit HI-03 (= FP-01 = NU-04) found that a plant with generating units whose computed FPHA cannot be
fitted (missing geometry, invalid parameters, fit failure) is skipped with a `Warning:` and then
generates without water, because it gets no `hydro_fpha` rows and no generation-turbining coupling.
Section 6 of `docs/auditoria/fpha-baseline-reproduction.md` holds the reproduced counts. The user
chooses how such a plant is handled; the audit lists fail-fast, re-validation, a per-plant fallback
and a configuration key.

### Relation to Epic

Decision gate of Epic 01 for A2. ticket-014 and ticket-015 (Epic 02) assume option (a)+(c) and stay
blocked until ADR-026 is `accepted`.

### Current State

- ADR-026 is `proposed` in the master-plan frontmatter with options (a), (c), (a)+(c), (b), (d) and
  the recommendation "(a)+(c), plants without units excluded from fitting".
- `compute_fpha` (`crates/labdessem-cli/src/fpha_integration.rs` lines 95-151) has three skip
  branches and one fatal branch (`MissingHydro`, raised for any plant missing from
  `inflow_history.parquet`, including plants without units: audit FP-12).
- `HydroPlant::validate` (`crates/labdessem-core/src/entities/hydro.rs` line 311) already rejects a
  plant with groups and no segments when `fpha_enabled` is true, but nothing calls it after injection.

## Specification

### Requirements

1. Write `docs/auditoria/fpha-decisions/ADR-026-skipped-plant-policy.md` in English with exactly the
   level-2 headings `## Status`, `## Context`, `## Methodology reference`, `## Options`,
   `## Recommendation`, `## Decision`, `## Consequences`, in that order.
2. `## Status` holds `Proposed`; `## Decision` holds only `PENDING`.
3. `## Context` cites section 6 of the evidence report and the code lines of Current State.
4. `## Methodology reference` states what DESSEM does when a plant lacks the data needed for its FPHA
   (MM section 9.16; Libs section 15.6.1) through the dessem skill reference
   `42-metodologia-hidreletricas.md` sections 15-16; claims not found there are tagged `[to confirm]`.
5. `## Options` has `### (a)`, `### (c)`, `### (a)+(c)`, `### (b)`, `### (d)`, each with `Gains:`,
   `Losses:`, `Risks:`, `Operator message:` (what the user sees) and `Tickets affected:`.
6. A separate subsection `### Sub-question: plants without units` asks whether plants with no
   generating units are excluded from fitting (and so from the `MissingHydro` lookup), with its own
   `Gains:` / `Risks:` lines.
7. The agent's final message is the `CLARIFICATION REQUIRED:` payload with options A = (a), B = (c),
   C = (a)+(c), D = (b), E = (d), the sub-question as a separate yes/no line, and the recommendation.

### Inputs/Props

Section 6 of the evidence report; audit sections 6 (HI-03), 7 (FP-01, FP-12) and 13; the code files
named in Current State; the dessem skill references.

### Outputs/Behavior

One committed brief and one `CLARIFICATION REQUIRED:` payload; no code change.

### Error Handling

- If the methodology reference shows DESSEM falling back to another production model for such plants,
  state it in `## Methodology reference` and in the payload; do not choose for the user.
- If section 6 of the evidence report is missing, stop and report that ticket-007 is incomplete.

### Out of Scope

- Code changes in `compute_fpha`, `main.rs` or `HydroPlant::validate` (Epic 02).
- FP-07, precomputed plants without units (ticket-023, Epic 03).
- Judging ONS deck contents.

## Acceptance Criteria

- [ ] Given the brief, when `grep -E "^## " docs/auditoria/fpha-decisions/ADR-026-skipped-plant-policy.md`
  runs, then it prints exactly the seven headings of requirement 1, in order.
- [ ] Given the brief, when `grep -cE "^### \((a|b|c|d)\)" docs/auditoria/fpha-decisions/ADR-026-skipped-plant-policy.md`
  runs, then it prints 5, and each option subsection contains the five labelled lines of requirement 5.
- [ ] Given the brief, when `grep -c "^### Sub-question: plants without units" docs/auditoria/fpha-decisions/ADR-026-skipped-plant-policy.md`
  runs, then it prints 1, and `## Status` / `## Decision` contain `Proposed` / `PENDING`.
- [ ] Given the brief, when `sed -n '/^## Methodology reference/,/^## Options/p'
  docs/auditoria/fpha-decisions/ADR-026-skipped-plant-policy.md | grep -c "42-metodologia-hidreletricas.md"` runs,
  then it prints at least 1, and every paragraph of that section contains an `MM` or `Libs` section
  number or the tag `[to confirm]`.
- [ ] Given the ticket's end, when the agent returns, then its final message starts with
  `CLARIFICATION REQUIRED:` and lists options A-E, the sub-question and a recommendation, and
  `git show --stat HEAD` lists only the brief.

## Implementation Guide

### Suggested Approach

1. Load the `dessem` skill; read sections 15-16 of `42-metodologia-hidreletricas.md`.
2. Read section 6 of the evidence report and the code lines of Current State.
3. Write the brief; commit `docs(audit): add ADR-026 skipped-plant policy brief`.
4. Return the payload.

### Key Files to Modify

- `docs/auditoria/fpha-decisions/ADR-026-skipped-plant-policy.md` (create): the decision brief.

### Patterns to Follow

- The brief structure of ticket-008 (`ADR-025-alpha-application.md`).
- Option names exactly as in the master-plan frontmatter.

### Pitfalls to Avoid

- The count grep matches all five option headings, `### (a)+(c)` included; no other level-3 heading
  may start with a parenthesized option letter, or the count no longer proves the option list.
- Option (b) (per-plant fallback) needs a per-plant `ConstraintSet` switch; state that cost
  explicitly.
- Do not edit the master plan.

## Testing Requirements

### Unit Tests

None: documentation-only ticket.

### Integration Tests

No Rust code changes, so no test run is required. The agent runs the grep commands of the
Acceptance Criteria before committing and quotes their output in its final report.

### E2E Tests (if applicable)

None.

## Dependencies

- **Blocked By**: ticket-007-compile-fpha-baseline-evidence.md
- **Blocks**: ticket-014-fail-on-skipped-fpha-plants.md
- **Notes**: after this ticket the orchestrator asks the user and records ADR-026 as `accepted`.

## Effort Estimate

**Points**: 1
**Confidence**: High
**Rationale**: the option list and the evidence exist; the methodology question is narrow.
