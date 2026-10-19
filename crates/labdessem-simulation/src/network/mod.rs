//! DC network model for power flow analysis.

use std::collections::HashMap;

use labdessem_core::{ids::BusId, system::System};

use crate::SimulationError;

mod cmb;
mod dual_extraction;
mod flow;
mod ptdf;

pub use cmb::{compute_cmb_per_bus, compute_cmo_from_cmb, CmbRecord};
pub use dual_extraction::extract_flow_duals;
pub use flow::{
    all_flow_constraints, compute_line_flows, constant_load_shift, detect_flow_violations,
    flow_terms_for_cut,
};
pub use ptdf::{
    build_branch_susceptance_diag, build_reduced_incidence, build_reduced_susceptance,
    invert_matrix, multiply_matrices,
};

/// Key identifying a flow cut by branch index and period.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct FlowCutKey {
    pub branch_idx: usize,
    pub period: usize,
}

/// DC network model with PTDF matrix for power flow analysis.
#[derive(Debug, Clone)]
pub struct DcNetworkModel {
    pub(crate) non_slack_bus_ids: Vec<BusId>,
    pub(crate) non_slack_bus_positions: HashMap<BusId, usize>,
    pub(crate) ptdf_rows: Vec<Vec<f64>>,
}

impl DcNetworkModel {
    /// Build a DC network model from a system.
    pub fn from_system(system: &System) -> Result<Self, SimulationError> {
        let _slack_bus = system
            .buses
            .iter()
            .find(|bus| bus.angle_reference)
            .ok_or_else(|| {
                SimulationError::InvalidNetwork(
                    "system does not define a reference bus for DC flow".into(),
                )
            })?;

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
    #[allow(dead_code)]
    pub fn ptdf_row_for_branch(&self, branch_idx: usize) -> &[f64] {
        &self.ptdf_rows[branch_idx]
    }

    /// Get the position of a bus in the non-slack bus list.
    #[allow(dead_code)]
    pub fn position_of_bus(&self, bus_id: BusId) -> Option<usize> {
        self.non_slack_bus_positions.get(&bus_id).copied()
    }

    /// Get the list of non-slack bus IDs.
    #[allow(dead_code)]
    pub fn non_slack_bus_ids(&self) -> &[BusId] {
        &self.non_slack_bus_ids
    }
}
