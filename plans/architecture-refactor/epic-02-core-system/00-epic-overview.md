# Epic 02: Core System + Builder Pattern

## Overview

Refactor the `System` struct and its construction to follow the Builder pattern,
separating construction from validation and improving error handling with typed
validation errors.

## Goals

- Create `system/` directory with separated modules
- Implement `SystemBuilder` for fluent construction API
- Separate validation logic into dedicated module
- Replace generic `String` errors with typed `ValidationError` variants
- Make `System` immutable after construction

## Scope

### In Scope

- Splitting `system.rs` into `system/mod.rs`, `system/builder.rs`, `system/validate.rs`
- Creating `ValidationError` enum with specific variants
- Updating `labdessem-io` to use `SystemBuilder`

### Out of Scope

- Changing the fields or structure of `System`
- Adding new validation rules
- Modifying how validation is triggered during loading

## Architecture

### Before

```
labdessem-core/src/
├── system.rs    # System struct + all helper types + mixed validation
└── error.rs     # CoreError::InvalidData(String)
```

### After

```
labdessem-core/src/
├── system/
│   ├── mod.rs       # System struct + helper types
│   ├── builder.rs   # SystemBuilder
│   └── validate.rs  # validate_system() + validation helpers
├── horizon.rs       # StudyHorizon (extracted)
└── error.rs         # ValidationError enum with typed variants
```

## Tickets

| ID | Title | Est. | Dependencies |
|----|-------|------|--------------|
| 001 | Create system module structure | 20min | Epic 01 |
| 002 | Implement SystemBuilder | 45min | 001 |
| 003 | Implement validation module | 30min | 001 |
| 004 | Create typed ValidationError | 30min | 003 |

## Success Criteria

- All tests pass
- `SystemBuilder` can construct a `System` with fluent API
- `ValidationError` has at least 5 typed variants
- `system.rs` no longer exists (replaced by `system/` directory)
- No changes to how `labdessem-io` constructs systems (adapter layer)

## Estimated Effort

**Total**: 3-4 hours
