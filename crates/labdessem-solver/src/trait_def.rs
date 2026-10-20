//! Solver interface trait definition.

use crate::{Basis, LpTemplate, RowBatch, SolutionView, SolverError, SolverStatistics};

/// Backend-agnostic interface for LP/MILP solvers.
///
/// Resolved via generic type parameter (monomorphization), not `dyn`,
/// for zero virtual dispatch overhead on the hot path.
///
/// Requires `Send` but not `Sync`: C solver handles maintain mutable
/// internal state that is not thread-safe.
pub trait SolverInterface: Send {
    /// Backend-specific profile type for solver configuration.
    type Profile: Copy + PartialEq + Default + Send;

    /// Apply all profile options to the solver.
    ///
    /// Call before `load_model()` or when changing solver parameters.
    fn apply_profile(&mut self, profile: &Self::Profile);

    /// Load a pre-assembled LP, replacing any previous model.
    ///
    /// The template must pass `validate()` before calling this method.
    /// After load, the solver is ready for `solve()`.
    fn load_model(&mut self, template: &LpTemplate);

    /// Add a batch of constraint rows to the current model.
    ///
    /// Used for iterative cut addition without rebuilding the entire model.
    /// The batch must pass `validate(num_cols)` before calling.
    fn add_rows(&mut self, rows: &RowBatch);

    /// Update row bounds at specified indices.
    ///
    /// # Panics
    ///
    /// - `indices.len() != lower.len()`
    /// - `indices.len() != upper.len()`
    /// - Any index >= current row count
    fn set_row_bounds(&mut self, indices: &[usize], lower: &[f64], upper: &[f64]);

    /// Update column bounds at specified indices.
    ///
    /// # Panics
    ///
    /// - `indices.len() != lower.len()`
    /// - `indices.len() != upper.len()`
    /// - Any index >= current column count
    fn set_col_bounds(&mut self, indices: &[usize], lower: &[f64], upper: &[f64]);

    /// Solve the LP, optionally warm-starting from a basis.
    ///
    /// Returns a view borrowing solver-internal buffers. The view is valid
    /// until the next `&mut self` call.
    ///
    /// # Arguments
    ///
    /// * `basis` - Optional warm-start basis. If provided and compatible,
    ///   the solver uses it as a starting point. Incompatible bases are
    ///   silently ignored (statistics track rejection count).
    ///
    /// # Errors
    ///
    /// Returns `SolverError` on terminal failure conditions:
    /// - `Infeasible`: No feasible solution exists
    /// - `Unbounded`: Objective is unbounded
    /// - `NumericalDifficulty`: Solver encountered numerical issues
    /// - `TimeLimitExceeded`: Wall-clock time limit exceeded
    /// - `IterationLimit`: Simplex iteration limit exceeded
    /// - `InternalError`: Unexpected solver error
    fn solve(&mut self, basis: Option<&Basis>) -> Result<SolutionView<'_>, SolverError>;

    /// Extract current basis into caller-owned buffer.
    ///
    /// Resizes `out` if necessary to match current model dimensions.
    /// Call after a successful `solve()` to capture the basis for warm-start.
    fn get_basis(&mut self, out: &mut Basis);

    /// Return accumulated solve statistics.
    ///
    /// Statistics are cumulative over the solver instance lifetime.
    fn statistics(&self) -> SolverStatistics;

    /// Return backend name (e.g., `"HiGHS"`).
    fn name(&self) -> &'static str;

    /// Return backend name and version (e.g., `"HiGHS 1.8.1"`).
    fn solver_name_version(&self) -> String;
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::BasisStatus;

    #[derive(Debug, Clone, Copy, PartialEq, Default)]
    struct MockProfile {
        time_limit: Option<f64>,
    }

    struct NoopSolver {
        num_cols: usize,
        num_rows: usize,
        primal: Vec<f64>,
        dual: Vec<f64>,
        reduced_costs: Vec<f64>,
        stats: SolverStatistics,
    }

    impl NoopSolver {
        fn new() -> Self {
            Self {
                num_cols: 0,
                num_rows: 0,
                primal: Vec::new(),
                dual: Vec::new(),
                reduced_costs: Vec::new(),
                stats: SolverStatistics::default(),
            }
        }
    }

    impl SolverInterface for NoopSolver {
        type Profile = MockProfile;

        fn apply_profile(&mut self, _profile: &Self::Profile) {}

        fn load_model(&mut self, template: &LpTemplate) {
            self.num_cols = template.num_cols;
            self.num_rows = template.num_rows;
            self.primal = vec![0.0; template.num_cols];
            self.dual = vec![0.0; template.num_rows];
            self.reduced_costs = vec![0.0; template.num_cols];
        }

        fn add_rows(&mut self, rows: &RowBatch) {
            self.num_rows += rows.num_rows;
            self.dual.resize(self.num_rows, 0.0);
        }

        fn set_row_bounds(&mut self, indices: &[usize], lower: &[f64], upper: &[f64]) {
            assert_eq!(indices.len(), lower.len());
            assert_eq!(indices.len(), upper.len());
            for &idx in indices {
                assert!(idx < self.num_rows, "row index out of range");
            }
        }

        fn set_col_bounds(&mut self, indices: &[usize], lower: &[f64], upper: &[f64]) {
            assert_eq!(indices.len(), lower.len());
            assert_eq!(indices.len(), upper.len());
            for &idx in indices {
                assert!(idx < self.num_cols, "column index out of range");
            }
        }

        fn solve(&mut self, _basis: Option<&Basis>) -> Result<SolutionView<'_>, SolverError> {
            self.stats.solve_count += 1;
            self.stats.success_count += 1;
            self.stats.first_try_successes += 1;

            Ok(SolutionView {
                objective: 0.0,
                primal: &self.primal,
                dual: &self.dual,
                reduced_costs: &self.reduced_costs,
                iterations: 0,
                solve_time_seconds: 0.0,
            })
        }

        fn get_basis(&mut self, out: &mut Basis) {
            out.resize(self.num_cols, self.num_rows);
            for status in &mut out.col_status {
                *status = BasisStatus::Lower;
            }
            for status in &mut out.row_status {
                *status = BasisStatus::Basic;
            }
        }

        fn statistics(&self) -> SolverStatistics {
            self.stats.clone()
        }

        fn name(&self) -> &'static str {
            "Noop"
        }

        fn solver_name_version(&self) -> String {
            "Noop 0.0.0".to_string()
        }
    }

    #[test]
    fn noop_solver_implements_trait() {
        fn assert_send<T: Send>() {}
        assert_send::<NoopSolver>();

        fn accepts<S: SolverInterface>(_: &S) {}
        let solver = NoopSolver::new();
        accepts(&solver);
    }

    #[test]
    fn noop_solver_name() {
        let solver = NoopSolver::new();
        assert_eq!(solver.name(), "Noop");
        assert_eq!(solver.solver_name_version(), "Noop 0.0.0");
    }

    #[test]
    fn noop_solver_load_and_solve() {
        use crate::VariableDomain;

        let mut solver = NoopSolver::new();

        let template = LpTemplate {
            num_cols: 2,
            num_rows: 1,
            num_nz: 2,
            col_starts: vec![0, 1, 2],
            row_indices: vec![0, 0],
            values: vec![1.0, 1.0],
            col_lower: vec![0.0, 0.0],
            col_upper: vec![1.0, 1.0],
            objective: vec![1.0, 2.0],
            row_lower: vec![0.0],
            row_upper: vec![1.0],
            col_domains: vec![VariableDomain::Continuous; 2],
        };

        solver.load_model(&template);

        let result = solver.solve(None);
        assert!(result.is_ok());

        let stats = solver.statistics();
        assert_eq!(stats.solve_count, 1);
        assert_eq!(stats.success_count, 1);
    }

    #[test]
    fn noop_solver_get_basis() {
        use crate::VariableDomain;

        let mut solver = NoopSolver::new();

        let template = LpTemplate {
            num_cols: 3,
            num_rows: 2,
            num_nz: 0,
            col_starts: vec![0, 0, 0, 0],
            row_indices: vec![],
            values: vec![],
            col_lower: vec![0.0; 3],
            col_upper: vec![1.0; 3],
            objective: vec![0.0; 3],
            row_lower: vec![0.0; 2],
            row_upper: vec![1.0; 2],
            col_domains: vec![VariableDomain::Continuous; 3],
        };

        solver.load_model(&template);
        let _ = solver.solve(None);

        let mut basis = Basis::new(0, 0);
        solver.get_basis(&mut basis);

        assert_eq!(basis.col_status.len(), 3);
        assert_eq!(basis.row_status.len(), 2);
    }

    #[test]
    fn profile_has_required_bounds() {
        fn assert_profile_bounds<P: Copy + PartialEq + Default + Send>() {}
        assert_profile_bounds::<MockProfile>();
    }
}
