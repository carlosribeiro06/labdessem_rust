# Epic 01: Config Structure Changes

## Overview

Modify the `StudyConfig` struct to remove `case_path`, update the IO functions to receive the case path as a separate parameter, and create `study_config.json` files in each example case directory.

## Scope

- Remove `case_path` from `StudyConfig` struct
- Update `read_study_from_config` to accept case path as parameter
- Create `study_config.json` in each example case
- Delete old `crates/labdessem-io/study_config.json`
- Update tests in `labdessem-io` and `labdessem-solver`

## Tickets

1. `ticket-001-update-study-config-struct.md` - Remove case_path from StudyConfig
2. `ticket-002-create-example-configs.md` - Add study_config.json to example cases
3. `ticket-003-update-tests.md` - Update test paths to use example cases

## Dependencies

None - this is the first epic.

## Acceptance Criteria

- [ ] `StudyConfig` no longer contains `case_path`
- [ ] `read_study_from_path` accepts case directory as parameter
- [ ] All 3 example cases have `study_config.json`
- [ ] Old config file removed from `crates/labdessem-io/`
- [ ] All tests pass with new structure
