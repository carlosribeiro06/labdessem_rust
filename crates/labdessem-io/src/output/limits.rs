//! Operational limits result writer.

use std::{collections::BTreeSet, path::Path};

use labdessem_core::system::{OperationalLimitTarget, System};

use crate::error::IoError;
use crate::output::{csv_with_header, operational_limit_variable_label, write_csv_file};

/// Write operational limits configuration to CSV.
pub fn write_operational_limits_csv(system: &System, output_dir: &Path) -> Result<(), IoError> {
    let mut csv = csv_with_header(
        "TipoUsina;CodigoUsina;NomeUsina;Variavel;Periodo;Linf;Lsup",
        &[
            ("TipoUsina", "tipo da usina associada ao limite"),
            ("CodigoUsina", "codigo da usina associada ao limite"),
            ("NomeUsina", "nome da usina associada ao limite"),
            ("Variavel", "variavel limitada"),
            ("Periodo", "periodo de aplicacao do limite"),
            ("Linf", "limite inferior informado"),
            ("Lsup", "limite superior informado"),
        ],
    );

    for limit in &system.operational_limits {
        let (plant_type, plant_code) = match limit.target {
            OperationalLimitTarget::ThermalPlant(id) => ("TERMICA", id.0),
            OperationalLimitTarget::HydroPlant(id) => ("HIDRAULICA", id.0),
            OperationalLimitTarget::PumpingPlant(id) => ("ELEVATORIA", id.0),
        };

        let original_periods = (limit.start_period..=limit.end_period)
            .filter_map(|period| {
                system
                    .horizon
                    .internal_to_original_period
                    .get(period - 1)
                    .copied()
            })
            .collect::<BTreeSet<_>>();

        for period in original_periods {
            let lower_bound = limit
                .lower_bound
                .map(|value| format!("{value:.6}"))
                .unwrap_or_default();
            let upper_bound = limit
                .upper_bound
                .map(|value| format!("{value:.6}"))
                .unwrap_or_default();

            csv.push_str(&format!(
                "{};{};{};{};{};{};{}\n",
                plant_type,
                plant_code,
                limit.plant_name,
                operational_limit_variable_label(limit.variable),
                period,
                lower_bound,
                upper_bound
            ));
        }
    }

    write_csv_file(output_dir.join("resultado_rest_lim.csv"), csv)?;
    Ok(())
}
