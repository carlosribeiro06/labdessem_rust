//! Common types, constants, and utilities for simulation stages.

use labdessem_model::Model;

pub(crate) const NETWORK_FLOW_SLACK_PENALTY_PER_MW: f64 = 1_000_000.0;

/// Get model dimensions (variable count, constraint count).
pub(crate) fn model_dimensions(model: &Model) -> (usize, usize) {
    let variables = &model.variables;
    let variable_count = variables.thermal_generation.len()
        + variables.hydro_generation.len()
        + variables.hydro_turbining.len()
        + variables.hydro_spillage.len()
        + variables.hydro_diversion.len()
        + variables.pumping.len()
        + variables.hydro_volume.len()
        + variables.future_cost.len()
        + variables.renewable_generation.len()
        + variables.interchange.len()
        + variables.thermal_commitment.len()
        + variables.thermal_startup.len()
        + variables.thermal_shutdown.len()
        + variables.hydro_commitment.len()
        + variables.network_flow_slack.len()
        + variables.operational_limit_slack.len();

    let constraint_count = model.constraints.linear_constraints.len();

    (variable_count, constraint_count)
}
