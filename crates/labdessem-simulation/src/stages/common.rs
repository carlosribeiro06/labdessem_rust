//! Common types, constants, and utilities for simulation stages.

use labdessem_model::Model;
use labdessem_model::variables::Variables;

pub(crate) const NETWORK_FLOW_SLACK_PENALTY_PER_MW: f64 = 1_000_000.0;

/// LP column of the first dynamic slack of a model built from `variables`, which is also the
/// number of its non-slack columns.
///
/// `model_to_lp_template` (`labdessem-solver` `conversion.rs`, `collect_variables`) lays out the
/// network-flow slacks and then the operational-limit slacks after every other block, so the
/// network-flow slacks start here and the operational-limit slacks start
/// `variables.network_flow_slack.len()` columns later.
///
/// The count is exact for every `SolveMode` because it reads the blocks the model holds.
/// `Indexing::compute_column_offsets` must not be used for slack columns: it counts the
/// commitment blocks whenever the UC flags are on, and it gives both slack blocks the same start.
pub(crate) fn first_slack_column(variables: &Variables) -> usize {
    variables.thermal_generation.len()
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
}

/// Get model dimensions (variable count, constraint count).
pub(crate) fn model_dimensions(model: &Model) -> (usize, usize) {
    let variables = &model.variables;
    let variable_count = first_slack_column(variables)
        + variables.network_flow_slack.len()
        + variables.operational_limit_slack.len();

    let constraint_count = model.constraints.linear_constraints.len();

    (variable_count, constraint_count)
}
