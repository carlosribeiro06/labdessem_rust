//! DC network model for power flow analysis.

use std::collections::HashMap;

use labdessem_core::{ids::BusId, system::System};

use crate::error::NetworkError;
use crate::linalg::{invert_matrix, multiply_matrices};
use crate::ptdf::{build_branch_susceptance_diag, build_reduced_incidence, build_reduced_susceptance};

/// Key identifying a flow cut by branch index and period.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct FlowCutKey {
    pub branch_idx: usize,
    pub period: usize,
}

/// DC network model with PTDF matrix for power flow analysis.
///
/// The PTDF (Power Transfer Distribution Factor) matrix maps bus power injections
/// to branch power flows in a linearized DC power flow model.
#[derive(Debug, Clone)]
pub struct DcNetworkModel {
    pub(crate) non_slack_bus_ids: Vec<BusId>,
    pub(crate) non_slack_bus_positions: HashMap<BusId, usize>,
    pub(crate) ptdf_rows: Vec<Vec<f64>>,
}

impl DcNetworkModel {
    /// Build a DC network model from a system.
    ///
    /// # Errors
    ///
    /// Returns `NetworkError::NoReferenceBus` if the system has no angle reference bus.
    /// Returns `NetworkError::SingularMatrix` if the reduced susceptance matrix is singular.
    pub fn from_system(system: &System) -> Result<Self, NetworkError> {
        let _slack_bus = system
            .buses
            .iter()
            .find(|bus| bus.angle_reference)
            .ok_or(NetworkError::NoReferenceBus)?;

        let non_slack_bus_ids = system
            .buses
            .iter()
            .filter(|bus| !bus.angle_reference)
            .map(|bus| bus.id)
            .collect::<Vec<_>>();
        let non_slack_bus_positions = non_slack_bus_ids
            .iter()
            .enumerate()
            .map(|(idx, bus_id)| (*bus_id, idx))
            .collect::<HashMap<_, _>>();

        if non_slack_bus_ids.is_empty() || system.branches.is_empty() {
            return Ok(Self {
                non_slack_bus_ids,
                non_slack_bus_positions,
                ptdf_rows: vec![Vec::new(); system.branches.len()],
            });
        }

        let reduced_b = build_reduced_susceptance(system, &non_slack_bus_positions);
        let branch_susceptance_diag = build_branch_susceptance_diag(system);
        let reduced_incidence = build_reduced_incidence(system, &non_slack_bus_positions);
        let inverse_reduced_b = invert_matrix(&reduced_b)?;
        let branch_times_incidence =
            multiply_matrices(&branch_susceptance_diag, &reduced_incidence)?;
        let ptdf_rows = multiply_matrices(&branch_times_incidence, &inverse_reduced_b)?;

        Ok(Self {
            non_slack_bus_ids,
            non_slack_bus_positions,
            ptdf_rows,
        })
    }

    /// Get the PTDF row for a specific branch.
    pub fn ptdf_row_for_branch(&self, branch_idx: usize) -> &[f64] {
        &self.ptdf_rows[branch_idx]
    }

    /// Get the position of a bus in the non-slack bus list.
    pub fn position_of_bus(&self, bus_id: BusId) -> Option<usize> {
        self.non_slack_bus_positions.get(&bus_id).copied()
    }

    /// Get the list of non-slack bus IDs.
    pub fn non_slack_bus_ids(&self) -> &[BusId] {
        &self.non_slack_bus_ids
    }

    /// Get the number of branches.
    pub fn branch_count(&self) -> usize {
        self.ptdf_rows.len()
    }

    /// Get the number of non-slack buses.
    pub fn bus_count(&self) -> usize {
        self.non_slack_bus_ids.len()
    }
}
