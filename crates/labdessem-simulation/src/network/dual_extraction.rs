//! Flow constraint dual extraction for CMB calculation.

use std::collections::HashMap;

use labdessem_solver::LpSolution;

use crate::results::FlowCutMapping;

/// Extract effective flow duals (λ_L) from LP solution using flow cut row indices.
///
/// For each (branch_idx, period) pair in the mapping, computes:
/// λ_L = dual[base_offset + upper_idx] + dual[base_offset + lower_idx]
///
/// # Panics
///
/// Panics if any row index in the mapping is out of bounds for the dual array.
pub fn extract_flow_duals(
    solution: &LpSolution,
    mapping: &FlowCutMapping,
    base_row_offset: usize,
) -> HashMap<(usize, usize), f64> {
    let mut result = HashMap::with_capacity(mapping.indices.len());

    for (&(branch_idx, period), &(upper_idx, lower_idx)) in &mapping.indices {
        let upper_row = base_row_offset + upper_idx;
        let lower_row = base_row_offset + lower_idx;

        assert!(
            upper_row < solution.dual.len(),
            "row index out of bounds: upper_row={upper_row} >= dual.len()={}",
            solution.dual.len()
        );
        assert!(
            lower_row < solution.dual.len(),
            "row index out of bounds: lower_row={lower_row} >= dual.len()={}",
            solution.dual.len()
        );

        let lambda_l = solution.dual[upper_row] + solution.dual[lower_row];
        result.insert((branch_idx, period), lambda_l);
    }

    result
}

#[cfg(test)]
mod tests {
    use super::*;

    fn make_solution(duals: Vec<f64>) -> LpSolution {
        LpSolution {
            objective: 0.0,
            primal: vec![],
            dual: duals,
            reduced_costs: vec![],
            iterations: 0,
            solve_time_seconds: 0.0,
        }
    }

    #[test]
    fn test_extract_single_branch_period() {
        let solution = make_solution(vec![0.0, 100.0, -30.0]);
        let mut indices = HashMap::new();
        indices.insert((0, 0), (0, 1));
        let mapping = FlowCutMapping { indices };

        let result = extract_flow_duals(&solution, &mapping, 1);

        assert_eq!(result.len(), 1);
        assert!((result[&(0, 0)] - 70.0).abs() < 1e-9);
    }

    #[test]
    fn test_extract_multiple_branches() {
        let solution = make_solution(vec![0.0, 10.0, -5.0, 20.0, -8.0]);
        let mut indices = HashMap::new();
        indices.insert((0, 0), (0, 1));
        indices.insert((1, 0), (2, 3));
        let mapping = FlowCutMapping { indices };

        let result = extract_flow_duals(&solution, &mapping, 1);

        assert_eq!(result.len(), 2);
        assert!((result[&(0, 0)] - 5.0).abs() < 1e-9);
        assert!((result[&(1, 0)] - 12.0).abs() < 1e-9);
    }

    #[test]
    fn test_extract_empty_mapping() {
        let solution = make_solution(vec![0.0, 10.0]);
        let mapping = FlowCutMapping {
            indices: HashMap::new(),
        };

        let result = extract_flow_duals(&solution, &mapping, 0);

        assert!(result.is_empty());
    }

    #[test]
    #[should_panic(expected = "row index out of bounds")]
    fn test_extract_panics_on_invalid_index() {
        let solution = make_solution(vec![0.0, 10.0]);
        let mut indices = HashMap::new();
        indices.insert((0, 0), (0, 5));
        let mapping = FlowCutMapping { indices };

        extract_flow_duals(&solution, &mapping, 0);
    }
}
