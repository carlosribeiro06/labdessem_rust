//! Power Transfer Distribution Factor (PTDF) matrix computation.
//!
//! The PTDF matrix relates bus power injections to branch power flows
//! in a linearized DC power flow model:
//!
//! ```text
//! P_branch = PTDF × P_injection
//! ```
//!
//! Computed as: `PTDF = D × A × B⁻¹`
//! where:
//! - D = diagonal branch susceptance matrix
//! - A = reduced incidence matrix
//! - B = reduced nodal susceptance matrix

use std::collections::HashMap;

use labdessem_core::{ids::BusId, system::System};

use super::incidence::build_reduced_incidence;
use super::susceptance::build_reduced_susceptance;
use super::susceptance_diag::build_branch_susceptance_diag;
use crate::error::NetworkError;
use crate::linalg::{invert_matrix, multiply_matrices};

/// Compute the PTDF matrix from system data.
///
/// Returns a matrix where `PTDF[l][n]` is the sensitivity of flow on branch `l`
/// to injection at non-slack bus `n`.
///
/// # Errors
///
/// Returns `NetworkError::SingularMatrix` if B is not invertible.
pub fn compute_ptdf(
    system: &System,
    non_slack_bus_positions: &HashMap<BusId, usize>,
) -> Result<Vec<Vec<f64>>, NetworkError> {
    let reduced_b = build_reduced_susceptance(system, non_slack_bus_positions);
    let branch_susceptance_diag = build_branch_susceptance_diag(system);
    let reduced_incidence = build_reduced_incidence(system, non_slack_bus_positions);

    let inverse_reduced_b = invert_matrix(&reduced_b)?;
    let branch_times_incidence = multiply_matrices(&branch_susceptance_diag, &reduced_incidence)?;
    multiply_matrices(&branch_times_incidence, &inverse_reduced_b)
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
    fn test_ptdf_empty() {
        let positions: HashMap<BusId, usize> = HashMap::new();
        let system = minimal_system(vec![]);
        let result = compute_ptdf(&system, &positions).unwrap();
        assert!(result.is_empty());
    }

    #[test]
    fn test_ptdf_single_branch() {
        // Bus 0 is slack, bus 1 is non-slack; branch from 0 to 1 with X=0.1
        // For a 2-bus system with one branch, |PTDF| = 1.0
        // Sign convention: positive injection at bus 1 flows from bus 0 to bus 1
        // Since bus 1 is the to_bus of the branch, incidence = -1
        // PTDF = D × A × B⁻¹ = (1/0.1) × (-1) × (1/(1/0.1)) = 10 × (-1) × 0.1 = -1.0
        let mut positions: HashMap<BusId, usize> = HashMap::new();
        positions.insert(BusId(1), 0);

        let system = minimal_system(vec![make_branch(1, 0, 1, 0.1)]);

        let result = compute_ptdf(&system, &positions).unwrap();
        assert_eq!(result.len(), 1); // 1 branch
        assert_eq!(result[0].len(), 1); // 1 non-slack bus
        // Magnitude should be 1.0 (sign depends on convention)
        assert!((result[0][0].abs() - 1.0).abs() < 1e-9);
    }

    #[test]
    fn test_ptdf_three_bus_radial() {
        // Radial network: 0 -- 1 -- 2
        // Bus 0 is slack, buses 1 and 2 are non-slack
        // Branch 0: 0-1, X=0.1 (from_bus=0 slack, to_bus=1 non-slack)
        // Branch 1: 1-2, X=0.1 (from_bus=1 non-slack, to_bus=2 non-slack)
        //
        // For radial networks, all flow from downstream buses passes through upstream branches
        let mut positions: HashMap<BusId, usize> = HashMap::new();
        positions.insert(BusId(1), 0);
        positions.insert(BusId(2), 1);

        let system = minimal_system(vec![
            make_branch(1, 0, 1, 0.1), // branch 0
            make_branch(2, 1, 2, 0.1), // branch 1
        ]);

        let result = compute_ptdf(&system, &positions).unwrap();
        assert_eq!(result.len(), 2); // 2 branches
        assert_eq!(result[0].len(), 2); // 2 non-slack buses

        // Branch 0-1: carries all injection from buses 1 and 2 (both exit through this branch)
        // |PTDF[0][0]| = 1.0 (injection at bus 1)
        // |PTDF[0][1]| = 1.0 (injection at bus 2)
        assert!((result[0][0].abs() - 1.0).abs() < 1e-9);
        assert!((result[0][1].abs() - 1.0).abs() < 1e-9);

        // Branch 1-2: carries only injection from bus 2
        // PTDF[1][0] ≈ 0.0 (injection at bus 1 doesn't go through 1-2)
        // |PTDF[1][1]| = 1.0 (injection at bus 2 goes through 1-2)
        assert!(result[1][0].abs() < 1e-9);
        assert!((result[1][1].abs() - 1.0).abs() < 1e-9);
    }
}
