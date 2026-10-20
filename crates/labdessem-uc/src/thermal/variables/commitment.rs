//! Thermal commitment variable construction.

use crate::types::{CommitmentDomain, UcVariable};

pub struct ThermalUnitInfo<'a> {
    pub plant_name: &'a str,
    pub unit_name: &'a str,
}

#[must_use]
pub fn build_thermal_commitment_variables(
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
                    "thermal_on[p={},u={},t={}]",
                    unit.plant_name,
                    unit.unit_name,
                    period + 1
                );
                match domain {
                    CommitmentDomain::Binary => UcVariable::binary(name),
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
        vec![
            ThermalUnitInfo {
                plant_name: "UTE1",
                unit_name: "UTE1-1",
            },
            ThermalUnitInfo {
                plant_name: "UTE2",
                unit_name: "UTE2-1",
            },
        ]
    }

    #[test]
    fn builds_correct_count() {
        let units = sample_units();
        let vars = build_thermal_commitment_variables(&units, 3, CommitmentDomain::Binary);
        assert_eq!(vars.len(), 6);
    }

    #[test]
    fn variable_names_are_correct() {
        let units = sample_units();
        let vars = build_thermal_commitment_variables(&units, 2, CommitmentDomain::Binary);

        assert_eq!(vars[0].name, "thermal_on[p=UTE1,u=UTE1-1,t=1]");
        assert_eq!(vars[1].name, "thermal_on[p=UTE1,u=UTE1-1,t=2]");
        assert_eq!(vars[2].name, "thermal_on[p=UTE2,u=UTE2-1,t=1]");
        assert_eq!(vars[3].name, "thermal_on[p=UTE2,u=UTE2-1,t=2]");
    }

    #[test]
    fn inactive_domain_returns_empty() {
        let units = sample_units();
        let vars = build_thermal_commitment_variables(&units, 3, CommitmentDomain::Inactive);
        assert!(vars.is_empty());
    }

    #[test]
    fn fixed_domain_sets_value() {
        let units = sample_units();
        let vars = build_thermal_commitment_variables(&units, 1, CommitmentDomain::Fixed(1));
        assert_eq!(vars[0].domain, CommitmentDomain::Fixed(1));
    }
}
