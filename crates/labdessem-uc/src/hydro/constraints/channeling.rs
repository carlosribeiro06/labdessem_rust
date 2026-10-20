//! Hydro commitment channeling constraint construction.

use crate::types::{UcConstraint, UcTerm, UcVariable};

pub struct HydroUnitChannelingInfo<'a> {
    pub plant_name: &'a str,
    pub group_name: &'a str,
    pub unit_name: &'a str,
    pub min_generation_mw: f64,
    pub max_generation_mw: f64,
}

#[must_use]
pub fn build_hydro_channeling_constraints(
    units: &[HydroUnitChannelingInfo<'_>],
    generation_var_names: &[String],
    commitment_vars: &[UcVariable],
    horizon: usize,
) -> Vec<UcConstraint> {
    if commitment_vars.is_empty() || generation_var_names.is_empty() {
        return Vec::new();
    }

    let mut constraints = Vec::with_capacity(units.len() * horizon * 2);

    for (unit_idx, unit) in units.iter().enumerate() {
        for period in 0..horizon {
            let var_idx = unit_idx * horizon + period;
            let gen_var_name = &generation_var_names[var_idx];
            let on_var = &commitment_vars[var_idx];

            constraints.push(UcConstraint::leq(
                format!(
                    "channeling_hydro_upper[p={},g={},u={},t={}]",
                    unit.plant_name,
                    unit.group_name,
                    unit.unit_name,
                    period + 1
                ),
                vec![
                    UcTerm::new(gen_var_name, 1.0),
                    UcTerm::new(&on_var.name, -unit.max_generation_mw),
                ],
                0.0,
            ));

            constraints.push(UcConstraint::geq(
                format!(
                    "channeling_hydro_lower[p={},g={},u={},t={}]",
                    unit.plant_name,
                    unit.group_name,
                    unit.unit_name,
                    period + 1
                ),
                vec![
                    UcTerm::new(gen_var_name, 1.0),
                    UcTerm::new(&on_var.name, -unit.min_generation_mw),
                ],
                0.0,
            ));
        }
    }

    constraints
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::hydro::variables::{HydroUnitInfo, build_hydro_commitment_variables};
    use crate::types::CommitmentDomain;

    fn sample_unit() -> HydroUnitChannelingInfo<'static> {
        HydroUnitChannelingInfo {
            plant_name: "UHE1",
            group_name: "CJ1",
            unit_name: "UG1",
            min_generation_mw: 50.0,
            max_generation_mw: 200.0,
        }
    }

    fn gen_var_names(horizon: usize) -> Vec<String> {
        (0..horizon)
            .map(|t| format!("hydro_generation[p=UHE1,g=CJ1,u=UG1,t={}]", t + 1))
            .collect()
    }

    fn commitment_vars(horizon: usize) -> Vec<UcVariable> {
        let units = vec![HydroUnitInfo {
            plant_name: "UHE1",
            group_name: "CJ1",
            unit_name: "UG1",
        }];
        build_hydro_commitment_variables(&units, horizon, CommitmentDomain::Binary)
    }

    #[test]
    fn builds_two_constraints_per_period() {
        let units = vec![sample_unit()];
        let gen_names = gen_var_names(3);
        let commitment = commitment_vars(3);

        let constraints = build_hydro_channeling_constraints(&units, &gen_names, &commitment, 3);

        assert_eq!(constraints.len(), 6);
    }

    #[test]
    fn upper_and_lower_constraints_alternate() {
        let units = vec![sample_unit()];
        let gen_names = gen_var_names(1);
        let commitment = commitment_vars(1);

        let constraints = build_hydro_channeling_constraints(&units, &gen_names, &commitment, 1);

        assert!(constraints[0].name.contains("channeling_hydro_upper"));
        assert!(constraints[1].name.contains("channeling_hydro_lower"));
    }

    #[test]
    fn upper_bound_uses_max_generation() {
        let units = vec![sample_unit()];
        let gen_names = gen_var_names(1);
        let commitment = commitment_vars(1);

        let constraints = build_hydro_channeling_constraints(&units, &gen_names, &commitment, 1);

        let upper = &constraints[0];
        assert_eq!(upper.terms[1].coefficient, -200.0);
    }

    #[test]
    fn lower_bound_uses_min_generation() {
        let units = vec![sample_unit()];
        let gen_names = gen_var_names(1);
        let commitment = commitment_vars(1);

        let constraints = build_hydro_channeling_constraints(&units, &gen_names, &commitment, 1);

        let lower = &constraints[1];
        assert_eq!(lower.terms[1].coefficient, -50.0);
    }

    #[test]
    fn empty_vars_returns_empty() {
        let units = vec![sample_unit()];
        let gen_names = gen_var_names(3);

        let constraints = build_hydro_channeling_constraints(&units, &gen_names, &[], 3);

        assert!(constraints.is_empty());
    }
}
