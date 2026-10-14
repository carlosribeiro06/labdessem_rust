# Epic 02: CLI Refactor

## Overview

Rename the binary from `labdessem-cli` to `labdessem`, implement the `run` subcommand, and update argument handling to receive the case directory as a positional argument.

## Scope

- Rename crate/binary from `labdessem-cli` to `labdessem`
- Implement `labdessem run <case_dir>` command structure
- Add fallback to current directory when no path provided
- Update workspace configuration

## Tickets

1. `ticket-004-rename-binary.md` - Rename labdessem-cli to labdessem
2. `ticket-005-implement-run-subcommand.md` - Implement run subcommand with case path argument

## Dependencies

- Epic 01 must be complete (config structure changes)

## Acceptance Criteria

- [ ] Binary is named `labdessem` (not `labdessem-cli`)
- [ ] `labdessem run /path/to/case` executes the simulation
- [ ] `labdessem run` (no arg) uses current directory
- [ ] Clear error messages for invalid paths or missing config
- [ ] Help text explains usage
