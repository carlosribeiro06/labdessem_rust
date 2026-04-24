use std::{collections::HashMap, error::Error, fmt, path::Path};

use good_lp::{
    DualValues, Expression, ProblemVariables, Solution, SolutionWithDual, SolverModel, Variable,
    constraint, constraint::ConstraintReference, default_solver, variable,
};
use labdessem_io::IoError;
use labdessem_model::{
    Model, SolveMode,
    constraints::{ConstraintSense, LinearConstraint},
    variables::VariableDomain,
};

#[derive(Debug)]
pub enum SolverError {
    UnsupportedSolveMode(SolveMode),
    UnknownVariable(String),
    InfeasibleOrUnbounded(String),
    Io(IoError),
}

impl fmt::Display for SolverError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::UnsupportedSolveMode(mode) => {
                write!(
                    f,
                    "solve mode {:?} is not supported by the current LP solver",
                    mode
                )
            }
            Self::UnknownVariable(name) => write!(f, "unknown variable in solver mapping: {name}"),
            Self::InfeasibleOrUnbounded(message) => write!(f, "{message}"),
            Self::Io(error) => write!(f, "{error}"),
        }
    }
}

impl Error for SolverError {
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        match self {
            Self::Io(error) => Some(error),
            Self::UnsupportedSolveMode(_)
            | Self::UnknownVariable(_)
            | Self::InfeasibleOrUnbounded(_) => None,
        }
    }
}

impl From<IoError> for SolverError {
    fn from(value: IoError) -> Self {
        Self::Io(value)
    }
}

#[derive(Debug, Clone)]
pub struct SolveSummary {
    pub objective_value: f64,
    pub variable_values: HashMap<String, f64>,
    pub constraint_duals: HashMap<String, f64>,
}

pub fn solve_model(model: &Model) -> Result<SolveSummary, SolverError> {
    match model.solve_mode {
        SolveMode::LinearProgramming => solve_problem(model, false),
        SolveMode::MixedIntegerLinearProgramming => solve_problem(model, true),
        SolveMode::LinearProgrammingWithFixedCommitment => solve_problem(model, false),
    }
}

pub fn solve_lp(model: &Model) -> Result<SolveSummary, SolverError> {
    solve_model(model)
}

pub fn solve_milp(model: &Model) -> Result<SolveSummary, SolverError> {
    solve_model(model)
}

pub fn solve_lp_from_config(config_path: impl AsRef<Path>) -> Result<SolveSummary, SolverError> {
    let system = labdessem_io::read_study_from_config(config_path)?;
    let model = Model::from_system(&system, SolveMode::LinearProgramming);
    solve_model(&model)
}

pub fn solve_milp_from_config(config_path: impl AsRef<Path>) -> Result<SolveSummary, SolverError> {
    let system = labdessem_io::read_study_from_config(config_path)?;
    let model = Model::from_system(&system, SolveMode::MixedIntegerLinearProgramming);
    solve_model(&model)
}

fn solve_problem(model: &Model, enforce_binary_domains: bool) -> Result<SolveSummary, SolverError> {
    let all_variables = collect_variables(model);
    let mut problem_variables = ProblemVariables::new();
    let mut variable_map = HashMap::<String, Variable>::new();

    for variable_definition in &all_variables {
        let mut builder =
            if enforce_binary_domains && variable_definition.domain == VariableDomain::Binary {
                variable().binary()
            } else {
                variable()
            };
        let fixed_value = variable_definition.fixed_value;

        if let Some(fixed_value) = fixed_value {
            builder = builder.min(fixed_value).max(fixed_value);
        } else {
            builder = builder.min(variable_definition.lower_bound);
            if let Some(upper_bound) = variable_definition.upper_bound {
                builder = builder.max(upper_bound);
            }
        }

        let variable = problem_variables.add(builder);
        variable_map.insert(variable_definition.name.clone(), variable);
    }

    let objective = build_expression(&model.objective.terms, &variable_map)?;
    let mut problem = problem_variables.minimise(objective).using(default_solver);
    let mut constraint_references = Vec::<(String, ConstraintReference)>::new();

    for linear_constraint in &model.constraints.linear_constraints {
        let constraint_reference = add_constraint(&mut problem, linear_constraint, &variable_map)?;
        constraint_references.push((linear_constraint.name.clone(), constraint_reference));
    }

    let mut solution = problem.solve().map_err(|error| {
        SolverError::InfeasibleOrUnbounded(format!("LP relaxation solve failed: {error}"))
    })?;

    let objective_value = solution.eval(build_expression(&model.objective.terms, &variable_map)?);
    let mut variable_values = HashMap::new();
    for variable_definition in &all_variables {
        let variable = variable_map
            .get(&variable_definition.name)
            .ok_or_else(|| SolverError::UnknownVariable(variable_definition.name.clone()))?;
        variable_values.insert(variable_definition.name.clone(), solution.value(*variable));
    }
    let constraint_duals = if enforce_binary_domains {
        HashMap::new()
    } else {
        let dual_values = solution.compute_dual();
        constraint_references
            .into_iter()
            .map(|(name, reference)| (name, dual_values.dual(reference)))
            .collect()
    };

    Ok(SolveSummary {
        objective_value,
        variable_values,
        constraint_duals,
    })
}

fn add_constraint<M: SolverModel>(
    problem: &mut M,
    linear_constraint: &LinearConstraint,
    variable_map: &HashMap<String, Variable>,
) -> Result<ConstraintReference, SolverError> {
    let expression = build_expression(&linear_constraint.terms, variable_map)?;
    let constraint_reference = match linear_constraint.sense {
        ConstraintSense::Equal => {
            problem.add_constraint(constraint!(expression == linear_constraint.rhs))
        }
        ConstraintSense::LessOrEqual => {
            problem.add_constraint(constraint!(expression <= linear_constraint.rhs))
        }
        ConstraintSense::GreaterOrEqual => {
            problem.add_constraint(constraint!(expression >= linear_constraint.rhs))
        }
    };

    Ok(constraint_reference)
}

fn build_expression<T>(
    terms: &[T],
    variable_map: &HashMap<String, Variable>,
) -> Result<Expression, SolverError>
where
    T: LinearLikeTerm,
{
    let mut expression = Expression::from(0.0);
    for term in terms {
        let variable = variable_map
            .get(term.variable_name())
            .ok_or_else(|| SolverError::UnknownVariable(term.variable_name().to_string()))?;
        expression += term.coefficient() * *variable;
    }

    Ok(expression)
}

fn collect_variables(model: &Model) -> Vec<&labdessem_model::variables::Variable> {
    let variables = &model.variables;
    let mut all = Vec::new();
    all.extend(variables.thermal_generation.iter());
    all.extend(variables.hydro_generation.iter());
    all.extend(variables.hydro_turbining.iter());
    all.extend(variables.hydro_spillage.iter());
    all.extend(variables.hydro_diversion.iter());
    all.extend(variables.pumping.iter());
    all.extend(variables.hydro_volume.iter());
    all.extend(variables.deficit.iter());
    all.extend(variables.renewable_generation.iter());
    all.extend(variables.interchange.iter());
    all.extend(variables.thermal_commitment.iter());
    all.extend(variables.thermal_startup.iter());
    all.extend(variables.thermal_shutdown.iter());
    all.extend(variables.hydro_commitment.iter());
    all.extend(variables.hydro_startup.iter());
    all.extend(variables.hydro_shutdown.iter());
    all.extend(variables.network_flow_slack.iter());
    all.extend(variables.operational_limit_slack.iter());
    all
}

trait LinearLikeTerm {
    fn variable_name(&self) -> &str;
    fn coefficient(&self) -> f64;
}

impl LinearLikeTerm for labdessem_model::constraints::LinearTerm {
    fn variable_name(&self) -> &str {
        &self.variable
    }

    fn coefficient(&self) -> f64 {
        self.coefficient
    }
}

impl LinearLikeTerm for labdessem_model::objective::ObjectiveTerm {
    fn variable_name(&self) -> &str {
        &self.variable
    }

    fn coefficient(&self) -> f64 {
        self.coefficient
    }
}

#[cfg(test)]
mod tests {
    use std::path::PathBuf;

    use super::{solve_lp_from_config, solve_milp_from_config};

    #[test]
    fn solves_lp_for_example_case() {
        let config_path =
            PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../labdessem-io/study_config.json");

        let result = solve_lp_from_config(config_path).expect("LP should solve for example case");

        assert!(result.objective_value >= 0.0);
        assert!(
            result
                .variable_values
                .keys()
                .any(|name| name.starts_with("thermal_generation["))
        );
        assert!(
            result
                .variable_values
                .keys()
                .any(|name| name.starts_with("hydro_volume["))
        );
        assert!(
            result
                .constraint_duals
                .keys()
                .any(|name| name.starts_with("demand_balance["))
        );
    }

    #[test]
    fn solves_milp_for_example_case() {
        let config_path =
            PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../labdessem-io/study_config.json");

        let result =
            solve_milp_from_config(config_path).expect("MILP should solve for example case");

        assert!(result.objective_value >= 0.0);
        if let Some(thermal_on) = result
            .variable_values
            .iter()
            .find_map(|(name, value)| name.starts_with("thermal_on[").then_some(*value))
        {
            assert!(thermal_on == 0.0 || thermal_on == 1.0);
        }
        assert!(result.constraint_duals.is_empty());
    }
}
