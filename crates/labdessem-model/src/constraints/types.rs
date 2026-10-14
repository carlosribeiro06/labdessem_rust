//! Constraint type definitions.

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ConstraintSense {
    Equal,
    LessOrEqual,
    GreaterOrEqual,
}

#[derive(Debug, Clone, PartialEq)]
pub struct LinearTerm {
    pub variable: String,
    pub coefficient: f64,
}

#[derive(Debug, Clone, PartialEq)]
pub struct LinearConstraint {
    pub name: String,
    pub terms: Vec<LinearTerm>,
    pub sense: ConstraintSense,
    pub rhs: f64,
}
