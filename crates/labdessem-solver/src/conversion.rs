//! Model to `LpTemplate` conversion.

use std::collections::HashMap;

use labdessem_model::{
    constraints::ConstraintSense,
    variables::{Variable, VariableDomain as ModelVariableDomain},
    Model,
};

use crate::{LpTemplate, VariableDomain};

/// Convert a `Model` to a pre-assembled `LpTemplate` in CSC format.
///
/// The template is ready to be loaded into a `SolverInterface` implementation.
pub fn model_to_lp_template(model: &Model) -> LpTemplate {
    let all_variables = collect_variables(model);
    let num_cols = all_variables.len();
    let num_rows = model.constraints.linear_constraints.len();

    // Build name → column index mapping
    let var_name_to_col: HashMap<&str, usize> = all_variables
        .iter()
        .enumerate()
        .map(|(idx, var)| (var.name.as_str(), idx))
        .collect();

    // Build column bounds and domains
    let mut col_lower = Vec::with_capacity(num_cols);
    let mut col_upper = Vec::with_capacity(num_cols);
    let mut col_domains = Vec::with_capacity(num_cols);
    let mut objective = vec![0.0; num_cols];

    for var in &all_variables {
        let lb = var.fixed_value.unwrap_or(var.lower_bound);
        let ub = var
            .fixed_value
            .unwrap_or_else(|| var.upper_bound.unwrap_or(f64::INFINITY));
        col_lower.push(lb);
        col_upper.push(ub);

        let domain = match var.domain {
            ModelVariableDomain::Continuous => VariableDomain::Continuous,
            ModelVariableDomain::Binary => VariableDomain::Binary,
        };
        col_domains.push(domain);
    }

    // Fill objective coefficients
    for term in &model.objective.terms {
        if let Some(&col) = var_name_to_col.get(term.variable.as_str()) {
            objective[col] = term.coefficient;
        }
    }

    // Build row bounds from constraints
    let mut row_lower = Vec::with_capacity(num_rows);
    let mut row_upper = Vec::with_capacity(num_rows);

    for constraint in &model.constraints.linear_constraints {
        let (lb, ub) = match constraint.sense {
            ConstraintSense::Equal => (constraint.rhs, constraint.rhs),
            ConstraintSense::LessOrEqual => (f64::NEG_INFINITY, constraint.rhs),
            ConstraintSense::GreaterOrEqual => (constraint.rhs, f64::INFINITY),
        };
        row_lower.push(lb);
        row_upper.push(ub);
    }

    // Build constraint matrix in COO format then convert to CSC
    let mut coo_entries: Vec<(usize, usize, f64)> = Vec::new();

    for (row_idx, constraint) in model.constraints.linear_constraints.iter().enumerate() {
        for term in &constraint.terms {
            if let Some(&col_idx) = var_name_to_col.get(term.variable.as_str()) {
                coo_entries.push((row_idx, col_idx, term.coefficient));
            }
        }
    }

    // Convert COO to CSC by sorting column-major
    coo_entries.sort_by_key(|&(row, col, _)| (col, row));

    let num_nz = coo_entries.len();
    let mut col_starts = vec![0i32; num_cols + 1];
    let mut row_indices = Vec::with_capacity(num_nz);
    let mut values = Vec::with_capacity(num_nz);

    let mut current_col = 0;
    for &(row, col, val) in &coo_entries {
        while current_col < col {
            current_col += 1;
            col_starts[current_col] = row_indices.len() as i32;
        }
        row_indices.push(row as i32);
        values.push(val);
    }
    while current_col < num_cols {
        current_col += 1;
        col_starts[current_col] = row_indices.len() as i32;
    }

    LpTemplate {
        num_cols,
        num_rows,
        num_nz,
        col_starts,
        row_indices,
        values,
        col_lower,
        col_upper,
        objective,
        row_lower,
        row_upper,
        col_domains,
    }
}

fn collect_variables(model: &Model) -> Vec<&Variable> {
    let vars = &model.variables;
    let mut all = Vec::new();
    all.extend(vars.thermal_generation.iter());
    all.extend(vars.hydro_generation.iter());
    all.extend(vars.hydro_turbining.iter());
    all.extend(vars.hydro_spillage.iter());
    all.extend(vars.hydro_diversion.iter());
    all.extend(vars.pumping.iter());
    all.extend(vars.hydro_volume.iter());
    all.extend(vars.future_cost.iter());
    all.extend(vars.deficit.iter());
    all.extend(vars.renewable_generation.iter());
    all.extend(vars.interchange.iter());
    all.extend(vars.thermal_commitment.iter());
    all.extend(vars.thermal_startup.iter());
    all.extend(vars.thermal_shutdown.iter());
    all.extend(vars.hydro_commitment.iter());
    all.extend(vars.network_flow_slack.iter());
    all.extend(vars.operational_limit_slack.iter());
    all
}

#[cfg(test)]
mod tests {
    use super::*;
    use labdessem_io::read_study_from_path;
    use labdessem_model::SolveMode;
    use std::path::PathBuf;

    fn examples_24barras_path() -> PathBuf {
        PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../examples/24Barras")
    }

    #[test]
    fn model_to_template_validates() {
        let system = read_study_from_path(examples_24barras_path()).expect("should read case");
        let model = Model::from_system(&system, SolveMode::LinearProgramming);

        let template = model_to_lp_template(&model);

        assert!(
            template.validate().is_ok(),
            "template should validate: {:?}",
            template.validate()
        );
    }

    #[test]
    fn model_to_template_dimensions() {
        let system = read_study_from_path(examples_24barras_path()).expect("should read case");
        let model = Model::from_system(&system, SolveMode::LinearProgramming);

        let template = model_to_lp_template(&model);

        assert!(template.num_cols > 0, "should have columns");
        assert!(template.num_rows > 0, "should have rows");
        assert!(template.num_nz > 0, "should have non-zeros");
    }
}
