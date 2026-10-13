# Epic 05: Simulation Split

## Overview

Split the remaining monolithic `lib.rs` in `labdessem-simulation` into focused
modules: workflow orchestration, iteration logic, network cuts, and results.

## Goals

- Create `workflow.rs` for high-level execution modes
- Create `iteration.rs` for LP/MILP iteration logic
- Create `network_cuts.rs` for network violation handling
- Create `results.rs` for result structs
- Reduce `lib.rs` to re-exports only

## Architecture

### After

```
labdessem-simulation/src/
├── lib.rs              # Re-exports only (~50 lines)
├── workflow.rs         # ExecutionOption, run_simulation()
├── iteration.rs        # run_iterative_workflow(), run_single_milp()
├── network_cuts.rs     # check_flow_violations(), add_violated_cuts()
├── dual_extraction.rs  # extract_duals()
└── results.rs          # SimulationResult, IterationStep, etc.
```

## Tickets

| ID | Title | Est. | Dependencies |
|----|-------|------|--------------|
| 001 | Extract workflow module | 30min | Epic 04 |
| 002 | Extract iteration module | 45min | 001 |
| 003 | Extract network cuts module | 30min | 001 |
| 004 | Extract results module | 30min | 001 |

## Success Criteria

- `lib.rs` under 100 lines
- Each new module under 500 lines
- `cargo test` passes

## Estimated Effort

**Total**: 4-5 hours
