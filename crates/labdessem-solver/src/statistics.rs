//! Solver performance statistics.

/// Accumulated metrics from solver operations.
///
/// All counters are monotonically increasing over the lifetime of a solver instance.
/// Used for performance diagnosis and warm-start effectiveness tracking.
#[derive(Debug, Clone, Default)]
pub struct SolverStatistics {
    /// Total number of `solve()` calls.
    pub solve_count: u64,

    /// Solves that returned `Ok` (optimal found).
    pub success_count: u64,

    /// Solves that returned `Err` (terminal failure).
    pub failure_count: u64,

    /// Total simplex iterations across all solves.
    pub total_iterations: u64,

    /// Total retry attempts (for future retry escalation).
    pub retry_count: u64,

    /// Cumulative wall-clock time in solver (seconds).
    pub total_solve_time_seconds: f64,

    /// Solves that succeeded on first attempt (no retry).
    pub first_try_successes: u64,

    /// Number of warm-start basis offers.
    pub basis_offered: u64,

    /// Basis offers rejected for inconsistency.
    pub basis_consistency_failures: u64,
}

impl SolverStatistics {
    /// Copy all fields from `src` into `self`.
    pub fn copy_from(&mut self, src: &Self) {
        self.solve_count = src.solve_count;
        self.success_count = src.success_count;
        self.failure_count = src.failure_count;
        self.total_iterations = src.total_iterations;
        self.retry_count = src.retry_count;
        self.total_solve_time_seconds = src.total_solve_time_seconds;
        self.first_try_successes = src.first_try_successes;
        self.basis_offered = src.basis_offered;
        self.basis_consistency_failures = src.basis_consistency_failures;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn statistics_default_is_zero() {
        let stats = SolverStatistics::default();
        assert_eq!(stats.solve_count, 0);
        assert_eq!(stats.success_count, 0);
        assert_eq!(stats.failure_count, 0);
        assert_eq!(stats.total_iterations, 0);
        assert_eq!(stats.retry_count, 0);
        assert!((stats.total_solve_time_seconds - 0.0).abs() < f64::EPSILON);
        assert_eq!(stats.first_try_successes, 0);
        assert_eq!(stats.basis_offered, 0);
        assert_eq!(stats.basis_consistency_failures, 0);
    }

    #[test]
    fn statistics_copy_from() {
        let mut dst = SolverStatistics::default();
        let src = SolverStatistics {
            solve_count: 10,
            success_count: 9,
            failure_count: 1,
            total_iterations: 500,
            retry_count: 2,
            total_solve_time_seconds: 1.5,
            first_try_successes: 8,
            basis_offered: 5,
            basis_consistency_failures: 1,
        };

        dst.copy_from(&src);

        assert_eq!(dst.solve_count, 10);
        assert_eq!(dst.success_count, 9);
        assert_eq!(dst.failure_count, 1);
        assert_eq!(dst.total_iterations, 500);
        assert_eq!(dst.retry_count, 2);
        assert!((dst.total_solve_time_seconds - 1.5).abs() < f64::EPSILON);
        assert_eq!(dst.first_try_successes, 8);
        assert_eq!(dst.basis_offered, 5);
        assert_eq!(dst.basis_consistency_failures, 1);
    }

    #[test]
    fn statistics_is_clone() {
        let src = SolverStatistics {
            solve_count: 42,
            ..Default::default()
        };
        let cloned = src.clone();
        assert_eq!(cloned.solve_count, 42);
    }
}
