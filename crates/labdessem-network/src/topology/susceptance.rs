//! Nodal susceptance matrix (B) construction.
//!
//! The bus susceptance matrix describes the electrical coupling between buses:
//! - `B[i][i] = Σ(1/X_ij)` for all branches connected to bus i (self-susceptance)
//! - `B[i][j] = -1/X_ij` for branches between buses i and j (mutual susceptance)
//!
//! The "reduced" matrix excludes the slack bus row and column.

use std::collections::HashMap;

use labdessem_core::{ids::BusId, system::System};

/// Build the reduced susceptance matrix excluding the slack bus.
///
/// Returns a symmetric n×n matrix where n is the number of non-slack buses.
pub fn build_reduced_susceptance(
    system: &System,
    non_slack_bus_positions: &HashMap<BusId, usize>,
) -> Vec<Vec<f64>> {
    let size = non_slack_bus_positions.len();
    let mut matrix = vec![vec![0.0; size]; size];

    for branch in &system.branches {
        let susceptance = 1.0 / branch.reactance_pu;
        let from_pos = non_slack_bus_positions.get(&branch.from_bus_id).copied();
        let to_pos = non_slack_bus_positions.get(&branch.to_bus_id).copied();

        match (from_pos, to_pos) {
            (Some(i), Some(j)) => {
                matrix[i][i] += susceptance;
                matrix[j][j] += susceptance;
                matrix[i][j] -= susceptance;
                matrix[j][i] -= susceptance;
            }
            (Some(i), None) => {
                matrix[i][i] += susceptance;
            }
            (None, Some(j)) => {
                matrix[j][j] += susceptance;
            }
            (None, None) => {}
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
            inflow_history: vec![],
        }
    }

    #[test]
    fn test_susceptance_empty() {
        let positions: HashMap<BusId, usize> = HashMap::new();
        let system = minimal_system(vec![]);
        let result = build_reduced_susceptance(&system, &positions);
        assert!(result.is_empty());
    }

    #[test]
    fn test_susceptance_single_branch_one_non_slack() {
        // Bus 0 is slack, bus 1 is non-slack; branch from 0 to 1 with X=0.1
        let mut positions: HashMap<BusId, usize> = HashMap::new();
        positions.insert(BusId(1), 0);

        let system = minimal_system(vec![make_branch(1, 0, 1, 0.1)]);

        let result = build_reduced_susceptance(&system, &positions);
        assert_eq!(result.len(), 1);
        assert_eq!(result[0].len(), 1);
        // B[0][0] = 1/0.1 = 10.0
        assert!((result[0][0] - 10.0).abs() < 1e-9);
    }

    #[test]
    fn test_susceptance_symmetry() {
        // Two non-slack buses connected by one branch
        let mut positions: HashMap<BusId, usize> = HashMap::new();
        positions.insert(BusId(1), 0);
        positions.insert(BusId(2), 1);

        let system = minimal_system(vec![make_branch(1, 1, 2, 0.1)]);

        let result = build_reduced_susceptance(&system, &positions);
        // Matrix should be symmetric
        assert_eq!(result[0][1], result[1][0]);
        // Diagonal should be positive (self-susceptance)
        assert!(result[0][0] > 0.0);
        assert!(result[1][1] > 0.0);
        // Off-diagonal should be negative (mutual susceptance)
        assert!(result[0][1] < 0.0);
    }

    #[test]
    fn test_susceptance_values() {
        let mut positions: HashMap<BusId, usize> = HashMap::new();
        positions.insert(BusId(1), 0);
        positions.insert(BusId(2), 1);

        // Branch with X=0.1 -> susceptance = 10.0
        let system = minimal_system(vec![make_branch(1, 1, 2, 0.1)]);

        let result = build_reduced_susceptance(&system, &positions);
        // B[0][0] = B[1][1] = 10.0 (one branch connected to each)
        // B[0][1] = B[1][0] = -10.0
        assert!((result[0][0] - 10.0).abs() < 1e-9);
        assert!((result[1][1] - 10.0).abs() < 1e-9);
        assert!((result[0][1] - (-10.0)).abs() < 1e-9);
        assert!((result[1][0] - (-10.0)).abs() < 1e-9);
    }
}
