//! Column and row offset types for LP matrix indexing.

/// Column offsets for each variable type in the LP matrix.
///
/// Used to locate variable values in solution vectors without
/// coupling to the model construction logic.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct ColumnOffsets {
    pub thermal_generation: usize,
    pub hydro_generation: usize,
    pub hydro_turbining: usize,
    pub hydro_spillage: usize,
    pub hydro_diversion: usize,
    pub pumping: usize,
    pub hydro_volume: usize,
    pub future_cost: usize,
    pub renewable_generation: usize,
    pub interchange: usize,
    pub thermal_commitment: usize,
    pub thermal_startup: usize,
    pub thermal_shutdown: usize,
    pub hydro_commitment: usize,
    pub network_flow_slack: usize,
    pub operational_limit_slack: usize,
    pub total: usize,
}
