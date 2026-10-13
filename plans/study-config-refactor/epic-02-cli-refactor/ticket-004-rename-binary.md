# [TICKET-004] Rename labdessem-cli to labdessem

## Context

### Background
The binary is currently named `labdessem-cli`, which is verbose. The standard practice is to name the binary after the project itself.

### Relation to Epic
This is a prerequisite for the CLI refactor - we rename first, then modify behavior.

### Current State
```toml
# crates/labdessem-cli/Cargo.toml
[package]
name = "labdessem-cli"
```

Binary produced: `target/release/labdessem-cli`

## Specification

### Requirements
1. Rename the crate from `labdessem-cli` to `labdessem`
2. Rename the directory from `crates/labdessem-cli` to `crates/labdessem`
3. Update workspace members in root `Cargo.toml`
4. Verify binary is now named `labdessem`

### Inputs/Props
N/A - configuration change

### Outputs/Behavior
Binary produced: `target/release/labdessem`

### Error Handling
N/A

## Acceptance Criteria
- [ ] Directory is `crates/labdessem/`
- [ ] Crate name in `Cargo.toml` is `labdessem`
- [ ] Workspace members list shows `crates/labdessem`
- [ ] `cargo build -p labdessem --release` produces `target/release/labdessem`

## Implementation Guide

### Suggested Approach
1. Rename directory: `mv crates/labdessem-cli crates/labdessem`
2. Update `crates/labdessem/Cargo.toml`:
   ```toml
   [package]
   name = "labdessem"
   ```
3. Update root `Cargo.toml` workspace members:
   ```toml
   members = [
       "crates/labdessem",  # was labdessem-cli
       ...
   ]
   ```
4. Run `cargo build -p labdessem` to verify

### Key Files to Modify
- RENAME: `crates/labdessem-cli/` → `crates/labdessem/`
- MODIFY: `crates/labdessem/Cargo.toml` (package name)
- MODIFY: `Cargo.toml` (workspace members)

### Patterns to Follow
- Keep all other Cargo.toml fields unchanged (version.workspace, etc.)

### Pitfalls to Avoid
- Don't forget to update the workspace members list
- The binary name comes from the package name, not the directory name

## Testing Requirements

### Unit Tests
N/A - configuration change

### Integration Tests
`cargo build -p labdessem` should succeed.

## Dependencies
- **Blocked By**: Epic 01 (all tickets)
- **Blocks**: ticket-005

## Effort Estimate
**Points**: 1
**Confidence**: High
