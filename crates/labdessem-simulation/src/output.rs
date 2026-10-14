//! Output writers for simulation results.

use std::collections::BTreeMap;
use std::path::Path;

use labdessem_core::system::System;
use labdessem_io::output::{
    csv_with_header, volume_hm3_to_flow_m3s, write_cmosist_csv, write_csv_file,
    write_future_cost_eco_csv, write_operational_limits_csv, write_pumping_csv,
    write_renewable_csv, write_thermal_csv,
};
use labdessem_model::names::{
    hydro_balance_constraint_name, hydro_commitment_name, hydro_diversion_name,
    hydro_generation_name, hydro_spillage_name, hydro_turbining_name, hydro_volume_name,
    pumping_name,
};
use labdessem_solver::SolveSummary;

use crate::error::SimulationError;
use crate::results::{
    AddedFlowCutLineResult, IterationReport, IterativeSimulationResult, LineFlowResult,
    NetworkInfeasibilityRecord, OperationalLimitInfeasibilityRecord,
};

/// Write all result CSV files for a simulation run.
pub fn write_results_csvs(
    system: &System,
    result: &IterativeSimulationResult,
    output_dir: impl AsRef<Path>,
    network_enabled: bool,
) -> Result<(), SimulationError> {
    let output_dir = output_dir.as_ref();
    std::fs::create_dir_all(output_dir).map_err(|error| {
        SimulationError::IterativeProcess(format!(
            "failed to create output directory {}: {error}",
            output_dir.display()
        ))
    })?;

    write_hydro_csv(
        system,
        &result.final_summary,
        &result.cmo_summary,
        output_dir,
    )?;
    write_thermal_csv(system, &result.final_summary.variable_values, output_dir)?;
    write_pumping_csv(system, &result.final_summary.variable_values, output_dir)?;
    write_cmosist_csv(system, &result.cmo_summary.constraint_duals, output_dir)?;
    write_process_iterations_csv(&result.steps, output_dir)?;
    write_operational_limits_csv(system, output_dir)?;
    write_operational_limit_infeasibility_csv(
        &result.operational_limit_infeasibilities,
        output_dir,
    )?;
    if system.future_cost_enabled {
        write_future_cost_eco_csv(system, output_dir)?;
    }
    if network_enabled {
        write_network_csv(system, &result.final_line_flows, output_dir)?;
        write_added_flow_cut_lines_csv(&result.added_flow_cut_lines, output_dir)?;
        write_network_infeasibility_csv(&result.network_infeasibilities, output_dir)?;
    }
    write_renewable_csv(system, &result.final_summary.variable_values, output_dir)?;

    Ok(())
}

// --- Period Helpers ---

fn original_period_internal_indices(system: &System, original_period: usize) -> Vec<usize> {
    system.horizon.internal_periods_for_original(original_period)
}

fn original_period_duration_hours(system: &System, original_period: usize) -> f64 {
    original_period_internal_indices(system, original_period).len() as f64
        * system.horizon.period_duration_hours
}

fn average_internal_values<F: Fn(usize) -> f64>(
    system: &System,
    periods: &[usize],
    values: F,
) -> f64 {
    if periods.is_empty() {
        return 0.0;
    }
    let total_duration: f64 = periods.len() as f64 * system.horizon.period_duration_hours;
    let weighted_sum: f64 = periods
        .iter()
        .map(|&period| values(period) * system.horizon.period_duration_hours)
        .sum();
    weighted_sum / total_duration
}

fn sum_internal_values<F: Fn(usize) -> f64>(periods: &[usize], values: F) -> f64 {
    periods.iter().map(|&period| values(period)).sum()
}

// --- Hydro Helpers ---

fn hydro_upstream_inflow_hm3(
    system: &System,
    summary: &SolveSummary,
    plant: &labdessem_core::entities::hydro::HydroPlant,
    period: usize,
) -> f64 {
    let mut total = 0.0;

    for upstream_id in &plant.upstream_plant_ids {
        if let Some(upstream) = system
            .hydro_plants
            .iter()
            .find(|candidate| candidate.id == *upstream_id)
        {
            total += hydro_total_turbining_hm3(summary, upstream, period);
            total += summary
                .variable_values
                .get(&hydro_spillage_name(upstream.name.as_str(), period))
                .copied()
                .unwrap_or(0.0);
        }
    }

    for upstream_id in &plant.diversion_upstream_plant_ids {
        if let Some(upstream) = system
            .hydro_plants
            .iter()
            .find(|candidate| candidate.id == *upstream_id)
        {
            total += summary
                .variable_values
                .get(&hydro_diversion_name(upstream.name.as_str(), period))
                .copied()
                .unwrap_or(0.0);
        }
    }

    for pumping_plant in &system.pumping_plants {
        if pumping_plant.upstream_hydro_id == plant.id {
            total += summary
                .variable_values
                .get(&pumping_name(pumping_plant.name.as_str(), period))
                .copied()
                .unwrap_or(0.0);
        }
    }

    total
}

fn hydro_pumping_out_hm3(
    system: &System,
    summary: &SolveSummary,
    plant_id: labdessem_core::ids::HydroPlantId,
    period: usize,
) -> f64 {
    system
        .pumping_plants
        .iter()
        .filter(|pumping_plant| pumping_plant.downstream_hydro_id == plant_id)
        .map(|pumping_plant| {
            summary
                .variable_values
                .get(&pumping_name(pumping_plant.name.as_str(), period))
                .copied()
                .unwrap_or(0.0)
        })
        .sum()
}

fn hydro_total_turbining_hm3(
    summary: &SolveSummary,
    plant: &labdessem_core::entities::hydro::HydroPlant,
    period: usize,
) -> f64 {
    plant
        .groups
        .iter()
        .flat_map(|group| {
            group.units.iter().map(move |unit| {
                summary
                    .variable_values
                    .get(&hydro_turbining_name(
                        plant.name.as_str(),
                        group.name.as_str(),
                        unit.name.as_str(),
                        period,
                    ))
                    .copied()
                    .unwrap_or(0.0)
            })
        })
        .sum()
}

fn hydro_generation_max_at_point_mw(
    system: &System,
    summary: &SolveSummary,
    plant: &labdessem_core::entities::hydro::HydroPlant,
    period: usize,
    plant_max_generation_mw: f64,
) -> f64 {
    let flow_conversion = system.horizon.period_duration_hours * 0.0036;
    let max_turbining = plant
        .groups
        .iter()
        .flat_map(|group| group.units.iter())
        .map(|unit| unit.max_turbining_hm3)
        .sum::<f64>();

    if !system.fpha_enabled || plant.fpha_segments.is_empty() {
        let generation_from_productivity =
            plant.specific_productivity_mw_per_m3s / flow_conversion * max_turbining;
        return plant_max_generation_mw
            .min(generation_from_productivity)
            .max(0.0);
    }

    let volume = summary
        .variable_values
        .get(&hydro_volume_name(plant.name.as_str(), period + 1))
        .copied()
        .unwrap_or(0.0);

    let fpha_max_generation = plant
        .fpha_segments
        .iter()
        .map(|segment| {
            segment.correction_factor * segment.rhs
                + segment.correction_factor * segment.volume_coefficient * volume
                + segment.correction_factor * segment.turbining_coefficient / flow_conversion
                    * max_turbining
        })
        .fold(f64::INFINITY, f64::min);

    plant_max_generation_mw.min(fpha_max_generation).max(0.0)
}

// --- Hydro Writer ---

fn write_hydro_csv(
    system: &System,
    summary: &SolveSummary,
    cmo_summary: &SolveSummary,
    output_dir: &Path,
) -> Result<(), SimulationError> {
    let mut csv = csv_with_header(
        "Usina;Conjunto;Unidade;Periodo;VolumeHM3;VolumeUtilHM3;VolumeUtilMaxHM3;VolumeUtilPct;GeracaoMW;PmaxMW;GeracaoMaxPontoMW;StatusOn;StatusOff;TurbinamentoHM3;TurbinamentoM3s;TurbinamentoMaxHM3;TurbinamentoMaxM3s;VertimentoHM3;VertimentoM3s;DesvioHM3;DesvioM3s;AfluenciaM3s;AfluenciaHM3;RetiradaM3s;RetiradaHM3;BombeamentoHM3;BombeamentoM3s;Qmon;Vmon;PiBalHidr",
        &[
            ("Usina", "nome da usina hidreletrica"),
            ("Conjunto", "codigo do conjunto; 99 indica agregado da usina"),
            ("Unidade", "codigo da unidade; 99 indica agregado da usina"),
            ("Periodo", "periodo do horizonte de estudo"),
            ("VolumeHM3", "volume armazenado em hm3"),
            ("VolumeUtilHM3", "volume util armazenado em hm3"),
            ("VolumeUtilMaxHM3", "volume util maximo da usina em hm3"),
            ("VolumeUtilPct", "volume util armazenado em percentual do volume util maximo"),
            ("GeracaoMW", "geracao hidreletrica em MW"),
            ("PmaxMW", "potencia maxima cadastrada da unidade em MW; no agregado, soma das unidades"),
            ("GeracaoMaxPontoMW", "capacidade maxima de geracao da usina com o volume do ponto de operacao"),
            ("StatusOn", "fracao do periodo original em que a unidade ficou ligada"),
            ("StatusOff", "fracao do periodo original em que a unidade ficou desligada"),
            ("TurbinamentoHM3", "turbinamento no periodo em hm3"),
            ("TurbinamentoM3s", "turbinamento medio no periodo em m3/s"),
            ("TurbinamentoMaxHM3", "turbinamento maximo da unidade no periodo em hm3"),
            ("TurbinamentoMaxM3s", "turbinamento maximo medio da unidade no periodo em m3/s"),
            ("VertimentoHM3", "vertimento no periodo em hm3"),
            ("VertimentoM3s", "vertimento medio no periodo em m3/s"),
            ("DesvioHM3", "vazao desviada no periodo em hm3"),
            ("DesvioM3s", "vazao desviada media no periodo em m3/s"),
            ("AfluenciaM3s", "afluencia natural media no periodo em m3/s"),
            ("AfluenciaHM3", "afluencia natural no periodo em hm3"),
            ("RetiradaM3s", "retirada media para usos alternativos no periodo em m3/s"),
            ("RetiradaHM3", "retirada para usos alternativos no periodo em hm3"),
            ("BombeamentoHM3", "saida de agua por bombeamento no periodo em hm3"),
            ("BombeamentoM3s", "saida media de agua por bombeamento no periodo em m3/s"),
            ("Qmon", "entrada media de agua de montante no periodo em m3/s"),
            ("Vmon", "entrada de agua de montante no periodo em hm3"),
            ("PiBalHidr", "dual do balanco hidrico no LP-CALC-CMO em R$/hm3"),
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
            let volume = summary
                .variable_values
                .get(&hydro_volume_name(plant.name.as_str(), last_internal_period + 1))
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
                summary
                    .variable_values
                    .get(&hydro_spillage_name(plant.name.as_str(), period))
                    .copied()
                    .unwrap_or(0.0)
            });
            let spillage_m3s = volume_hm3_to_flow_m3s(spillage, period_duration_hours);

            // Diversion
            let diversion = sum_internal_values(&internal_periods, |period| {
                summary
                    .variable_values
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
                hydro_pumping_out_hm3(system, summary, plant.id, period)
            });
            let pumping_out_m3s = volume_hm3_to_flow_m3s(pumping_out_hm3, period_duration_hours);

            // Upstream inflow
            let upstream_inflow_hm3 = sum_internal_values(&internal_periods, |period| {
                hydro_upstream_inflow_hm3(system, summary, plant, period)
            });
            let upstream_inflow_m3s =
                volume_hm3_to_flow_m3s(upstream_inflow_hm3, period_duration_hours);

            // Dual
            let pi_bal_hidr = average_internal_values(system, &internal_periods, |period| {
                cmo_summary
                    .constraint_duals
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
                        summary,
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
                        summary
                            .variable_values
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
                            summary
                                .variable_values
                                .get(&commitment_name)
                                .copied()
                                .unwrap_or_else(|| {
                                    if summary
                                        .variable_values
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
                        summary
                            .variable_values
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

    write_csv_to_file(output_dir.join("resultado_hidreletricas.csv"), csv)
}

// --- Process Iterations Writer ---

fn write_process_iterations_csv(
    steps: &[IterationReport],
    output_dir: &Path,
) -> Result<(), SimulationError> {
    let mut csv = csv_with_header(
        "Etapa;Iteracao;NumeroRestricoes;NumeroVariaveis;ValorFuncaoObjetivo;StatusSolucao;TempoResolucaoSeg",
        &[
            ("Etapa", "etapa do processo iterativo"),
            ("Iteracao", "numero da iteracao dentro da etapa"),
            ("NumeroRestricoes", "quantidade de restricoes do problema resolvido"),
            ("NumeroVariaveis", "quantidade de variaveis do problema resolvido"),
            ("ValorFuncaoObjetivo", "valor da funcao objetivo em R$"),
            ("StatusSolucao", "status retornado para a solucao"),
            ("TempoResolucaoSeg", "tempo de resolucao em segundos"),
        ],
    );

    for step in steps {
        csv.push_str(&format!(
            "{};{};{};{};{:.6};{};{:.6}\n",
            step.stage.label(),
            step.iteration,
            step.constraint_count,
            step.variable_count,
            step.objective_value,
            step.solution_status,
            step.solve_time.as_secs_f64()
        ));
    }

    write_csv_to_file(output_dir.join("resultado_processo_iter.csv"), csv)
}

// --- Network Writers ---

fn write_network_csv(
    system: &System,
    line_flows: &[LineFlowResult],
    output_dir: &Path,
) -> Result<(), SimulationError> {
    let mut csv = csv_with_header(
        "Linha;BarraDe;BarraPara;Periodo;FluxoMW;LimiteMW;Violacao",
        &[
            ("Linha", "nome da linha de transmissao"),
            ("BarraDe", "barra de origem da linha"),
            ("BarraPara", "barra de destino da linha"),
            ("Periodo", "periodo do horizonte de estudo"),
            ("FluxoMW", "fluxo de potencia calculado em MW"),
            ("LimiteMW", "capacidade da linha em MW"),
            ("Violacao", "indicador de violacao do limite de fluxo"),
        ],
    );

    #[derive(Default)]
    struct NetworkOutputAccumulator {
        branch_name: String,
        from_bus_name: String,
        to_bus_name: String,
        weighted_flow_sum: f64,
        duration_hours: f64,
        limit_mw: f64,
        violated: bool,
    }

    let mut aggregated = BTreeMap::<(usize, usize), NetworkOutputAccumulator>::new();
    for line_flow in line_flows {
        let Some(original_period) = line_flow
            .period
            .checked_sub(1)
            .and_then(|period| system.horizon.internal_to_original_period.get(period))
            .copied()
        else {
            continue;
        };
        let entry = aggregated
            .entry((line_flow.branch_id.0, original_period))
            .or_default();
        entry.branch_name = line_flow.branch_name.clone();
        entry.from_bus_name = line_flow.from_bus_name.clone();
        entry.to_bus_name = line_flow.to_bus_name.clone();
        entry.weighted_flow_sum += line_flow.flow_mw * system.horizon.period_duration_hours;
        entry.duration_hours += system.horizon.period_duration_hours;
        entry.limit_mw = line_flow.limit_mw;
        entry.violated |= line_flow.flow_mw.abs() > line_flow.limit_mw;
    }

    for ((_, original_period), line_flow) in aggregated {
        let flow_mw = if line_flow.duration_hours > 0.0 {
            line_flow.weighted_flow_sum / line_flow.duration_hours
        } else {
            0.0
        };
        csv.push_str(&format!(
            "{};{};{};{};{:.6};{:.6};{}\n",
            line_flow.branch_name,
            line_flow.from_bus_name,
            line_flow.to_bus_name,
            original_period,
            flow_mw,
            line_flow.limit_mw,
            if line_flow.violated { 1 } else { 0 }
        ));
    }

    write_csv_to_file(output_dir.join("resultado_rede.csv"), csv)
}

fn write_added_flow_cut_lines_csv(
    added_lines: &[AddedFlowCutLineResult],
    output_dir: &Path,
) -> Result<(), SimulationError> {
    let mut csv = csv_with_header(
        "NomeLinha;CodigoLinha;BarraDe;BarraPara;FluxoMW;CapacidadeMW",
        &[
            ("NomeLinha", "nome da linha adicionada ao problema"),
            ("CodigoLinha", "codigo da linha adicionada ao problema"),
            ("BarraDe", "codigo da barra de origem"),
            ("BarraPara", "codigo da barra de destino"),
            ("FluxoMW", "fluxo de potencia da linha em MW"),
            ("CapacidadeMW", "capacidade da linha em MW"),
        ],
    );

    for line in added_lines {
        csv.push_str(&format!(
            "{};{};{};{};{:.6};{:.6}\n",
            line.branch_name,
            line.branch_id.0,
            line.from_bus_id.0,
            line.to_bus_id.0,
            line.flow_mw,
            line.limit_mw
        ));
    }

    write_csv_to_file(output_dir.join("resultado_linhas_adicionadas.csv"), csv)
}

fn write_network_infeasibility_csv(
    infeasibilities: &[NetworkInfeasibilityRecord],
    output_dir: &Path,
) -> Result<(), SimulationError> {
    let mut csv = csv_with_header(
        "Etapa;Iteracao;Periodo;Linha;ViolacaoMW",
        &[
            ("Etapa", "etapa em que a violacao ocorreu"),
            ("Iteracao", "iteracao em que a violacao ocorreu"),
            ("Periodo", "periodo da violacao"),
            ("Linha", "nome da linha violada"),
            ("ViolacaoMW", "violacao de fluxo em MW"),
        ],
    );

    for record in infeasibilities {
        csv.push_str(&format!(
            "{};{};{};{};{:.6}\n",
            record.stage.label(),
            record.iteration,
            record.period,
            record.branch_name,
            record.violation_mw
        ));
    }

    write_csv_to_file(output_dir.join("resultado_inviabilidade_rede.csv"), csv)
}

fn write_operational_limit_infeasibility_csv(
    infeasibilities: &[OperationalLimitInfeasibilityRecord],
    output_dir: &Path,
) -> Result<(), SimulationError> {
    let mut csv = csv_with_header(
        "Etapa;Iteracao;Periodo;CodigoUsina;NomeUsina;Variavel;Unidade;ViolacaoLinf;ViolacaoLsup",
        &[
            ("Etapa", "etapa em que a violacao ocorreu"),
            ("Iteracao", "iteracao em que a violacao ocorreu"),
            ("Periodo", "periodo da violacao"),
            ("CodigoUsina", "codigo da usina associada ao limite"),
            ("NomeUsina", "nome da usina associada ao limite"),
            ("Variavel", "variavel operacional violada"),
            ("Unidade", "unidade das violacoes reportadas"),
            ("ViolacaoLinf", "violacao do limite inferior"),
            ("ViolacaoLsup", "violacao do limite superior"),
        ],
    );

    for record in infeasibilities {
        csv.push_str(&format!(
            "{};{};{};{};{};{};{};{:.6};{:.6}\n",
            record.stage.label(),
            record.iteration,
            record.period,
            record.plant_code,
            record.plant_name,
            operational_limit_variable_label(record.variable),
            record.unit,
            record.lower_violation,
            record.upper_violation
        ));
    }

    write_csv_to_file(output_dir.join("resultado_inviabilidade_lim.csv"), csv)
}

// --- Helpers ---

fn write_csv_to_file(path: impl AsRef<Path>, contents: String) -> Result<(), SimulationError> {
    let path = path.as_ref();
    write_csv_file(path, contents).map_err(|error| {
        SimulationError::IterativeProcess(format!(
            "failed to write output file {}: {error}",
            path.display()
        ))
    })
}

fn operational_limit_variable_label(
    variable: labdessem_core::system::OperationalLimitVariable,
) -> &'static str {
    use labdessem_core::system::OperationalLimitVariable;
    match variable {
        OperationalLimitVariable::Generation => "GER",
        OperationalLimitVariable::Spillage => "VERT",
        OperationalLimitVariable::Volume => "VOL",
        OperationalLimitVariable::Defluence => "DEFLU",
        OperationalLimitVariable::Turbining => "TURB",
        OperationalLimitVariable::Diversion => "QDES",
        OperationalLimitVariable::Pumping => "QBOM",
    }
}
