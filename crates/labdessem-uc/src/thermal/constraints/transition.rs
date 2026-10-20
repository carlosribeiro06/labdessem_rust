//! Thermal state transition constraint construction.

use crate::types::{UcConstraint, UcTerm, UcVariable};

pub struct ThermalUnitTransitionInfo<'a> {
    pub plant_name: &'a str,
    pub unit_name: &'a str,
    pub initial_is_on: bool,
}

#[must_use]
pub fn build_thermal_transition_constraints(
    units: &[ThermalUnitTransitionInfo<'_>],
    commitment_vars: &[UcVariable],
    startup_vars: &[UcVariable],
    shutdown_vars: &[UcVariable],
    horizon: usize,
) -> Vec<UcConstraint> {
    if commitment_vars.is_empty() {
        return Vec::new();
    }

    let mut constraints = Vec::with_capacity(units.len() * horizon);

    for (unit_idx, unit) in units.iter().enumerate() {
        let initial_status = if unit.initial_is_on { 1.0 } else { 0.0 };

        for period in 0..horizon {
            let var_idx = unit_idx * horizon + period;

            let on_var = &commitment_vars[var_idx];
            let startup_var = &startup_vars[var_idx];
            let shutdown_var = &shutdown_vars[var_idx];

            let mut terms = vec![
                UcTerm::new(&on_var.name, 1.0),
                UcTerm::new(&startup_var.name, -1.0),
                UcTerm::new(&shutdown_var.name, 1.0),
            ];

            let rhs = if period == 0 {
                initial_status
            } else {
                let prev_on_var = &commitment_vars[unit_idx * horizon + period - 1];
                terms.push(UcTerm::new(&prev_on_var.name, -1.0));
                0.0
            };

            constraints.push(UcConstraint::equal(
                format!(
                    "thermal_transition[p={},u={},t={}]",
                    unit.plant_name, unit.unit_name, period + 1
                ),
                terms,
                rhs,
            ));
        }
    }

    constraints
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::thermal::variables::{
        build_thermal_commitment_variables, build_thermal_shutdown_variables,
        build_thermal_startup_variables, ThermalUnitInfo,
    };
    use crate::types::CommitmentDomain;

    fn sample_unit() -> ThermalUnitTransitionInfo<'static> {
        ThermalUnitTransitionInfo {
            plant_name: "UTE1",
            unit_name: "UTE1-1",
            initial_is_on: false,
        }
    }

    fn sample_vars(horizon: usize) -> (Vec<UcVariable>, Vec<UcVariable>, Vec<UcVariable>) {
        let units = vec![ThermalUnitInfo {
            plant_name: "UTE1",
            unit_name: "UTE1-1",
        }];
        let commitment =
            build_thermal_commitment_variables(&units, horizon, CommitmentDomain::Binary);
        let startup = build_thermal_startup_variables(&units, horizon, CommitmentDomain::Binary);
        let shutdown = build_thermal_shutdown_variables(&units, horizon, CommitmentDomain::Binary);
        (commitment, startup, shutdown)
    }

    #[test]
    fn builds_correct_count() {
        let units = vec![sample_unit()];
        let (commitment, startup, shutdown) = sample_vars(3);
        let constraints =
            build_thermal_transition_constraints(&units, &commitment, &startup, &shutdown, 3);
        assert_eq!(constraints.len(), 3);
    }

    #[test]
    fn period_0_uses_initial_status_off() {
        let units = vec![ThermalUnitTransitionInfo {
            plant_name: "UTE1",
            unit_name: "UTE1-1",
            initial_is_on: false,
        }];
        let (commitment, startup, shutdown) = sample_vars(2);
        let constraints =
            build_thermal_transition_constraints(&units, &commitment, &startup, &shutdown, 2);

        assert_eq!(constraints[0].rhs, 0.0);
        assert_eq!(constraints[0].terms.len(), 3);
    }

    #[test]
    fn period_0_uses_initial_status_on() {
        let units = vec![ThermalUnitTransitionInfo {
            plant_name: "UTE1",
            unit_name: "UTE1-1",
            initial_is_on: true,
        }];
        let (commitment, startup, shutdown) = sample_vars(2);
        let constraints =
            build_thermal_transition_constraints(&units, &commitment, &startup, &shutdown, 2);

        assert_eq!(constraints[0].rhs, 1.0);
    }

    #[test]
    fn subsequent_periods_use_previous_commitment() {
        let units = vec![sample_unit()];
        let (commitment, startup, shutdown) = sample_vars(2);
        let constraints =
            build_thermal_transition_constraints(&units, &commitment, &startup, &shutdown, 2);

        assert_eq!(constraints[1].rhs, 0.0);
        assert_eq!(constraints[1].terms.len(), 4);
    }

    #[test]
    fn empty_vars_returns_empty() {
        let units = vec![sample_unit()];
        let constraints = build_thermal_transition_constraints(&units, &[], &[], &[], 3);
        assert!(constraints.is_empty());
    }
}
