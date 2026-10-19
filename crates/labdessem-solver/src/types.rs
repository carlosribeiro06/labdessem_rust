//! Core types for the solver abstraction layer.
//!
//! Defines canonical representations of LP structure, solutions, and basis
//! management used throughout the solver interface.

use crate::basis_status::BasisStatus;

/// Domain of a decision variable.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum VariableDomain {
    /// Continuous variable (can take any value within bounds).
    #[default]
    Continuous,
    /// Binary variable (0 or 1).
    Binary,
}

/// Pre-assembled LP in Compressed Sparse Column (CSC) format.
///
/// This is the native format for `HiGHS`, eliminating conversion overhead
/// when loading models. Built once by the model layer and reused across
/// multiple solves.
///
/// # CSC Format
///
/// The constraint matrix A is stored in CSC format:
/// - `col_starts[j]` is the index in `row_indices`/`values` where column j begins
/// - `col_starts[num_cols]` equals `num_nz` (total non-zeros)
/// - `row_indices[k]` is the row index of the k-th non-zero
/// - `values[k]` is the value of the k-th non-zero
#[derive(Debug, Clone)]
pub struct LpTemplate {
    /// Number of columns (decision variables).
    pub num_cols: usize,

    /// Number of rows (constraints).
    pub num_rows: usize,

    /// Number of non-zero entries in the constraint matrix.
    pub num_nz: usize,

    /// CSC column start pointers (length: `num_cols + 1`).
    pub col_starts: Vec<i32>,

    /// Row indices for each non-zero (length: `num_nz`).
    pub row_indices: Vec<i32>,

    /// Non-zero values (length: `num_nz`).
    pub values: Vec<f64>,

    /// Column lower bounds (length: `num_cols`).
    pub col_lower: Vec<f64>,

    /// Column upper bounds (length: `num_cols`).
    pub col_upper: Vec<f64>,

    /// Objective coefficients, minimization sense (length: `num_cols`).
    pub objective: Vec<f64>,

    /// Row lower bounds (length: `num_rows`).
    pub row_lower: Vec<f64>,

    /// Row upper bounds (length: `num_rows`).
    pub row_upper: Vec<f64>,

    /// Variable domains (length: `num_cols`).
    pub col_domains: Vec<VariableDomain>,
}

impl LpTemplate {
    /// Validates dimensional consistency of the template.
    ///
    /// # Errors
    ///
    /// Returns an error message if any dimensional invariant is violated.
    pub fn validate(&self) -> Result<(), String> {
        // col_starts length
        if self.col_starts.len() != self.num_cols + 1 {
            return Err(format!(
                "col_starts length {} != num_cols + 1 ({})",
                self.col_starts.len(),
                self.num_cols + 1
            ));
        }

        // col_starts terminal value
        if !self.col_starts.is_empty() {
            let terminal = self.col_starts[self.num_cols] as usize;
            if terminal != self.num_nz {
                return Err(format!(
                    "col_starts[num_cols] ({terminal}) != num_nz ({})",
                    self.num_nz
                ));
            }
        }

        // row_indices length
        if self.row_indices.len() != self.num_nz {
            return Err(format!(
                "row_indices length {} != num_nz ({})",
                self.row_indices.len(),
                self.num_nz
            ));
        }

        // values length
        if self.values.len() != self.num_nz {
            return Err(format!(
                "values length {} != num_nz ({})",
                self.values.len(),
                self.num_nz
            ));
        }

        // col_lower length
        if self.col_lower.len() != self.num_cols {
            return Err(format!(
                "col_lower length {} != num_cols ({})",
                self.col_lower.len(),
                self.num_cols
            ));
        }

        // col_upper length
        if self.col_upper.len() != self.num_cols {
            return Err(format!(
                "col_upper length {} != num_cols ({})",
                self.col_upper.len(),
                self.num_cols
            ));
        }

        // objective length
        if self.objective.len() != self.num_cols {
            return Err(format!(
                "objective length {} != num_cols ({})",
                self.objective.len(),
                self.num_cols
            ));
        }

        // row_lower length
        if self.row_lower.len() != self.num_rows {
            return Err(format!(
                "row_lower length {} != num_rows ({})",
                self.row_lower.len(),
                self.num_rows
            ));
        }

        // row_upper length
        if self.row_upper.len() != self.num_rows {
            return Err(format!(
                "row_upper length {} != num_rows ({})",
                self.row_upper.len(),
                self.num_rows
            ));
        }

        // col_domains length
        if self.col_domains.len() != self.num_cols {
            return Err(format!(
                "col_domains length {} != num_cols ({})",
                self.col_domains.len(),
                self.num_cols
            ));
        }

        // Validate col_starts is monotonically non-decreasing
        for i in 0..self.num_cols {
            if self.col_starts[i] > self.col_starts[i + 1] {
                return Err(format!(
                    "col_starts not monotonic: col_starts[{}] ({}) > col_starts[{}] ({})",
                    i,
                    self.col_starts[i],
                    i + 1,
                    self.col_starts[i + 1]
                ));
            }
        }

        // Validate row_indices are in range
        for (k, &row_idx) in self.row_indices.iter().enumerate() {
            if row_idx < 0 || row_idx as usize >= self.num_rows {
                return Err(format!(
                    "row_indices[{k}] ({row_idx}) out of range [0, {})",
                    self.num_rows
                ));
            }
        }

        Ok(())
    }
}

/// Batch of constraint rows for incremental addition, in CSR format.
///
/// Used when adding network flow cuts iteratively without rebuilding
/// the entire model.
#[derive(Debug, Clone)]
pub struct RowBatch {
    /// Number of rows in this batch.
    pub num_rows: usize,

    /// CSR row start pointers (length: `num_rows + 1`).
    pub row_starts: Vec<i32>,

    /// Column indices for each non-zero (length: total non-zeros).
    pub col_indices: Vec<i32>,

    /// Non-zero values (length: total non-zeros).
    pub values: Vec<f64>,

    /// Row lower bounds (length: `num_rows`).
    pub row_lower: Vec<f64>,

    /// Row upper bounds (length: `num_rows`).
    pub row_upper: Vec<f64>,
}

impl RowBatch {
    /// Returns the total number of non-zero entries.
    #[must_use]
    pub fn num_nz(&self) -> usize {
        self.values.len()
    }

    /// Validates dimensional consistency of the batch.
    ///
    /// # Arguments
    ///
    /// * `num_cols` - Number of columns in the LP (for index validation)
    ///
    /// # Errors
    ///
    /// Returns an error message if any invariant is violated.
    pub fn validate(&self, num_cols: usize) -> Result<(), String> {
        // row_starts length
        if self.row_starts.len() != self.num_rows + 1 {
            return Err(format!(
                "row_starts length {} != num_rows + 1 ({})",
                self.row_starts.len(),
                self.num_rows + 1
            ));
        }

        // row_starts terminal value
        let num_nz = self.num_nz();
        if !self.row_starts.is_empty() {
            let terminal = self.row_starts[self.num_rows] as usize;
            if terminal != num_nz {
                return Err(format!(
                    "row_starts[num_rows] ({terminal}) != num_nz ({num_nz})"
                ));
            }
        }

        // col_indices length
        if self.col_indices.len() != num_nz {
            return Err(format!(
                "col_indices length {} != num_nz ({num_nz})",
                self.col_indices.len()
            ));
        }

        // row_lower length
        if self.row_lower.len() != self.num_rows {
            return Err(format!(
                "row_lower length {} != num_rows ({})",
                self.row_lower.len(),
                self.num_rows
            ));
        }

        // row_upper length
        if self.row_upper.len() != self.num_rows {
            return Err(format!(
                "row_upper length {} != num_rows ({})",
                self.row_upper.len(),
                self.num_rows
            ));
        }

        // Validate row_starts is monotonically non-decreasing
        for i in 0..self.num_rows {
            if self.row_starts[i] > self.row_starts[i + 1] {
                return Err(format!(
                    "row_starts not monotonic: row_starts[{}] ({}) > row_starts[{}] ({})",
                    i,
                    self.row_starts[i],
                    i + 1,
                    self.row_starts[i + 1]
                ));
            }
        }

        // Validate col_indices are in range
        for (k, &col_idx) in self.col_indices.iter().enumerate() {
            if col_idx < 0 || col_idx as usize >= num_cols {
                return Err(format!(
                    "col_indices[{k}] ({col_idx}) out of range [0, {num_cols})"
                ));
            }
        }

        Ok(())
    }
}

/// Zero-copy view of an LP solution, borrowing solver-internal buffers.
///
/// Valid until the next `&mut self` call on the solver. Use [`to_owned`]
/// when the solution must outlive the borrow.
///
/// [`to_owned`]: SolutionView::to_owned
#[derive(Debug, Clone, Copy)]
pub struct SolutionView<'a> {
    /// Optimal objective value (minimization sense).
    pub objective: f64,

    /// Primal variable values (length: `num_cols`).
    pub primal: &'a [f64],

    /// Dual multipliers (shadow prices) for constraints (length: `num_rows`).
    pub dual: &'a [f64],

    /// Reduced costs for variables (length: `num_cols`).
    pub reduced_costs: &'a [f64],

    /// Number of simplex iterations performed.
    pub iterations: u64,

    /// Wall-clock solve time in seconds.
    pub solve_time_seconds: f64,
}

impl SolutionView<'_> {
    /// Converts to an owned [`LpSolution`] by cloning the borrowed slices.
    #[must_use]
    pub fn to_owned(&self) -> LpSolution {
        LpSolution {
            objective: self.objective,
            primal: self.primal.to_vec(),
            dual: self.dual.to_vec(),
            reduced_costs: self.reduced_costs.to_vec(),
            iterations: self.iterations,
            solve_time_seconds: self.solve_time_seconds,
        }
    }
}

/// Owned LP solution for when data must outlive the solver borrow.
#[derive(Debug, Clone)]
pub struct LpSolution {
    /// Optimal objective value (minimization sense).
    pub objective: f64,

    /// Primal variable values (length: `num_cols`).
    pub primal: Vec<f64>,

    /// Dual multipliers (shadow prices) for constraints (length: `num_rows`).
    pub dual: Vec<f64>,

    /// Reduced costs for variables (length: `num_cols`).
    pub reduced_costs: Vec<f64>,

    /// Number of simplex iterations performed.
    pub iterations: u64,

    /// Wall-clock solve time in seconds.
    pub solve_time_seconds: f64,
}

/// Simplex basis for warm-starting solves.
///
/// Stores the basis status for each column (variable) and row (constraint).
/// When provided to a subsequent solve, can reduce solve time by 5-10x
/// if the LP structure is similar (e.g., only bounds changed).
#[derive(Debug, Clone)]
pub struct Basis {
    /// Basis status for each column (length: `num_cols`).
    pub col_status: Vec<BasisStatus>,

    /// Basis status for each row (length: `num_rows`).
    pub row_status: Vec<BasisStatus>,
}

impl Basis {
    /// Creates a new basis with all entries set to `Lower`.
    #[must_use]
    pub fn new(num_cols: usize, num_rows: usize) -> Self {
        Self {
            col_status: vec![BasisStatus::Lower; num_cols],
            row_status: vec![BasisStatus::Lower; num_rows],
        }
    }

    /// Resizes the basis, preserving existing entries and filling new entries with `Lower`.
    pub fn resize(&mut self, num_cols: usize, num_rows: usize) {
        self.col_status.resize(num_cols, BasisStatus::Lower);
        self.row_status.resize(num_rows, BasisStatus::Lower);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn variable_domain_default_is_continuous() {
        assert_eq!(VariableDomain::default(), VariableDomain::Continuous);
    }

    #[test]
    fn valid_lp_template_passes_validation() {
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
        assert!(template.validate().is_ok());
    }

    #[test]
    fn empty_lp_template_passes_validation() {
        let template = LpTemplate {
            num_cols: 0,
            num_rows: 0,
            num_nz: 0,
            col_starts: vec![0],
            row_indices: vec![],
            values: vec![],
            col_lower: vec![],
            col_upper: vec![],
            objective: vec![],
            row_lower: vec![],
            row_upper: vec![],
            col_domains: vec![],
        };
        assert!(template.validate().is_ok());
    }

    #[test]
    fn invalid_col_starts_length_fails() {
        let template = LpTemplate {
            num_cols: 2,
            num_rows: 1,
            num_nz: 2,
            col_starts: vec![0, 1], // Should be length 3
            row_indices: vec![0, 0],
            values: vec![1.0, 1.0],
            col_lower: vec![0.0, 0.0],
            col_upper: vec![1.0, 1.0],
            objective: vec![1.0, 2.0],
            row_lower: vec![0.0],
            row_upper: vec![1.0],
            col_domains: vec![VariableDomain::Continuous; 2],
        };
        let err = template.validate().unwrap_err();
        assert!(err.contains("col_starts length"));
    }

    #[test]
    fn invalid_col_starts_terminal_fails() {
        let template = LpTemplate {
            num_cols: 2,
            num_rows: 1,
            num_nz: 2,
            col_starts: vec![0, 1, 3], // Terminal should be 2, not 3
            row_indices: vec![0, 0],
            values: vec![1.0, 1.0],
            col_lower: vec![0.0, 0.0],
            col_upper: vec![1.0, 1.0],
            objective: vec![1.0, 2.0],
            row_lower: vec![0.0],
            row_upper: vec![1.0],
            col_domains: vec![VariableDomain::Continuous; 2],
        };
        let err = template.validate().unwrap_err();
        assert!(err.contains("col_starts[num_cols]"));
    }

    #[test]
    fn invalid_row_indices_length_fails() {
        let template = LpTemplate {
            num_cols: 2,
            num_rows: 1,
            num_nz: 2,
            col_starts: vec![0, 1, 2],
            row_indices: vec![0], // Should be length 2
            values: vec![1.0, 1.0],
            col_lower: vec![0.0, 0.0],
            col_upper: vec![1.0, 1.0],
            objective: vec![1.0, 2.0],
            row_lower: vec![0.0],
            row_upper: vec![1.0],
            col_domains: vec![VariableDomain::Continuous; 2],
        };
        let err = template.validate().unwrap_err();
        assert!(err.contains("row_indices length"));
    }

    #[test]
    fn row_index_out_of_range_fails() {
        let template = LpTemplate {
            num_cols: 2,
            num_rows: 1,
            num_nz: 2,
            col_starts: vec![0, 1, 2],
            row_indices: vec![0, 5], // 5 is out of range for 1 row
            values: vec![1.0, 1.0],
            col_lower: vec![0.0, 0.0],
            col_upper: vec![1.0, 1.0],
            objective: vec![1.0, 2.0],
            row_lower: vec![0.0],
            row_upper: vec![1.0],
            col_domains: vec![VariableDomain::Continuous; 2],
        };
        let err = template.validate().unwrap_err();
        assert!(err.contains("out of range"));
    }

    #[test]
    fn col_starts_not_monotonic_fails() {
        let template = LpTemplate {
            num_cols: 2,
            num_rows: 1,
            num_nz: 2,
            col_starts: vec![1, 0, 2], // Not monotonic
            row_indices: vec![0, 0],
            values: vec![1.0, 1.0],
            col_lower: vec![0.0, 0.0],
            col_upper: vec![1.0, 1.0],
            objective: vec![1.0, 2.0],
            row_lower: vec![0.0],
            row_upper: vec![1.0],
            col_domains: vec![VariableDomain::Continuous; 2],
        };
        let err = template.validate().unwrap_err();
        assert!(err.contains("not monotonic"));
    }

    #[test]
    fn valid_row_batch_passes_validation() {
        let batch = RowBatch {
            num_rows: 2,
            row_starts: vec![0, 2, 4],
            col_indices: vec![0, 1, 0, 1],
            values: vec![1.0, -1.0, 1.0, 1.0],
            row_lower: vec![0.0, 0.0],
            row_upper: vec![f64::INFINITY, 100.0],
        };
        assert!(batch.validate(2).is_ok());
        assert_eq!(batch.num_nz(), 4);
    }

    #[test]
    fn row_batch_col_index_out_of_range_fails() {
        let batch = RowBatch {
            num_rows: 1,
            row_starts: vec![0, 1],
            col_indices: vec![5], // out of range for num_cols=2
            values: vec![1.0],
            row_lower: vec![0.0],
            row_upper: vec![1.0],
        };
        let err = batch.validate(2).unwrap_err();
        assert!(err.contains("out of range"));
    }

    #[test]
    fn solution_view_to_owned() {
        let primal = vec![1.0, 2.0, 3.0];
        let dual = vec![0.5, 0.5];
        let reduced_costs = vec![0.0, 0.0, 0.0];

        let view = SolutionView {
            objective: 42.0,
            primal: &primal,
            dual: &dual,
            reduced_costs: &reduced_costs,
            iterations: 100,
            solve_time_seconds: 0.5,
        };

        let owned = view.to_owned();

        assert!((owned.objective - 42.0).abs() < 1e-9);
        assert_eq!(owned.primal, primal);
        assert_eq!(owned.dual, dual);
        assert_eq!(owned.reduced_costs, reduced_costs);
        assert_eq!(owned.iterations, 100);
        assert!((owned.solve_time_seconds - 0.5).abs() < 1e-9);
    }

    #[test]
    fn basis_new_initializes_to_lower() {
        let basis = Basis::new(3, 2);
        assert_eq!(basis.col_status.len(), 3);
        assert_eq!(basis.row_status.len(), 2);
        assert!(basis.col_status.iter().all(|&s| s == BasisStatus::Lower));
        assert!(basis.row_status.iter().all(|&s| s == BasisStatus::Lower));
    }

    #[test]
    fn basis_resize_preserves_existing() {
        let mut basis = Basis::new(2, 1);
        basis.col_status[0] = BasisStatus::Basic;
        basis.row_status[0] = BasisStatus::Upper;

        basis.resize(3, 2);

        assert_eq!(basis.col_status.len(), 3);
        assert_eq!(basis.row_status.len(), 2);
        assert_eq!(basis.col_status[0], BasisStatus::Basic);
        assert_eq!(basis.col_status[1], BasisStatus::Lower);
        assert_eq!(basis.col_status[2], BasisStatus::Lower);
        assert_eq!(basis.row_status[0], BasisStatus::Upper);
        assert_eq!(basis.row_status[1], BasisStatus::Lower);
    }

    #[test]
    fn basis_resize_shrinks() {
        let mut basis = Basis::new(5, 3);
        basis.col_status[0] = BasisStatus::Basic;

        basis.resize(2, 1);

        assert_eq!(basis.col_status.len(), 2);
        assert_eq!(basis.row_status.len(), 1);
        assert_eq!(basis.col_status[0], BasisStatus::Basic);
    }
}
