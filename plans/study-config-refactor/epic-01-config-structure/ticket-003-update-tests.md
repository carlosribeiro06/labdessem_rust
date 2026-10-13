# [TICKET-003] Update tests to use example case directories

## Context

### Background
Tests in `labdessem-io` and `labdessem-solver` currently reference `crates/labdessem-io/study_config.json`. They need to be updated to use the example case directories.

### Relation to Epic
This completes Epic 01 by ensuring all tests work with the new structure.

### Current State
```rust
// crates/labdessem-solver/src/lib.rs
let config_path = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
    .join("../labdessem-io/study_config.json");
```

## Specification

### Requirements
1. Update tests in `labdessem-io` to use `examples/24Barras`
2. Update tests in `labdessem-solver` to use `examples/24Barras`
3. Use the new `read_study_from_path` function signature
4. All tests must pass

### Inputs/Props
Tests receive path to `examples/24Barras` directory.

### Outputs/Behavior
Tests execute successfully using the example case.

### Error Handling
Tests should fail gracefully with clear messages if example case is missing.

## Acceptance Criteria
- [ ] `cargo test -p labdessem-io` passes
- [ ] `cargo test -p labdessem-solver` passes
- [ ] No references to `crates/labdessem-io/study_config.json` remain in test code
- [ ] Tests use `examples/24Barras` as the test case

## Implementation Guide

### Suggested Approach
1. Find all test functions that reference the old config path
2. Update path construction:
   ```rust
   // Old
   let config_path = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
       .join("../labdessem-io/study_config.json");
   
   // New
   let case_path = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
       .join("../../examples/24Barras");
   ```
3. Update function calls to use new signature:
   ```rust
   // Old
   let system = read_study_from_config(&config_path)?;
   
   // New
   let system = read_study_from_path(&case_path)?;
   ```
4. Run tests and fix any issues

### Key Files to Modify
- `crates/labdessem-io/src/study.rs` (test module)
- `crates/labdessem-solver/src/lib.rs` (test module)

### Patterns to Follow
- Use `env!("CARGO_MANIFEST_DIR")` for workspace-relative paths
- Path from `labdessem-io` to examples: `../../examples/24Barras`
- Path from `labdessem-solver` to examples: `../../examples/24Barras`

### Pitfalls to Avoid
- Don't hardcode absolute paths
- Remember that `CARGO_MANIFEST_DIR` points to the crate's directory, not workspace root

## Testing Requirements

### Unit Tests
The tests themselves are being updated.

### Integration Tests
Run `cargo test` for the entire workspace to verify nothing is broken.

## Dependencies
- **Blocked By**: ticket-001, ticket-002
- **Blocks**: Epic 02 tickets

## Effort Estimate
**Points**: 2
**Confidence**: High
