//! System validation tests.

// Rationale: Tests use expect() for cleaner failure messages
#![allow(clippy::unwrap_used, clippy::expect_used)]

use super::*;
use crate::{
    entities::{
        HydroFphaSegment, HydroGroup, HydroInitialCondition, HydroPlant, HydroUnit, RenewablePlant,
        Reservoir, ThermalInitialCondition, ThermalPlant, ThermalUnit,
    },
    ids::{
        BranchId, BusId, HydroGroupId, HydroPlantId, HydroUnitId, RenewablePlantId, SubmarketId,
        ThermalPlantId, ThermalUnitId,
    },
};

fn fpha_segments() -> Vec<HydroFphaSegment> {
    vec![HydroFphaSegment {
        segment: 1,
        correction_factor: 1.0,
        rhs: 0.0,
        volume_coefficient: 0.0,
        turbining_coefficient: 1.0,
        lateral_flow_coefficient: 0.0,
    }]
}

fn valid_system() -> System {
    System {
        horizon: StudyHorizon {
            periods: 2,
            period_duration_hours: 1.0,
            original_periods: 2,
            original_period_durations_hours: vec![1.0, 1.0],
            internal_to_original_period: vec![1, 2],
            internal_subperiod_index: vec![1, 1],
        },
        thermal_unit_commitment_enabled: true,
        hydro_unit_commitment_enabled: true,
        ton_residual_enabled: false,
        fpha_enabled: true,
        future_cost_enabled: false,
        residual_costs: vec![],
        future_cost_cuts: vec![],
        submarkets: vec![Submarket {
            id: SubmarketId(1),
            name: "SE".into(),
            demand_mw: vec![100.0, 110.0],
        }],
        interchange_limits: vec![],
        operational_limits: vec![],
        buses: vec![Bus {
            id: BusId(1),
            name: "BUS-1".into(),
            submarket_id: SubmarketId(1),
            angle_reference: true,
            demand_mw: vec![100.0, 110.0],
        }],
        branches: vec![],
        thermal_plants: vec![ThermalPlant {
            id: ThermalPlantId(1),
            name: "UTE-1".into(),
            submarket_id: SubmarketId(1),
            bus_id: BusId(1),
            units: vec![ThermalUnit {
                id: ThermalUnitId(1),
                name: "GT-1".into(),
                min_generation_mw: 20.0,
                max_generation_mw: 100.0,
                startup_trajectory_mw: vec![20.0, 40.0, 80.0],
                shutdown_trajectory_mw: vec![80.0, 40.0, 20.0],
                min_up_time: 1,
                min_down_time: 1,
                startup_cost: 500.0,
                shutdown_cost: 100.0,
                variable_cost_per_mwh: 250.0,
                initial_condition: ThermalInitialCondition {
                    is_on: true,
                    generation_mw: 40.0,
                    time_in_state: 2,
                    time_in_ramp: 2,
                    is_ramping_up: false,
                    is_ramping_down: false,
                },
            }],
        }],
        hydro_plants: vec![HydroPlant {
            id: HydroPlantId(1),
            name: "UHE-1".into(),
            submarket_id: SubmarketId(1),
            bus_id: BusId(1),
            upstream_plant_ids: vec![],
            downstream_plant_id: None,
            diversion_upstream_plant_ids: vec![],
            diversion_plant_id: None,
            fpha_segments: fpha_segments(),
            reservoir: Reservoir {
                min_volume_hm3: 10.0,
                max_volume_hm3: 100.0,
                initial_volume_hm3: 50.0,
            },
            natural_inflow_hm3: vec![5.0, 6.0],
            water_withdrawal_hm3: vec![0.0, 0.0],
            spillage_cost_per_hm3: 1.0,
            turbining_cost_per_hm3: 0.0,
            specific_productivity_mw_per_m3s: 1.0,
            groups: vec![HydroGroup {
                id: HydroGroupId(1),
                name: "CJ-1".into(),
                units: vec![HydroUnit {
                    id: HydroUnitId(1),
                    name: "UG-1".into(),
                    min_generation_mw: 10.0,
                    max_generation_mw: 80.0,
                    max_turbining_hm3: 40.0,
                    initial_condition: HydroInitialCondition {
                        is_on: true,
                        generation_mw: 20.0,
                        time_in_state: 1,
                    },
                }],
            }],
        }],
        pumping_plants: Vec::new(),
        renewable_plants: vec![RenewablePlant {
            id: RenewablePlantId(1),
            name: "REN-1".into(),
            submarket_id: SubmarketId(1),
            bus_id: BusId(1),
            available_generation_mw: vec![40.0, 40.0],
        }],
    }
}

#[test]
fn validates_a_consistent_system() {
    assert!(valid_system().validate().is_ok());
}

#[test]
fn rejects_system_without_single_reference_bus() {
    let mut system = valid_system();
    system.buses.push(Bus {
        id: BusId(2),
        name: "BUS-2".into(),
        submarket_id: SubmarketId(1),
        angle_reference: true,
        demand_mw: vec![0.0, 0.0],
    });
    system.branches.push(Branch {
        id: BranchId(1),
        name: "LT-1".into(),
        from_bus_id: BusId(1),
        to_bus_id: BusId(2),
        reactance_pu: 0.1,
        thermal_limit_mw: 100.0,
    });

    assert!(system.validate().is_err());
}

#[test]
fn rejects_inconsistent_hydro_cascade_links() {
    let mut system = valid_system();
    system.hydro_plants.push(HydroPlant {
        id: HydroPlantId(2),
        name: "UHE-2".into(),
        submarket_id: SubmarketId(1),
        bus_id: BusId(1),
        upstream_plant_ids: vec![],
        downstream_plant_id: None,
        diversion_upstream_plant_ids: vec![],
        diversion_plant_id: None,
        fpha_segments: fpha_segments(),
        reservoir: Reservoir {
            min_volume_hm3: 5.0,
            max_volume_hm3: 50.0,
            initial_volume_hm3: 20.0,
        },
        natural_inflow_hm3: vec![2.0, 2.0],
        water_withdrawal_hm3: vec![0.0, 0.0],
        spillage_cost_per_hm3: 1.0,
        turbining_cost_per_hm3: 0.0,
        specific_productivity_mw_per_m3s: 1.0,
        groups: vec![HydroGroup {
            id: HydroGroupId(2),
            name: "CJ-2".into(),
            units: vec![HydroUnit {
                id: HydroUnitId(2),
                name: "UG-2".into(),
                min_generation_mw: 5.0,
                max_generation_mw: 30.0,
                max_turbining_hm3: 15.0,
                initial_condition: HydroInitialCondition {
                    is_on: true,
                    generation_mw: 10.0,
                    time_in_state: 1,
                },
            }],
        }],
    });

    system.hydro_plants[0].downstream_plant_id = Some(HydroPlantId(2));

    assert!(system.validate().is_err());
}

#[test]
fn rejects_duplicated_upstream_reference() {
    let mut system = valid_system();
    system.hydro_plants.push(HydroPlant {
        id: HydroPlantId(2),
        name: "UHE-2".into(),
        submarket_id: SubmarketId(1),
        bus_id: BusId(1),
        upstream_plant_ids: vec![],
        downstream_plant_id: Some(HydroPlantId(1)),
        diversion_upstream_plant_ids: vec![],
        diversion_plant_id: None,
        fpha_segments: fpha_segments(),
        reservoir: Reservoir {
            min_volume_hm3: 5.0,
            max_volume_hm3: 50.0,
            initial_volume_hm3: 20.0,
        },
        natural_inflow_hm3: vec![2.0, 2.0],
        water_withdrawal_hm3: vec![0.0, 0.0],
        spillage_cost_per_hm3: 1.0,
        turbining_cost_per_hm3: 0.0,
        specific_productivity_mw_per_m3s: 1.0,
        groups: vec![HydroGroup {
            id: HydroGroupId(2),
            name: "CJ-2".into(),
            units: vec![HydroUnit {
                id: HydroUnitId(2),
                name: "UG-2".into(),
                min_generation_mw: 5.0,
                max_generation_mw: 30.0,
                max_turbining_hm3: 15.0,
                initial_condition: HydroInitialCondition {
                    is_on: true,
                    generation_mw: 10.0,
                    time_in_state: 1,
                },
            }],
        }],
    });
    system.hydro_plants[0].upstream_plant_ids = vec![HydroPlantId(2), HydroPlantId(2)];

    assert!(system.validate().is_err());
}

#[test]
fn rejects_duplicated_directed_interchange_limit() {
    let mut system = valid_system();
    system.submarkets.push(Submarket {
        id: SubmarketId(2),
        name: "S".into(),
        demand_mw: vec![50.0, 50.0],
    });
    system.interchange_limits = vec![
        InterchangeLimit {
            from_submarket_id: SubmarketId(1),
            to_submarket_id: SubmarketId(2),
            max_flow_mw: 100.0,
            penalty_cost_per_mwh: 1.0,
        },
        InterchangeLimit {
            from_submarket_id: SubmarketId(1),
            to_submarket_id: SubmarketId(2),
            max_flow_mw: 80.0,
            penalty_cost_per_mwh: 1.0,
        },
    ];

    assert!(system.validate().is_err());
}
