# [001] Extract workflow module

## Context

Extract high-level execution mode logic into dedicated module.

## Specification

1. Create `workflow.rs` with:
   - `ExecutionOption` enum (Iterative, SingleMilp)
   - `run_simulation()` main entry point
2. Move execution mode parsing logic

## Acceptance Criteria

- [ ] `workflow.rs` exists
- [ ] `ExecutionOption` with `from_config_value()` and `description()`
- [ ] `cargo test` passes

## Implementation Guide

```rust
// workflow.rs
pub enum ExecutionOption {
    Iterative,    // LP → MILP → LP-FIXED → LP-CALC-CMO
    SingleMilp,   // Single MILP solve
}

impl ExecutionOption {
    pub fn from_config_value(value: u8) -> Result<Self, String> { ... }
    pub fn description(&self) -> &'static str { ... }
}

pub fn run_simulation(
    system: &System,
    network_enabled: bool,
    execution_option: ExecutionOption,
) -> Result<SimulationResult, Box<dyn std::error::Error>> { ... }
```

## Dependencies

- **Blocked By**: Epic 04
- **Blocks**: 002, 003, 004

## Effort Estimate

**Points**: 2
