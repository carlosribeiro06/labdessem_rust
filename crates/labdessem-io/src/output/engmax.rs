//! EngMax (dynamic maximum swallowing) result writer.

use std::path::Path;

use labdessem_core::entities::EngMaxBounds;
use labdessem_core::system::System;

use crate::error::IoError;
use crate::output::{csv_with_header, write_csv_file};

/// Write EngMax bounds to CSV.
///
/// Output file: resultado_engmax.csv
/// Columns: Usina;Unidade;Periodo;EngMax_hm3
pub fn write_engmax_csv(
    system: &System,
    engmax_bounds: &EngMaxBounds,
    output_dir: &Path,
) -> Result<(), IoError> {
    let csv = build_engmax_csv(system, engmax_bounds);
    let path = output_dir.join("resultado_engmax.csv");
    write_csv_file(&path, csv)?;
    Ok(())
}

fn build_engmax_csv(system: &System, engmax_bounds: &EngMaxBounds) -> String {
    let mut csv = csv_with_header(
        "Usina;Unidade;Periodo;EngMax_hm3",
        &[
            ("Usina", "nome da usina hidreletrica"),
            ("Unidade", "nome da unidade geradora"),
            ("Periodo", "periodo do horizonte de estudo"),
            ("EngMax_hm3", "engolimento maximo dinamico em hm3"),
        ],
    );

    for (plant_idx, plant) in system.hydro_plants.iter().enumerate() {
        let mut unit_flat_idx = 0usize;
        for group in &plant.groups {
            for unit in &group.units {
                for period in 0..system.horizon.periods {
                    if let Some(bound) = engmax_bounds.get(plant_idx, unit_flat_idx, period) {
                        csv.push_str(&format!(
                            "{};{};{};{:.6}\n",
                            plant.name,
                            unit.name,
                            period + 1, // 1-indexed for output
                            bound
                        ));
                    }
                }
                unit_flat_idx += 1;
            }
        }
    }

    csv
}

#[cfg(test)]
mod tests {
    use super::*;
    use labdessem_core::entities::{
        HydroGroup, HydroInitialCondition, HydroPlant, HydroUnit, Reservoir, TurbineType,
    };
    use labdessem_core::ids::{BusId, HydroGroupId, HydroPlantId, HydroUnitId, SubmarketId};
    use labdessem_core::system::{StudyHorizon, Submarket, System};

    fn build_test_system() -> System {
        System {
            horizon: StudyHorizon {
                periods: 2,
                period_duration_hours: 1.0,
                original_periods: 2,
                original_period_durations_hours: vec![1.0, 1.0],
                internal_to_original_period: vec![1, 2],
                internal_subperiod_index: vec![1, 1],
            },
            thermal_unit_commitment_enabled: false,
            hydro_unit_commitment_enabled: false,
            ton_residual_enabled: false,
            fpha_enabled: false,
            future_cost_enabled: false,
            residual_costs: vec![],
            future_cost_cuts: vec![],
            submarkets: vec![Submarket {
                id: SubmarketId(1),
                name: "SE".into(),
                demand_mw: vec![100.0, 100.0],
            }],
            interchange_limits: vec![],
            operational_limits: vec![],
            buses: vec![],
            branches: vec![],
            thermal_plants: vec![],
            hydro_plants: vec![HydroPlant {
                id: HydroPlantId(1),
                name: "UHE-1".into(),
                submarket_id: SubmarketId(1),
                bus_id: BusId(1),
                upstream_plant_ids: vec![],
                downstream_plant_id: None,
                diversion_upstream_plant_ids: vec![],
                diversion_plant_id: None,
                fpha_segments: vec![],
                reservoir: Reservoir {
                    min_volume_hm3: 10.0,
                    max_volume_hm3: 100.0,
                    initial_volume_hm3: 50.0,
                },
                natural_inflow_hm3: vec![1.0, 1.0],
                water_withdrawal_hm3: vec![0.0, 0.0],
                spillage_cost_per_hm3: 0.0,
                turbining_cost_per_hm3: 0.0,
                specific_productivity_mw_per_m3s: 1.0,
                hydraulic_loss_m: None,
                efficiency: None,
                groups: vec![HydroGroup {
                    id: HydroGroupId(1),
                    name: "CJ-1".into(),
                    units: vec![HydroUnit {
                        id: HydroUnitId(1),
                        name: "UG-1".into(),
                        min_generation_mw: 0.0,
                        max_generation_mw: 50.0,
                        max_turbining_hm3: 10.0,
                        effective_head_m: 50.0,
                        initial_condition: HydroInitialCondition {
                            is_on: true,
                            generation_mw: 0.0,
                            time_in_state: 1,
                        },
                    }],
                }],
                turbine_type: TurbineType::Francis,
            }],
            pumping_plants: vec![],
            renewable_plants: vec![],
            inflow_history: vec![],
        }
    }

    #[test]
    fn build_engmax_csv_formats_correctly() {
        let system = build_test_system();
        let mut bounds = EngMaxBounds::new(10);
        bounds.set(0, 0, 0, 8.123456789);
        bounds.set(0, 0, 1, 9.987654321);

        let csv = build_engmax_csv(&system, &bounds);

        assert!(csv.contains("Usina;Unidade;Periodo;EngMax_hm3"));
        assert!(csv.contains("UHE-1;UG-1;1;8.123457"));
        assert!(csv.contains("UHE-1;UG-1;2;9.987654"));
    }

    #[test]
    fn build_engmax_csv_empty_bounds() {
        let system = build_test_system();
        let bounds = EngMaxBounds::new(10);

        let csv = build_engmax_csv(&system, &bounds);

        // Should have header but no data rows
        assert!(csv.contains("Usina;Unidade;Periodo;EngMax_hm3"));
        let lines: Vec<&str> = csv.lines().collect();
        // Header has several lines from csv_with_header, data would be at the end
        let data_lines: Vec<&str> = lines
            .iter()
            .filter(|l| l.starts_with("UHE"))
            .cloned()
            .collect();
        assert!(data_lines.is_empty());
    }
}
