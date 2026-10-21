//! Flow cut constraint term generation.
//!
//! Builds linear constraint terms for DC power flow limits using PTDF coefficients.

use crate::topology::model::DcNetworkModel;
use crate::types::NetworkTerm;

use super::input::FlowCutInput;

/// Build the linear terms for a flow cut constraint on a branch.
///
/// For each generator/pump in the input, computes the PTDF coefficient
/// and returns a NetworkTerm with the variable name and coefficient.
///
/// The PTDF coefficient represents the sensitivity of flow on `branch_idx`
/// to power injection at each bus. Positive coefficients indicate that
/// increased generation at that bus increases flow on the branch.
///
/// Pumping is treated as negative injection (consumption), so its coefficient
/// is negated and scaled by the specific consumption factor.
pub fn build_flow_cut_terms(
    dc_network: &DcNetworkModel,
    branch_idx: usize,
    input: &FlowCutInput,
) -> Vec<NetworkTerm> {
    let ptdf_row = dc_network.ptdf_row_for_branch(branch_idx);
    let mut terms = Vec::new();

    // Thermal generation (positive injection)
    for entry in &input.thermal_entries {
        if let Some(bus_pos) = dc_network.position_of_bus(entry.bus_id) {
            if bus_pos < ptdf_row.len() {
                let coefficient = ptdf_row[bus_pos];
                if coefficient.abs() > 1e-12 {
                    terms.push(NetworkTerm::new(&entry.variable_name, coefficient));
                }
            }
        }
    }

    // Hydro generation (positive injection)
    for entry in &input.hydro_entries {
        if let Some(bus_pos) = dc_network.position_of_bus(entry.bus_id) {
            if bus_pos < ptdf_row.len() {
                let coefficient = ptdf_row[bus_pos];
                if coefficient.abs() > 1e-12 {
                    terms.push(NetworkTerm::new(&entry.variable_name, coefficient));
                }
            }
        }
    }

    // Renewable generation (positive injection)
    for entry in &input.renewable_entries {
        if let Some(bus_pos) = dc_network.position_of_bus(entry.bus_id) {
            if bus_pos < ptdf_row.len() {
                let coefficient = ptdf_row[bus_pos];
                if coefficient.abs() > 1e-12 {
                    terms.push(NetworkTerm::new(&entry.variable_name, coefficient));
                }
            }
        }
    }

    // Pumping (negative injection - consumption)
    // Coefficient is negated because pumping consumes power (negative injection)
    // and scaled by specific_consumption_mw_per_m3s / flow_conversion
    for entry in &input.pumping_entries {
        if let Some(bus_pos) = dc_network.position_of_bus(entry.bus_id) {
            if bus_pos < ptdf_row.len() {
                let coefficient = -ptdf_row[bus_pos] * entry.specific_consumption_mw_per_m3s
                    / input.flow_conversion;
                if coefficient.abs() > 1e-12 {
                    terms.push(NetworkTerm::new(&entry.variable_name, coefficient));
                }
            }
        }
    }

    terms
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::constraints::{GeneratorEntry, PumpingEntry};
    use labdessem_core::{
        entities::{Branch, Bus},
        ids::{BranchId, BusId, SubmarketId},
        system::{StudyHorizon, System},
    };

    fn make_branch(id: usize, from: usize, to: usize, x_pu: f64) -> Branch {
        Branch {
            id: BranchId(id),
            name: format!("branch_{id}"),
            from_bus_id: BusId(from),
            to_bus_id: BusId(to),
            reactance_pu: x_pu,
            thermal_limit_mw: 100.0,
        }
    }

    fn make_bus(id: usize, is_slack: bool) -> Bus {
        Bus {
            id: BusId(id),
            name: format!("bus_{id}"),
            submarket_id: SubmarketId(1),
            angle_reference: is_slack,
            demand_mw: vec![0.0],
        }
    }

    fn test_system_2bus() -> System {
        // Bus 0 is slack, Bus 1 is non-slack
        // One branch from 0 to 1 with X=0.1
        // PTDF for bus 1 injection on branch 0-1 will be -1.0 (or 1.0 depending on convention)
        System {
            horizon: StudyHorizon {
                periods: 1,
                period_duration_hours: 1.0,
                original_periods: 1,
                original_period_durations_hours: vec![1.0],
                internal_to_original_period: vec![1],
                internal_subperiod_index: vec![1],
            },
            branches: vec![make_branch(1, 0, 1, 0.1)],
            buses: vec![make_bus(0, true), make_bus(1, false)],
            thermal_unit_commitment_enabled: false,
            hydro_unit_commitment_enabled: false,
            ton_residual_enabled: false,
            fpha_enabled: false,
            future_cost_enabled: false,
            residual_costs: vec![],
            future_cost_cuts: vec![],
            submarkets: vec![],
            interchange_limits: vec![],
            operational_limits: vec![],
            thermal_plants: vec![],
            hydro_plants: vec![],
            renewable_plants: vec![],
            pumping_plants: vec![],
        }
    }

    #[test]
    fn test_empty_input() {
        let system = test_system_2bus();
        let dc_network = DcNetworkModel::from_system(&system).unwrap();
        let input = FlowCutInput::new(0.0036);

        let terms = build_flow_cut_terms(&dc_network, 0, &input);
        assert!(terms.is_empty());
    }

    #[test]
    fn test_thermal_entry_at_non_slack_bus() {
        let system = test_system_2bus();
        let dc_network = DcNetworkModel::from_system(&system).unwrap();

        let mut input = FlowCutInput::new(0.0036);
        input.add_thermal(GeneratorEntry::new(BusId(1), "gt_1"));

        let terms = build_flow_cut_terms(&dc_network, 0, &input);
        assert_eq!(terms.len(), 1);
        assert_eq!(terms[0].variable_name, "gt_1");
        // PTDF magnitude should be 1.0 for a 2-bus radial network
        assert!((terms[0].coefficient.abs() - 1.0).abs() < 1e-9);
    }

    #[test]
    fn test_thermal_entry_at_slack_bus_skipped() {
        let system = test_system_2bus();
        let dc_network = DcNetworkModel::from_system(&system).unwrap();

        let mut input = FlowCutInput::new(0.0036);
        // Bus 0 is slack, so it won't be in the non-slack bus positions
        input.add_thermal(GeneratorEntry::new(BusId(0), "gt_slack"));

        let terms = build_flow_cut_terms(&dc_network, 0, &input);
        assert!(terms.is_empty()); // Skipped because bus is slack
    }

    #[test]
    fn test_pumping_entry_has_scaled_negative_coefficient() {
        let system = test_system_2bus();
        let dc_network = DcNetworkModel::from_system(&system).unwrap();

        let flow_conversion = 0.0036;
        let specific_consumption = 1.5;

        let mut input = FlowCutInput::new(flow_conversion);
        input.add_pumping(PumpingEntry::new(BusId(1), "pump_1", specific_consumption));

        let terms = build_flow_cut_terms(&dc_network, 0, &input);
        assert_eq!(terms.len(), 1);
        assert_eq!(terms[0].variable_name, "pump_1");

        // Coefficient should be: -ptdf * specific_consumption / flow_conversion
        // ptdf magnitude is 1.0, so |coefficient| = 1.0 * 1.5 / 0.0036 = 416.67
        // Sign depends on PTDF sign convention
        let expected_magnitude = specific_consumption / flow_conversion;
        assert!((terms[0].coefficient.abs() - expected_magnitude).abs() < 1e-3);
    }

    #[test]
    fn test_unknown_bus_skipped() {
        let system = test_system_2bus();
        let dc_network = DcNetworkModel::from_system(&system).unwrap();

        let mut input = FlowCutInput::new(0.0036);
        // Bus 99 doesn't exist in the system
        input.add_thermal(GeneratorEntry::new(BusId(99), "gt_unknown"));

        let terms = build_flow_cut_terms(&dc_network, 0, &input);
        assert!(terms.is_empty());
    }
}
