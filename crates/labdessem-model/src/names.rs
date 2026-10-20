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
    format!("hydro_volume[p={plant_name},t={period}]")
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

// =============================================================================
// Flow cut constraint names (for CMB dual extraction)
// =============================================================================

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FlowCutDirection {
    Upper,
    Lower,
}

pub fn flow_cut_upper_name(branch_name: &str, period: usize) -> String {
    format!("flow_cut_upper[branch={branch_name},t={period}]")
}

pub fn flow_cut_lower_name(branch_name: &str, period: usize) -> String {
    format!("flow_cut_lower[branch={branch_name},t={period}]")
}

pub fn flow_cut_name(branch_name: &str, period: usize, direction: FlowCutDirection) -> String {
    match direction {
        FlowCutDirection::Upper => flow_cut_upper_name(branch_name, period),
        FlowCutDirection::Lower => flow_cut_lower_name(branch_name, period),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_flow_cut_upper_name() {
        assert_eq!(
            flow_cut_upper_name("L001", 1),
            "flow_cut_upper[branch=L001,t=1]"
        );
        assert_eq!(
            flow_cut_upper_name("L001", 24),
            "flow_cut_upper[branch=L001,t=24]"
        );
    }

    #[test]
    fn test_flow_cut_lower_name() {
        assert_eq!(
            flow_cut_lower_name("L001", 1),
            "flow_cut_lower[branch=L001,t=1]"
        );
    }

    #[test]
    fn test_flow_cut_name_delegates_correctly() {
        assert_eq!(
            flow_cut_name("L001", 5, FlowCutDirection::Upper),
            flow_cut_upper_name("L001", 5)
        );
        assert_eq!(
            flow_cut_name("L001", 5, FlowCutDirection::Lower),
            flow_cut_lower_name("L001", 5)
        );
    }
}
