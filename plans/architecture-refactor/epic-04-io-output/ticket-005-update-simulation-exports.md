# [005] Update simulation exports

## Context

After moving writers, update simulation to re-export from labdessem-io.

## Specification

1. Remove all writer code from `labdessem-simulation/lib.rs`
2. Add `labdessem-io` as dependency if not already
3. Re-export `labdessem_io::output::write_results`
4. Update any internal calls

## Acceptance Criteria

- [ ] `lib.rs` has no CSV writing code
- [ ] `lib.rs` reduced by ~800 lines
- [ ] Public API unchanged (re-exports)
- [ ] `cargo test` passes

## Verification

```bash
wc -l crates/labdessem-simulation/src/lib.rs
# Should be significantly reduced
```

## Dependencies

- **Blocked By**: 002, 003, 004
- **Blocks**: Epic 05

## Effort Estimate

**Points**: 1
