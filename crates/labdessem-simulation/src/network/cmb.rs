//! CMB (Custo Marginal de Barra) computation.
//!
//! Implements eq. 2.43: CMB(i,t) = λ_DS(Sb_i,t) + Σ[k(B_i,l) × λ_L(l,t)]

use std::collections::HashMap;

use labdessem_core::ids::{BusId, SubmarketId};
use labdessem_core::system::System;

use super::DcNetworkModel;

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
///
/// # Returns
///
/// Vec of CmbRecord, one per (bus, period) combination for non-slack buses.
pub fn compute_cmb_per_bus(
    system: &System,
    dc_network: &DcNetworkModel,
    demand_duals: &HashMap<(SubmarketId, usize), f64>,
    flow_duals: &HashMap<(usize, usize), f64>,
    periods: &[usize],
) -> Vec<CmbRecord> {
    let mut records = Vec::with_capacity(dc_network.non_slack_bus_ids.len() * periods.len());

    for &period in periods {
        for (bus_pos, &bus_id) in dc_network.non_slack_bus_ids.iter().enumerate() {
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
                if branch_idx < dc_network.ptdf_rows.len() {
                    let ptdf_row = &dc_network.ptdf_rows[branch_idx];
                    if bus_pos < ptdf_row.len() {
                        network_component += ptdf_row[bus_pos] * lambda_l;
                    }
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
///
/// # Arguments
///
/// * `cmb_records` - CMB values from `compute_cmb_per_bus()`
/// * `system` - System with bus data including demand per period
///
/// # Returns
///
/// HashMap from (SubmarketId, period) to CMO value in R$/MWh.
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

#[cfg(test)]
mod tests {
    use super::*;
    use labdessem_core::entities::{Branch, Bus, Submarket};
    use labdessem_core::ids::BranchId;
    use labdessem_core::system::{StudyHorizon, System};

    fn make_horizon(periods: usize) -> StudyHorizon {
        StudyHorizon {
            periods,
            period_duration_hours: 1.0,
            original_periods: periods,
            original_period_durations_hours: vec![1.0; periods],
            internal_to_original_period: (1..=periods).collect(),
            internal_subperiod_index: vec![1; periods],
        }
    }

    fn make_test_system_2_buses() -> System {
        let periods = 2;
        let horizon = make_horizon(periods);

        System {
            horizon,
            thermal_unit_commitment_enabled: false,
            hydro_unit_commitment_enabled: false,
            ton_residual_enabled: false,
            fpha_enabled: false,
            future_cost_enabled: false,
            residual_costs: vec![],
            future_cost_cuts: vec![],
            submarkets: vec![Submarket {
                id: SubmarketId(1),
                name: "SBM-1".to_string(),
                demand_mw: vec![100.0; periods],
            }],
            interchange_limits: vec![],
            operational_limits: vec![],
            buses: vec![
                Bus {
                    id: BusId(1),
                    name: "SLACK".to_string(),
                    submarket_id: SubmarketId(1),
                    angle_reference: true,
                    demand_mw: vec![0.0; periods],
                },
                Bus {
                    id: BusId(2),
                    name: "BUS-2".to_string(),
                    submarket_id: SubmarketId(1),
                    angle_reference: false,
                    demand_mw: vec![100.0; periods],
                },
                Bus {
                    id: BusId(3),
                    name: "BUS-3".to_string(),
                    submarket_id: SubmarketId(1),
                    angle_reference: false,
                    demand_mw: vec![200.0; periods],
                },
            ],
            branches: vec![],
            thermal_plants: vec![],
            hydro_plants: vec![],
            pumping_plants: vec![],
            renewable_plants: vec![],
        }
    }

    fn make_test_system_with_branch() -> System {
        let periods = 2;
        let horizon = make_horizon(periods);

        System {
            horizon,
            thermal_unit_commitment_enabled: false,
            hydro_unit_commitment_enabled: false,
            ton_residual_enabled: false,
            fpha_enabled: false,
            future_cost_enabled: false,
            residual_costs: vec![],
            future_cost_cuts: vec![],
            submarkets: vec![Submarket {
                id: SubmarketId(1),
                name: "SBM-1".to_string(),
                demand_mw: vec![100.0; periods],
            }],
            interchange_limits: vec![],
            operational_limits: vec![],
            buses: vec![
                Bus {
                    id: BusId(1),
                    name: "SLACK".to_string(),
                    submarket_id: SubmarketId(1),
                    angle_reference: true,
                    demand_mw: vec![0.0; periods],
                },
                Bus {
                    id: BusId(2),
                    name: "BUS-2".to_string(),
                    submarket_id: SubmarketId(1),
                    angle_reference: false,
                    demand_mw: vec![100.0; periods],
                },
            ],
            branches: vec![Branch {
                id: BranchId(1),
                name: "L1".to_string(),
                from_bus_id: BusId(1),
                to_bus_id: BusId(2),
                reactance_pu: 0.1,
                thermal_limit_mw: 100.0,
            }],
            thermal_plants: vec![],
            hydro_plants: vec![],
            pumping_plants: vec![],
            renewable_plants: vec![],
        }
    }

    #[test]
    fn test_cmb_no_congestion() {
        let system = make_test_system_2_buses();
        let dc_network = DcNetworkModel::from_system(&system).unwrap();

        let mut demand_duals = HashMap::new();
        demand_duals.insert((SubmarketId(1), 0), 100.0);
        demand_duals.insert((SubmarketId(1), 1), 100.0);
        let flow_duals = HashMap::new();

        let records = compute_cmb_per_bus(&system, &dc_network, &demand_duals, &flow_duals, &[0]);

        assert_eq!(records.len(), 2);
        for r in &records {
            assert!(
                (r.cmb - 100.0).abs() < 1e-9,
                "CMB should equal λ_DS when no congestion"
            );
        }
    }

    #[test]
    fn test_cmb_empty_flow_duals_returns_demand_dual() {
        let system = make_test_system_2_buses();
        let dc_network = DcNetworkModel::from_system(&system).unwrap();

        let mut demand_duals = HashMap::new();
        demand_duals.insert((SubmarketId(1), 0), 50.0);
        demand_duals.insert((SubmarketId(1), 1), 50.0);
        let flow_duals = HashMap::new();

        let records = compute_cmb_per_bus(&system, &dc_network, &demand_duals, &flow_duals, &[0]);

        assert!(!records.is_empty());
        for r in &records {
            assert!((r.cmb - 50.0).abs() < 1e-9);
        }
    }

    #[test]
    fn test_cmb_multiple_periods() {
        let system = make_test_system_2_buses();
        let dc_network = DcNetworkModel::from_system(&system).unwrap();

        let mut demand_duals = HashMap::new();
        demand_duals.insert((SubmarketId(1), 0), 100.0);
        demand_duals.insert((SubmarketId(1), 1), 120.0);
        let flow_duals = HashMap::new();

        let records =
            compute_cmb_per_bus(&system, &dc_network, &demand_duals, &flow_duals, &[0, 1]);

        assert_eq!(records.len(), 4);
    }

    #[test]
    fn test_cmb_with_congestion() {
        let system = make_test_system_with_branch();
        let dc_network = DcNetworkModel::from_system(&system).unwrap();

        let mut demand_duals = HashMap::new();
        demand_duals.insert((SubmarketId(1), 0), 100.0);
        demand_duals.insert((SubmarketId(1), 1), 100.0);

        let mut flow_duals = HashMap::new();
        flow_duals.insert((0, 0), 20.0);

        let records = compute_cmb_per_bus(&system, &dc_network, &demand_duals, &flow_duals, &[0]);

        assert_eq!(records.len(), 1);
        let r = &records[0];
        let ptdf_value = dc_network.ptdf_rows[0][0];
        let expected_cmb = 100.0 + ptdf_value * 20.0;
        assert!(
            (r.cmb - expected_cmb).abs() < 1e-9,
            "CMB = λ_DS + PTDF * λ_L = {expected_cmb}, got {}",
            r.cmb
        );
    }

    // --- CMO tests ---

    fn make_system_for_cmo(bus_demands: Vec<(usize, f64)>) -> System {
        let periods = 1;
        let horizon = make_horizon(periods);

        let mut buses = vec![Bus {
            id: BusId(99),
            name: "SLACK".to_string(),
            submarket_id: SubmarketId(1),
            angle_reference: true,
            demand_mw: vec![0.0],
        }];

        for (bus_id, demand) in bus_demands {
            buses.push(Bus {
                id: BusId(bus_id),
                name: format!("BUS-{bus_id}"),
                submarket_id: SubmarketId(1),
                angle_reference: false,
                demand_mw: vec![demand],
            });
        }

        System {
            horizon,
            thermal_unit_commitment_enabled: false,
            hydro_unit_commitment_enabled: false,
            ton_residual_enabled: false,
            fpha_enabled: false,
            future_cost_enabled: false,
            residual_costs: vec![],
            future_cost_cuts: vec![],
            submarkets: vec![Submarket {
                id: SubmarketId(1),
                name: "SBM-1".to_string(),
                demand_mw: vec![400.0],
            }],
            interchange_limits: vec![],
            operational_limits: vec![],
            buses,
            branches: vec![],
            thermal_plants: vec![],
            hydro_plants: vec![],
            pumping_plants: vec![],
            renewable_plants: vec![],
        }
    }

    #[test]
    fn test_cmo_weighted_average() {
        // B1: demand=100, CMB=100
        // B2: demand=300, CMB=120
        // Expected: (100*100 + 120*300) / 400 = 115
        let system = make_system_for_cmo(vec![(1, 100.0), (2, 300.0)]);

        let cmb_records = vec![
            CmbRecord {
                bus_id: BusId(1),
                period: 0,
                cmb: 100.0,
                submarket_id: SubmarketId(1),
            },
            CmbRecord {
                bus_id: BusId(2),
                period: 0,
                cmb: 120.0,
                submarket_id: SubmarketId(1),
            },
        ];

        let cmo = compute_cmo_from_cmb(&cmb_records, &system);

        assert_eq!(cmo.len(), 1);
        assert!(
            (cmo[&(SubmarketId(1), 0)] - 115.0).abs() < 1e-9,
            "CMO = (100*100 + 120*300)/400 = 115, got {}",
            cmo[&(SubmarketId(1), 0)]
        );
    }

    #[test]
    fn test_cmo_zero_demand_fallback() {
        // All zero demand → simple average
        let system = make_system_for_cmo(vec![(1, 0.0), (2, 0.0)]);

        let cmb_records = vec![
            CmbRecord {
                bus_id: BusId(1),
                period: 0,
                cmb: 100.0,
                submarket_id: SubmarketId(1),
            },
            CmbRecord {
                bus_id: BusId(2),
                period: 0,
                cmb: 120.0,
                submarket_id: SubmarketId(1),
            },
        ];

        let cmo = compute_cmo_from_cmb(&cmb_records, &system);

        // Simple average: (100 + 120) / 2 = 110
        assert!(
            (cmo[&(SubmarketId(1), 0)] - 110.0).abs() < 1e-9,
            "CMO should be simple avg 110 when demand=0, got {}",
            cmo[&(SubmarketId(1), 0)]
        );
    }

    #[test]
    fn test_cmo_multiple_submarkets() {
        let periods = 1;
        let horizon = make_horizon(periods);

        let system = System {
            horizon,
            thermal_unit_commitment_enabled: false,
            hydro_unit_commitment_enabled: false,
            ton_residual_enabled: false,
            fpha_enabled: false,
            future_cost_enabled: false,
            residual_costs: vec![],
            future_cost_cuts: vec![],
            submarkets: vec![
                Submarket {
                    id: SubmarketId(1),
                    name: "SBM-1".to_string(),
                    demand_mw: vec![200.0],
                },
                Submarket {
                    id: SubmarketId(2),
                    name: "SBM-2".to_string(),
                    demand_mw: vec![300.0],
                },
            ],
            interchange_limits: vec![],
            operational_limits: vec![],
            buses: vec![
                Bus {
                    id: BusId(99),
                    name: "SLACK".to_string(),
                    submarket_id: SubmarketId(1),
                    angle_reference: true,
                    demand_mw: vec![0.0],
                },
                Bus {
                    id: BusId(1),
                    name: "BUS-1".to_string(),
                    submarket_id: SubmarketId(1),
                    angle_reference: false,
                    demand_mw: vec![100.0],
                },
                Bus {
                    id: BusId(2),
                    name: "BUS-2".to_string(),
                    submarket_id: SubmarketId(1),
                    angle_reference: false,
                    demand_mw: vec![100.0],
                },
                Bus {
                    id: BusId(3),
                    name: "BUS-3".to_string(),
                    submarket_id: SubmarketId(2),
                    angle_reference: false,
                    demand_mw: vec![300.0],
                },
            ],
            branches: vec![],
            thermal_plants: vec![],
            hydro_plants: vec![],
            pumping_plants: vec![],
            renewable_plants: vec![],
        };

        let cmb_records = vec![
            CmbRecord {
                bus_id: BusId(1),
                period: 0,
                cmb: 100.0,
                submarket_id: SubmarketId(1),
            },
            CmbRecord {
                bus_id: BusId(2),
                period: 0,
                cmb: 120.0,
                submarket_id: SubmarketId(1),
            },
            CmbRecord {
                bus_id: BusId(3),
                period: 0,
                cmb: 80.0,
                submarket_id: SubmarketId(2),
            },
        ];

        let cmo = compute_cmo_from_cmb(&cmb_records, &system);

        assert_eq!(cmo.len(), 2);
        // SBM-1: (100*100 + 120*100)/200 = 110
        assert!(
            (cmo[&(SubmarketId(1), 0)] - 110.0).abs() < 1e-9,
            "SBM-1 CMO = 110, got {}",
            cmo[&(SubmarketId(1), 0)]
        );
        // SBM-2: 80 (single bus)
        assert!(
            (cmo[&(SubmarketId(2), 0)] - 80.0).abs() < 1e-9,
            "SBM-2 CMO = 80, got {}",
            cmo[&(SubmarketId(2), 0)]
        );
    }
}

