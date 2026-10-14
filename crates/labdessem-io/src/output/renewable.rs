//! Renewable plant result writer.

use std::{collections::HashMap, path::Path};

use labdessem_core::system::System;
use labdessem_model::names::renewable_generation_name;

use crate::error::IoError;
use crate::output::{
    csv_with_header, write_csv_file,
    helpers::{average_internal_values, original_period_internal_indices},
};

/// Write renewable plant results to CSV.
pub fn write_renewable_csv(
    system: &System,
    variable_values: &HashMap<String, f64>,
    output_dir: &Path,
) -> Result<(), IoError> {
    let mut csv = csv_with_header(
        "Periodo;Codigo;Nome;Submercado;GerProg;GeracaoMW",
        &[
            ("Periodo", "periodo do horizonte de estudo"),
            ("Codigo", "codigo da usina renovavel"),
            ("Nome", "nome da usina renovavel"),
            ("Submercado", "submercado da usina renovavel"),
            ("GerProg", "geracao programada maxima media no periodo em MW"),
            ("GeracaoMW", "geracao renovavel em MW"),
        ],
    );

    for plant in &system.renewable_plants {
        let submarket_name = system
            .submarkets
            .iter()
            .find(|s| s.id == plant.submarket_id)
            .map(|s| s.name.as_str())
            .unwrap_or("");

        for original_period in 1..=system.horizon.original_periods {
            let internal_periods = original_period_internal_indices(system, original_period);
            let ger_prog = average_internal_values(system, &internal_periods, |period| {
                plant.available_generation_mw[period]
            });
            let generation = average_internal_values(system, &internal_periods, |period| {
                variable_values
                    .get(&renewable_generation_name(&plant.name, period))
                    .copied()
                    .unwrap_or(0.0)
            });
            csv.push_str(&format!(
                "{};{};{};{};{:.6};{:.6}\n",
                original_period, plant.id.0, plant.name, submarket_name, ger_prog, generation
            ));
        }
    }

    write_csv_file(output_dir.join("resultado_renovaveis.csv"), csv)?;
    Ok(())
}
