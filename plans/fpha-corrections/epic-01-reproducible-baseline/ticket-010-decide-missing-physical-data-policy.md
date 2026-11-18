---
filesToBeCreated:
  - docs/auditoria/fpha-decisions/ADR-027-missing-physical-data-policy.md
filesToBeModified: []
filesToBeDeleted: []
implements: [ADR-027, R4]
agent: dessem-specialist
tier: standard
---

# ticket-010 Decide Missing Physical-Data Policy (ADR-027)

## Context

### Background

Finding A3 (audit FP-R02 for efficiency and loss; the missing tailrace is new) shows that computed
FPHA replaces a missing tailrace polynomial by a 0 m level, a missing `Eficiencia` by 1.0 and a missing
`Perda.m` by 0 m, without any message (`ProductionFunction::new`,
`crates/labdessem-fpha/src/fitting/production.rs` lines 47-48 and 67). Section 7 of
`docs/auditoria/fpha-baseline-reproduction.md` holds the reproduced FPH inflation. The user chooses
the policy.

### Relation to Epic

Decision gate of Epic 01 for A3. ticket-016 (Epic 02) assumes option (a) and stays blocked until
ADR-027 is `accepted`.

### Current State

- ADR-027 is `proposed` in the master-plan frontmatter with options (a)-(d) and the recommendation
  "(a) fail-fast typed error, scoped to computed FPHA fitting (EngMax untouched)".
- The EngMax calculator (`crates/labdessem-fpha/src/calculator/`) builds `ProductionFunction` with the
  same defaults; EngMax is out of scope of this plan.
- `CAD/CAD_UHE.csv` reads `Perda.m` and `Eficiencia` with serde `default`
  (`crates/labdessem-io/src/readers/hydro.rs` lines 453-456), so a missing column and an empty cell
  both become `None`.

## Specification

### Requirements

1. Write `docs/auditoria/fpha-decisions/ADR-027-missing-physical-data-policy.md` in English with
   exactly the level-2 headings `## Status`, `## Context`, `## Methodology reference`, `## Options`,
   `## Recommendation`, `## Decision`, `## Consequences`, in that order.
2. `## Status` holds `Proposed`; `## Decision` holds only `PENDING`.
3. `## Context` cites section 7 of the evidence report and the code lines of Background.
4. `## Methodology reference` states which hydro registry data the DESSEM FPH requires (tailrace
   polynomial, efficiency, hydraulic loss; MM section 9.16.1-9.16.10; Libs sections 15.3.4-15.3.5)
   through `42-metodologia-hidreletricas.md` sections 1 and 15; claims not found there are tagged
   `[to confirm]`.
5. `## Options` has `### (a)`, `### (b)`, `### (c)`, `### (d)`, each with `Gains:`, `Losses:`,
   `Risks:`, `Operator message:` and `Tickets affected:`.
6. Two subsections: `### Sub-question: explicit zero loss` (is `Perda.m = 0` present in the CSV a
   valid value?) and `### Sub-question: EngMax scope` (does the policy also apply to EngMax? the plan
   recommends no), each with `Gains:` / `Risks:` lines.
7. The agent's final message is the `CLARIFICATION REQUIRED:` payload with options A-D, both
   sub-questions and the recommendation.

### Inputs/Props

Section 7 of the evidence report; audit sections 7 (FP-R02) and 13; the code files named above; the
dessem skill references.

### Outputs/Behavior

One committed brief and one `CLARIFICATION REQUIRED:` payload; no code change.

### Error Handling

- If the methodology reference shows that DESSEM itself defaults one of these inputs, state the
  default and its source in `## Methodology reference` and in the payload.
- If section 7 of the evidence report is missing, stop and report that ticket-007 is incomplete.

### Out of Scope

- Code changes in `build_fit_params` or `ProductionFunction` (Epic 02).
- EngMax behavior (EM-01..EM-03) beyond the scope sub-question.
- Geometry reader validation (NaN, duplicates: ticket-024).

## Acceptance Criteria

- [ ] Given the brief, when `grep -E "^## " docs/auditoria/fpha-decisions/ADR-027-missing-physical-data-policy.md`
  runs, then it prints exactly the seven headings of requirement 1, in order.
- [ ] Given the brief, when `grep -cE "^### \((a|b|c|d)\)$" docs/auditoria/fpha-decisions/ADR-027-missing-physical-data-policy.md`
  runs, then it prints 4, and each option subsection contains the five labelled lines of requirement 5.
- [ ] Given the brief, when `grep -c "^### Sub-question: " docs/auditoria/fpha-decisions/ADR-027-missing-physical-data-policy.md`
  runs, then it prints 2, and `## Status` / `## Decision` contain `Proposed` / `PENDING`.
- [ ] Given the brief, when `sed -n '/^## Methodology reference/,/^## Options/p'
  docs/auditoria/fpha-decisions/ADR-027-missing-physical-data-policy.md | grep -c "42-metodologia-hidreletricas.md"` runs,
  then it prints at least 1, and every paragraph of that section contains an `MM` or `Libs` section
  number or the tag `[to confirm]`.
- [ ] Given the ticket's end, when the agent returns, then its final message starts with
  `CLARIFICATION REQUIRED:` and lists options A-D, both sub-questions and a recommendation, and
  `git show --stat HEAD` lists only the brief.

## Implementation Guide

### Suggested Approach

1. Load the `dessem` skill; read sections 1 and 15 of `42-metodologia-hidreletricas.md`.
2. Read section 7 of the evidence report and the code lines cited.
3. Write the brief; commit `docs(audit): add ADR-027 missing physical-data brief`.
4. Return the payload.

### Key Files to Modify

- `docs/auditoria/fpha-decisions/ADR-027-missing-physical-data-policy.md` (create): the brief.

### Patterns to Follow

- The brief structure of ticket-008.
- Option names exactly as in the master-plan frontmatter.

### Pitfalls to Avoid

- A missing column and an empty cell are indistinguishable after reading (both `None`); say so under
  the sub-question on explicit zero loss.
- Option (a) stops decks that run today; state the operational impact neutrally, without judging ONS
  deck choices.
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
- **Blocks**: ticket-016-reject-missing-fpha-physical-data.md
- **Notes**: after this ticket the orchestrator asks the user and records ADR-027 as `accepted`.

## Effort Estimate

**Points**: 1
**Confidence**: High
**Rationale**: narrow methodology question; evidence and options exist.
