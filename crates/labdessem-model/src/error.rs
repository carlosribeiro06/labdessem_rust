//! Errors raised while building the optimization model.

/// Errors raised while building the optimization model.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum ModelError {
    /// EngMax is enabled but no bound is stored for this plant and period.
    #[error("missing EngMax bound for plant {plant} period {period}")]
    MissingEngMaxBound {
        /// Hydro plant name.
        plant: String,
        /// Internal 0-based model period.
        period: usize,
    },
}

#[cfg(test)]
mod tests {
    use super::ModelError;

    #[test]
    fn missing_engmax_bound_display_names_plant_and_period() {
        let err = ModelError::MissingEngMaxBound {
            plant: "UHE-1".into(),
            period: 3,
        };

        assert_eq!(
            err.to_string(),
            "missing EngMax bound for plant UHE-1 period 3"
        );
        assert!(std::error::Error::source(&err).is_none());
    }
}
