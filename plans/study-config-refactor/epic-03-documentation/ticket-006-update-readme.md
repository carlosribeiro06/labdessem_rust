# [TICKET-006] Update README documentation

## Context

### Background
The README contains documentation for the old CLI interface and configuration structure. It needs to be updated to reflect all the changes made in Epics 01 and 02.

### Relation to Epic
This ensures users have accurate documentation.

### Current State
README shows:
- `labdessem-cli` in workspace structure
- `case_path` in config example
- Old CLI invocation commands

## Specification

### Requirements
1. Update workspace structure to show `labdessem` instead of `labdessem-cli`
2. Update Crates section description
3. Update Study Configuration section:
   - Remove `case_path` from keys list
   - Update example JSON (no `case_path`)
   - Note that config file is inside each case directory
4. Update Build and Run section:
   - New command: `labdessem run <case_dir>`
   - Show fallback behavior
5. Update any other references to old structure

### Inputs/Props
N/A - documentation update

### Outputs/Behavior
README reflects current state of the project.

### Error Handling
N/A

## Acceptance Criteria
- [ ] Workspace structure shows `labdessem` (6 crates total now)
- [ ] Crates section has no `labdessem-cli` or `labdessem-common`
- [ ] Config example has no `case_path`
- [ ] Study Configuration explains config is in case directory
- [ ] Build and Run shows `labdessem run examples/24Barras`
- [ ] No references to old `crates/labdessem-io/study_config.json`

## Implementation Guide

### Suggested Approach

1. Update Workspace Structure:
   ```text
   labdessem_rust/
   |-- crates/
   |   |-- labdessem
   |   |-- labdessem-core
   |   |-- labdessem-io
   |   |-- labdessem-model
   |   |-- labdessem-simulation
   |   `-- labdessem-solver
   ```

2. Update Crates section - change `labdessem-cli` description to:
   ```markdown
   - `labdessem`
     Command-line interface for running studies.
   ```

3. Update Study Configuration:
   - Remove `case_path` from keys list
   - Add note: "The configuration file must be placed in the root of each study case directory."
   - Update example:
   ```json
   {
     "opcao_execucao": 1,
     "rede": 1,
     "UCT": 1,
     "UCH": 0,
     "TON_Residual": 0,
     "FPHA": 0,
     "FCF": 0
   }
   ```

4. Update Build and Run:
   ```markdown
   ### Run a study case

   ```bash
   cargo run -p labdessem -- run examples/24Barras
   ```

   ### Run from the case directory

   ```bash
   cd examples/24Barras
   cargo run -p labdessem -- run
   ```

   ### Build release binary

   ```bash
   cargo build --release -p labdessem
   ./target/release/labdessem run examples/24Barras
   ```
   ```

### Key Files to Modify
- `README.md`

### Patterns to Follow
- Keep documentation concise
- Use consistent formatting

### Pitfalls to Avoid
- Don't miss any old references

## Testing Requirements

### Unit Tests
N/A - documentation

### Integration Tests
Review README for accuracy after changes.

## Dependencies
- **Blocked By**: ticket-005 (CLI must be implemented first)
- **Blocks**: ticket-007

## Effort Estimate
**Points**: 2
**Confidence**: High
