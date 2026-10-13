# [TICKET-001] Remove case_path from StudyConfig struct

## Context

### Background
The `StudyConfig` struct currently contains a `case_path` field that specifies the directory of the study case. This creates coupling between the configuration file and its location, making cases non-portable.

### Relation to Epic
This is the foundational change for Epic 01. All other tickets depend on this structural change.

### Current State
```rust
// crates/labdessem-io/src/study.rs
pub struct StudyConfig {
    pub case_path: PathBuf,  // ← to be removed
    pub opcao_execucao: u8,
    pub rede: u8,
    pub uct: u8,
    pub uch: u8,
    pub fpha: u8,
    pub fcf: u8,
    pub ton_residual: u8,
}
```

## Specification

### Requirements
1. Remove `case_path` field from `StudyConfig` struct
2. Create new function `read_study_config(config_path: &Path) -> Result<StudyConfig, IoError>`
3. Modify `read_study_from_config` to become `read_study_from_path(case_path: &Path) -> Result<System, IoError>`
4. The new function should:
   - Receive the case directory path
   - Look for `study_config.json` inside that directory
   - Read CAD/ and OPER/ relative to the case directory

### Inputs/Props
- `case_path: &Path` - absolute or relative path to the case directory

### Outputs/Behavior
- `Result<System, IoError>` - the loaded system or an error

### Error Handling
- If `study_config.json` does not exist in the case directory:
  ```
  Error: Configuration file not found.
  Expected: /path/to/case/study_config.json
  
  Each study case must contain a study_config.json file in its root directory.
  ```

## Acceptance Criteria
- [ ] `StudyConfig` struct has no `case_path` field
- [ ] `read_study_from_path(case_path)` reads config from `<case_path>/study_config.json`
- [ ] All CAD/OPER files are read relative to the provided case_path
- [ ] RESULTADOS are written to `<case_path>/RESULTADOS/`
- [ ] Clear error message when config file is missing

## Implementation Guide

### Suggested Approach
1. Modify `StudyConfig` struct - remove `case_path` field
2. Rename/refactor `read_study_from_config`:
   - Old: `read_study_from_config(config_path: &Path)`
   - New: `read_study_from_path(case_path: &Path)`
3. Inside the new function:
   - Build config path: `case_path.join("study_config.json")`
   - Read and parse config
   - Pass `case_path` to `read_study_from_path_with_options`
4. Update `read_study_config` to not require `case_path` in JSON

### Key Files to Modify
- `crates/labdessem-io/src/study.rs`
  - `StudyConfig` struct
  - `read_study_config` function
  - `read_study_from_config` → `read_study_from_path`

### Patterns to Follow
- Use `case_path.join("study_config.json")` for config location
- Validate file existence before attempting to read
- Keep error messages user-friendly

### Pitfalls to Avoid
- Don't forget to update the serde deserialization (remove case_path)
- Don't break the public API without updating callers (CLI)

## Testing Requirements

### Unit Tests
- Test that `StudyConfig` deserializes without `case_path`
- Test error message when config file missing

### Integration Tests
- Defer to ticket-003 (test updates)

## Dependencies
- **Blocked By**: None
- **Blocks**: ticket-002, ticket-003, all Epic 02 tickets

## Effort Estimate
**Points**: 3
**Confidence**: High
