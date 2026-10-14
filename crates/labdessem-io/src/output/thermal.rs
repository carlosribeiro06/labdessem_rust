//! Thermal plant result writer.

use std::{collections::HashMap, path::Path};

use labdessem_core::{entities::thermal::ThermalUnit, system::System};
use labdessem_model::names::{thermal_commitment_name, thermal_generation_name};

use crate::error::IoError;
use crate::output::{
    csv_with_header, write_csv_file,
    helpers::{
        average_internal_values, original_period_internal_indices, thermal_residence_times,
    },
};

/// Write thermal plant results to CSV.
///
/// # Arguments
/// * `system` - The system definition
/// * `variable_values` - Map of variable names to their values from the solution
/// * `output_dir` - Directory to write the output file
pub fn write_thermal_csv(
    system: &System,
    variable_values: &HashMap<String, f64>,
    output_dir: &Path,
) -> Result<(), IoError> {
    let residual_enabled = system.ton_residual_enabled;
    let mut csv = if residual_enabled {
        csv_with_header(
            "Usina;Unidade;Periodo;GeracaoMW;CVU;StatusOn;StatusOff;TempoPermanenciaOn;TempoPermanenciaOff;TempoResidualHoras;CustoResidual;CustoResidualIncremental",
            &[
                ("Usina", "nome da usina termica"),
                ("Unidade", "codigo da unidade; 99 indica agregado da usina"),
                ("Periodo", "periodo do horizonte de estudo"),
                ("GeracaoMW", "geracao termica em MW"),
                ("CVU", "custo variavel unitario em R$/MWh"),
                ("StatusOn", "fracao do periodo original em que a unidade ou agregado ficou ligado"),
                ("StatusOff", "fracao do periodo original em que a unidade ou agregado ficou desligado"),
                ("TempoPermanenciaOn", "tempo consecutivo ligado em periodos"),
                ("TempoPermanenciaOff", "tempo consecutivo desligado em periodos"),
                ("TempoResidualHoras", "tempo residual fora do horizonte em horas caso a unidade acione no inicio do periodo"),
                ("CustoResidual", "custo residual fora do horizonte em R$ caso a unidade acione no inicio do periodo"),
                ("CustoResidualIncremental", "custo residual incremental fora do horizonte em R$ caso a unidade acione no inicio do periodo, usando max(CVU - CMO, 0)"),
            ],
        )
    } else {
        csv_with_header(
            "Usina;Unidade;Periodo;GeracaoMW;CVU;StatusOn;StatusOff;TempoPermanenciaOn;TempoPermanenciaOff",
            &[
                ("Usina", "nome da usina termica"),
                ("Unidade", "codigo da unidade; 99 indica agregado da usina"),
                ("Periodo", "periodo do horizonte de estudo"),
                ("GeracaoMW", "geracao termica em MW"),
                ("CVU", "custo variavel unitario em R$/MWh"),
                ("StatusOn", "fracao do periodo original em que a unidade ou agregado ficou ligado"),
                ("StatusOff", "fracao do periodo original em que a unidade ou agregado ficou desligado"),
                ("TempoPermanenciaOn", "tempo consecutivo ligado em periodos"),
                ("TempoPermanenciaOff", "tempo consecutivo desligado em periodos"),
            ],
        )
    };

    for plant in &system.thermal_plants {
        let mut unit_outputs = Vec::new();
        let mut aggregated_generation = vec![0.0; system.horizon.original_periods];
        let mut aggregated_cvu_weight = vec![0.0; system.horizon.original_periods];
        let mut aggregated_status_fraction = vec![0.0; system.horizon.original_periods];
        let mut aggregated_status = vec![false; system.horizon.periods];
        let mut aggregated_residual_hours = vec![0.0; system.horizon.original_periods];
        let mut aggregated_residual_cost = vec![0.0; system.horizon.original_periods];
        let mut aggregated_residual_incremental_cost = vec![0.0; system.horizon.original_periods];
        
        let average_unit_cvu = if plant.units.is_empty() {
            0.0
        } else {
            plant.units.iter().map(|u| u.variable_cost_per_mwh).sum::<f64>() / plant.units.len() as f64
        };
        
        let residual_cmo_per_mwh = if residual_enabled {
            system.residual_costs.iter().find_map(|rc| {
                (rc.submarket_id == plant.submarket_id).then_some(rc.cmo_per_mwh)
            })
        } else {
            None
        };

        for unit in &plant.units {
            let statuses = (0..system.horizon.periods)
                .map(|period| {
                    let commitment_name = thermal_commitment_name(&plant.name, &unit.name, period);
                    variable_values
                        .get(&commitment_name)
                        .copied().map_or_else(|| {
                            let generation_val = variable_values
                                .get(&thermal_generation_name(&plant.name, &unit.name, period))
                                .copied()
                                .unwrap_or(0.0);
                            generation_val.abs() > 1e-8
                        }, |value| value >= 0.5)
                })
                .collect::<Vec<_>>();

            let (times_on, times_off) = thermal_residence_times(
                unit.initial_condition.is_on,
                unit.initial_condition.time_in_state,
                &statuses,
            );

            for original_period in 1..=system.horizon.original_periods {
                let internal_periods = original_period_internal_indices(system, original_period);
                let generation = average_internal_values(system, &internal_periods, |p| {
                    variable_values
                        .get(&thermal_generation_name(&plant.name, &unit.name, p))
                        .copied()
                        .unwrap_or(0.0)
                });

                let status_on_fraction = if internal_periods.is_empty() {
                    0.0
                } else {
                    internal_periods.iter().filter(|p| statuses[**p]).count() as f64
                        / internal_periods.len() as f64
                };
                let status_off_fraction = 1.0 - status_on_fraction;

                let last_internal_period = internal_periods.last().copied().unwrap_or(original_period - 1);

                let (residual_hours, residual_cost, residual_incremental_cost) = if residual_enabled {
                    thermal_period_residual_values(system, unit, &internal_periods, residual_cmo_per_mwh)
                } else {
                    (0.0, 0.0, 0.0)
                };

                let original_idx = original_period - 1;
                aggregated_generation[original_idx] += generation;
                aggregated_cvu_weight[original_idx] += generation * unit.variable_cost_per_mwh;
                aggregated_status_fraction[original_idx] += status_on_fraction;
                for p in &internal_periods {
                    aggregated_status[*p] |= statuses[*p];
                }
                aggregated_residual_hours[original_idx] += residual_hours;
                aggregated_residual_cost[original_idx] += residual_cost;
                aggregated_residual_incremental_cost[original_idx] += residual_incremental_cost;

                unit_outputs.push((
                    original_period,
                    unit.id.0,
                    generation,
                    unit.variable_cost_per_mwh,
                    status_on_fraction,
                    status_off_fraction,
                    times_on[last_internal_period],
                    times_off[last_internal_period],
                    residual_hours,
                    residual_cost,
                    residual_incremental_cost,
                ));
            }
        }

        let initial_is_on = plant.units.iter().any(|u| u.initial_condition.is_on);
        let initial_time = plant.units.iter().map(|u| u.initial_condition.time_in_state).max().unwrap_or(0);
        let (agg_times_on, agg_times_off) = thermal_residence_times(initial_is_on, initial_time, &aggregated_status);

        for original_period in 1..=system.horizon.original_periods {
            let internal_periods = original_period_internal_indices(system, original_period);
            let last_internal_period = internal_periods.last().copied().unwrap_or(original_period - 1);

            for (row_period, unit_id, generation_val, cvu, on, off, t_on, t_off, r_hrs, r_cost, r_inc) in &unit_outputs {
                if *row_period == original_period {
                    if residual_enabled {
                        csv.push_str(&format!(
                            "{};{};{};{:.6};{:.6};{:.6};{:.6};{};{};{:.6};{:.6};{:.6}\n",
                            plant.name, unit_id, original_period, generation_val, cvu, on, off, t_on, t_off, r_hrs, r_cost, r_inc
                        ));
                    } else {
                        csv.push_str(&format!(
                            "{};{};{};{:.6};{:.6};{:.6};{:.6};{};{}\n",
                            plant.name, unit_id, original_period, generation_val, cvu, on, off, t_on, t_off
                        ));
                    }
                }
            }

            let original_idx = original_period - 1;
            let agg_cvu = if aggregated_generation[original_idx].abs() > 1e-8 {
                aggregated_cvu_weight[original_idx] / aggregated_generation[original_idx]
            } else {
                average_unit_cvu
            };
            let unit_count = plant.units.len().max(1) as f64;
            let agg_on = aggregated_status_fraction[original_idx] / unit_count;
            let agg_off = 1.0 - agg_on;

            if residual_enabled {
                csv.push_str(&format!(
                    "{};99;{};{:.6};{:.6};{:.6};{:.6};{};{};{:.6};{:.6};{:.6}\n",
                    plant.name, original_period,
                    aggregated_generation[original_idx], agg_cvu, agg_on, agg_off,
                    agg_times_on[last_internal_period], agg_times_off[last_internal_period],
                    aggregated_residual_hours[original_idx],
                    aggregated_residual_cost[original_idx],
                    aggregated_residual_incremental_cost[original_idx]
                ));
            } else {
                csv.push_str(&format!(
                    "{};99;{};{:.6};{:.6};{:.6};{:.6};{};{}\n",
                    plant.name, original_period,
                    aggregated_generation[original_idx], agg_cvu, agg_on, agg_off,
                    agg_times_on[last_internal_period], agg_times_off[last_internal_period]
                ));
            }
        }
    }

    write_csv_file(output_dir.join("resultado_termicas.csv"), csv)?;
    Ok(())
}

fn thermal_period_residual_values(
    system: &System,
    unit: &ThermalUnit,
    periods: &[usize],
    residual_cmo_per_mwh: Option<f64>,
) -> (f64, f64, f64) {
    if periods.is_empty() {
        return (0.0, 0.0, 0.0);
    }
    thermal_startup_residual_values(system, unit, periods[0], residual_cmo_per_mwh)
}

fn thermal_startup_residual_values(
    system: &System,
    unit: &ThermalUnit,
    startup_period: usize,
    residual_cmo_per_mwh: Option<f64>,
) -> (f64, f64, f64) {
    let residual_profile = thermal_startup_residual_profile(unit);
    let periods_inside_horizon = system.horizon.periods.saturating_sub(startup_period);
    let residual_steps = residual_profile.len().saturating_sub(periods_inside_horizon);

    if residual_steps == 0 {
        return (0.0, 0.0, 0.0);
    }

    let residual_mw_sum: f64 = residual_profile[periods_inside_horizon..].iter().sum();
    let residual_hours = residual_steps as f64 * system.horizon.period_duration_hours;
    let residual_cost = residual_mw_sum * unit.variable_cost_per_mwh * system.horizon.period_duration_hours;
    let residual_incremental_cost = residual_mw_sum
        * (unit.variable_cost_per_mwh - residual_cmo_per_mwh.unwrap_or(0.0)).max(0.0)
        * system.horizon.period_duration_hours;

    (residual_hours, residual_cost, residual_incremental_cost)
}

fn thermal_startup_residual_profile(unit: &ThermalUnit) -> Vec<f64> {
    let mut profile = unit.startup_trajectory_mw.clone();
    let steady_periods = unit.min_up_time.saturating_sub(unit.startup_trajectory_mw.len());
    profile.extend(std::iter::repeat_n(unit.min_generation_mw, steady_periods));
    profile
}
