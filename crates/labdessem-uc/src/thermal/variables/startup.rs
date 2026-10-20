//! Thermal startup variable construction.

use crate::types::{CommitmentDomain, UcVariable};

use super::commitment::ThermalUnitInfo;

#[must_use]
pub fn build_thermal_startup_variables(
    units: &[ThermalUnitInfo<'_>],
    horizon: usize,
    domain: CommitmentDomain,
) -> Vec<UcVariable> {
    if !domain.is_active() {
        return Vec::new();
    }

    units
        .iter()
        .flat_map(|unit| {
            (0..horizon).map(move |period| {
                let name = format!(
                    "thermal_startup[p={},u={},t={}]",
                    unit.plant_name,
                    unit.unit_name,
                    period + 1
                );
                match domain {
                    CommitmentDomain::Binary => UcVariable::unit_interval(name),
                    CommitmentDomain::Fixed(v) => UcVariable::fixed(name, v),
                    CommitmentDomain::Inactive => unreachable!(),
                }
            })
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn sample_units() -> Vec<ThermalUnitInfo<'static>> {
        vec![ThermalUnitInfo {
            plant_name: "UTE1",
            unit_name: "UTE1-1",
        }]
    }

    #[test]
    fn builds_correct_count() {
        let units = sample_units();
        let vars = build_thermal_startup_variables(&units, 3, CommitmentDomain::Binary);
        assert_eq!(vars.len(), 3);
    }

    #[test]
    fn variable_names_are_correct() {
        let units = sample_units();
        let vars = build_thermal_startup_variables(&units, 2, CommitmentDomain::Binary);

        assert_eq!(vars[0].name, "thermal_startup[p=UTE1,u=UTE1-1,t=1]");
        assert_eq!(vars[1].name, "thermal_startup[p=UTE1,u=UTE1-1,t=2]");
    }

    #[test]
    fn inactive_domain_returns_empty() {
        let units = sample_units();
        let vars = build_thermal_startup_variables(&units, 3, CommitmentDomain::Inactive);
        assert!(vars.is_empty());
    }

    #[test]
    fn milp_uses_unit_interval() {
        let units = sample_units();
        let vars = build_thermal_startup_variables(&units, 1, CommitmentDomain::Binary);
        assert_eq!(vars[0].lower_bound, 0.0);
        assert_eq!(vars[0].upper_bound, 1.0);
    }
}
