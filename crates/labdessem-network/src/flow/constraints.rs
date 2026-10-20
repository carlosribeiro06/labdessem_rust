//! Flow constraint key generation.

use std::collections::BTreeSet;

use labdessem_core::system::System;

use crate::topology::model::FlowCutKey;

/// Generate all possible flow cut keys for a system.
pub fn all_flow_constraints(system: &System) -> BTreeSet<FlowCutKey> {
    (0..system.branches.len())
        .flat_map(|branch_idx| {
            (0..system.horizon.periods).map(move |period| FlowCutKey { branch_idx, period })
        })
        .collect()
}
