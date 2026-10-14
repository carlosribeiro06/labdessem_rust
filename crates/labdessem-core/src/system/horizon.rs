//! Study horizon and temporal discretization.

use crate::error::CoreError;

/// Temporal structure for the optimization horizon.
///
/// Represents the mapping between original input periods (from the study case)
/// and internal periods used in the optimization model. The internal periods
/// have a fixed duration (`period_duration_hours`), while original periods
/// may have variable durations.
#[derive(Debug, Clone, PartialEq)]
pub struct StudyHorizon {
    /// Number of internal periods in the optimization model.
    pub periods: usize,
    /// Duration of each internal period in hours.
    pub period_duration_hours: f64,
    /// Number of original periods from input data.
    pub original_periods: usize,
    /// Duration of each original period in hours.
    pub original_period_durations_hours: Vec<f64>,
    /// Maps internal period index to original period number (1-indexed).
    pub internal_to_original_period: Vec<usize>,
    /// Subperiod index within the original period for each internal period.
    pub internal_subperiod_index: Vec<usize>,
}

impl StudyHorizon {
    /// Validate the horizon configuration.
    pub fn validate(&self) -> Result<(), CoreError> {
        if self.periods == 0 {
            return Err(CoreError::validation(
                "study horizon must contain at least one period",
            ));
        }
        if self.period_duration_hours <= 0.0 {
            return Err(CoreError::validation(
                "study horizon period duration must be positive",
            ));
        }
        if self.original_periods == 0 {
            return Err(CoreError::validation(
                "study horizon must contain at least one original period",
            ));
        }
        if self.original_period_durations_hours.len() != self.original_periods {
            return Err(CoreError::validation(
                "study horizon original duration horizon mismatch",
            ));
        }
        if self
            .original_period_durations_hours
            .iter()
            .any(|duration| *duration <= 0.0)
        {
            return Err(CoreError::validation(
                "study horizon original period durations must be positive",
            ));
        }
        if self.internal_to_original_period.len() != self.periods
            || self.internal_subperiod_index.len() != self.periods
        {
            return Err(CoreError::validation(
                "study horizon internal period mapping mismatch",
            ));
        }
        if self
            .internal_to_original_period
            .iter()
            .any(|period| *period == 0 || *period > self.original_periods)
        {
            return Err(CoreError::validation(
                "study horizon internal mapping references invalid original period",
            ));
        }

        Ok(())
    }

    /// Get the internal period indices that correspond to a given original period.
    pub fn internal_periods_for_original(&self, original_period: usize) -> Vec<usize> {
        self.internal_to_original_period
            .iter()
            .enumerate()
            .filter_map(|(internal_idx, mapped_period)| {
                (*mapped_period == original_period).then_some(internal_idx)
            })
            .collect()
    }
}
