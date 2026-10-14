# Epic 03: Documentation and Cleanup

## Overview

Update all documentation to reflect the new CLI interface and configuration structure. Ensure README, CHANGELOG, and any other docs are consistent with the changes.

## Scope

- Update README with new CLI usage
- Update study configuration documentation
- Update workspace structure section
- Add entry to CHANGELOG for this change

## Tickets

1. `ticket-006-update-readme.md` - Update README documentation
2. `ticket-007-update-changelog.md` - Add changelog entry

## Dependencies

- Epic 01 and Epic 02 must be complete

## Acceptance Criteria

- [ ] README shows correct CLI usage (`labdessem run <path>`)
- [ ] README shows correct config file location (inside case directory)
- [ ] README example config has no `case_path` field
- [ ] Workspace structure shows `labdessem` (not `labdessem-cli`)
- [ ] CHANGELOG documents the breaking change
