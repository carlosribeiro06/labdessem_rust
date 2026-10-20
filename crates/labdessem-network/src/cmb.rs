//! CMB (Custo Marginal de Barra) computation.
//!
//! Implements eq. 2.43: CMB(i,t) = λ_DS(Sb_i,t) + Σ[k(B_i,l) × λ_L(l,t)]

use std::collections::HashMap;

use labdessem_core::ids::{BusId, SubmarketId};
use labdessem_core::system::System;

use crate::model::DcNetworkModel;

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

/// Compute CMO per submarket as demand-weighted average of CMB (eq. 2.44).
///
/// CMO(s,t) = Σ[CMB(k,t) × d(k,t)] / Σ[d(k,t)] for k ∈ Sb_s
pub fn compute_cmo_from_cmb(
    cmb_records: &[CmbRecord],
    system: &System,
) -> HashMap<(SubmarketId, usize), f64> {
    let bus_map: HashMap<BusId, &labdessem_core::entities::Bus> =
        system.buses.iter().map(|b| (b.id, b)).collect();

    let mut grouped: HashMap<(SubmarketId, usize), Vec<&CmbRecord>> = HashMap::new();
    for record in cmb_records {
        grouped
            .entry((record.submarket_id, record.period))
            .or_default()
            .push(record);
    }

    let mut cmo_map = HashMap::with_capacity(grouped.len());

    for ((submarket_id, period), records) in grouped {
        let mut weighted_sum = 0.0;
        let mut demand_sum = 0.0;

        for record in &records {
            let demand = bus_map
                .get(&record.bus_id)
                .and_then(|bus| bus.demand_mw.get(period).copied())
                .unwrap_or(0.0);

            weighted_sum += record.cmb * demand;
            demand_sum += demand;
        }

        let cmo = if demand_sum.abs() < 1e-9 {
            let simple_avg: f64 = records.iter().map(|r| r.cmb).sum::<f64>() / records.len() as f64;
            simple_avg
        } else {
            weighted_sum / demand_sum
        };

        cmo_map.insert((submarket_id, period), cmo);
    }

    cmo_map
}
