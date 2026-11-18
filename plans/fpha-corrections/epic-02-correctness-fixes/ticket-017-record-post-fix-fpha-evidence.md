---
filesToBeCreated: []
filesToBeModified:
  - docs/auditoria/fpha-baseline-reproduction.md
filesToBeDeleted: []
implements: [R8, R1, R3, R4]
agent: monorepo-test-developer
tier: mechanical
---

# ticket-017 Record Post-Fix FPHA Evidence

## Context

### Background

Tickets 011-016 fixed A1-A3 and replaced every `baseline_*` test. The evidence report
`docs/auditoria/fpha-baseline-reproduction.md` holds the pre-fix numbers (sections 4-7a) and a
placeholder section 8. The plan's success metrics and Epic 03 refinement need a committed pre/post
comparison: alpha per plant (the fix must not change it), plane counts (the angle reduction may
change them), deviations, objectives, the round-trip gap and the new failure messages.

### Relation to Epic

Closing ticket of Epic 02; Epic 03 refinement and ADR-028/ADR-029 briefs read section 8.

### Current State

- Section 8 contains only `Pending: ticket-017.`.
- Printing tests after Epic 02: `lp_envelope_matches_alpha_scaled_planes`,
  `lp_envelope_deviation_matches_reported_deviation`, `computed_csv_round_trip_reproduces_objective`,
  `computed_csv_round_trip_reproduces_segments`; failure-path tests `plant_without_geometry_fails_run`,
  `missing_tailrace_row_fails_fitting`, `missing_efficiency_and_loss_fail_fitting`.
- Section 3 describes the derived-case procedure and the fast configuration.

## Specification

### Requirements

1. Run `cargo build --workspace --release`, `cargo test --workspace`, `cargo clippy --workspace
   --all-targets -- -D warnings`, and the tests of Current State with `-- --nocapture`; keep logs
   outside the repository.
2. Run the "fast computed" and "fast constant_productivity" derived cases of section 4 with the
   release binary.
3. Replace `Pending: ticket-017.` in `## 8. Post-fix comparison` with:
   - a build/test/clippy table in the format of section 2, with pre and post counts;
   - an A1 table, one row per plant code 0-3, each row starting `| <code> |`: alpha pre/post (6 decimals), plane count pre/post,
     reported max deviation pre/post, LP-based max deviation pre/post;
   - an objective table: computed pre/post, constant_productivity pre/post, round-trip gap post (row
     label `round-trip gap`);
   - A2 and A3 rows with the post-fix exit code (`exit code 1`) and the first error line of each
     failing case;
   - a "Differences" paragraph that names every pre/post change other than the intended ones.

### Inputs/Props

Sections 2-7a of the report; the logs of requirements 1-2.

### Outputs/Behavior

Section 8 of the evidence report, in English.

### Error Handling

- If any test fails or clippy warnings exceed section 2, stop and return the `CLARIFICATION
  REQUIRED:` payload with the output; do not edit code.
- If an alpha differs between pre and post by more than 1e-12, record it and flag it in the ticket
  report (the fix should not change alpha).

### Out of Scope

- Code or test changes.
- Sections 1-7a (frozen pre-fix evidence).
- Recommendations for Epic 03 (its decision briefs).

## Acceptance Criteria

- [ ] Given the post-fix branch, when the commands of requirement 1 run, then each exits 0 and the
  logs hold the printed tables.
- [ ] Given the updated report, when `grep -c "Pending: ticket-017." docs/auditoria/fpha-baseline-reproduction.md`
  runs, then it prints 0.
- [ ] Given section 8, when `sed -n '/^## 8\. /,$p' docs/auditoria/fpha-baseline-reproduction.md |
  grep -cE '^\| [0-3] \|'` runs, then it prints at least 4 (one A1 row per plant code with alpha,
  plane count and both maximum deviations, pre and post).
- [ ] Given section 8, when `sed -n '/^## 8\. /,$p' docs/auditoria/fpha-baseline-reproduction.md`
  runs, then its output contains `round-trip gap`, `constant_productivity`, `exit code 1` at least
  twice and `computed FPHA could not be fitted`.
- [ ] Given the ticket commit, when `git show --stat HEAD` runs, then it lists only
  `docs/auditoria/fpha-baseline-reproduction.md`.

## Implementation Guide

### Suggested Approach

1. Run the commands; copy the printed values.
2. Write section 8; commit `docs(audit): record post-fix FPHA evidence`.

### Key Files to Modify

- `docs/auditoria/fpha-baseline-reproduction.md`: section 8 only.

### Patterns to Follow

- Tables of sections 2, 4 and 5 (same columns, same decimals).
- Commands in fenced `bash` blocks.

### Pitfalls to Avoid

- The constant_productivity objective must not change (FPHA rows are not built in that mode); a change
  is a finding to flag, not to explain away.
- Run the derived cases on fresh temp copies, never on `examples/24Barras`.
- Stage the report path only.

## Testing Requirements

### Unit Tests

None: documentation-only ticket.

### Integration Tests

`cargo test --workspace` as a measurement (requirement 1).

### E2E Tests (if applicable)

Two release CLI runs on derived cases (requirement 2).

## Dependencies

- **Blocked By**: ticket-013-add-precomputed-round-trip-regression.md, ticket-015-revalidate-system-after-fpha-injection.md, ticket-016-reject-missing-fpha-physical-data.md
- **Blocks**: ticket-018-decide-fpha-config-semantics.md, ticket-020-decide-plane-count-and-milp-selection.md, ticket-023-reject-fpha-rows-for-plants-without-units.md, ticket-024-validate-fpha-geometry-inputs.md, ticket-025-decide-fpha-volume-reference.md, ticket-026-decide-turbining-axis-and-pmax-cap.md, ticket-027-decide-tailrace-family-modelling.md, ticket-028-decide-alpha-evaluation-points.md
- **Notes**: closes Epic 02; every Epic 03 and Epic 04 entry ticket starts after it.

## Effort Estimate

**Points**: 1
**Confidence**: High
**Rationale**: command runs and one report section in an existing format.
