//! Reduced incidence matrix (A) construction.
//!
//! The incidence matrix encodes the network topology by indicating how
//! branches connect to buses:
//! - `A[k][i] = +1` if branch k leaves bus i (from_bus)
//! - `A[k][i] = -1` if branch k enters bus i (to_bus)
//! - `A[k][i] = 0` otherwise
//!
//! The "reduced" matrix excludes the slack (angle reference) bus.

use std::collections::HashMap;

use labdessem_core::{ids::BusId, system::System};

/// Build the reduced incidence matrix excluding the slack bus.
///
/// Returns a matrix of dimension `n_branches × n_non_slack_buses`.
/// Element `A[l][n] = +1` if branch l leaves bus n, `-1` if it enters, `0` otherwise.
pub fn build_reduced_incidence(
    system: &System,
    non_slack_bus_positions: &HashMap<BusId, usize>,
) -> Vec<Vec<f64>> {
    let row_count = system.branches.len();
    let column_count = non_slack_bus_positions.len();
    let mut matrix = vec![vec![0.0; column_count]; row_count];

    for (branch_idx, branch) in system.branches.iter().enumerate() {
        if let Some(from_pos) = non_slack_bus_positions.get(&branch.from_bus_id) {
            matrix[branch_idx][*from_pos] = 1.0;
        }
        if let Some(to_pos) = non_slack_bus_positions.get(&branch.to_bus_id) {
            matrix[branch_idx][*to_pos] = -1.0;
        }
    }

    matrix
}

#[cfg(test)]
mod tests {
    use super::*;
    use labdessem_core::{
        entities::Branch,
        ids::BranchId,
        system::{StudyHorizon, System},
    };

    fn make_branch(id: usize, from: usize, to: usize) -> Branch {
        Branch {
            id: BranchId(id),
            name: format!("branch_{id}"),
            from_bus_id: BusId(from),
            to_bus_id: BusId(to),
            reactance_pu: 0.1,
            thermal_limit_mw: 100.0,
        }
    }

    fn minimal_system(branches: Vec<Branch>) -> System {
        System {
            horizon: StudyHorizon {
                periods: 1,
                period_duration_hours: 1.0,
                original_periods: 1,
                original_period_durations_hours: vec![1.0],
                internal_to_original_period: vec![1],
                internal_subperiod_index: vec![1],
            },
            branches,
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
            buses: vec![],
            thermal_plants: vec![],
            hydro_plants: vec![],
            renewable_plants: vec![],
            pumping_plants: vec![],
        }
    }

    #[test]
    fn test_incidence_empty_system() {
        let positions: HashMap<BusId, usize> = HashMap::new();
        let system = minimal_system(vec![]);
        let result = build_reduced_incidence(&system, &positions);
        assert!(result.is_empty());
    }

    #[test]
    fn test_incidence_dimensions() {
        let mut positions: HashMap<BusId, usize> = HashMap::new();
        positions.insert(BusId(1), 0);
        positions.insert(BusId(2), 1);

        let system = minimal_system(vec![make_branch(1, 1, 2)]);

        let result = build_reduced_incidence(&system, &positions);
        assert_eq!(result.len(), 1); // 1 branch
        assert_eq!(result[0].len(), 2); // 2 non-slack buses
    }

    #[test]
    fn test_incidence_signs() {
        let mut positions: HashMap<BusId, usize> = HashMap::new();
        positions.insert(BusId(1), 0);
        positions.insert(BusId(2), 1);

        let system = minimal_system(vec![make_branch(1, 1, 2)]);

        let result = build_reduced_incidence(&system, &positions);
        assert_eq!(result[0][0], 1.0); // +1 for from_bus
        assert_eq!(result[0][1], -1.0); // -1 for to_bus
    }

    #[test]
    fn test_incidence_slack_excluded() {
        // Bus 0 is slack (not in positions), buses 1,2 are non-slack
        let mut positions: HashMap<BusId, usize> = HashMap::new();
        positions.insert(BusId(1), 0);
        positions.insert(BusId(2), 1);

        let system = minimal_system(vec![make_branch(1, 0, 1)]); // Branch from slack (0) to bus 1

        let result = build_reduced_incidence(&system, &positions);
        // from_bus (0) is slack, so no +1 entry
        // to_bus (1) has position 0, so A[0][0] = -1
        assert_eq!(result[0][0], -1.0);
        assert_eq!(result[0][1], 0.0);
    }
}
