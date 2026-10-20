//! Startup/shutdown exclusivity constraint construction.

use crate::types::{UcConstraint, UcTerm, UcVariable};

pub struct ThermalUnitExclusiveInfo<'a> {
    pub plant_name: &'a str,
    pub unit_name: &'a str,
}

#[must_use]
pub fn build_startup_shutdown_exclusive_constraints(
    units: &[ThermalUnitExclusiveInfo<'_>],
    startup_vars: &[UcVariable],
    shutdown_vars: &[UcVariable],
    horizon: usize,
) -> Vec<UcConstraint> {
    if startup_vars.is_empty() || shutdown_vars.is_empty() {
        return Vec::new();
    }

    let mut constraints = Vec::with_capacity(units.len() * horizon);

    for (unit_idx, unit) in units.iter().enumerate() {
        for period in 0..horizon {
            let var_idx = unit_idx * horizon + period;

            let startup_var = &startup_vars[var_idx];
            let shutdown_var = &shutdown_vars[var_idx];

            constraints.push(UcConstraint::leq(
                format!(
                    "thermal_startup_shutdown_exclusive[p={},u={},t={}]",
                    unit.plant_name, unit.unit_name, period + 1
                ),
                vec![
                    UcTerm::new(&startup_var.name, 1.0),
                    UcTerm::new(&shutdown_var.name, 1.0),
                ],
                1.0,
            ));
        }
    }

    constraints
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::thermal::variables::{
        build_thermal_shutdown_variables, build_thermal_startup_variables, ThermalUnitInfo,
    };
    use crate::types::CommitmentDomain;

    fn sample_units() -> Vec<ThermalUnitExclusiveInfo<'static>> {
        vec![ThermalUnitExclusiveInfo {
            plant_name: "UTE1",
            unit_name: "UTE1-1",
        }]
    }

    fn sample_vars(horizon: usize) -> (Vec<UcVariable>, Vec<UcVariable>) {
        let units = vec![ThermalUnitInfo {
            plant_name: "UTE1",
            unit_name: "UTE1-1",
        }];
        let startup = build_thermal_startup_variables(&units, horizon, CommitmentDomain::Binary);
        let shutdown = build_thermal_shutdown_variables(&units, horizon, CommitmentDomain::Binary);
        (startup, shutdown)
    }

    #[test]
    fn builds_correct_count() {
        let units = sample_units();
        let (startup, shutdown) = sample_vars(3);
        let constraints =
            build_startup_shutdown_exclusive_constraints(&units, &startup, &shutdown, 3);
        assert_eq!(constraints.len(), 3);
    }

    #[test]
    fn constraint_is_leq() {
        let units = sample_units();
        let (startup, shutdown) = sample_vars(1);
        let constraints =
            build_startup_shutdown_exclusive_constraints(&units, &startup, &shutdown, 1);

        assert_eq!(
            constraints[0].sense,
            crate::types::ConstraintSense::LessOrEqual
        );
        assert_eq!(constraints[0].rhs, 1.0);
    }

    #[test]
    fn constraint_has_correct_terms() {
        let units = sample_units();
        let (startup, shutdown) = sample_vars(1);
        let constraints =
            build_startup_shutdown_exclusive_constraints(&units, &startup, &shutdown, 1);

        assert_eq!(constraints[0].terms.len(), 2);
        assert_eq!(constraints[0].terms[0].coefficient, 1.0);
        assert_eq!(constraints[0].terms[1].coefficient, 1.0);
    }

    #[test]
    fn empty_vars_returns_empty() {
        let units = sample_units();
        let constraints = build_startup_shutdown_exclusive_constraints(&units, &[], &[], 3);
        assert!(constraints.is_empty());
    }
}
