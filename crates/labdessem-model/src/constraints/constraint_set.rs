//! Constraint set builder and accessors.

use labdessem_core::system::System;

use crate::{indexing::Indexing, variables::Variables, SolveMode};

use super::{
    build_demand_balance_constraints, build_future_cost_constraints,
    build_hydro_balance_constraints, build_hydro_commitment_channeling_constraints,
    build_hydro_fpha_constraints, build_hydro_generation_turbining_coupling_constraints,
    build_hydro_productivity_constraints, build_hydro_spillage_nonnegativity_constraints,
    build_hydro_turbining_limit_constraints, build_interchange_limit_constraints,
    build_operational_limit_constraints, build_thermal_min_up_down_constraints,
    build_thermal_ramp_constraints, ConstraintSense, LinearConstraint,
};

#[derive(Debug, Clone, PartialEq)]
pub struct ConstraintSet {
    pub linear_constraints: Vec<LinearConstraint>,
}

impl ConstraintSet {
    pub fn for_system(
        system: &System,
        indexing: &Indexing,
        variables: &Variables,
        solve_mode: SolveMode,
    ) -> Self {
        let mut linear_constraints = build_demand_balance_constraints(system, indexing, variables);
        linear_constraints.extend(build_hydro_balance_constraints(system, indexing, variables));
        linear_constraints.extend(build_interchange_limit_constraints(
            system, indexing, variables,
        ));
        linear_constraints.extend(build_hydro_turbining_limit_constraints(
            system, indexing, variables,
        ));
        linear_constraints.extend(build_hydro_spillage_nonnegativity_constraints(
            indexing, variables, system,
        ));
        if system.fpha_enabled {
            linear_constraints.extend(build_hydro_fpha_constraints(system, indexing, variables));
            linear_constraints.extend(build_hydro_generation_turbining_coupling_constraints(
                system, indexing, variables,
            ));
        } else {
            linear_constraints.extend(build_hydro_productivity_constraints(
                system, indexing, variables,
            ));
        }
        if system.future_cost_enabled {
            linear_constraints.extend(build_future_cost_constraints(system, indexing, variables));
        }
        linear_constraints.extend(build_operational_limit_constraints(
            system, indexing, variables,
        ));

        if matches!(
            solve_mode,
            SolveMode::MixedIntegerLinearProgramming
                | SolveMode::LinearProgrammingWithFixedCommitment
        ) {
            if system.thermal_unit_commitment_enabled {
                linear_constraints.extend(build_thermal_min_up_down_constraints(
                    system, indexing, variables,
                ));
                linear_constraints
                    .extend(build_thermal_ramp_constraints(system, indexing, variables));
            }
            if system.hydro_unit_commitment_enabled {
                linear_constraints.extend(build_hydro_commitment_channeling_constraints(
                    system, indexing, variables,
                ));
            }
        }

        linear_constraints.push(LinearConstraint {
            name: "linearized_network_flow".into(),
            terms: Vec::new(),
            sense: ConstraintSense::Equal,
            rhs: 0.0,
        });

        if matches!(
            solve_mode,
            SolveMode::MixedIntegerLinearProgramming
                | SolveMode::LinearProgrammingWithFixedCommitment
        ) && (system.thermal_unit_commitment_enabled || system.hydro_unit_commitment_enabled)
        {
            linear_constraints.push(LinearConstraint {
                name: "unit_commitment".into(),
                terms: Vec::new(),
                sense: ConstraintSense::Equal,
                rhs: 0.0,
            });
        }

        Self { linear_constraints }
    }

    pub fn names(&self) -> Vec<&str> {
        self.linear_constraints
            .iter()
            .map(|constraint| constraint.name.as_str())
            .collect()
    }

    pub fn demand_balance(&self) -> Vec<&LinearConstraint> {
        self.linear_constraints
            .iter()
            .filter(|constraint| constraint.name.starts_with("demand_balance["))
            .collect()
    }

    pub fn channeling(&self) -> Vec<&LinearConstraint> {
        self.linear_constraints
            .iter()
            .filter(|constraint| constraint.name.starts_with("channeling_"))
            .collect()
    }

    pub fn thermal_min_up_down(&self) -> Vec<&LinearConstraint> {
        self.linear_constraints
            .iter()
            .filter(|constraint| {
                constraint.name.starts_with("thermal_min_up[")
                    || constraint.name.starts_with("thermal_min_down[")
                    || constraint.name.starts_with("thermal_initial_")
            })
            .collect()
    }

    pub fn thermal_ramps(&self) -> Vec<&LinearConstraint> {
        self.linear_constraints
            .iter()
            .filter(|constraint| {
                constraint.name.starts_with("thermal_transition[")
                    || constraint
                        .name
                        .starts_with("thermal_startup_shutdown_exclusive[")
                    || constraint.name.starts_with("thermal_ramp_")
            })
            .collect()
    }

    pub fn hydro_balance(&self) -> Vec<&LinearConstraint> {
        self.linear_constraints
            .iter()
            .filter(|constraint| constraint.name.starts_with("hydro_balance["))
            .collect()
    }

    pub fn interchange_limits(&self) -> Vec<&LinearConstraint> {
        self.linear_constraints
            .iter()
            .filter(|constraint| constraint.name.starts_with("interchange_limit["))
            .collect()
    }

    pub fn hydro_turbining_limits(&self) -> Vec<&LinearConstraint> {
        self.linear_constraints
            .iter()
            .filter(|constraint| constraint.name.starts_with("hydro_turbining_"))
            .collect()
    }

    pub fn hydro_spillage_nonnegativity(&self) -> Vec<&LinearConstraint> {
        self.linear_constraints
            .iter()
            .filter(|constraint| constraint.name.starts_with("hydro_spillage_nonnegative["))
            .collect()
    }

    pub fn hydro_fpha(&self) -> Vec<&LinearConstraint> {
        self.linear_constraints
            .iter()
            .filter(|constraint| constraint.name.starts_with("hydro_fpha["))
            .collect()
    }

    pub fn hydro_generation_turbining_coupling(&self) -> Vec<&LinearConstraint> {
        self.linear_constraints
            .iter()
            .filter(|constraint| {
                constraint
                    .name
                    .starts_with("hydro_generation_turbining_coupling[")
            })
            .collect()
    }

    pub fn hydro_productivity(&self) -> Vec<&LinearConstraint> {
        self.linear_constraints
            .iter()
            .filter(|constraint| constraint.name.starts_with("hydro_productivity["))
            .collect()
    }

    pub fn future_cost(&self) -> Vec<&LinearConstraint> {
        self.linear_constraints
            .iter()
            .filter(|constraint| constraint.name.starts_with("future_cost_cut["))
            .collect()
    }

    pub fn operational_limits(&self) -> Vec<&LinearConstraint> {
        self.linear_constraints
            .iter()
            .filter(|constraint| constraint.name.starts_with("operational_limit_"))
            .collect()
    }
}
