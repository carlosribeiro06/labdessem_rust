//! Hydro commitment variable construction.

use crate::types::{CommitmentDomain, UcVariable};

pub struct HydroUnitInfo<'a> {
    pub plant_name: &'a str,
    pub group_name: &'a str,
    pub unit_name: &'a str,
}

#[must_use]
pub fn build_hydro_commitment_variables(
    units: &[HydroUnitInfo<'_>],
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
                    "hydro_on[p={},g={},u={},t={}]",
                    unit.plant_name,
                    unit.group_name,
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

    fn sample_units() -> Vec<HydroUnitInfo<'static>> {
        vec![
            HydroUnitInfo {
                plant_name: "UHE1",
                group_name: "CJ1",
                unit_name: "UG1",
            },
            HydroUnitInfo {
                plant_name: "UHE1",
                group_name: "CJ1",
                unit_name: "UG2",
            },
        ]
    }

    #[test]
    fn builds_correct_count() {
        let units = sample_units();
        let vars = build_hydro_commitment_variables(&units, 3, CommitmentDomain::Binary);
        assert_eq!(vars.len(), 6);
    }

    #[test]
    fn variable_names_are_correct() {
        let units = sample_units();
        let vars = build_hydro_commitment_variables(&units, 2, CommitmentDomain::Binary);

        assert_eq!(vars[0].name, "hydro_on[p=UHE1,g=CJ1,u=UG1,t=1]");
        assert_eq!(vars[1].name, "hydro_on[p=UHE1,g=CJ1,u=UG1,t=2]");
        assert_eq!(vars[2].name, "hydro_on[p=UHE1,g=CJ1,u=UG2,t=1]");
        assert_eq!(vars[3].name, "hydro_on[p=UHE1,g=CJ1,u=UG2,t=2]");
    }

    #[test]
    fn inactive_domain_returns_empty() {
        let units = sample_units();
        let vars = build_hydro_commitment_variables(&units, 3, CommitmentDomain::Inactive);
        assert!(vars.is_empty());
    }

    #[test]
    fn fixed_domain_sets_value() {
        let units = sample_units();
        let vars = build_hydro_commitment_variables(&units, 1, CommitmentDomain::Fixed(1));
        assert_eq!(vars[0].domain, CommitmentDomain::Fixed(1));
    }
}
