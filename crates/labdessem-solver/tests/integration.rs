//! Integration tests for `HighsSolver`.

use labdessem_solver::{
    Basis, HighsProfile, HighsSolver, LpTemplate, RowBatch, SolverInterface, VariableDomain,
};

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

fn binary_lp() -> LpTemplate {
    // min x s.t. x binary
    // Optimal: x = 0, obj = 0
    LpTemplate {
        num_cols: 1,
        num_rows: 0,
        num_nz: 0,
        col_starts: vec![0, 0],
        row_indices: vec![],
        values: vec![],
        col_lower: vec![0.0],
        col_upper: vec![1.0],
        objective: vec![1.0],
        row_lower: vec![],
        row_upper: vec![],
        col_domains: vec![VariableDomain::Binary],
    }
}

fn two_var_lp() -> LpTemplate {
    // min x + 2y s.t. x + y >= 1, x,y >= 0
    // Optimal: x = 1, y = 0, obj = 1
    LpTemplate {
        num_cols: 2,
        num_rows: 1,
        num_nz: 2,
        col_starts: vec![0, 1, 2],
        row_indices: vec![0, 0],
        values: vec![1.0, 1.0],
        col_lower: vec![0.0, 0.0],
        col_upper: vec![100.0, 100.0],
        objective: vec![1.0, 2.0],
        row_lower: vec![1.0],
        row_upper: vec![f64::INFINITY],
        col_domains: vec![VariableDomain::Continuous; 2],
    }
}

fn infeasible_lp() -> LpTemplate {
    // min x s.t. x >= 10, x <= 5 (infeasible)
    LpTemplate {
        num_cols: 1,
        num_rows: 0,
        num_nz: 0,
        col_starts: vec![0, 0],
        row_indices: vec![],
        values: vec![],
        col_lower: vec![10.0],
        col_upper: vec![5.0],
        objective: vec![1.0],
        row_lower: vec![],
        row_upper: vec![],
        col_domains: vec![VariableDomain::Continuous],
    }
}

#[test]
fn solve_simple_bounded_lp() {
    let mut solver = HighsSolver::new().expect("should create solver");
    solver.load_model(&simple_bounded_lp());

    let sol = solver.solve(None).expect("should solve");

    assert!(
        (sol.objective - 1.0).abs() < 1e-6,
        "objective should be 1.0, got {}",
        sol.objective
    );
    assert!(
        (sol.primal[0] - 1.0).abs() < 1e-6,
        "x should be 1.0, got {}",
        sol.primal[0]
    );
}

#[test]
fn solve_binary_lp() {
    let mut solver = HighsSolver::new().expect("should create solver");
    solver.load_model(&binary_lp());

    let sol = solver.solve(None).expect("should solve");

    assert!(
        sol.objective.abs() < 1e-6,
        "objective should be 0.0, got {}",
        sol.objective
    );
    assert!(
        sol.primal[0].abs() < 1e-6,
        "x should be 0.0, got {}",
        sol.primal[0]
    );
}

#[test]
fn solve_two_var_lp() {
    let mut solver = HighsSolver::new().expect("should create solver");
    solver.load_model(&two_var_lp());

    let sol = solver.solve(None).expect("should solve");

    assert!(
        (sol.objective - 1.0).abs() < 1e-6,
        "objective should be 1.0, got {}",
        sol.objective
    );
    assert!(sol.primal[0] >= 0.99, "x should be ~1.0, got {}", sol.primal[0]);
}

#[test]
fn solve_infeasible_returns_error() {
    let mut solver = HighsSolver::new().expect("should create solver");
    solver.load_model(&infeasible_lp());

    let result = solver.solve(None);

    assert!(result.is_err(), "infeasible LP should return error");
}

#[test]
fn statistics_accumulate_across_solves() {
    let mut solver = HighsSolver::new().expect("should create solver");
    solver.load_model(&simple_bounded_lp());

    let _ = solver.solve(None).expect("first solve");
    let _ = solver.solve(None).expect("second solve");

    let stats = solver.statistics();
    assert_eq!(stats.solve_count, 2, "should have 2 solve calls");
    assert_eq!(stats.success_count, 2, "should have 2 successes");
    assert!(
        stats.total_solve_time_seconds > 0.0,
        "should have accumulated time"
    );
}

#[test]
fn add_rows_tightens_objective() {
    let mut solver = HighsSolver::new().expect("should create solver");

    // Start with x + y >= 0 (unconstrained except bounds)
    let template = LpTemplate {
        num_cols: 2,
        num_rows: 0,
        num_nz: 0,
        col_starts: vec![0, 0, 0],
        row_indices: vec![],
        values: vec![],
        col_lower: vec![0.0, 0.0],
        col_upper: vec![100.0, 100.0],
        objective: vec![1.0, 1.0], // min x + y
        row_lower: vec![],
        row_upper: vec![],
        col_domains: vec![VariableDomain::Continuous; 2],
    };
    solver.load_model(&template);

    let sol1 = solver.solve(None).expect("first solve");
    assert!(sol1.objective.abs() < 1e-6, "unrestricted: obj should be 0");

    // Add constraint x + y >= 5
    let batch = RowBatch {
        num_rows: 1,
        row_starts: vec![0, 2],
        col_indices: vec![0, 1],
        values: vec![1.0, 1.0],
        row_lower: vec![5.0],
        row_upper: vec![f64::INFINITY],
    };
    solver.add_rows(&batch);

    let sol2 = solver.solve(None).expect("second solve");
    assert!(
        (sol2.objective - 5.0).abs() < 1e-6,
        "with constraint: obj should be 5.0, got {}",
        sol2.objective
    );
}

#[test]
fn set_col_bounds_changes_solution() {
    let mut solver = HighsSolver::new().expect("should create solver");
    solver.load_model(&simple_bounded_lp());

    let sol1 = solver.solve(None).expect("first solve");
    assert!((sol1.objective - 1.0).abs() < 1e-6);

    // Tighten lower bound
    solver.set_col_bounds(&[0], &[5.0], &[10.0]);

    let sol2 = solver.solve(None).expect("second solve");
    assert!(
        (sol2.objective - 5.0).abs() < 1e-6,
        "with tighter bound: obj should be 5.0, got {}",
        sol2.objective
    );
}

#[test]
fn apply_profile_does_not_crash() {
    let mut solver = HighsSolver::new().expect("should create solver");

    let profile = HighsProfile {
        primal_feasibility_tolerance: 1e-8,
        dual_feasibility_tolerance: 1e-8,
        ..Default::default()
    };
    solver.apply_profile(&profile);

    solver.load_model(&simple_bounded_lp());
    let sol = solver.solve(None).expect("should solve with custom profile");
    assert!((sol.objective - 1.0).abs() < 1e-6);
}

#[test]
fn get_basis_returns_correct_dimensions() {
    let mut solver = HighsSolver::new().expect("should create solver");
    solver.load_model(&two_var_lp());
    let _ = solver.solve(None).expect("should solve");

    let mut basis = Basis::new(0, 0);
    solver.get_basis(&mut basis);

    assert_eq!(basis.col_status.len(), 2, "should have 2 column statuses");
    assert_eq!(basis.row_status.len(), 1, "should have 1 row status");
}

#[test]
fn solver_name_and_version() {
    let solver = HighsSolver::new().expect("should create solver");

    assert_eq!(solver.name(), "HiGHS");
    assert!(solver.solver_name_version().contains("HiGHS"));
}
