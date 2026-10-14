# Master Plan: Study Config Per Case Refactor

## Executive Summary

Move `study_config.json` from the `labdessem-io` crate into each case directory, remove the `case_path` field, and change the CLI to receive the case directory as a command-line argument. This decouples configuration from code and makes each study case self-contained.

## Goals & Non-Goals

### Goals
- Make each case directory self-contained with its own `study_config.json`
- Remove `case_path` from config (the CLI receives it as argument)
- Rename binary from `labdessem-cli` to `labdessem`
- Implement `labdessem run <case_dir>` command interface
- Fallback to current directory if no path provided
- Update all tests to use example cases
- Update documentation

### Non-Goals
- Change the format of other config fields
- Add new config options
- Modify the simulation logic
- Change output file locations (still `<case>/RESULTADOS/`)

## Architecture Overview

### Current State
```
crates/labdessem-io/study_config.json  ← config with case_path
examples/24Barras/
  CAD/
  OPER/
```

CLI: `labdessem-cli [config_path]`

### Target State
```
examples/24Barras/
  study_config.json  ← config without case_path
  CAD/
  OPER/
```

CLI: `labdessem run <case_dir>` or `labdessem run` (uses current dir)

### Key Design Decisions

1. **Config location**: Always `<case_dir>/study_config.json`
2. **CLI structure**: Subcommand `run` for execution (allows future subcommands)
3. **Fallback**: Current directory if no argument provided
4. **Binary name**: `labdessem` (not `labdessem-cli`)

## Technical Approach

### Tech Stack
- Rust (existing)
- No new dependencies required

### Component/Module Breakdown

| Component | Changes |
|-----------|---------|
| `labdessem-cli` | Rename to `labdessem`, implement subcommand parsing, change argument handling |
| `labdessem-io` | Remove `case_path` from `StudyConfig`, update `read_study_from_config` signature |
| `labdessem-solver` | Update test paths |
| Example cases | Add `study_config.json` to each |
| Documentation | Update README, remove old config file |

### Data Flow

```
User runs: labdessem run /path/to/case
  ↓
CLI parses argument, resolves to absolute path
  ↓
CLI reads /path/to/case/study_config.json
  ↓
CLI passes case_path + config to read_study_from_path()
  ↓
IO reads CAD/*.csv and OPER/*.csv from case_path
  ↓
Simulation runs
  ↓
Results written to /path/to/case/RESULTADOS/
```

### Testing Strategy

- Update existing tests to point to `examples/24Barras`
- Tests will read the real `study_config.json` from that case
- No mock configs needed

## Phases & Milestones

| Phase | Description | Tickets |
|-------|-------------|---------|
| Epic 1 | Config structure changes | 3 tickets |
| Epic 2 | CLI refactor | 2 tickets |
| Epic 3 | Documentation and cleanup | 2 tickets |

## Risk Analysis

| Risk | Mitigation |
|------|------------|
| Breaking existing workflows | Clear error message if old config format detected |
| Test failures | Update tests before merging |
| Missing config in examples | Verify all 3 example cases have config |

## Success Metrics

- [ ] `cargo build -p labdessem` produces binary named `labdessem`
- [ ] `labdessem run examples/24Barras` executes successfully
- [ ] `labdessem run` in a case directory works (fallback)
- [ ] All tests pass
- [ ] No `study_config.json` in `crates/labdessem-io/`
- [ ] All 3 example cases have their own `study_config.json`
