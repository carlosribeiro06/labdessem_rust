//! Variable naming functions for model construction and result extraction.
//!
//! These functions generate consistent variable names used both when building
//! the optimization model and when reading results from the solver.

/// Format period number for display (1-indexed).
#[inline]
pub fn display_period(period: usize) -> usize {
    period + 1
}

// =============================================================================
// Thermal variable names
// =============================================================================

pub fn thermal_generation_name(plant_name: &str, unit_name: &str, period: usize) -> String {
    format!(
        "thermal_generation[p={},u={},t={}]",
        plant_name,
        unit_name,
        display_period(period)
    )
}

pub fn thermal_commitment_name(plant_name: &str, unit_name: &str, period: usize) -> String {
    format!(
        "thermal_on[p={},u={},t={}]",
        plant_name,
        unit_name,
        display_period(period)
    )
}

pub fn thermal_startup_name(plant_name: &str, unit_name: &str, period: usize) -> String {
    format!(
        "thermal_startup[p={},u={},t={}]",
        plant_name,
        unit_name,
        display_period(period)
    )
}

pub fn thermal_shutdown_name(plant_name: &str, unit_name: &str, period: usize) -> String {
    format!(
        "thermal_shutdown[p={},u={},t={}]",
        plant_name,
        unit_name,
        display_period(period)
    )
}

// =============================================================================
// Hydro variable names
// =============================================================================

pub fn hydro_generation_name(
    plant_name: &str,
    group_name: &str,
    unit_name: &str,
    period: usize,
) -> String {
    format!(
        "hydro_generation[p={},g={},u={},t={}]",
        plant_name,
        group_name,
        unit_name,
        display_period(period)
    )
}

pub fn hydro_turbining_name(
    plant_name: &str,
    group_name: &str,
    unit_name: &str,
    period: usize,
) -> String {
    format!(
        "hydro_turbining[p={},g={},u={},t={}]",
        plant_name,
        group_name,
        unit_name,
        display_period(period)
    )
}

pub fn hydro_commitment_name(
    plant_name: &str,
    group_name: &str,
    unit_name: &str,
    period: usize,
) -> String {
    format!(
        "hydro_on[p={},g={},u={},t={}]",
        plant_name,
        group_name,
        unit_name,
        display_period(period)
    )
}

pub fn hydro_spillage_name(plant_name: &str, period: usize) -> String {
    format!(
        "hydro_spillage[p={},t={}]",
        plant_name,
        display_period(period)
    )
}

pub fn hydro_diversion_name(plant_name: &str, period: usize) -> String {
    format!(
        "hydro_diversion[p={},t={}]",
        plant_name,
        display_period(period)
    )
}

pub fn hydro_volume_name(plant_name: &str, period: usize) -> String {
    format!("hydro_volume[p={},t={}]", plant_name, period)
}

// =============================================================================
// Pumping variable names
// =============================================================================

pub fn pumping_name(plant_name: &str, period: usize) -> String {
    format!("pumping[p={},t={}]", plant_name, display_period(period))
}

pub fn pumping_consumption_name(plant_name: &str, period: usize) -> String {
    format!(
        "pumping_consumption[p={},t={}]",
        plant_name,
        display_period(period)
    )
}

// =============================================================================
// Renewable variable names
// =============================================================================

pub fn renewable_generation_name(plant_name: &str, period: usize) -> String {
    format!(
        "renewable_generation[p={},t={}]",
        plant_name,
        display_period(period)
    )
}

// =============================================================================
// Network variable names
// =============================================================================

pub fn bus_angle_name(bus_id: usize, period: usize) -> String {
    format!("bus_angle[b={},t={}]", bus_id, display_period(period))
}

pub fn branch_flow_name(branch_id: usize, period: usize) -> String {
    format!("branch_flow[l={},t={}]", branch_id, display_period(period))
}

// =============================================================================
// Submarket variable names
// =============================================================================

pub fn deficit_name(submarket_id: usize, period: usize) -> String {
    format!(
        "deficit[s={},t={}]",
        submarket_id,
        display_period(period)
    )
}

// =============================================================================
// Constraint names (for dual extraction)
// =============================================================================

pub fn demand_balance_constraint_name(submarket_name: &str, period: usize) -> String {
    format!(
        "demand_balance[submarket={},t={}]",
        submarket_name,
        display_period(period)
    )
}

pub fn hydro_balance_constraint_name(plant_name: &str, period: usize) -> String {
    format!(
        "hydro_balance[p={},t={}]",
        plant_name,
        display_period(period)
    )
}
