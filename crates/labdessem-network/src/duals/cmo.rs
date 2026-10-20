//! CMO (Custo Marginal de Operação) computation.
//!
//! Implements eq. 2.44: CMO(s,t) = Σ[CMB(k,t) × d(k,t)] / Σ[d(k,t)] for k ∈ Sb_s

use std::collections::HashMap;

use labdessem_core::ids::{BusId, SubmarketId};
use labdessem_core::system::System;

use super::cmb::CmbRecord;

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
