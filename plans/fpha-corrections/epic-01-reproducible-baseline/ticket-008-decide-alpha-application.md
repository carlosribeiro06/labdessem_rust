---
filesToBeCreated:
  - docs/auditoria/fpha-decisions/ADR-025-alpha-application.md
filesToBeModified: []
filesToBeDeleted: []
implements: [ADR-025, R1, R2, R9]
agent: dessem-specialist
tier: standard
---

# ticket-008 Decide Alpha Application (ADR-025)

## Context

### Background

Audit FP-02 (with FP-08 and decision D2) found that computed FPHA applies the correction factor alpha
twice: `fit_plant` (`crates/labdessem-fpha/src/pipeline.rs` line 85) scales gamma_0..gamma_S by alpha,
the gamma_S secant fit then overwrites gamma_S without alpha, and `convert_to_fpha_segments`
(`crates/labdessem-cli/src/fpha_integration.rs` line 315) passes `correction_factor = alpha` to the
model, which multiplies again (`crates/labdessem-model/src/constraints/hydro.rs`,
`build_hydro_fpha_constraints`). Sections 5 and 7a of `docs/auditoria/fpha-baseline-reproduction.md`
hold the reproduced numbers. Three fixes exist, with different consequences for the
`resultado_fpha_computed.csv` contract and the `OPER_FPHA.csv` copy; the user chooses.

### Relation to Epic

Decision gate of Epic 01 for A1. ticket-011, ticket-012 and ticket-013 (Epic 02) assume option (b)
and stay blocked until ADR-025 is `accepted`.

### Current State

- ADR-025 is `proposed` in the frontmatter of `plans/fpha-corrections/00-master-plan.md`, with options
  (a1), (a2), (b) and the recommendation (b).
- `docs/auditoria/fpha-decisions/` does not exist; this ticket creates it.
- The precomputed path reads `OPER/OPER_FPHA.csv` (`USIH;Nome;SegFPHA;Fcorrec;Rhs.mw;Varm.mw/hm3;
  Qtur.mw/m3s;Qlat.mw/m3s`) and the model multiplies every coefficient by `Fcorrec` once.

## Specification

### Requirements

1. Write `docs/auditoria/fpha-decisions/ADR-025-alpha-application.md` in English with exactly these
   level-2 headings, in order: `## Status`, `## Context`, `## Methodology reference`, `## Options`,
   `## Recommendation`, `## Decision`, `## Consequences`.
2. `## Status` holds `Proposed`. `## Decision` holds only `PENDING` (the orchestrator fills it after
   the user's choice).
3. `## Context` summarizes the defect with the reproduced numbers of sections 5 and 7a of the
   evidence report (alpha per plant, `E_LP / intended`, objective gaps) and cites the code lines.
4. `## Methodology reference` states how DESSEM applies the correction factor to the FPHA planes
   (MM section 9.16.11-9.16.12; Libs section 15.6.1) and how `OPER_FPHA` / `Fcorrec` is meant to be
   used, citing the dessem skill reference
   `~/.claude/skills/dessem/references/42-metodologia-hidreletricas.md` section 16 and
   `fpha-formulation.md`. Any claim not found there is tagged `[to confirm]`.
5. `## Options` has one level-3 subsection per option, `### (a1)`, `### (a2)`, `### (b)` (as named in
   the master-plan frontmatter), each with the labelled lines `Gains:`, `Losses:`, `Risks:`,
   `resultado_fpha_computed.csv:`, `OPER_FPHA.csv copy:` and `Tickets affected:`.
6. `## Recommendation` names one option and gives the reasons; `## Consequences` lists the changes
   each option implies for tickets 011-013 and for `CHANGELOG.md`.
7. The agent's final message is the `CLARIFICATION REQUIRED:` payload of CLAUDE.md with options
   A = (a1), B = (a2), C = (b) and the recommendation letter.

### Inputs/Props

Sections 5 and 7a of `docs/auditoria/fpha-baseline-reproduction.md`; audit sections 7 (FP-02, FP-08)
and 13 (D2); `crates/labdessem-fpha/src/pipeline.rs`; `crates/labdessem-cli/src/fpha_integration.rs`;
`crates/labdessem-io/src/output/fpha_computed.rs`; the dessem skill references.

### Outputs/Behavior

One committed brief and one `CLARIFICATION REQUIRED:` payload; no code change.

### Error Handling

- If the methodology reference contradicts the audit's description of the Libs/`Fcorrec` form, state
  the contradiction in `## Methodology reference` and add it to the payload as the first line of the
  situation; do not choose for the user.
- If sections 5-7a of the evidence report are missing, stop and report that ticket-007 is incomplete.

### Out of Scope

- Changing `pipeline.rs`, `fpha_integration.rs` or any test (Epic 02).
- Judging ONS operational choices or official deck parameters.
- The alpha evaluation points (B5, ADR-033) and the volume reference (B1, ADR-030).

## Acceptance Criteria

- [ ] Given the brief, when `grep -E "^## " docs/auditoria/fpha-decisions/ADR-025-alpha-application.md`
  runs, then it prints exactly the seven headings of requirement 1, in order.
- [ ] Given the brief, when `grep -cE "^### \((a1|a2|b)\)" docs/auditoria/fpha-decisions/ADR-025-alpha-application.md`
  runs, then it prints 3, and each option subsection contains the six labelled lines of requirement 5.
- [ ] Given the brief, when the `## Status` and `## Decision` sections are read, then they contain
  `Proposed` and `PENDING` respectively.
- [ ] Given the brief, when `sed -n '/^## Methodology reference/,/^## Options/p'
  docs/auditoria/fpha-decisions/ADR-025-alpha-application.md | grep -c "42-metodologia-hidreletricas.md"` runs,
  then it prints at least 1, and every paragraph of that section contains an `MM` or `Libs` section
  number or the tag `[to confirm]`.
- [ ] Given the ticket's end, when the agent returns, then its final message starts with
  `CLARIFICATION REQUIRED:` and lists options A, B, C and a recommendation, and `git show --stat HEAD`
  lists only `docs/auditoria/fpha-decisions/ADR-025-alpha-application.md`.

## Implementation Guide

### Suggested Approach

1. Load the `dessem` skill; read section 16 of `42-metodologia-hidreletricas.md` and
   `fpha-formulation.md`.
2. Read sections 5 and 7a of the evidence report and the code lines cited in Background.
3. Write the brief; commit `docs(audit): add ADR-025 alpha application brief`.
4. Return the payload as the final message.

### Key Files to Modify

- `docs/auditoria/fpha-decisions/ADR-025-alpha-application.md` (create): the decision brief.

### Patterns to Follow

- Audit section 13 ("Decisões que dependem de você") for presenting options to the user, written in
  English here.
- The ADR option names and wording of the master-plan frontmatter, so the orchestrator can map the
  answer to `chosen`.

### Pitfalls to Avoid

- Option (b) changes the meaning of the `gamma_*` columns of `resultado_fpha_computed.csv`; state it
  as a BREAKING change, not as a cosmetic one.
- Removing `scale_planes` can change the angle-reduction result (plane normals are not
  scale-invariant); list it under the Risks of (b).
- Do not edit `plans/fpha-corrections/00-master-plan.md`; the orchestrator records the decision.

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
- **Blocks**: ticket-011-apply-alpha-once-in-fit-pipeline.md
- **Notes**: after this ticket the orchestrator asks the user and records ADR-025 as `accepted`.

## Effort Estimate

**Points**: 2
**Confidence**: High
**Rationale**: the evidence and the option list exist; the work is methodology citation and a
structured brief.
