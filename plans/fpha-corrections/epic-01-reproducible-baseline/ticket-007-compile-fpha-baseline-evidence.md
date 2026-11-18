---
filesToBeCreated: []
filesToBeModified:
  - docs/auditoria/fpha-baseline-reproduction.md
filesToBeDeleted: []
implements: [R8]
agent: monorepo-test-developer
tier: mechanical
---

# ticket-007 Compile FPHA Baseline Evidence Report

## Context

### Background

Tickets 003-006 committed characterization tests that print the A1, A2 and A3 measurements on the
pre-fix code. The three decision briefs (tickets 008-010) and the post-fix comparison (ticket-017)
need those numbers in one committed, readable place, next to the values the audit
(`docs/auditoria/2026-11-05-auditoria-modelo.md`, sections 7 and 12.4) recorded with its release
binary and its full configuration. This ticket runs the tests with `--nocapture` and fills
sections 5-7 of the evidence report created by ticket-001.

### Relation to Epic

Closing evidence ticket of Epic 01; it gates the three decision tickets (008, 009, 010).

### Current State

- `docs/auditoria/fpha-baseline-reproduction.md` has sections 1-4 filled; sections 5-7 hold only
  `Pending: ticket-007.`; section 8 holds `Pending: ticket-017.`.
- Printing tests: `baseline_a1_*` (ticket-003, in-process), `baseline_a1_computed_vs_precomputed_copies`
  (ticket-004, E2E), `baseline_a2_*` (ticket-005), `baseline_a3_*` (ticket-006).
- Audit reference values: alpha CAMARGOS 0.969607, FURNAS 0.991295, ITUTINGA and FUNIL-GRANDE
  1.000000; computed objective +3,125.65 R$ above the precomputed copy with `Fcorrec = 1` (official
  configuration, objective 4,502,332.7276); FUNIL-GRANDE 180 MW with Q = 0 in 48 of 48 periods; FURNAS
  FPH about +9 % with efficiency 1 and loss 0 (FP-R02).

## Specification

### Requirements

1. Run, from the repository root on `fix/fpha-corrections`:
   `cargo test -p labdessem-cli baseline_a1 -- --nocapture`,
   `cargo test -p labdessem-cli --test fpha_e2e -- --nocapture`,
   `cargo test -p labdessem-cli baseline_a2 -- --nocapture`,
   `cargo test -p labdessem-cli baseline_a3 -- --nocapture`; keep the logs outside the repository.
2. Replace `Pending: ticket-007.` in section 5 (`## 5. A1 alpha application`) with: the command; a
   table with one row per plant code 0-3, each row starting `| <code> |` (alpha to 6 decimals, plane count, minimum and maximum
   `E_LP / intended`, reported and LP-based max/mean/RMS deviation on the fit grid, LP-based deviation
   on the 11 x 11 grid); a table of the three E2E objectives labelled `O_c`, `O_alpha`, `O_one`, with
   both relative gaps and run times.
3. Replace it in section 6 (`## 6. A2 skipped plants`) with: the in-process counts (skipped plants,
   FPHA rows, coupling rows for plant 2; the validation message) and the E2E counts (periods with
   `GeracaoMW > 1` and `TurbinamentoM3s <= 1e-6`, maximum generation).
4. Replace it in section 7 (`## 7. A3 physical defaults`) with: one table per case (missing tailrace,
   missing efficiency and loss) with mean and maximum FPH ratio and mean LP-envelope ratio per plant.
5. Add `## 7a. Comparison with the audit` (between sections 7 and 8): one table row per audit value
   listed in Current State with the reproduced value, and a "Notes" column that states why a value
   differs (fast configuration versus the audit's official configuration, debug versus release).
6. Section 8 keeps `Pending: ticket-017.` unchanged.

### Inputs/Props

The test logs of requirement 1; the audit report (read-only).

### Outputs/Behavior

Sections 5, 6, 7 and 7a of `docs/auditoria/fpha-baseline-reproduction.md`, in English.

### Error Handling

- If any `baseline_*` test fails, stop and return the `CLARIFICATION REQUIRED:` payload with the
  failure output; do not edit a test to make it pass.
- If a reproduced alpha differs from the audit value by more than 1e-6 for the same plant, record it
  in section 7a and flag it in the ticket report; the decision tickets read it.

### Out of Scope

- Any code or test change (tickets 003-006 own the tests).
- Interpreting the numbers into recommendations (tickets 008-010).
- Section 8 (ticket-017) and sections 1-4 (ticket-001).

## Acceptance Criteria

- [ ] Given the four test commands of requirement 1, when they run, then each exits 0 and the logs
  hold the tables that sections 5-7 copy.
- [ ] Given the updated report, when `grep -c "Pending: ticket-007." docs/auditoria/fpha-baseline-reproduction.md`
  runs, then it prints 0, and `grep -c "Pending: ticket-017." docs/auditoria/fpha-baseline-reproduction.md`
  prints 1.
- [ ] Given section 5 of the report, when `sed -n '/^## 5\. /,/^## 6\. /p'
  docs/auditoria/fpha-baseline-reproduction.md | grep -cE '^\| [0-3] \|'` runs, then it prints at
  least 4, and the same section contains `O_c`, `O_alpha` and `O_one`.
- [ ] Given section `## 7a. Comparison with the audit`, when `sed -n '/^## 7a\. /,/^## 8\. /p'
  docs/auditoria/fpha-baseline-reproduction.md | grep -c '^| '` runs, then it prints at least 7
  (header, separator and the five audit rows: two alphas, objective gap, FUNIL-GRANDE periods, FURNAS
  FPH inflation).
- [ ] Given the ticket commit, when `git show --stat HEAD` runs, then it lists only
  `docs/auditoria/fpha-baseline-reproduction.md`.

## Implementation Guide

### Suggested Approach

1. Run the four commands with `2> <scratch>/a1.log` style redirection.
2. Copy the printed tables into the report sections verbatim (same decimals), adding one-line captions
   that name the test that prints them.
3. Write section 7a, then commit `docs(audit): record FPHA A1-A3 baseline evidence`.

### Key Files to Modify

- `docs/auditoria/fpha-baseline-reproduction.md`: sections 5, 6, 7 and the new 7a.

### Patterns to Follow

- Tables in the style of section 12.4 of the audit, in English.
- Commands in fenced `bash` blocks so a reader can rerun them.

### Pitfalls to Avoid

- The audit used the official configuration (`rede` 1, `UCT` 1); objectives are not comparable in
  absolute value, only the sign and order of magnitude of the gap.
- The audit uses comma decimals (Portuguese); write dot decimals in this English report.
- Stage the report path only (`plans/dynamic-max-swallowing/` deletions stay unstaged).

## Testing Requirements

### Unit Tests

None: no code changes. The `baseline_*` tests run as measurements.

### Integration Tests

`cargo test --workspace` is not required here beyond the four commands; no code changed since
ticket-006.

### E2E Tests (if applicable)

`cargo test -p labdessem-cli --test fpha_e2e -- --nocapture` (requirement 1) supplies the objectives.

## Dependencies

- **Blocked By**: ticket-003-characterize-double-alpha-envelope.md, ticket-004-add-fpha-e2e-derived-case-harness.md, ticket-005-characterize-skipped-plant-generation.md, ticket-006-characterize-silent-physical-defaults.md
- **Blocks**: ticket-008-decide-alpha-application.md, ticket-009-decide-skipped-plant-policy.md, ticket-010-decide-missing-physical-data-policy.md
- **Notes**: the decision briefs cite sections 5-7a of this report.

## Effort Estimate

**Points**: 1
**Confidence**: High
**Rationale**: four test runs and copying their printed tables into an existing report.
