//! Minimum up/down time and initial state fixing constraints.

use crate::types::{UcConstraint, UcTerm, UcVariable};

pub struct ThermalUnitMinUpDownInfo<'a> {
    pub plant_name: &'a str,
    pub unit_name: &'a str,
    pub min_up_time: usize,
    pub min_down_time: usize,
    pub initial_is_on: bool,
    pub initial_time_in_state: usize,
}

#[must_use]
pub fn build_initial_state_fixing_constraints(
    units: &[ThermalUnitMinUpDownInfo<'_>],
    commitment_vars: &[UcVariable],
    horizon: usize,
) -> Vec<UcConstraint> {
    if commitment_vars.is_empty() {
        return Vec::new();
    }

    let mut constraints = Vec::new();

    for (unit_idx, unit) in units.iter().enumerate() {
        let remaining_on = if unit.initial_is_on {
            unit.min_up_time.saturating_sub(unit.initial_time_in_state)
        } else {
            0
        };
        let remaining_off = if unit.initial_is_on {
            0
        } else {
            unit.min_down_time
                .saturating_sub(unit.initial_time_in_state)
        };

        for period in 0..remaining_on.min(horizon) {
            let var_idx = unit_idx * horizon + period;
            let on_var = &commitment_vars[var_idx];
            constraints.push(UcConstraint::equal(
                format!(
                    "thermal_initial_on_fix[p={},u={},t={}]",
                    unit.plant_name,
                    unit.unit_name,
                    period + 1
                ),
                vec![UcTerm::new(&on_var.name, 1.0)],
                1.0,
            ));
        }

        for period in 0..remaining_off.min(horizon) {
            let var_idx = unit_idx * horizon + period;
            let on_var = &commitment_vars[var_idx];
            constraints.push(UcConstraint::equal(
                format!(
                    "thermal_initial_off_fix[p={},u={},t={}]",
                    unit.plant_name,
                    unit.unit_name,
                    period + 1
                ),
                vec![UcTerm::new(&on_var.name, 1.0)],
                0.0,
            ));
        }
    }

    constraints
}

#[must_use]
pub fn build_min_up_time_constraints(
    units: &[ThermalUnitMinUpDownInfo<'_>],
    commitment_vars: &[UcVariable],
    horizon: usize,
) -> Vec<UcConstraint> {
    if commitment_vars.is_empty() {
        return Vec::new();
    }

    let mut constraints = Vec::new();

    for (unit_idx, unit) in units.iter().enumerate() {
        let initial_status = if unit.initial_is_on { 1.0 } else { 0.0 };

        for period in 0..horizon {
            let min_up_window = unit.min_up_time.min(horizon - period);

            let mut terms: Vec<UcTerm> = (period..(period + min_up_window))
                .map(|ts| {
                    let var_idx = unit_idx * horizon + ts;
                    UcTerm::new(&commitment_vars[var_idx].name, 1.0)
                })
                .collect();

            let current_var_idx = unit_idx * horizon + period;
            terms.push(UcTerm::new(
                &commitment_vars[current_var_idx].name,
                -(min_up_window as f64),
            ));

            let rhs = if period == 0 {
                -(min_up_window as f64) * initial_status
            } else {
                let prev_var_idx = unit_idx * horizon + period - 1;
                terms.push(UcTerm::new(
                    &commitment_vars[prev_var_idx].name,
                    min_up_window as f64,
                ));
                0.0
            };

            constraints.push(UcConstraint::geq(
                format!(
                    "thermal_min_up[p={},u={},t={}]",
                    unit.plant_name,
                    unit.unit_name,
                    period + 1
                ),
                terms,
                rhs,
            ));
        }
    }

    constraints
}

#[must_use]
pub fn build_min_down_time_constraints(
    units: &[ThermalUnitMinUpDownInfo<'_>],
    commitment_vars: &[UcVariable],
    horizon: usize,
) -> Vec<UcConstraint> {
    if commitment_vars.is_empty() {
        return Vec::new();
    }

    let mut constraints = Vec::new();

    for (unit_idx, unit) in units.iter().enumerate() {
        let initial_status = if unit.initial_is_on { 1.0 } else { 0.0 };

        for period in 0..horizon {
            let min_down_window = unit.min_down_time.min(horizon - period);

            let mut terms: Vec<UcTerm> = (period..(period + min_down_window))
                .map(|ts| {
                    let var_idx = unit_idx * horizon + ts;
                    UcTerm::new(&commitment_vars[var_idx].name, 1.0)
                })
                .collect();

            let current_var_idx = unit_idx * horizon + period;
            terms.push(UcTerm::new(
                &commitment_vars[current_var_idx].name,
                -(min_down_window as f64),
            ));

            let rhs = if period == 0 {
                (min_down_window as f64) * (1.0 - initial_status)
            } else {
                let prev_var_idx = unit_idx * horizon + period - 1;
                terms.push(UcTerm::new(
                    &commitment_vars[prev_var_idx].name,
                    min_down_window as f64,
                ));
                min_down_window as f64
            };

            constraints.push(UcConstraint::leq(
                format!(
                    "thermal_min_down[p={},u={},t={}]",
                    unit.plant_name,
                    unit.unit_name,
                    period + 1
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
    use crate::thermal::variables::{ThermalUnitInfo, build_thermal_commitment_variables};
    use crate::types::CommitmentDomain;

    fn sample_unit(is_on: bool, time_in_state: usize) -> ThermalUnitMinUpDownInfo<'static> {
        ThermalUnitMinUpDownInfo {
            plant_name: "UTE1",
            unit_name: "UTE1-1",
            min_up_time: 3,
            min_down_time: 2,
            initial_is_on: is_on,
            initial_time_in_state: time_in_state,
        }
    }

    fn commitment_vars(horizon: usize) -> Vec<UcVariable> {
        let units = vec![ThermalUnitInfo {
            plant_name: "UTE1",
            unit_name: "UTE1-1",
        }];
        build_thermal_commitment_variables(&units, horizon, CommitmentDomain::Binary)
    }

    #[test]
    fn initial_fixing_on_remaining() {
        let units = vec![sample_unit(true, 1)];
        let vars = commitment_vars(5);
        let constraints = build_initial_state_fixing_constraints(&units, &vars, 5);

        assert_eq!(constraints.len(), 2);
        assert_eq!(constraints[0].rhs, 1.0);
        assert_eq!(constraints[1].rhs, 1.0);
    }

    #[test]
    fn initial_fixing_off_remaining() {
        let units = vec![sample_unit(false, 0)];
        let vars = commitment_vars(5);
        let constraints = build_initial_state_fixing_constraints(&units, &vars, 5);

        assert_eq!(constraints.len(), 2);
        assert_eq!(constraints[0].rhs, 0.0);
        assert_eq!(constraints[1].rhs, 0.0);
    }

    #[test]
    fn min_up_creates_constraints_for_all_periods() {
        let units = vec![sample_unit(false, 3)];
        let vars = commitment_vars(5);
        let constraints = build_min_up_time_constraints(&units, &vars, 5);

        assert_eq!(constraints.len(), 5);
    }

    #[test]
    fn min_down_creates_constraints_for_all_periods() {
        let units = vec![sample_unit(true, 3)];
        let vars = commitment_vars(5);
        let constraints = build_min_down_time_constraints(&units, &vars, 5);

        assert_eq!(constraints.len(), 5);
    }

    #[test]
    fn empty_vars_returns_empty() {
        let units = vec![sample_unit(false, 0)];
        assert!(build_initial_state_fixing_constraints(&units, &[], 5).is_empty());
        assert!(build_min_up_time_constraints(&units, &[], 5).is_empty());
        assert!(build_min_down_time_constraints(&units, &[], 5).is_empty());
    }
}
