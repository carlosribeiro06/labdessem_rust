//! Startup/shutdown trajectory bounds constraint construction.

use crate::types::{UcConstraint, UcTerm, UcVariable};

pub struct ThermalUnitTrajectoryInfo<'a> {
    pub plant_name: &'a str,
    pub unit_name: &'a str,
    pub min_generation_mw: f64,
    pub max_generation_mw: f64,
    pub startup_trajectory_mw: &'a [f64],
    pub shutdown_trajectory_mw: &'a [f64],
    pub initial_startup_remaining: Option<&'a [f64]>,
    pub initial_shutdown_remaining: Option<&'a [f64]>,
}

#[must_use]
pub fn build_trajectory_bounds_constraints(
    units: &[ThermalUnitTrajectoryInfo<'_>],
    generation_var_names: &[String],
    commitment_vars: &[UcVariable],
    startup_vars: &[UcVariable],
    shutdown_vars: &[UcVariable],
    horizon: usize,
) -> Vec<UcConstraint> {
    if commitment_vars.is_empty() || generation_var_names.is_empty() {
        return Vec::new();
    }

    let mut constraints = Vec::new();

    for (unit_idx, unit) in units.iter().enumerate() {
        for period in 0..horizon {
            let var_idx = unit_idx * horizon + period;
            let gen_var_name = &generation_var_names[var_idx];
            let on_var = &commitment_vars[var_idx];

            let (startup_sum_terms, startup_weighted_terms) =
                build_startup_terms(unit, startup_vars, unit_idx, horizon, period);

            let (shutdown_sum_terms, shutdown_weighted_terms) =
                build_shutdown_terms(unit, shutdown_vars, unit_idx, horizon, period);

            let (initial_startup_active, initial_startup_generation) =
                compute_initial_startup_effect(unit, period);
            let (initial_shutdown_active, initial_shutdown_generation) =
                compute_initial_shutdown_effect(unit, period);

            // Lower bound
            let mut lower_terms = vec![
                UcTerm::new(gen_var_name, 1.0),
                UcTerm::new(&on_var.name, -unit.min_generation_mw),
            ];
            for term in &startup_sum_terms {
                lower_terms.push(UcTerm::new(&term.variable, unit.min_generation_mw));
            }
            for term in &shutdown_sum_terms {
                lower_terms.push(UcTerm::new(&term.variable, unit.min_generation_mw));
            }
            for term in &startup_weighted_terms {
                lower_terms.push(UcTerm::new(&term.variable, -term.coefficient));
            }
            for term in &shutdown_weighted_terms {
                lower_terms.push(UcTerm::new(&term.variable, -term.coefficient));
            }

            let lower_rhs = -unit.min_generation_mw
                * (initial_startup_active + initial_shutdown_active)
                + initial_startup_generation
                + initial_shutdown_generation;

            constraints.push(UcConstraint::geq(
                format!(
                    "thermal_ramp_lower[p={},u={},t={}]",
                    unit.plant_name, unit.unit_name, period + 1
                ),
                lower_terms,
                lower_rhs,
            ));

            // Upper bound
            let mut upper_terms = vec![
                UcTerm::new(gen_var_name, 1.0),
                UcTerm::new(&on_var.name, -unit.max_generation_mw),
            ];
            for term in &startup_sum_terms {
                upper_terms.push(UcTerm::new(&term.variable, unit.max_generation_mw));
            }
            for term in &shutdown_sum_terms {
                upper_terms.push(UcTerm::new(&term.variable, unit.max_generation_mw));
            }
            for term in &startup_weighted_terms {
                upper_terms.push(UcTerm::new(&term.variable, -term.coefficient));
            }
            for term in &shutdown_weighted_terms {
                upper_terms.push(UcTerm::new(&term.variable, -term.coefficient));
            }

            let upper_rhs = -unit.max_generation_mw
                * (initial_startup_active + initial_shutdown_active)
                + initial_startup_generation
                + initial_shutdown_generation;

            constraints.push(UcConstraint::leq(
                format!(
                    "thermal_ramp_upper[p={},u={},t={}]",
                    unit.plant_name, unit.unit_name, period + 1
                ),
                upper_terms,
                upper_rhs,
            ));
        }
    }

    constraints
}

fn build_startup_terms(
    unit: &ThermalUnitTrajectoryInfo<'_>,
    startup_vars: &[UcVariable],
    unit_idx: usize,
    horizon: usize,
    period: usize,
) -> (Vec<UcTerm>, Vec<UcTerm>) {
    let mut sum_terms = Vec::new();
    let mut weighted_terms = Vec::new();

    for (k, startup_level) in unit.startup_trajectory_mw.iter().enumerate() {
        if period >= k {
            let name = &startup_vars[unit_idx * horizon + (period - k)].name;
            sum_terms.push(UcTerm::new(name, 1.0));
            weighted_terms.push(UcTerm::new(name, *startup_level));
        }
    }

    (sum_terms, weighted_terms)
}

fn build_shutdown_terms(
    unit: &ThermalUnitTrajectoryInfo<'_>,
    shutdown_vars: &[UcVariable],
    unit_idx: usize,
    horizon: usize,
    period: usize,
) -> (Vec<UcTerm>, Vec<UcTerm>) {
    let mut sum_terms = Vec::new();
    let mut weighted_terms = Vec::new();

    let shutdown_len = unit.shutdown_trajectory_mw.len();
    for k in 1..=shutdown_len {
        if period + k < horizon {
            let name = &shutdown_vars[unit_idx * horizon + (period + k)].name;
            sum_terms.push(UcTerm::new(name, 1.0));
            weighted_terms.push(UcTerm::new(name, unit.shutdown_trajectory_mw[shutdown_len - k]));
        }
    }

    (sum_terms, weighted_terms)
}

fn compute_initial_startup_effect(unit: &ThermalUnitTrajectoryInfo<'_>, period: usize) -> (f64, f64) {
    match unit.initial_startup_remaining {
        Some(remaining) if period < remaining.len() => (1.0, remaining[period]),
        _ => (0.0, 0.0),
    }
}

fn compute_initial_shutdown_effect(
    unit: &ThermalUnitTrajectoryInfo<'_>,
    period: usize,
) -> (f64, f64) {
    match unit.initial_shutdown_remaining {
        Some(remaining) if period < remaining.len() => (1.0, remaining[period]),
        _ => (0.0, 0.0),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::thermal::variables::{
        build_thermal_commitment_variables, build_thermal_shutdown_variables,
        build_thermal_startup_variables, ThermalUnitInfo,
    };
    use crate::types::CommitmentDomain;

    fn sample_unit() -> ThermalUnitTrajectoryInfo<'static> {
        ThermalUnitTrajectoryInfo {
            plant_name: "UTE1",
            unit_name: "UTE1-1",
            min_generation_mw: 50.0,
            max_generation_mw: 200.0,
            startup_trajectory_mw: &[50.0, 100.0],
            shutdown_trajectory_mw: &[100.0, 50.0],
            initial_startup_remaining: None,
            initial_shutdown_remaining: None,
        }
    }

    fn gen_var_names(horizon: usize) -> Vec<String> {
        (0..horizon)
            .map(|t| format!("thermal_generation[p=UTE1,u=UTE1-1,t={}]", t + 1))
            .collect()
    }

    fn uc_vars(horizon: usize) -> (Vec<UcVariable>, Vec<UcVariable>, Vec<UcVariable>) {
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
    fn builds_two_constraints_per_period() {
        let units = vec![sample_unit()];
        let gen_names = gen_var_names(3);
        let (commitment, startup, shutdown) = uc_vars(3);

        let constraints = build_trajectory_bounds_constraints(
            &units, &gen_names, &commitment, &startup, &shutdown, 3,
        );

        assert_eq!(constraints.len(), 6);
    }

    #[test]
    fn lower_and_upper_constraints_alternate() {
        let units = vec![sample_unit()];
        let gen_names = gen_var_names(2);
        let (commitment, startup, shutdown) = uc_vars(2);

        let constraints = build_trajectory_bounds_constraints(
            &units, &gen_names, &commitment, &startup, &shutdown, 2,
        );

        assert!(constraints[0].name.contains("thermal_ramp_lower"));
        assert!(constraints[1].name.contains("thermal_ramp_upper"));
    }

    #[test]
    fn empty_vars_returns_empty() {
        let units = vec![sample_unit()];
        let gen_names = gen_var_names(3);

        let constraints =
            build_trajectory_bounds_constraints(&units, &gen_names, &[], &[], &[], 3);

        assert!(constraints.is_empty());
    }
}
