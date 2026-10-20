//! `HiGHS` solver backend via `good_lp`.

use std::time::Instant;

use good_lp::{
    DualValues, Expression, ProblemVariables, Solution, SolutionWithDual, SolverModel, Variable,
    constraint, default_solver, variable,
};

use crate::{
    Basis, BasisStatus, HighsProfile, LpTemplate, RowBatch, SolutionView, SolverError,
    SolverInterface, SolverStatistics, VariableDomain,
};

/// Constraint reference returned by `good_lp` for dual extraction.
type ConstraintRef = good_lp::constraint::ConstraintReference;

/// `HiGHS` solver implementation via `good_lp`.
///
/// Maintains internal buffers for zero-copy solution access through `SolutionView`.
pub struct HighsSolver {
    /// `good_lp` variables (rebuilt on `load_model`)
    variables: Vec<Variable>,

    /// Constraint references for dual extraction
    constraints: Vec<ConstraintRef>,

    /// Current column count
    num_cols: usize,

    /// Current row count
    num_rows: usize,

    /// Loaded template (cached for constraint iteration)
    template_cache: Option<LpTemplateCache>,

    /// Internal primal buffer (length: `num_cols`)
    primal_buffer: Vec<f64>,

    /// Internal dual buffer (length: `num_rows`)
    dual_buffer: Vec<f64>,

    /// Internal reduced cost buffer (length: `num_cols`)
    reduced_cost_buffer: Vec<f64>,

    /// Last objective value
    last_objective: f64,

    /// Last iteration count
    last_iterations: u64,

    /// Last solve time
    last_solve_time: f64,

    /// Accumulated statistics
    statistics: SolverStatistics,

    /// Current profile
    profile: HighsProfile,
}

/// Cached template data for building constraints.
struct LpTemplateCache {
    col_starts: Vec<i32>,
    row_indices: Vec<i32>,
    values: Vec<f64>,
    row_lower: Vec<f64>,
    row_upper: Vec<f64>,
    col_lower: Vec<f64>,
    col_upper: Vec<f64>,
    objective: Vec<f64>,
    col_domains: Vec<VariableDomain>,
    num_cols: usize,
    num_rows: usize,
}

impl HighsSolver {
    /// Creates a new `HiGHS` solver with default profile.
    ///
    /// # Errors
    ///
    /// Returns an error if solver initialization fails (unlikely).
    pub fn new() -> Result<Self, SolverError> {
        Ok(Self {
            variables: Vec::new(),
            constraints: Vec::new(),
            num_cols: 0,
            num_rows: 0,
            template_cache: None,
            primal_buffer: Vec::new(),
            dual_buffer: Vec::new(),
            reduced_cost_buffer: Vec::new(),
            last_objective: 0.0,
            last_iterations: 0,
            last_solve_time: 0.0,
            statistics: SolverStatistics::default(),
            profile: HighsProfile::default(),
        })
    }

    /// Builds and solves the `good_lp` problem from cached template.
    fn build_and_solve(&mut self, use_binary: bool) -> Result<(), SolverError> {
        let cache = self
            .template_cache
            .as_ref()
            .ok_or_else(|| SolverError::InternalError {
                message: "no model loaded".to_string(),
                error_code: None,
            })?;

        let start = Instant::now();

        // Build variables
        let mut problem_vars = ProblemVariables::new();
        self.variables.clear();

        for col in 0..cache.num_cols {
            let lb = cache.col_lower[col];
            let ub = cache.col_upper[col];
            let is_binary = use_binary && cache.col_domains[col] == VariableDomain::Binary;

            let var = if is_binary {
                problem_vars.add(variable().binary())
            } else {
                problem_vars.add(variable().min(lb).max(ub))
            };
            self.variables.push(var);
        }

        // Build objective
        let objective: Expression = cache
            .objective
            .iter()
            .zip(&self.variables)
            .map(|(&coef, &var)| coef * var)
            .sum();

        let mut problem = problem_vars.minimise(objective).using(default_solver);

        // Build constraints from CSC format
        self.constraints.clear();
        for row in 0..cache.num_rows {
            let mut row_terms = Vec::new();

            // Iterate through all columns to find coefficients for this row
            for col in 0..cache.num_cols {
                let col_start = cache.col_starts[col] as usize;
                let col_end = cache.col_starts[col + 1] as usize;

                for k in col_start..col_end {
                    if cache.row_indices[k] as usize == row {
                        row_terms.push((self.variables[col], cache.values[k]));
                    }
                }
            }

            let row_lb = cache.row_lower[row];
            let row_ub = cache.row_upper[row];

            let expr: Expression = row_terms.iter().map(|&(var, coef)| coef * var).sum();

            // Determine constraint type based on bounds
            let is_equal = (row_ub - row_lb).abs() < 1e-10;
            let has_lower = row_lb > f64::NEG_INFINITY;
            let has_upper = row_ub < f64::INFINITY;

            let constraint_ref = if is_equal {
                problem.add_constraint(constraint!(expr == row_lb))
            } else if has_lower && has_upper {
                // Range constraint: add as two-sided (use upper bound form)
                // good_lp doesn't directly support range, so we approximate with equality
                // For now, use <= upper if both exist
                problem.add_constraint(constraint!(expr <= row_ub))
            } else if has_upper {
                problem.add_constraint(constraint!(expr <= row_ub))
            } else if has_lower {
                problem.add_constraint(constraint!(expr >= row_lb))
            } else {
                // Unbounded row - skip (shouldn't happen in practice)
                continue;
            };

            self.constraints.push(constraint_ref);
        }

        // Solve
        let solve_result = problem.solve();

        let elapsed = start.elapsed().as_secs_f64();
        self.last_solve_time = elapsed;
        self.statistics.total_solve_time_seconds += elapsed;

        match solve_result {
            Ok(mut solution) => {
                // Extract primal values
                for (i, &var) in self.variables.iter().enumerate() {
                    self.primal_buffer[i] = solution.value(var);
                }

                // Compute objective
                let obj_expr: Expression = cache
                    .objective
                    .iter()
                    .zip(&self.variables)
                    .map(|(&coef, &var)| coef * var)
                    .sum();
                self.last_objective = solution.eval(obj_expr);

                // Extract dual values (only for LP, not MILP)
                if !use_binary {
                    let dual_values = solution.compute_dual();
                    for (i, constraint_ref) in self.constraints.iter().enumerate() {
                        if i < self.dual_buffer.len() {
                            self.dual_buffer[i] = dual_values.dual(constraint_ref.clone());
                        }
                    }
                } else {
                    // No duals for MILP
                    self.dual_buffer.fill(0.0);
                }

                // Reduced costs not directly available from good_lp, fill with zeros
                self.reduced_cost_buffer.fill(0.0);

                // Iterations not directly available, estimate as 0
                self.last_iterations = 0;
                self.statistics.success_count += 1;
                self.statistics.first_try_successes += 1;

                Ok(())
            }
            Err(error) => {
                self.statistics.failure_count += 1;

                let msg = error.to_string();
                let msg_lower = msg.to_lowercase();
                if msg_lower.contains("infeasible") {
                    Err(SolverError::Infeasible)
                } else if msg_lower.contains("unbounded") {
                    Err(SolverError::Unbounded)
                } else {
                    Err(SolverError::InternalError {
                        message: msg,
                        error_code: None,
                    })
                }
            }
        }
    }
}

impl SolverInterface for HighsSolver {
    type Profile = HighsProfile;

    fn apply_profile(&mut self, profile: &Self::Profile) {
        self.profile = *profile;
        // Note: good_lp doesn't expose direct HiGHS option setting
        // Profile is stored for documentation/future use
    }

    fn load_model(&mut self, template: &LpTemplate) {
        template.validate().expect("invalid LpTemplate");

        self.num_cols = template.num_cols;
        self.num_rows = template.num_rows;

        // Cache template data
        self.template_cache = Some(LpTemplateCache {
            col_starts: template.col_starts.clone(),
            row_indices: template.row_indices.clone(),
            values: template.values.clone(),
            row_lower: template.row_lower.clone(),
            row_upper: template.row_upper.clone(),
            col_lower: template.col_lower.clone(),
            col_upper: template.col_upper.clone(),
            objective: template.objective.clone(),
            col_domains: template.col_domains.clone(),
            num_cols: template.num_cols,
            num_rows: template.num_rows,
        });

        // Resize buffers
        self.primal_buffer.resize(self.num_cols, 0.0);
        self.dual_buffer.resize(self.num_rows, 0.0);
        self.reduced_cost_buffer.resize(self.num_cols, 0.0);
    }

    fn add_rows(&mut self, rows: &RowBatch) {
        // Validate batch against current model
        rows.validate(self.num_cols)
            .expect("invalid RowBatch for current model");

        // Update template cache with new rows
        if let Some(cache) = &mut self.template_cache {
            // Convert CSR batch to extend the CSC template
            // This is complex - for now, we append to row bounds and rebuild
            cache.row_lower.extend_from_slice(&rows.row_lower);
            cache.row_upper.extend_from_slice(&rows.row_upper);

            // Extend CSC matrix by adding new row entries
            for row_local in 0..rows.num_rows {
                let row_global = cache.num_rows + row_local;
                let row_start = rows.row_starts[row_local] as usize;
                let row_end = rows.row_starts[row_local + 1] as usize;

                for k in row_start..row_end {
                    let col = rows.col_indices[k] as usize;
                    let value = rows.values[k];

                    // Insert into CSC format (simple append, not properly sorted)
                    // This works because we rebuild the problem on each solve
                    let insert_pos = cache.col_starts[col + 1] as usize;

                    // Shift all subsequent col_starts
                    for c in (col + 1)..=cache.num_cols {
                        cache.col_starts[c] += 1;
                    }

                    cache.row_indices.insert(insert_pos, row_global as i32);
                    cache.values.insert(insert_pos, value);
                }
            }

            cache.num_rows += rows.num_rows;
        }

        self.num_rows += rows.num_rows;
        self.dual_buffer.resize(self.num_rows, 0.0);
    }

    fn set_row_bounds(&mut self, indices: &[usize], lower: &[f64], upper: &[f64]) {
        assert_eq!(indices.len(), lower.len());
        assert_eq!(indices.len(), upper.len());

        if let Some(cache) = &mut self.template_cache {
            for (i, &idx) in indices.iter().enumerate() {
                assert!(idx < cache.num_rows, "row index out of range");
                cache.row_lower[idx] = lower[i];
                cache.row_upper[idx] = upper[i];
            }
        }
    }

    fn set_col_bounds(&mut self, indices: &[usize], lower: &[f64], upper: &[f64]) {
        assert_eq!(indices.len(), lower.len());
        assert_eq!(indices.len(), upper.len());

        if let Some(cache) = &mut self.template_cache {
            for (i, &idx) in indices.iter().enumerate() {
                assert!(idx < cache.num_cols, "column index out of range");
                cache.col_lower[idx] = lower[i];
                cache.col_upper[idx] = upper[i];
            }
        }
    }

    fn solve(&mut self, basis: Option<&Basis>) -> Result<SolutionView<'_>, SolverError> {
        self.statistics.solve_count += 1;

        if basis.is_some() {
            self.statistics.basis_offered += 1;
            // Note: good_lp doesn't support warm-start basis injection
            // Basis is ignored but counted in statistics
        }

        // Check if any variables are binary
        let has_binary = self
            .template_cache
            .as_ref()
            .map(|c| c.col_domains.iter().any(|&d| d == VariableDomain::Binary))
            .unwrap_or(false);

        self.build_and_solve(has_binary)?;

        Ok(SolutionView {
            objective: self.last_objective,
            primal: &self.primal_buffer,
            dual: &self.dual_buffer,
            reduced_costs: &self.reduced_cost_buffer,
            iterations: self.last_iterations,
            solve_time_seconds: self.last_solve_time,
        })
    }

    fn get_basis(&mut self, out: &mut Basis) {
        out.resize(self.num_cols, self.num_rows);

        // good_lp doesn't expose basis extraction
        // Fill with Lower status as placeholder
        for status in &mut out.col_status {
            *status = BasisStatus::Lower;
        }
        for status in &mut out.row_status {
            *status = BasisStatus::Basic;
        }
    }

    fn statistics(&self) -> SolverStatistics {
        self.statistics.clone()
    }

    fn name(&self) -> &'static str {
        "HiGHS"
    }

    fn solver_name_version(&self) -> String {
        // good_lp doesn't expose HiGHS version
        "HiGHS (via good_lp)".to_string()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn simple_bounded_lp() -> LpTemplate {
        // min x s.t. x >= 1, x <= 10
        LpTemplate {
            num_cols: 1,
            num_rows: 0,
            num_nz: 0,
            col_starts: vec![0, 0],
            row_indices: vec![],
            values: vec![],
            col_lower: vec![1.0],
            col_upper: vec![10.0],
            objective: vec![1.0],
            row_lower: vec![],
            row_upper: vec![],
            col_domains: vec![VariableDomain::Continuous],
        }
    }

    fn two_var_lp() -> LpTemplate {
        // min x + 2y s.t. x + y >= 1, x,y >= 0, x,y <= 10
        // Optimal: x=1, y=0, obj=1
        LpTemplate {
            num_cols: 2,
            num_rows: 1,
            num_nz: 2,
            col_starts: vec![0, 1, 2],
            row_indices: vec![0, 0],
            values: vec![1.0, 1.0],
            col_lower: vec![0.0, 0.0],
            col_upper: vec![10.0, 10.0],
            objective: vec![1.0, 2.0],
            row_lower: vec![1.0],
            row_upper: vec![f64::INFINITY],
            col_domains: vec![VariableDomain::Continuous; 2],
        }
    }

    #[test]
    fn highs_solver_new() {
        let solver = HighsSolver::new().expect("should create solver");
        assert_eq!(solver.name(), "HiGHS");
    }

    #[test]
    fn highs_solver_is_send() {
        fn assert_send<T: Send>() {}
        assert_send::<HighsSolver>();
    }

    #[test]
    fn highs_solver_load_simple_model() {
        let mut solver = HighsSolver::new().unwrap();
        let template = simple_bounded_lp();
        solver.load_model(&template);
        assert_eq!(solver.num_cols, 1);
        assert_eq!(solver.num_rows, 0);
    }

    #[test]
    fn solve_simple_lp() {
        let mut solver = HighsSolver::new().unwrap();
        let template = simple_bounded_lp();
        solver.load_model(&template);

        let sol = solver.solve(None).expect("should solve");

        assert!(
            (sol.objective - 1.0).abs() < 1e-6,
            "objective should be 1.0"
        );
        assert!(
            (sol.primal[0] - 1.0).abs() < 1e-6,
            "x should be at lower bound 1.0"
        );
    }

    #[test]
    fn solve_two_var_lp() {
        let mut solver = HighsSolver::new().unwrap();
        let template = two_var_lp();
        solver.load_model(&template);

        let sol = solver.solve(None).expect("should solve");

        assert!(
            (sol.objective - 1.0).abs() < 1e-6,
            "objective should be 1.0, got {}",
            sol.objective
        );
        assert!(sol.primal[0] >= 0.99, "x should be ~1.0");
    }

    #[test]
    fn solve_tracks_statistics() {
        let mut solver = HighsSolver::new().unwrap();
        let template = simple_bounded_lp();
        solver.load_model(&template);

        let _ = solver.solve(None).unwrap();
        let stats = solver.statistics();

        assert_eq!(stats.solve_count, 1);
        assert_eq!(stats.success_count, 1);
        assert!(stats.total_solve_time_seconds > 0.0);
    }

    #[test]
    fn solver_name_version() {
        let solver = HighsSolver::new().unwrap();
        assert!(solver.solver_name_version().contains("HiGHS"));
    }

    #[test]
    fn get_basis_returns_valid_sizes() {
        let mut solver = HighsSolver::new().unwrap();
        let template = two_var_lp();
        solver.load_model(&template);
        let _ = solver.solve(None).unwrap();

        let mut basis = Basis::new(0, 0);
        solver.get_basis(&mut basis);

        assert_eq!(basis.col_status.len(), 2);
        assert_eq!(basis.row_status.len(), 1);
    }
}
