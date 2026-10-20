//! Network result types.

use std::collections::HashMap;

use labdessem_core::ids::{BranchId, BusId};

/// Maps flow cut keys to their constraint row indices in the LP model.
///
/// Each entry maps `(branch_idx, period)` to `(upper_row_idx, lower_row_idx)`
/// representing the indices of the upper and lower flow limit constraints.
#[derive(Debug, Clone, Default)]
pub struct FlowCutMapping {
    pub indices: HashMap<(usize, usize), (usize, usize)>,
}

/// Represents a flow constraint violation on a branch.
#[derive(Debug, Clone, PartialEq)]
pub struct FlowViolation {
    pub branch_id: BranchId,
    pub branch_name: String,
    pub period: usize,
    pub flow_mw: f64,
    pub limit_mw: f64,
}

/// Line flow result for network output.
#[derive(Debug, Clone, PartialEq)]
pub struct LineFlowResult {
    pub branch_id: BranchId,
    pub branch_name: String,
    pub from_bus_name: String,
    pub to_bus_name: String,
    pub period: usize,
    pub flow_mw: f64,
    pub limit_mw: f64,
}

/// Record of a flow cut line added during iterations.
#[derive(Debug, Clone, PartialEq)]
pub struct AddedFlowCutLineResult {
    pub branch_id: BranchId,
    pub branch_name: String,
    pub from_bus_id: BusId,
    pub to_bus_id: BusId,
    pub flow_mw: f64,
    pub limit_mw: f64,
}

/// Constraint sense for network constraints.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ConstraintSense {
    LessOrEqual,
    GreaterOrEqual,
    Equal,
}

/// A term in a network flow constraint.
#[derive(Debug, Clone, PartialEq)]
pub struct NetworkTerm {
    pub variable_name: String,
    pub coefficient: f64,
}

impl NetworkTerm {
    #[must_use]
    pub fn new(variable_name: impl Into<String>, coefficient: f64) -> Self {
        Self {
            variable_name: variable_name.into(),
            coefficient,
        }
    }
}

/// A network constraint ready for LP/MILP integration.
#[derive(Debug, Clone, PartialEq)]
pub struct NetworkConstraint {
    pub name: String,
    pub terms: Vec<NetworkTerm>,
    pub sense: ConstraintSense,
    pub rhs: f64,
}

impl NetworkConstraint {
    #[must_use]
    pub fn new(
        name: impl Into<String>,
        terms: Vec<NetworkTerm>,
        sense: ConstraintSense,
        rhs: f64,
    ) -> Self {
        Self {
            name: name.into(),
            terms,
            sense,
            rhs,
        }
    }

    #[must_use]
    pub fn leq(name: impl Into<String>, terms: Vec<NetworkTerm>, rhs: f64) -> Self {
        Self::new(name, terms, ConstraintSense::LessOrEqual, rhs)
    }

    #[must_use]
    pub fn geq(name: impl Into<String>, terms: Vec<NetworkTerm>, rhs: f64) -> Self {
        Self::new(name, terms, ConstraintSense::GreaterOrEqual, rhs)
    }

    #[must_use]
    pub fn eq(name: impl Into<String>, terms: Vec<NetworkTerm>, rhs: f64) -> Self {
        Self::new(name, terms, ConstraintSense::Equal, rhs)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_network_term_construction() {
        let term = NetworkTerm::new("x", 2.5);
        assert_eq!(term.variable_name, "x");
        assert!((term.coefficient - 2.5).abs() < 1e-9);
    }

    #[test]
    fn test_network_constraint_leq() {
        let terms = vec![NetworkTerm::new("x", 1.0), NetworkTerm::new("y", -1.0)];
        let constraint = NetworkConstraint::leq("flow_upper", terms, 100.0);
        assert_eq!(constraint.sense, ConstraintSense::LessOrEqual);
        assert!((constraint.rhs - 100.0).abs() < 1e-9);
    }

    #[test]
    fn test_network_constraint_geq() {
        let terms = vec![NetworkTerm::new("p", 3.0)];
        let constraint = NetworkConstraint::geq("flow_lower", terms, -50.0);
        assert_eq!(constraint.sense, ConstraintSense::GreaterOrEqual);
        assert!((constraint.rhs - (-50.0)).abs() < 1e-9);
    }

    #[test]
    fn test_network_constraint_eq() {
        let terms = vec![NetworkTerm::new("balance", 1.0)];
        let constraint = NetworkConstraint::eq("balance", terms, 0.0);
        assert_eq!(constraint.sense, ConstraintSense::Equal);
    }

    #[test]
    fn test_constraint_sense_equality() {
        assert_eq!(ConstraintSense::LessOrEqual, ConstraintSense::LessOrEqual);
        assert_ne!(ConstraintSense::LessOrEqual, ConstraintSense::GreaterOrEqual);
    }
}
