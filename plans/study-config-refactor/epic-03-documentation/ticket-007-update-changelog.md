# [TICKET-007] Add changelog entry

## Context

### Background
The CHANGELOG needs to document this refactoring as it's a breaking change to the CLI interface and configuration format.

### Relation to Epic
Final documentation step to complete the refactor.

### Current State
CHANGELOG last entry is v0.5.0.

## Specification

### Requirements
1. Add "Unreleased" section at the top of CHANGELOG
2. Document all breaking changes and improvements
3. Follow Keep a Changelog format

### Inputs/Props
N/A

### Outputs/Behavior
CHANGELOG has new unreleased section documenting changes.

### Error Handling
N/A

## Acceptance Criteria
- [ ] CHANGELOG has "Unreleased" section
- [ ] Breaking changes are clearly marked
- [ ] All changes from this refactor are documented

## Implementation Guide

### Suggested Approach

Add after the header, before [0.5.0]:

```markdown
## [Unreleased]

### Changed
- **BREAKING**: CLI now uses subcommand pattern: `labdessem run <case_dir>`
- **BREAKING**: `study_config.json` must be inside each case directory (not in crate)
- **BREAKING**: Removed `case_path` field from `study_config.json`
- Renamed binary from `labdessem-cli` to `labdessem`
- Renamed crate from `labdessem-cli` to `labdessem`

### Removed
- `labdessem-common` crate (was unused)
- `case_path` configuration field

### Fixed
- CLI now properly uses current directory as fallback when no path provided
```

### Key Files to Modify
- `CHANGELOG.md`

### Patterns to Follow
- Keep a Changelog format
- Mark breaking changes clearly

### Pitfalls to Avoid
- Don't assign a version number yet (use Unreleased)

## Testing Requirements

### Unit Tests
N/A

### Integration Tests
N/A

## Dependencies
- **Blocked By**: ticket-006
- **Blocks**: None (final ticket)

## Effort Estimate
**Points**: 1
**Confidence**: High
