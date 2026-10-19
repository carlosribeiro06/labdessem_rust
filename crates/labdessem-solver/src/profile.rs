//! Solver profile configuration.

/// Configuration profile for `HiGHS` solver options.
///
/// Implements the bounds required by `SolverInterface::Profile`:
/// `Copy + PartialEq + Default + Send`.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct HighsProfile {
    /// Primal feasibility tolerance (default: 1e-7).
    pub primal_feasibility_tolerance: f64,

    /// Dual feasibility tolerance (default: 1e-7).
    pub dual_feasibility_tolerance: f64,

    /// Simplex iteration limit (0 = solver heuristic).
    pub simplex_iteration_limit: u32,

    /// Scaling strategy (0 = off, 1 = equilibration, 2 = max value).
    pub simplex_scale_strategy: i32,

    /// Time limit in seconds (0 = unlimited).
    pub time_limit: f64,

    /// Presolve mode (0 = off, 1 = on, 2 = choose).
    pub presolve: i32,

    /// Output log to console (0 = off, 1 = on).
    pub output_flag: bool,
}

impl Default for HighsProfile {
    fn default() -> Self {
        Self {
            primal_feasibility_tolerance: 1e-7,
            dual_feasibility_tolerance: 1e-7,
            simplex_iteration_limit: 0,
            simplex_scale_strategy: 1, // equilibration scaling
            time_limit: 0.0,           // unlimited
            presolve: 2,               // let HiGHS choose
            output_flag: false,        // suppress solver output
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn profile_is_copy() {
        let p1 = HighsProfile::default();
        let p2 = p1; // Copy
        assert_eq!(p1, p2);
    }

    #[test]
    fn profile_is_send() {
        fn assert_send<T: Send>() {}
        assert_send::<HighsProfile>();
    }

    #[test]
    fn profile_default_tolerances() {
        let p = HighsProfile::default();
        assert!(p.primal_feasibility_tolerance < 1e-6);
        assert!(p.dual_feasibility_tolerance < 1e-6);
    }

    #[test]
    fn profile_default_values() {
        let p = HighsProfile::default();
        assert_eq!(p.simplex_iteration_limit, 0);
        assert_eq!(p.simplex_scale_strategy, 1);
        assert!((p.time_limit - 0.0).abs() < f64::EPSILON);
        assert_eq!(p.presolve, 2);
        assert!(!p.output_flag);
    }
}
