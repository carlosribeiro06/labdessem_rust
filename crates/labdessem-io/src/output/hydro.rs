//! Hydro plant result writer.

use std::collections::HashMap;
use std::path::Path;

use labdessem_core::system::System;
use labdessem_model::names::{
    hydro_balance_constraint_name, hydro_commitment_name, hydro_diversion_name,
    hydro_generation_name, hydro_spillage_name, hydro_turbining_name, hydro_volume_name,
};

use crate::error::IoError;
use crate::output::{
    average_internal_values, csv_with_header, hydro_generation_max_at_point_mw,
    hydro_pumping_out_hm3, hydro_upstream_inflow_hm3, original_period_duration_hours,
    original_period_internal_indices, sum_internal_values, volume_hm3_to_flow_m3s, write_csv_file,
};

/// Write hydro plant results to CSV.
pub fn write_hydro_csv(
    system: &System,
    variable_values: &HashMap<String, f64>,
    constraint_duals: &HashMap<String, f64>,
    output_dir: &Path,
) -> Result<(), IoError> {
    let mut csv = csv_with_header(
        "Usina;Conjunto;Unidade;Periodo;VolumeHM3;VolumeUtilHM3;VolumeUtilMaxHM3;VolumeUtilPct;GeracaoMW;PmaxMW;GeracaoMaxPontoMW;StatusOn;StatusOff;TurbinamentoHM3;TurbinamentoM3s;TurbinamentoMaxHM3;TurbinamentoMaxM3s;VertimentoHM3;VertimentoM3s;DesvioHM3;DesvioM3s;AfluenciaM3s;AfluenciaHM3;RetiradaM3s;RetiradaHM3;BombeamentoHM3;BombeamentoM3s;Qmon;Vmon;PiBalHidr",
        &[
            ("Usina", "nome da usina hidreletrica"),
            (
                "Conjunto",
                "codigo do conjunto; 99 indica agregado da usina",
            ),
            ("Unidade", "codigo da unidade; 99 indica agregado da usina"),
            ("Periodo", "periodo do horizonte de estudo"),
            ("VolumeHM3", "volume armazenado em hm3"),
            ("VolumeUtilHM3", "volume util armazenado em hm3"),
            ("VolumeUtilMaxHM3", "volume util maximo da usina em hm3"),
            (
                "VolumeUtilPct",
                "volume util armazenado em percentual do volume util maximo",
            ),
            ("GeracaoMW", "geracao hidreletrica em MW"),
            (
                "PmaxMW",
                "potencia maxima cadastrada da unidade em MW; no agregado, soma das unidades",
            ),
            (
                "GeracaoMaxPontoMW",
                "capacidade maxima de geracao da usina com o volume do ponto de operacao",
            ),
            (
                "StatusOn",
                "fracao do periodo original em que a unidade ficou ligada",
            ),
            (
                "StatusOff",
                "fracao do periodo original em que a unidade ficou desligada",
            ),
            ("TurbinamentoHM3", "turbinamento no periodo em hm3"),
            ("TurbinamentoM3s", "turbinamento medio no periodo em m3/s"),
            (
                "TurbinamentoMaxHM3",
                "turbinamento maximo da unidade no periodo em hm3",
            ),
            (
                "TurbinamentoMaxM3s",
                "turbinamento maximo medio da unidade no periodo em m3/s",
            ),
            ("VertimentoHM3", "vertimento no periodo em hm3"),
            ("VertimentoM3s", "vertimento medio no periodo em m3/s"),
            ("DesvioHM3", "vazao desviada no periodo em hm3"),
            ("DesvioM3s", "vazao desviada media no periodo em m3/s"),
            ("AfluenciaM3s", "afluencia natural media no periodo em m3/s"),
            ("AfluenciaHM3", "afluencia natural no periodo em hm3"),
            (
                "RetiradaM3s",
                "retirada media para usos alternativos no periodo em m3/s",
            ),
            (
                "RetiradaHM3",
                "retirada para usos alternativos no periodo em hm3",
            ),
            (
                "BombeamentoHM3",
                "saida de agua por bombeamento no periodo em hm3",
            ),
            (
                "BombeamentoM3s",
                "saida media de agua por bombeamento no periodo em m3/s",
            ),
            (
                "Qmon",
                "entrada media de agua de montante no periodo em m3/s",
            ),
            ("Vmon", "entrada de agua de montante no periodo em hm3"),
            (
                "PiBalHidr",
                "dual do balanco hidrico no LP-CALC-CMO em R$/hm3",
            ),
        ],
    );

    for plant in &system.hydro_plants {
        for original_period in 1..=system.horizon.original_periods {
            let internal_periods = original_period_internal_indices(system, original_period);
            let period_duration_hours = original_period_duration_hours(system, original_period);
            let last_internal_period = internal_periods
                .last()
                .copied()
                .unwrap_or(original_period - 1);

            // Volume
            let volume = variable_values
                .get(&hydro_volume_name(
                    plant.name.as_str(),
                    last_internal_period + 1,
                ))
                .copied()
                .unwrap_or(0.0);
            let useful_volume_hm3 = volume - plant.reservoir.min_volume_hm3;
            let max_useful_volume_hm3 =
                plant.reservoir.max_volume_hm3 - plant.reservoir.min_volume_hm3;
            let useful_volume_pct = if max_useful_volume_hm3.abs() <= 1e-8 {
                0.0
            } else {
                100.0 * useful_volume_hm3 / max_useful_volume_hm3
            };

            // Spillage
            let spillage = sum_internal_values(&internal_periods, |period| {
                variable_values
                    .get(&hydro_spillage_name(plant.name.as_str(), period))
                    .copied()
                    .unwrap_or(0.0)
            });
            let spillage_m3s = volume_hm3_to_flow_m3s(spillage, period_duration_hours);

            // Diversion
            let diversion = sum_internal_values(&internal_periods, |period| {
                variable_values
                    .get(&hydro_diversion_name(plant.name.as_str(), period))
                    .copied()
                    .unwrap_or(0.0)
            });
            let diversion_m3s = volume_hm3_to_flow_m3s(diversion, period_duration_hours);

            // Inflow
            let inflow_hm3 =
                sum_internal_values(&internal_periods, |period| plant.natural_inflow_hm3[period]);
            let inflow_m3s = volume_hm3_to_flow_m3s(inflow_hm3, period_duration_hours);

            // Withdrawal
            let withdrawal_hm3 = sum_internal_values(&internal_periods, |period| {
                plant.water_withdrawal_hm3[period]
            });
            let withdrawal_m3s = volume_hm3_to_flow_m3s(withdrawal_hm3, period_duration_hours);

            // Pumping out
            let pumping_out_hm3 = sum_internal_values(&internal_periods, |period| {
                hydro_pumping_out_hm3(system, variable_values, plant.id, period)
            });
            let pumping_out_m3s = volume_hm3_to_flow_m3s(pumping_out_hm3, period_duration_hours);

            // Upstream inflow
            let upstream_inflow_hm3 = sum_internal_values(&internal_periods, |period| {
                hydro_upstream_inflow_hm3(system, variable_values, plant, period)
            });
            let upstream_inflow_m3s =
                volume_hm3_to_flow_m3s(upstream_inflow_hm3, period_duration_hours);

            // Dual
            let pi_bal_hidr = average_internal_values(system, &internal_periods, |period| {
                constraint_duals
                    .get(&hydro_balance_constraint_name(plant.name.as_str(), period))
                    .copied()
                    .unwrap_or(0.0)
            });

            // Plant-level aggregates
            let total_max_generation = plant
                .groups
                .iter()
                .flat_map(|group| group.units.iter())
                .map(|unit| unit.max_generation_mw)
                .sum::<f64>();
            let plant_generation_max_at_point =
                average_internal_values(system, &internal_periods, |period| {
                    hydro_generation_max_at_point_mw(
                        system,
                        variable_values,
                        plant,
                        period,
                        total_max_generation,
                    )
                });

            let mut total_generation = 0.0;
            let mut total_turbining = 0.0;
            let mut total_status_fraction = 0.0;
            let mut unit_count = 0usize;

            // Unit-level rows
            for group in &plant.groups {
                for unit in &group.units {
                    let generation = average_internal_values(system, &internal_periods, |period| {
                        variable_values
                            .get(&hydro_generation_name(
                                plant.name.as_str(),
                                group.name.as_str(),
                                unit.name.as_str(),
                                period,
                            ))
                            .copied()
                            .unwrap_or(0.0)
                    });
                    let status_on_fraction =
                        average_internal_values(system, &internal_periods, |period| {
                            let commitment_name = hydro_commitment_name(
                                plant.name.as_str(),
                                group.name.as_str(),
                                unit.name.as_str(),
                                period,
                            );
                            variable_values
                                .get(&commitment_name)
                                .copied()
                                .unwrap_or_else(|| {
                                    if variable_values
                                        .get(&hydro_generation_name(
                                            plant.name.as_str(),
                                            group.name.as_str(),
                                            unit.name.as_str(),
                                            period,
                                        ))
                                        .copied()
                                        .unwrap_or(0.0)
                                        .abs()
                                        > 1e-8
                                    {
                                        1.0
                                    } else {
                                        0.0
                                    }
                                })
                        });
                    let status_off_fraction = 1.0 - status_on_fraction;
                    let turbining = sum_internal_values(&internal_periods, |period| {
                        variable_values
                            .get(&hydro_turbining_name(
                                plant.name.as_str(),
                                group.name.as_str(),
                                unit.name.as_str(),
                                period,
                            ))
                            .copied()
                            .unwrap_or(0.0)
                    });
                    let unit_turbining_max_hm3 =
                        unit.max_turbining_hm3 * internal_periods.len() as f64;
                    let unit_turbining_max_m3s =
                        volume_hm3_to_flow_m3s(unit_turbining_max_hm3, period_duration_hours);
                    let turbining_m3s = volume_hm3_to_flow_m3s(turbining, period_duration_hours);

                    total_generation += generation;
                    total_turbining += turbining;
                    total_status_fraction += status_on_fraction;
                    unit_count += 1;

                    csv.push_str(&format!(
                        "{};{};{};{};{:.6};{:.6};{:.6};{:.6};{:.6};{:.6};;{:.6};{:.6};{:.6};{:.6};{:.6};{:.6};{:.6};{:.6};{:.6};{:.6};{:.6};{:.6};{:.6};{:.6};{:.6};{:.6};{:.6};{:.6};{:.6}\n",
                        plant.name,
                        group.id.0,
                        unit.id.0,
                        original_period,
                        volume,
                        useful_volume_hm3,
                        max_useful_volume_hm3,
                        useful_volume_pct,
                        generation,
                        unit.max_generation_mw,
                        status_on_fraction,
                        status_off_fraction,
                        turbining,
                        turbining_m3s,
                        unit_turbining_max_hm3,
                        unit_turbining_max_m3s,
                        spillage,
                        spillage_m3s,
                        diversion,
                        diversion_m3s,
                        inflow_m3s,
                        inflow_hm3,
                        withdrawal_m3s,
                        withdrawal_hm3,
                        pumping_out_hm3,
                        pumping_out_m3s,
                        upstream_inflow_m3s,
                        upstream_inflow_hm3,
                        pi_bal_hidr
                    ));
                }
            }

            // Aggregate row (99;99)
            let total_turbining_m3s =
                volume_hm3_to_flow_m3s(total_turbining, period_duration_hours);
            let total_turbining_max_hm3 = plant
                .groups
                .iter()
                .flat_map(|group| group.units.iter())
                .map(|unit| unit.max_turbining_hm3)
                .sum::<f64>()
                * internal_periods.len() as f64;
            let total_turbining_max_m3s =
                volume_hm3_to_flow_m3s(total_turbining_max_hm3, period_duration_hours);
            let aggregate_status_on = if unit_count == 0 {
                0.0
            } else {
                total_status_fraction / unit_count as f64
            };
            let aggregate_status_off = 1.0 - aggregate_status_on;

            csv.push_str(&format!(
                "{};99;99;{};{:.6};{:.6};{:.6};{:.6};{:.6};{:.6};{:.6};{:.6};{:.6};{:.6};{:.6};{:.6};{:.6};{:.6};{:.6};{:.6};{:.6};{:.6};{:.6};{:.6};{:.6};{:.6};{:.6};{:.6};{:.6};{:.6}\n",
                plant.name,
                original_period,
                volume,
                useful_volume_hm3,
                max_useful_volume_hm3,
                useful_volume_pct,
                total_generation,
                total_max_generation,
                plant_generation_max_at_point,
                aggregate_status_on,
                aggregate_status_off,
                total_turbining,
                total_turbining_m3s,
                total_turbining_max_hm3,
                total_turbining_max_m3s,
                spillage,
                spillage_m3s,
                diversion,
                diversion_m3s,
                inflow_m3s,
                inflow_hm3,
                withdrawal_m3s,
                withdrawal_hm3,
                pumping_out_hm3,
                pumping_out_m3s,
                upstream_inflow_m3s,
                upstream_inflow_hm3,
                pi_bal_hidr
            ));
        }
    }

    write_csv_file(output_dir.join("resultado_hidreletricas.csv"), csv)?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn csv_header_has_correct_columns() {
        let header = csv_with_header("Usina;Conjunto;Unidade;Periodo", &[]);
        assert!(header.contains("ONS - Operador Nacional"));
        assert!(header.contains("Usina;Conjunto;Unidade;Periodo"));
    }
}
