//! Helper functions for output generation.
//!
//! These functions extract and aggregate simulation results for CSV output.

use labdessem_core::system::System;

/// Get internal period indices for an original period (1-indexed).
pub fn original_period_internal_indices(system: &System, original_period: usize) -> Vec<usize> {
    system.horizon.internal_periods_for_original(original_period)
}

/// Get the duration in hours for an original period (1-indexed).
pub fn original_period_duration_hours(system: &System, original_period: usize) -> f64 {
    system.horizon.original_period_durations_hours[original_period - 1]
}

/// Compute weighted average of values across internal periods.
pub fn average_internal_values(
    system: &System,
    periods: &[usize],
    values: impl Fn(usize) -> f64,
) -> f64 {
    if periods.is_empty() {
        return 0.0;
    }

    let mut weighted_sum = 0.0;
    let mut total_duration = 0.0;

    for &period in periods {
        let original = system.horizon.internal_to_original_period[period];
        let duration = system.horizon.original_period_durations_hours[original - 1];
        weighted_sum += values(period) * duration;
        total_duration += duration;
    }

    if total_duration.abs() < 1e-12 {
        0.0
    } else {
        weighted_sum / total_duration
    }
}

/// Sum values across internal periods.
pub fn sum_internal_values(periods: &[usize], values: impl Fn(usize) -> f64) -> f64 {
    periods.iter().map(|&period| values(period)).sum()
}

/// Calculate thermal unit residence times (consecutive on/off periods).
pub fn thermal_residence_times(
    initial_is_on: bool,
    initial_time_in_state: usize,
    statuses: &[bool],
) -> (Vec<usize>, Vec<usize>) {
    let mut times_on = Vec::with_capacity(statuses.len());
    let mut times_off = Vec::with_capacity(statuses.len());
    let mut previous_status = initial_is_on;
    let mut current_residence = initial_time_in_state;

    for &status in statuses {
        if status == previous_status {
            current_residence += 1;
        } else {
            current_residence = 1;
            previous_status = status;
        }

        if status {
            times_on.push(current_residence);
            times_off.push(0);
        } else {
            times_on.push(0);
            times_off.push(current_residence);
        }
    }

    (times_on, times_off)
}
