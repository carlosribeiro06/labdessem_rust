# [004] Extract network and demand constraints

## Context

Network constraints include demand balance, DC flow, and operational limits.

## Specification

1. Create `constraints/network.rs` with DC flow constraints
2. Create `constraints/demand.rs` with `add_demand_balance_constraints()`
3. Create `constraints/limits.rs` with operational limit constraints
4. Delete original `constraints.rs`
5. Update `constraints/mod.rs` with `add_all_constraints()` that calls each module

## Acceptance Criteria

- [ ] All three files created
- [ ] Original `constraints.rs` deleted
- [ ] `cargo test` passes
- [ ] Example runs correctly

## Implementation Guide

```rust
// constraints/mod.rs
pub mod demand;
pub mod hydro;
pub mod limits;
pub mod network;
pub mod thermal;

use crate::Model;
use labdessem_core::System;

pub fn add_all_constraints(model: &mut Model, system: &System) {
    thermal::add_thermal_constraints(model, system);
    hydro::add_hydro_constraints(model, system);
    demand::add_demand_balance_constraints(model, system);
    network::add_network_constraints(model, system);
    limits::add_operational_limit_constraints(model, system);
}
```

## Dependencies

- **Blocked By**: 001, 002, 003
- **Blocks**: Epic 07

## Effort Estimate

**Points**: 2
