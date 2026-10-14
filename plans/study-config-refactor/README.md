# Study Config Per Case Refactor

## Summary

This plan refactors the project to make each study case self-contained by moving `study_config.json` into each case directory and updating the CLI to receive the case path as an argument.

## Mode

**Rigoroso** - English throughout, minimal comments.

## Epics

| Epic | Description | Tickets |
|------|-------------|---------|
| Epic 01 | Config structure changes | 3 |
| Epic 02 | CLI refactor | 2 |
| Epic 03 | Documentation | 2 |

**Total tickets: 7**

## Execution Order

```
ticket-001 (struct changes)
    ↓
ticket-002 (create configs)
    ↓
ticket-003 (update tests)
    ↓
ticket-004 (rename binary)
    ↓
ticket-005 (implement run subcommand)
    ↓
ticket-006 (update README)
    ↓
ticket-007 (update CHANGELOG)
```

## Quick Start

Execute the plan:
```
/implement-plan plans/study-config-refactor
```

Or execute individual tickets:
```
/implement plans/study-config-refactor/epic-01-config-structure/ticket-001-update-study-config-struct.md
```
