//! Flow constraint term generation (glue between network and model).

use labdessem_core::system::System;
use labdessem_model::{constraints::LinearTerm, Model};
use labdessem_network::DcNetworkModel;

/// Build the linear terms for a flow cut constraint.
///
/// This function bridges `labdessem-network` with `labdessem-model` by generating
/// the LP constraint terms for a flow cut on a specific branch and period.
pub fn flow_terms_for_cut(
    model: &Model,
    system: &System,
    dc_network: &DcNetworkModel,
    branch_idx: usize,
    period: usize,
) -> Vec<LinearTerm> {
    let horizon = system.horizon.periods;
    let mut terms = Vec::new();

    for (entry_idx, entry) in model.indexing.thermal_unit_entries.iter().enumerate() {
        let plant = &system.thermal_plants[entry.plant_idx];
        let Some(bus_pos) = dc_network.position_of_bus(plant.bus_id) else {
            continue;
        };
        let ptdf_row = dc_network.ptdf_row_for_branch(branch_idx);
        let coefficient = ptdf_row[bus_pos];
        if coefficient.abs() <= 1e-12 {
            continue;
        }

        let variable = &model.variables.thermal_generation[entry_idx * horizon + period];
        terms.push(LinearTerm {
            variable: variable.name.clone(),
            coefficient,
        });
    }

    for (entry_idx, entry) in model.indexing.hydro_unit_entries.iter().enumerate() {
        let plant = &system.hydro_plants[entry.plant_idx];
        let Some(bus_pos) = dc_network.position_of_bus(plant.bus_id) else {
            continue;
        };
        let ptdf_row = dc_network.ptdf_row_for_branch(branch_idx);
        let coefficient = ptdf_row[bus_pos];
        if coefficient.abs() <= 1e-12 {
            continue;
        }

        let variable = &model.variables.hydro_generation[entry_idx * horizon + period];
        terms.push(LinearTerm {
            variable: variable.name.clone(),
            coefficient,
        });
    }

    for (entry_idx, entry) in model.indexing.renewable_plant_entries.iter().enumerate() {
        let plant = &system.renewable_plants[entry.plant_idx];
        let Some(bus_pos) = dc_network.position_of_bus(plant.bus_id) else {
            continue;
        };
        let ptdf_row = dc_network.ptdf_row_for_branch(branch_idx);
        let coefficient = ptdf_row[bus_pos];
        if coefficient.abs() <= 1e-12 {
            continue;
        }

        let variable = &model.variables.renewable_generation[entry_idx * horizon + period];
        terms.push(LinearTerm {
            variable: variable.name.clone(),
            coefficient,
        });
    }

    let flow_conversion = system.horizon.period_duration_hours * 0.0036;
    for (entry_idx, entry) in model.indexing.pumping_plant_entries.iter().enumerate() {
        let plant = &system.pumping_plants[entry.plant_idx];
        let Some(bus_pos) = dc_network.position_of_bus(plant.bus_id) else {
            continue;
        };
        let ptdf_row = dc_network.ptdf_row_for_branch(branch_idx);
        let coefficient = -ptdf_row[bus_pos]
            * plant.specific_consumption_mw_per_m3s
            / flow_conversion;
        if coefficient.abs() <= 1e-12 {
            continue;
        }

        let variable = &model.variables.pumping[entry_idx * horizon + period];
        terms.push(LinearTerm {
            variable: variable.name.clone(),
            coefficient,
        });
    }

    terms
}
