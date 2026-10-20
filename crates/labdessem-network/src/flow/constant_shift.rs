//! Constant load shift computation for PTDF constraints.

use labdessem_core::system::System;

use crate::topology::model::DcNetworkModel;

/// Compute the constant load shift for a branch and period.
#[allow(clippy::expect_used)]
pub fn constant_load_shift(
    system: &System,
    dc_network: &DcNetworkModel,
    branch_idx: usize,
    period: usize,
) -> f64 {
    dc_network
        .non_slack_bus_ids
        .iter()
        .enumerate()
        .map(|(bus_pos, bus_id)| {
            let bus = system
                .buses
                .iter()
                .find(|candidate| candidate.id == *bus_id)
                .expect("bus should exist for non-slack PTDF position");
            dc_network.ptdf_rows[branch_idx][bus_pos] * bus.demand_mw[period]
        })
        .sum()
}
