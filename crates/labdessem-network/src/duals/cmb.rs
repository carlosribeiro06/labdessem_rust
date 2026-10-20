//! CMB (Custo Marginal de Barra) computation.
//!
//! Implements eq. 2.43: CMB(i,t) = λ_DS(Sb_i,t) + Σ[k(B_i,l) × λ_L(l,t)]

use std::collections::HashMap;

use labdessem_core::ids::{BusId, SubmarketId};
use labdessem_core::system::System;

use crate::topology::model::DcNetworkModel;

/// CMB record for a single bus and period.
#[derive(Debug, Clone, PartialEq)]
pub struct CmbRecord {
    pub bus_id: BusId,
    pub period: usize,
    pub cmb: f64,
    pub submarket_id: SubmarketId,
}

/// Compute CMB per bus for given periods.
///
/// # Arguments
///
/// * `system` - System with bus and branch data
/// * `dc_network` - DC network model with PTDF matrix
/// * `demand_duals` - λ_DS per (submarket_id, period)
/// * `flow_duals` - λ_L per (branch_idx, period)
/// * `periods` - List of periods to compute (0-based internal)
#[allow(clippy::expect_used)]
pub fn compute_cmb_per_bus(
    system: &System,
    dc_network: &DcNetworkModel,
    demand_duals: &HashMap<(SubmarketId, usize), f64>,
    flow_duals: &HashMap<(usize, usize), f64>,
    periods: &[usize],
) -> Vec<CmbRecord> {
    let mut records = Vec::with_capacity(dc_network.non_slack_bus_ids().len() * periods.len());

    for &period in periods {
        for (bus_pos, &bus_id) in dc_network.non_slack_bus_ids().iter().enumerate() {
            let bus = system
                .buses
                .iter()
                .find(|b| b.id == bus_id)
                .expect("bus_id from dc_network must exist in system");

            let lambda_ds = demand_duals
                .get(&(bus.submarket_id, period))
                .copied()
                .unwrap_or(0.0);

            let mut network_component = 0.0;
            for (&(branch_idx, flow_period), &lambda_l) in flow_duals {
                if flow_period != period {
                    continue;
                }
                let ptdf_row = dc_network.ptdf_row_for_branch(branch_idx);
                if bus_pos < ptdf_row.len() {
                    network_component += ptdf_row[bus_pos] * lambda_l;
                }
            }

            let cmb = lambda_ds + network_component;

            records.push(CmbRecord {
                bus_id,
                period,
                cmb,
                submarket_id: bus.submarket_id,
            });
        }
    }

    records
}
