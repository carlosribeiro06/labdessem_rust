//! Flow constraint dual extraction for CMB calculation.

use std::collections::HashMap;

use crate::types::FlowCutMapping;

/// Extract effective flow duals (λ_L) from dual values using flow cut row indices.
///
/// For each (branch_idx, period) pair in the mapping, computes:
/// λ_L = dual[base_offset + upper_idx] + dual[base_offset + lower_idx]
///
/// # Panics
///
/// Panics if any row index in the mapping is out of bounds for the dual array.
pub fn extract_flow_duals(
    dual_values: &[f64],
    mapping: &FlowCutMapping,
    base_row_offset: usize,
) -> HashMap<(usize, usize), f64> {
    let mut result = HashMap::with_capacity(mapping.indices.len());

    for (&(branch_idx, period), &(upper_idx, lower_idx)) in &mapping.indices {
        let upper_row = base_row_offset + upper_idx;
        let lower_row = base_row_offset + lower_idx;

        assert!(
            upper_row < dual_values.len(),
            "row index out of bounds: upper_row={upper_row} >= dual.len()={}",
            dual_values.len()
        );
        assert!(
            lower_row < dual_values.len(),
            "row index out of bounds: lower_row={lower_row} >= dual.len()={}",
            dual_values.len()
        );

        let lambda_l = dual_values[upper_row] + dual_values[lower_row];
        result.insert((branch_idx, period), lambda_l);
    }

    result
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_extract_single_branch_period() {
        let duals = vec![0.0, 100.0, -30.0];
        let mut indices = HashMap::new();
        indices.insert((0, 0), (0, 1));
        let mapping = FlowCutMapping { indices };

        let result = extract_flow_duals(&duals, &mapping, 1);

        assert_eq!(result.len(), 1);
        assert!((result[&(0, 0)] - 70.0).abs() < 1e-9);
    }

    #[test]
    fn test_extract_multiple_branches() {
        let duals = vec![0.0, 10.0, -5.0, 20.0, -8.0];
        let mut indices = HashMap::new();
        indices.insert((0, 0), (0, 1));
        indices.insert((1, 0), (2, 3));
        let mapping = FlowCutMapping { indices };

        let result = extract_flow_duals(&duals, &mapping, 1);

        assert_eq!(result.len(), 2);
        assert!((result[&(0, 0)] - 5.0).abs() < 1e-9);
        assert!((result[&(1, 0)] - 12.0).abs() < 1e-9);
    }

    #[test]
    fn test_extract_empty_mapping() {
        let duals = vec![0.0, 10.0];
        let mapping = FlowCutMapping {
            indices: HashMap::new(),
        };

        let result = extract_flow_duals(&duals, &mapping, 0);

        assert!(result.is_empty());
    }

    #[test]
    #[should_panic(expected = "row index out of bounds")]
    fn test_extract_panics_on_invalid_index() {
        let duals = vec![0.0, 10.0];
        let mut indices = HashMap::new();
        indices.insert((0, 0), (0, 5));
        let mapping = FlowCutMapping { indices };

        extract_flow_duals(&duals, &mapping, 0);
    }
}
