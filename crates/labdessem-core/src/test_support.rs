//! Test support utilities for labdessem-core.
//!
//! Provides builders for creating minimal entities and systems in tests.

use crate::entities::{
    Bus, HydroFphaSegment, HydroGroup, HydroInitialCondition, HydroPlant, HydroUnit, Reservoir,
    Submarket, ThermalInitialCondition, ThermalPlant, ThermalUnit,
};
use crate::ids::{
    BusId, HydroGroupId, HydroPlantId, HydroUnitId, SubmarketId, ThermalPlantId, ThermalUnitId,
};
use crate::system::{StudyHorizon, System};

/// Create a minimal study horizon with 2 one-hour periods.
pub fn minimal_horizon() -> StudyHorizon {
    StudyHorizon {
        periods: 2,
        period_duration_hours: 1.0,
        original_periods: 2,
        original_period_durations_hours: vec![1.0, 1.0],
        internal_to_original_period: vec![1, 2],
        internal_subperiod_index: vec![1, 1],
    }
}

/// Create a minimal submarket with constant demand.
pub fn minimal_submarket(id: usize, demand_mw: f64, periods: usize) -> Submarket {
    Submarket {
        id: SubmarketId(id),
        name: format!("SBM-{id}"),
        demand_mw: vec![demand_mw; periods],
    }
}

/// Create a minimal bus.
pub fn minimal_bus(id: usize, submarket_id: usize, demand_mw: f64, periods: usize) -> Bus {
    Bus {
        id: BusId(id),
        name: format!("BUS-{id}"),
        submarket_id: SubmarketId(submarket_id),
        angle_reference: id == 1,
        demand_mw: vec![demand_mw; periods],
    }
}

/// Create a minimal thermal plant with one unit.
pub fn minimal_thermal(
    id: usize,
    submarket_id: usize,
    bus_id: usize,
    cost_per_mwh: f64,
    max_generation_mw: f64,
) -> ThermalPlant {
    ThermalPlant {
        id: ThermalPlantId(id),
        name: format!("TERM-{id}"),
        submarket_id: SubmarketId(submarket_id),
        bus_id: BusId(bus_id),
        units: vec![ThermalUnit {
            id: ThermalUnitId(id * 100 + 1),
            name: format!("UNIT-{id}-1"),
            min_generation_mw: 0.0,
            max_generation_mw,
            variable_cost_per_mwh: cost_per_mwh,
            startup_cost: 0.0,
            shutdown_cost: 0.0,
            min_up_time: 0,
            min_down_time: 0,
            startup_trajectory_mw: vec![],
            shutdown_trajectory_mw: vec![],
            initial_condition: ThermalInitialCondition {
                is_on: true,
                generation_mw: 0.0,
                time_in_state: 24,
                time_in_ramp: 0,
                is_ramping_up: false,
                is_ramping_down: false,
            },
        }],
    }
}

/// Create a minimal hydro plant with one unit.
pub fn minimal_hydro(
    id: usize,
    submarket_id: usize,
    bus_id: usize,
    productivity: f64,
    max_turbining_hm3: f64,
    inflow_hm3: f64,
    periods: usize,
) -> HydroPlant {
    HydroPlant {
        id: HydroPlantId(id),
        name: format!("HYDRO-{id}"),
        submarket_id: SubmarketId(submarket_id),
        bus_id: BusId(bus_id),
        reservoir: Reservoir {
            min_volume_hm3: 0.0,
            max_volume_hm3: 1000.0,
            initial_volume_hm3: 500.0,
        },
        groups: vec![HydroGroup {
            id: HydroGroupId(id * 100 + 1),
            name: format!("GROUP-{id}-1"),
            units: vec![HydroUnit {
                id: HydroUnitId(id * 100 + 1),
                name: format!("UNIT-{id}-1"),
                min_generation_mw: 0.0,
                max_generation_mw: productivity * max_turbining_hm3 * 1000.0,
                max_turbining_hm3,
                initial_condition: HydroInitialCondition {
                    is_on: true,
                    generation_mw: 0.0,
                    time_in_state: 24,
                },
            }],
        }],
        fpha_segments: vec![],
        natural_inflow_hm3: vec![inflow_hm3; periods],
        water_withdrawal_hm3: vec![0.0; periods],
        spillage_cost_per_hm3: 0.0,
        turbining_cost_per_hm3: 0.0,
        specific_productivity_mw_per_m3s: productivity,
        hydraulic_loss_m: None,
        efficiency: None,
        upstream_plant_ids: vec![],
        downstream_plant_id: None,
        diversion_upstream_plant_ids: vec![],
        diversion_plant_id: None,
    }
}

/// Create FPHA segments for a hydro plant.
pub fn fpha_segments(productivity: f64) -> Vec<HydroFphaSegment> {
    vec![HydroFphaSegment {
        segment: 1,
        turbining_coefficient: productivity,
        volume_coefficient: 0.0,
        lateral_flow_coefficient: 0.0,
        rhs: 0.0,
        correction_factor: 1.0,
    }]
}

/// Create a minimal system with one thermal and optionally one hydro plant.
pub fn minimal_system() -> System {
    let horizon = minimal_horizon();
    let periods = horizon.periods;

    System {
        horizon,
        thermal_unit_commitment_enabled: false,
        hydro_unit_commitment_enabled: false,
        ton_residual_enabled: false,
        fpha_enabled: false,
        future_cost_enabled: false,
        residual_costs: vec![],
        future_cost_cuts: vec![],
        submarkets: vec![minimal_submarket(1, 100.0, periods)],
        interchange_limits: vec![],
        operational_limits: vec![],
        buses: vec![minimal_bus(1, 1, 100.0, periods)],
        branches: vec![],
        thermal_plants: vec![minimal_thermal(1, 1, 1, 50.0, 200.0)],
        hydro_plants: vec![],
        pumping_plants: vec![],
        renewable_plants: vec![],
    }
}

/// Create a system with both thermal and hydro plants.
pub fn minimal_system_with_hydro() -> System {
    let mut system = minimal_system();
    system.hydro_plants.push(minimal_hydro(
        1,
        1,
        1,
        1.0,
        0.1,
        10.0,
        system.horizon.periods,
    ));
    system
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn minimal_system_validates() {
        let system = minimal_system();
        assert!(system.validate().is_ok());
    }

    #[test]
    fn minimal_system_with_hydro_validates() {
        let system = minimal_system_with_hydro();
        assert!(system.validate().is_ok());
    }
}
