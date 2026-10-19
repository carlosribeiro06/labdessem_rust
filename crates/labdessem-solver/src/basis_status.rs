//! Simplex basis status representation.

/// Status of a variable or constraint in a simplex basis.
///
/// Maps directly to `HiGHS` constants from `HTypes.h`:
/// - `kBasisStatusLower = 0`
/// - `kBasisStatusBasic = 1`
/// - `kBasisStatusUpper = 2`
/// - `kBasisStatusZero = 3` (free variable)
/// - `kBasisStatusNonbasic = 4`
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(u8)]
pub enum BasisStatus {
    /// Variable at its lower bound.
    Lower = 0,
    /// Variable in the basis (between bounds).
    Basic = 1,
    /// Variable at its upper bound.
    Upper = 2,
    /// Free variable (no bounds).
    Free = 3,
    /// Non-basic variable (generic).
    NonBasic = 4,
}

impl BasisStatus {
    /// Convert from `HiGHS` integer code.
    ///
    /// Returns `None` for unknown codes.
    #[must_use]
    pub fn from_highs_code(code: i32) -> Option<Self> {
        match code {
            0 => Some(Self::Lower),
            1 => Some(Self::Basic),
            2 => Some(Self::Upper),
            3 => Some(Self::Free),
            4 => Some(Self::NonBasic),
            _ => None,
        }
    }

    /// Convert to `HiGHS` integer code.
    #[must_use]
    pub const fn to_highs_code(self) -> i32 {
        self as i32
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn basis_status_roundtrip() {
        for code in 0..=4 {
            let status = BasisStatus::from_highs_code(code).unwrap();
            assert_eq!(status.to_highs_code(), code);
        }
    }

    #[test]
    fn unknown_code_returns_none() {
        assert!(BasisStatus::from_highs_code(-1).is_none());
        assert!(BasisStatus::from_highs_code(5).is_none());
        assert!(BasisStatus::from_highs_code(99).is_none());
    }

    #[test]
    fn explicit_values_match_highs_constants() {
        assert_eq!(BasisStatus::Lower.to_highs_code(), 0);
        assert_eq!(BasisStatus::Basic.to_highs_code(), 1);
        assert_eq!(BasisStatus::Upper.to_highs_code(), 2);
        assert_eq!(BasisStatus::Free.to_highs_code(), 3);
        assert_eq!(BasisStatus::NonBasic.to_highs_code(), 4);
    }
}
