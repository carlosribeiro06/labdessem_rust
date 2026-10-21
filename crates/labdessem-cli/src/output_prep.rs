//! Output preparation functions for converting solution data to writer format.

use std::collections::HashMap;

use labdessem_core::system::System;
use labdessem_model::indexing::{ConstraintOffsetParams, Indexing};
use labdessem_model::names;
use labdessem_solver::{LpSolution, SolutionAccessor};

fn build_indexing_and_params(system: &System) -> (Indexing, ConstraintOffsetParams) {
    let indexing = Indexing::from_system(system);
    let params = ConstraintOffsetParams {
        horizon: system.horizon.periods,
        fpha_enabled: system.fpha_enabled,
        num_fpha_segments: system
            .hydro_plants
            .iter()
            .flat_map(|p| &p.fpha_segments)
            .count(),
        future_cost_enabled: system.future_cost_enabled,
        num_future_cost_cuts: system.future_cost_cuts.len(),
        num_operational_limits: system.operational_limits.len(),
        thermal_uct_enabled: system.thermal_unit_commitment_enabled,
        hydro_uch_enabled: system.hydro_unit_commitment_enabled,
        is_commitment_mode: true,
    };
    (indexing, params)
}

/// Build variable_values HashMap from solution for output writers.
pub fn build_variable_values(system: &System, solution: &LpSolution) -> HashMap<String, f64> {
    let (indexing, params) = build_indexing_and_params(system);
    let accessor = SolutionAccessor::new(solution, &indexing, &params);
    let horizon = system.horizon.periods;

    let mut variable_values = HashMap::new();

    // Thermal generation
    let mut unit_idx = 0;
    for plant in &system.thermal_plants {
        for unit in &plant.units {
            for period in 0..horizon {
                let name = names::thermal_generation_name(&plant.name, &unit.name, period);
                variable_values.insert(name, accessor.thermal_generation(unit_idx, period));
            }
            unit_idx += 1;
        }
    }

    // Hydro generation, turbining, commitment
    let mut hydro_unit_idx = 0;
    for plant in &system.hydro_plants {
        for group in &plant.groups {
            for unit in &group.units {
                for period in 0..horizon {
                    variable_values.insert(
                        names::hydro_generation_name(&plant.name, &group.name, &unit.name, period),
                        accessor.hydro_generation(hydro_unit_idx, period),
                    );
                    variable_values.insert(
                        names::hydro_turbining_name(&plant.name, &group.name, &unit.name, period),
                        accessor.hydro_turbining(hydro_unit_idx, period),
                    );
                    variable_values.insert(
                        names::hydro_commitment_name(&plant.name, &group.name, &unit.name, period),
                        accessor.hydro_commitment(hydro_unit_idx, period),
                    );
                }
                hydro_unit_idx += 1;
            }
        }
    }

    // Hydro spillage, diversion, volume
    for (plant_idx, plant) in system.hydro_plants.iter().enumerate() {
        for period in 0..horizon {
            variable_values.insert(
                names::hydro_spillage_name(&plant.name, period),
                accessor.hydro_spillage(plant_idx, period),
            );
            variable_values.insert(
                names::hydro_diversion_name(&plant.name, period),
                accessor.hydro_diversion(plant_idx, period),
            );
        }
        for period in 0..=horizon {
            variable_values.insert(
                names::hydro_volume_name(&plant.name, period),
                accessor.hydro_volume(plant_idx, period),
            );
        }
    }

    // Pumping
    for (plant_idx, plant) in system.pumping_plants.iter().enumerate() {
        for period in 0..horizon {
            variable_values.insert(
                names::pumping_name(&plant.name, period),
                accessor.pumping(plant_idx, period),
            );
        }
    }

    // Renewable
    for (plant_idx, plant) in system.renewable_plants.iter().enumerate() {
        for period in 0..horizon {
            variable_values.insert(
                names::renewable_generation_name(&plant.name, period),
                accessor.renewable_generation(plant_idx, period),
            );
        }
    }

    variable_values
}

/// Build constraint_duals HashMap from solution for output writers.
pub fn build_constraint_duals(system: &System, solution: &LpSolution) -> HashMap<String, f64> {
    let (indexing, params) = build_indexing_and_params(system);
    let accessor = SolutionAccessor::new(solution, &indexing, &params);
    let horizon = system.horizon.periods;

    let mut constraint_duals = HashMap::new();

    for (submarket_idx, submarket) in system.submarkets.iter().enumerate() {
        for period in 0..horizon {
            let name = names::demand_balance_constraint_name(&submarket.name, period);
            constraint_duals.insert(name, accessor.demand_balance_dual(submarket_idx, period));
        }
    }

    for (plant_idx, plant) in system.hydro_plants.iter().enumerate() {
        for period in 0..horizon {
            let name = names::hydro_balance_constraint_name(&plant.name, period);
            constraint_duals.insert(name, accessor.hydro_balance_dual(plant_idx, period));
        }
    }

    constraint_duals
}
