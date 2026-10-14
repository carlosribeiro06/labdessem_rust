//! Pumping plant result writer.

use std::{collections::HashMap, path::Path};

use labdessem_core::system::System;
use labdessem_model::names::pumping_name;

use crate::error::IoError;
use crate::output::{
    csv_with_header, volume_hm3_to_flow_m3s, write_csv_file,
    helpers::{original_period_duration_hours, original_period_internal_indices, sum_internal_values},
};

/// Write pumping plant results to CSV.
pub fn write_pumping_csv(
    system: &System,
    variable_values: &HashMap<String, f64>,
    output_dir: &Path,
) -> Result<(), IoError> {
    let mut csv = csv_with_header(
        "Periodo;Codigo;Nome;Submercado;QbombMin;QbombMax;Qbomb;Consumo",
        &[
            ("Periodo", "periodo do horizonte de estudo"),
            ("Codigo", "codigo da usina elevatoria"),
            ("Nome", "nome da usina elevatoria"),
            ("Submercado", "submercado onde a elevatoria consome energia"),
            ("QbombMin", "vazao bombeada minima em m3/s"),
            ("QbombMax", "vazao bombeada maxima em m3/s"),
            ("Qbomb", "vazao bombeada media no periodo em m3/s"),
            ("Consumo", "potencia consumida no bombeamento em MW"),
        ],
    );

    for plant in &system.pumping_plants {
        let submarket_name = system
            .submarkets
            .iter()
            .find(|s| s.id == plant.submarket_id)
            .map_or("", |s| s.name.as_str());
        let min_pumping_m3s =
            volume_hm3_to_flow_m3s(plant.min_pumping_hm3, system.horizon.period_duration_hours);
        let max_pumping_m3s =
            volume_hm3_to_flow_m3s(plant.max_pumping_hm3, system.horizon.period_duration_hours);

        for original_period in 1..=system.horizon.original_periods {
            let internal_periods = original_period_internal_indices(system, original_period);
            let pumping_hm3 = sum_internal_values(&internal_periods, |period| {
                variable_values
                    .get(&pumping_name(&plant.name, period))
                    .copied()
                    .unwrap_or(0.0)
            });
            let pumping_m3s = volume_hm3_to_flow_m3s(
                pumping_hm3,
                original_period_duration_hours(system, original_period),
            );
            let consumption_mw = plant.specific_consumption_mw_per_m3s * pumping_m3s;

            csv.push_str(&format!(
                "{};{};{};{};{:.6};{:.6};{:.6};{:.6}\n",
                original_period,
                plant.id.0,
                plant.name,
                submarket_name,
                min_pumping_m3s,
                max_pumping_m3s,
                pumping_m3s,
                consumption_mw
            ));
        }
    }

    write_csv_file(output_dir.join("resultado_elevatorias.csv"), csv)?;
    Ok(())
}
