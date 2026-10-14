//! Summary and auxiliary result writers.

use std::{collections::HashMap, path::Path};

use labdessem_core::system::System;
use labdessem_model::names::demand_balance_constraint_name;

use crate::error::IoError;
use crate::output::{
    csv_with_header, write_csv_file,
    helpers::{average_internal_values, original_period_internal_indices},
};

/// Write marginal operating cost (CMO) results to CSV.
pub fn write_cmosist_csv(
    system: &System,
    constraint_duals: &HashMap<String, f64>,
    output_dir: &Path,
) -> Result<(), IoError> {
    let mut csv = csv_with_header(
        "Periodo;Submercado;PiDemanda",
        &[
            ("Periodo", "periodo do horizonte de estudo"),
            ("Submercado", "nome do submercado"),
            (
                "PiDemanda",
                "dual da restricao de atendimento a demanda no LP-CALC-CMO em R$/MWh",
            ),
        ],
    );

    for original_period in 1..=system.horizon.original_periods {
        let internal_periods = original_period_internal_indices(system, original_period);
        for submarket in &system.submarkets {
            let pi_demand = average_internal_values(system, &internal_periods, |period| {
                let constraint_name =
                    demand_balance_constraint_name(&submarket.name, period);
                let raw_pi_demand = constraint_duals
                    .get(&constraint_name)
                    .copied()
                    .unwrap_or(0.0);
                raw_pi_demand / system.horizon.period_duration_hours
            });
            csv.push_str(&format!(
                "{};{};{:.6}\n",
                original_period, submarket.name, pi_demand
            ));
        }
    }

    write_csv_file(output_dir.join("resultado_cmosist.csv"), csv)?;
    Ok(())
}

/// Write future cost function economic data to CSV.
pub fn write_future_cost_eco_csv(system: &System, output_dir: &Path) -> Result<(), IoError> {
    let mut csv = csv_with_header(
        "Estagio;Corte;Slot;Iteracao;ForwardPass;Ativo;CodigoUsina;Usina;CoefLinear;CoefAngular",
        &[
            (
                "Estagio",
                "estagio do arquivo binario da funcao de custo futuro",
            ),
            ("Corte", "identificador unico do corte"),
            ("Slot", "slot do corte na politica"),
            ("Iteracao", "iteracao que gerou o corte"),
            ("ForwardPass", "indice do forward pass que gerou o corte"),
            ("Ativo", "indica se o corte estava ativo na politica"),
            ("CodigoUsina", "codigo da usina hidreletrica"),
            ("Usina", "nome da usina hidreletrica"),
            ("CoefLinear", "coeficiente linear comum do corte"),
            ("CoefAngular", "coeficiente angular da usina no corte"),
        ],
    );

    for cut in &system.future_cost_cuts {
        for (plant_idx, plant) in system.hydro_plants.iter().enumerate() {
            let coefficient = cut.coefficients.get(plant_idx).copied().unwrap_or(0.0);
            csv.push_str(&format!(
                "{};{};{};{};{};{};{};{};{:.6};{:.6}\n",
                cut.stage_id,
                cut.cut_id,
                cut.slot_index,
                cut.iteration,
                cut.forward_pass_index,
                if cut.is_active { "SIM" } else { "NAO" },
                plant.id.0,
                plant.name,
                cut.intercept,
                coefficient
            ));
        }
    }

    write_csv_file(output_dir.join("resultado_fcf_eco.csv"), csv)?;
    Ok(())
}
