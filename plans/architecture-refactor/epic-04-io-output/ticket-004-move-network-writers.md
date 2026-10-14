# [004] Move network and summary writers

## Context

Network results (branch flows, violations) and summary/iteration log writers.

## Specification

1. Create `output/network.rs` with:
   - `write_network_results()`
   - `write_line_additions()`
   - `write_network_infeasibilities()`
2. Create `output/summary.rs` with:
   - `write_cmo_results()`
   - `write_iteration_log()`
   - `write_fcf_results()`

## Acceptance Criteria

- [ ] Both files created
- [ ] All result CSVs generated correctly
- [ ] `cargo test` passes

## Dependencies

- **Blocked By**: 001
- **Blocks**: 005

## Effort Estimate

**Points**: 2
