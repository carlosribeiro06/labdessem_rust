//! Diagonal branch susceptance matrix (D) construction.
//!
//! A diagonal matrix where `D[k][k] = 1/X_k` represents the susceptance
//! of branch k. Used in the PTDF computation: `PTDF = D × A × B⁻¹`.

use labdessem_core::system::System;

/// Build the diagonal branch susceptance matrix.
///
/// Returns a n×n diagonal matrix where n is the number of branches,
/// with `D[k][k] = 1/X_k` for branch k.
pub fn build_branch_susceptance_diag(system: &System) -> Vec<Vec<f64>> {
    let size = system.branches.len();
    let mut matrix = vec![vec![0.0; size]; size];

    for (branch_idx, branch) in system.branches.iter().enumerate() {
        matrix[branch_idx][branch_idx] = 1.0 / branch.reactance_pu;
    }

    matrix
}

#[cfg(test)]
mod tests {
    use super::*;
    use labdessem_core::{
        entities::Branch,
        ids::{BranchId, BusId},
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
    fn test_susceptance_diag_empty() {
        let system = minimal_system(vec![]);
        let result = build_branch_susceptance_diag(&system);
        assert!(result.is_empty());
    }

    #[test]
    fn test_susceptance_diag_single_branch() {
        // Branch with X=0.1 -> susceptance = 10.0
        let system = minimal_system(vec![make_branch(1, 0, 1, 0.1)]);

        let result = build_branch_susceptance_diag(&system);
        assert_eq!(result.len(), 1);
        assert_eq!(result[0].len(), 1);
        assert!((result[0][0] - 10.0).abs() < 1e-9);
    }

    #[test]
    fn test_susceptance_diag_multiple_branches() {
        let system = minimal_system(vec![
            make_branch(1, 0, 1, 0.1), // 1/0.1 = 10.0
            make_branch(2, 1, 2, 0.2), // 1/0.2 = 5.0
            make_branch(3, 0, 2, 0.5), // 1/0.5 = 2.0
        ]);

        let result = build_branch_susceptance_diag(&system);
        assert_eq!(result.len(), 3);

        // Check diagonal values
        assert!((result[0][0] - 10.0).abs() < 1e-9);
        assert!((result[1][1] - 5.0).abs() < 1e-9);
        assert!((result[2][2] - 2.0).abs() < 1e-9);

        // Check off-diagonal are zero
        assert_eq!(result[0][1], 0.0);
        assert_eq!(result[0][2], 0.0);
        assert_eq!(result[1][0], 0.0);
        assert_eq!(result[1][2], 0.0);
        assert_eq!(result[2][0], 0.0);
        assert_eq!(result[2][1], 0.0);
    }
}
