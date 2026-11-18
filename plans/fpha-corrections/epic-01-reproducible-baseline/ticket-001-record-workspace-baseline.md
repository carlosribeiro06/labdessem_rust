---
filesToBeCreated:
  - docs/auditoria/fpha-baseline-reproduction.md
filesToBeModified: []
filesToBeDeleted: []
implements: [R8, R10]
agent: hpc-rust-developer
tier: mechanical
---

# ticket-001 Record Workspace Baseline Before FPHA Changes

## Context

### Background

The FPHA corrections start from `develop` at commit 4ac43e0 (v0.12.0 plus the model audit report
`docs/auditoria/2026-11-05-auditoria-modelo.md`). The workspace does not build today: the qhull
submodule `crates/labdessem-fpha/vendor/qhull` is not initialized, and `crates/labdessem-fpha/build.rs`
panics with "Run: git submodule update --init --recursive". There is no `target/` directory. Every
later measurement needs a known-good build and a recorded test and lint status, so that any later
failure is attributable to a ticket of this plan and not to the starting point.

### Relation to Epic

First ticket of Epic 01. It creates the evidence report that ticket-007 and ticket-017 extend, and
it measures the debug-profile run time that ticket-004 needs to size the end-to-end tests.

### Current State

- `git submodule status` prints `-613debe... crates/labdessem-fpha/vendor/qhull` (leading `-`).
- `examples/24Barras/study_config.json` sets `FPHA` to `constant_productivity`, `rede` 1, `UCT` 1,
  `EngMax` 1; the official example must stay unchanged.
- The working tree shows four unstaged deletions under `plans/dynamic-max-swallowing/`; they belong
  to another session and must never be restored, staged or committed.

## Specification

### Requirements

1. If the current branch is not `fix/fpha-corrections`, create it from `develop` with
   `git switch -c fix/fpha-corrections develop`.
2. Initialize qhull: `git submodule update --init --recursive crates/labdessem-fpha/vendor/qhull`.
3. Run, without fixing anything: `cargo build --workspace --release`, `cargo build --workspace`,
   `cargo test --workspace`, `cargo clippy --workspace --all-targets -- -D warnings`. Record exit
   codes, per-test-binary passed/failed/ignored counts and the clippy warning count per crate.
4. Run three derived cases, each in a fresh temp directory outside the repository (`mktemp -d`),
   holding copies of `examples/24Barras/CAD`, `examples/24Barras/OPER` and a new `study_config.json`:
   - "fast computed": the JSON below, with the release binary `target/release/labdessem run <dir>`;
   - "fast computed (debug)": the same JSON, with `target/debug/labdessem run <dir>`;
   - "fast constant_productivity": the JSON below with `"FPHA": "constant_productivity"`, release.
5. Create the report with the sections listed in Outputs.

### Inputs/Props

Fast configuration (written verbatim, no `$schema` key):

```json
{
  "opcao_execucao": 2,
  "rede": 0,
  "UCT": 0,
  "UCH": 0,
  "TON_Residual": 0,
  "FPHA": "computed",
  "FCF": 1,
  "EngMax": 0
}
```

`EngMax` 0 avoids the computed + UCT + EngMax infeasibility the audit recorded as EM-01; `rede` 0
and `UCT` 0 isolate the FPHA rows and shorten the solve.

### Outputs/Behavior

`docs/auditoria/fpha-baseline-reproduction.md`, in English, with these headings in order:

- `# FPHA Baseline Reproduction` and a one-paragraph purpose statement;
- `## 1. Environment`: date, commit SHA, branch, `rustc -V`, `cargo -V`, `uname -r`, qhull submodule
  SHA;
- `## 2. Build and test baseline`: each command with its `exit code`, a table with header
  `| Test binary | Passed | Failed | Ignored |`, and a table with header `| Crate | Warnings |`;
- `## 3. Derived case configurations`: the fast configuration JSON and the copy procedure;
- `## 4. Fast derived case runs`: one table row per run, the first cell being the run label
  (`fast computed`, `fast computed (debug)`, `fast constant_productivity`), then binary, exit code,
  wall time (s), `Objective:` value and the `Processed N plants, M skipped` counts;
- `## 5. A1 alpha application`, `## 6. A2 skipped plants`, `## 7. A3 physical defaults`,
  `## 8. Post-fix comparison`: each containing only the line `Pending: ticket-007.` (section 8:
  `Pending: ticket-017.`).

### Error Handling

- If the submodule fetch fails (network) or any build fails, stop and return the
  `CLARIFICATION REQUIRED:` payload with the command output; do not patch code.
- If a derived run exits nonzero, record it in section 4 and stop with the payload.
- If the debug run takes more than 120 s, record it and add a line `Risk: debug E2E run above 120 s`
  under section 4; ticket-004 reads it.

### Out of Scope

- Fixing clippy warnings or failing tests found by the baseline.
- Editing any file under `examples/24Barras/` or running the CLI inside it (it writes `RESULTADOS/`).
- Staging or restoring `plans/dynamic-max-swallowing/`; committing the submodule gitlink (it does not
  change).

## Acceptance Criteria

- [ ] Given the repository on `develop`, when the ticket work starts, then
  `git rev-parse --abbrev-ref HEAD` prints `fix/fpha-corrections` and `git diff --cached --name-only`
  lists no path under `plans/dynamic-max-swallowing/`.
- [ ] Given the uninitialized submodule, when `git submodule status crates/labdessem-fpha/vendor/qhull`
  runs after the init, then its output line does not start with `-`, and `cargo build --workspace
  --release` exits 0.
- [ ] Given the built workspace, when `sed -n '/^## 2\. /,/^## 3\. /p'
  docs/auditoria/fpha-baseline-reproduction.md` runs after both commands, then its output contains
  `cargo test --workspace`, `cargo clippy --workspace --all-targets -- -D warnings`, the word
  `exit code` at least twice, a table header containing `Passed | Failed | Ignored` and a table header
  containing `Crate | Warnings`.
- [ ] Given the three derived cases, when `sed -n '/^## 4\. /,/^## 5\. /p'
  docs/auditoria/fpha-baseline-reproduction.md | grep -cE '^\| fast '` runs after them, then it prints
  3, and each of those rows holds exit code, wall time (s), `Objective:` value, processed and skipped
  plant counts.
- [ ] Given the ticket commit, when `git show --stat HEAD` and `git status --short -- examples/` run,
  then the commit lists only `docs/auditoria/fpha-baseline-reproduction.md` and the status output is
  empty.

## Implementation Guide

### Suggested Approach

1. `git switch -c fix/fpha-corrections develop` (the unstaged deletions follow the switch; leave them).
2. Init the submodule, then build release and debug, run tests and clippy, saving the logs to the
   scratch directory (not to the repository).
3. Prepare each derived case: `d=$(mktemp -d); cp -r examples/24Barras/CAD examples/24Barras/OPER "$d"/`,
   then write the JSON to `"$d"/study_config.json`; time each run with `/usr/bin/time -f %e` or bash
   `time`; copy the `Objective:` and `Processed` lines from stderr.
4. Write the report, then `git add docs/auditoria/fpha-baseline-reproduction.md` and commit
   `docs(audit): record FPHA correction workspace baseline`.

### Key Files to Modify

- `docs/auditoria/fpha-baseline-reproduction.md` (create): the evidence report all later evidence
  tickets extend.

### Patterns to Follow

- Tables in the style of section 12.4 of `docs/auditoria/2026-11-05-auditoria-modelo.md`, written in
  English (Rigoroso).
- Commands shown in fenced `bash` blocks so a reader can rerun them.

### Pitfalls to Avoid

- `git add -A` or `git commit -a` would stage the `plans/dynamic-max-swallowing/` deletions; stage the
  report path only.
- Running `labdessem run examples/24Barras` writes `examples/24Barras/RESULTADOS/`; always run on
  temp copies.
- The objective is printed on stderr (`  Objective:   <value>`), not stdout.

## Testing Requirements

### Unit Tests

None: this ticket adds no code. Its verification is the command set of the Acceptance Criteria.

### Integration Tests

`cargo test --workspace` runs as a measurement; its result is recorded, not changed. A failing test
is recorded and escalated, never skipped or edited.

### E2E Tests (if applicable)

The three derived-case CLI runs of requirement 4 act as the manual end-to-end baseline; ticket-004
automates them.

## Dependencies

- **Blocked By**: None
- **Blocks**: ticket-002-add-fpha-lp-envelope-probe.md
- **Notes**: every later ticket depends on this one transitively through ticket-002.

## Effort Estimate

**Points**: 1
**Confidence**: High
**Rationale**: command execution and one report; the only uncertainty is network access for the
submodule.
