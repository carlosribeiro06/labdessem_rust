# [002] Add workspace lints

## Context

Centralized lint configuration ensures consistent code quality across all crates.

## Specification

Add `[workspace.lints]` section to root `Cargo.toml`:

```toml
[workspace.lints.rust]
unsafe_code = "forbid"
missing_docs = "warn"

[workspace.lints.clippy]
unwrap_used = "warn"
expect_used = "warn"
panic = "warn"
todo = "warn"
unimplemented = "warn"
dbg_macro = "warn"
```

Update each crate's `Cargo.toml` to inherit:

```toml
[lints]
workspace = true
```

## Acceptance Criteria

- [ ] `[workspace.lints]` in root `Cargo.toml`
- [ ] Each crate has `[lints] workspace = true`
- [ ] `cargo clippy` passes (fix any new warnings)
- [ ] `cargo test` passes

## Implementation Guide

1. Add lint section to workspace `Cargo.toml`
2. Add `[lints] workspace = true` to each crate's `Cargo.toml`
3. Run `cargo clippy` and fix any new warnings
4. Consider which warnings should be errors vs warnings

## Dependencies

- **Blocked By**: 001
- **Blocks**: None (final ticket)

## Effort Estimate

**Points**: 1
