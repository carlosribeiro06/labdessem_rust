//! Helper functions for output generation.
//!
//! These functions extract and aggregate simulation results for CSV output.

use std::collections::HashMap;

use labdessem_core::entities::hydro::HydroPlant;
use labdessem_core::ids::HydroPlantId;
use labdessem_core::system::System;
use labdessem_model::names::{
    hydro_diversion_name, hydro_spillage_name, hydro_turbining_name, pumping_name,
};

/// Get internal period indices for an original period (1-indexed).
pub fn original_period_internal_indices(system: &System, original_period: usize) -> Vec<usize> {
    system
        .horizon
        .internal_periods_for_original(original_period)
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

/// Calculate total upstream inflow to a hydro plant in hm3.
pub fn hydro_upstream_inflow_hm3(
    system: &System,
    variable_values: &HashMap<String, f64>,
    plant: &HydroPlant,
    period: usize,
) -> f64 {
    let mut total = 0.0;

    for upstream_id in &plant.upstream_plant_ids {
        if let Some(upstream) = system
            .hydro_plants
            .iter()
            .find(|candidate| candidate.id == *upstream_id)
        {
            total += hydro_total_turbining_hm3(variable_values, upstream, period);
            total += variable_values
                .get(&hydro_spillage_name(&upstream.name, period))
                .copied()
                .unwrap_or(0.0);
        }
    }

    for upstream_id in &plant.diversion_upstream_plant_ids {
        if let Some(upstream) = system
            .hydro_plants
            .iter()
            .find(|candidate| candidate.id == *upstream_id)
        {
            total += variable_values
                .get(&hydro_diversion_name(&upstream.name, period))
                .copied()
                .unwrap_or(0.0);
        }
    }

    for pumping_plant in &system.pumping_plants {
        if pumping_plant.upstream_hydro_id == plant.id {
            total += variable_values
                .get(&pumping_name(&pumping_plant.name, period))
                .copied()
                .unwrap_or(0.0);
        }
    }

    total
}

/// Calculate pumping outflow from a hydro plant in hm3.
pub fn hydro_pumping_out_hm3(
    system: &System,
    variable_values: &HashMap<String, f64>,
    plant_id: HydroPlantId,
    period: usize,
) -> f64 {
    system
        .pumping_plants
        .iter()
        .filter(|pumping_plant| pumping_plant.downstream_hydro_id == plant_id)
        .map(|pumping_plant| {
            variable_values
                .get(&pumping_name(&pumping_plant.name, period))
                .copied()
                .unwrap_or(0.0)
        })
        .sum()
}

/// Calculate total turbining across all units of a hydro plant in hm3.
pub fn hydro_total_turbining_hm3(
    variable_values: &HashMap<String, f64>,
    plant: &HydroPlant,
    period: usize,
) -> f64 {
    plant
        .groups
        .iter()
        .flat_map(|group| {
            group.units.iter().map(move |unit| {
                variable_values
                    .get(&hydro_turbining_name(
                        &plant.name,
                        &group.name,
                        &unit.name,
                        period,
                    ))
                    .copied()
                    .unwrap_or(0.0)
            })
        })
        .sum()
}

/// Calculate max generation at current operating point for a hydro plant.
pub fn hydro_generation_max_at_point_mw(
    system: &System,
    variable_values: &HashMap<String, f64>,
    plant: &HydroPlant,
    period: usize,
    plant_max_generation_mw: f64,
) -> f64 {
    use labdessem_model::names::hydro_volume_name;

    let flow_conversion = system.horizon.period_duration_hours * 0.0036;
    let max_turbining: f64 = plant
        .groups
        .iter()
        .flat_map(|group| group.units.iter())
        .map(|unit| unit.max_turbining_hm3)
        .sum();

    if !system.fpha_enabled || plant.fpha_segments.is_empty() {
        let generation_from_productivity =
            plant.specific_productivity_mw_per_m3s / flow_conversion * max_turbining;
        return plant_max_generation_mw
            .min(generation_from_productivity)
            .max(0.0);
    }

    let volume = variable_values
        .get(&hydro_volume_name(&plant.name, period + 1))
        .copied()
        .unwrap_or(0.0);

    let fpha_max_generation = plant
        .fpha_segments
        .iter()
        .map(|segment| {
            segment.correction_factor * segment.rhs
                + segment.correction_factor * segment.volume_coefficient * volume
                + segment.correction_factor * segment.turbining_coefficient / flow_conversion
                    * max_turbining
        })
        .fold(f64::INFINITY, f64::min);

    plant_max_generation_mw.min(fpha_max_generation).max(0.0)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn sum_internal_values_sums_correctly() {
        let periods = vec![0, 1, 2];
        let values = |p: usize| p as f64 * 10.0;
        assert!((sum_internal_values(&periods, values) - 30.0).abs() < 1e-9);
    }
}
