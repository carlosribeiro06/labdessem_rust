# [003] Move renewable and pumping writers

## Context

Simpler writers for renewable plants and pumping stations.

## Specification

1. Create `output/renewable.rs` with `write_renewable_results()`
2. Create `output/pumping.rs` with `write_pumping_results()`
3. Move logic from simulation

## Acceptance Criteria

- [ ] Both files created
- [ ] Result CSVs generated correctly
- [ ] `cargo test` passes

## Dependencies

- **Blocked By**: 001
- **Blocks**: 005

## Effort Estimate

**Points**: 1
