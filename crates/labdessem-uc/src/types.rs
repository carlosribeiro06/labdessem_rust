//! Shared types for unit commitment formulation.

/// Domain of a commitment variable.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CommitmentDomain {
    /// Binary variable in MILP.
    Binary,
    /// Continuous variable with fixed value (for LP-FIXED mode).
    Fixed(i8),
    /// Not created (for LP mode without commitment).
    Inactive,
}

impl CommitmentDomain {
    #[must_use]
    pub fn fixed_value(&self) -> Option<f64> {
        if let Self::Fixed(v) = self {
            Some(f64::from(*v))
        } else {
            None
        }
    }

    #[must_use]
    pub fn is_active(&self) -> bool {
        !matches!(self, Self::Inactive)
    }
}

/// A unit commitment variable.
#[derive(Debug, Clone, PartialEq)]
pub struct UcVariable {
    pub name: String,
    pub lower_bound: f64,
    pub upper_bound: f64,
    pub domain: CommitmentDomain,
}

impl UcVariable {
    /// Create a new binary commitment variable.
    #[must_use]
    pub fn binary(name: String) -> Self {
        Self {
            name,
            lower_bound: 0.0,
            upper_bound: 1.0,
            domain: CommitmentDomain::Binary,
        }
    }

    /// Create a new fixed commitment variable.
    #[must_use]
    pub fn fixed(name: String, value: i8) -> Self {
        Self {
            name,
            lower_bound: 0.0,
            upper_bound: 1.0,
            domain: CommitmentDomain::Fixed(value),
        }
    }

    /// Create a new continuous variable in [0, 1] (for startup variables in MILP).
    #[must_use]
    pub fn unit_interval(name: String) -> Self {
        Self {
            name,
            lower_bound: 0.0,
            upper_bound: 1.0,
            domain: CommitmentDomain::Binary,
        }
    }
}

/// Constraint sense.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ConstraintSense {
    /// ≤
    LessOrEqual,
    /// ≥
    GreaterOrEqual,
    /// =
    Equal,
}

/// A term in a linear constraint.
#[derive(Debug, Clone, PartialEq)]
pub struct UcTerm {
    pub variable: String,
    pub coefficient: f64,
}

impl UcTerm {
    #[must_use]
    pub fn new(variable: impl Into<String>, coefficient: f64) -> Self {
        Self {
            variable: variable.into(),
            coefficient,
        }
    }
}

/// A linear constraint in unit commitment formulation.
#[derive(Debug, Clone, PartialEq)]
pub struct UcConstraint {
    pub name: String,
    pub terms: Vec<UcTerm>,
    pub sense: ConstraintSense,
    pub rhs: f64,
}

impl UcConstraint {
    #[must_use]
    pub fn new(
        name: impl Into<String>,
        terms: Vec<UcTerm>,
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

    /// Create an equality constraint.
    #[must_use]
    pub fn equal(name: impl Into<String>, terms: Vec<UcTerm>, rhs: f64) -> Self {
        Self::new(name, terms, ConstraintSense::Equal, rhs)
    }

    /// Create a less-or-equal constraint.
    #[must_use]
    pub fn leq(name: impl Into<String>, terms: Vec<UcTerm>, rhs: f64) -> Self {
        Self::new(name, terms, ConstraintSense::LessOrEqual, rhs)
    }

    /// Create a greater-or-equal constraint.
    #[must_use]
    pub fn geq(name: impl Into<String>, terms: Vec<UcTerm>, rhs: f64) -> Self {
        Self::new(name, terms, ConstraintSense::GreaterOrEqual, rhs)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn commitment_domain_fixed_value() {
        assert_eq!(CommitmentDomain::Binary.fixed_value(), None);
        assert_eq!(CommitmentDomain::Fixed(1).fixed_value(), Some(1.0));
        assert_eq!(CommitmentDomain::Fixed(0).fixed_value(), Some(0.0));
        assert_eq!(CommitmentDomain::Inactive.fixed_value(), None);
    }

    #[test]
    fn commitment_domain_is_active() {
        assert!(CommitmentDomain::Binary.is_active());
        assert!(CommitmentDomain::Fixed(1).is_active());
        assert!(!CommitmentDomain::Inactive.is_active());
    }

    #[test]
    fn uc_variable_binary() {
        let var = UcVariable::binary("test".into());
        assert_eq!(var.name, "test");
        assert_eq!(var.lower_bound, 0.0);
        assert_eq!(var.upper_bound, 1.0);
        assert_eq!(var.domain, CommitmentDomain::Binary);
    }

    #[test]
    fn uc_variable_fixed() {
        let var = UcVariable::fixed("test".into(), 1);
        assert_eq!(var.domain, CommitmentDomain::Fixed(1));
    }

    #[test]
    fn uc_constraint_constructors() {
        let terms = vec![UcTerm::new("x", 1.0), UcTerm::new("y", -1.0)];

        let eq = UcConstraint::equal("eq", terms.clone(), 0.0);
        assert_eq!(eq.sense, ConstraintSense::Equal);

        let leq = UcConstraint::leq("leq", terms.clone(), 1.0);
        assert_eq!(leq.sense, ConstraintSense::LessOrEqual);

        let geq = UcConstraint::geq("geq", terms, -1.0);
        assert_eq!(geq.sense, ConstraintSense::GreaterOrEqual);
    }
}
