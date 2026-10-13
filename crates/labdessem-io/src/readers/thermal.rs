//! Thermal plant CSV reader.

use std::collections::{BTreeMap, HashMap};
use std::path::Path;

use labdessem_core::entities::{Submarket, ThermalInitialCondition, ThermalPlant, ThermalUnit};
use labdessem_core::ids::{BusId, ThermalPlantId, ThermalUnitId};
use serde::Deserialize;

use crate::error::IoError;
use crate::readers::common::{
    bus_id_or_dummy, hours_to_periods, normalize_max_bound, read_csv, submarket_id_by_name,
    trajectory_for,
};

/// Read thermal plants from CAD_UNID_UTE.csv and optionally CAD_RAMPAS_TERMICAS.csv.
pub fn read_thermal_plants(
    cad_path: &Path,
    submarkets: &[Submarket],
    period_duration_hours: f64,
    network_enabled: bool,
    uct_enabled: bool,
) -> Result<Vec<ThermalPlant>, IoError> {
    let rows: Vec<ThermalUnitRow> = read_csv(cad_path.join("CAD_UNID_UTE.csv"))?;

    let (startup_trajectories, shutdown_trajectories) = if uct_enabled {
        read_trajectories(cad_path.join("CAD_RAMPAS_TERMICAS.csv"))?
    } else {
        (HashMap::new(), HashMap::new())
    };

    build_thermal_plants(
        &rows,
        &startup_trajectories,
        &shutdown_trajectories,
        submarkets,
        period_duration_hours,
        network_enabled,
        uct_enabled,
    )
}

fn read_trajectories(
    path: impl AsRef<Path>,
) -> Result<(HashMap<String, Vec<f64>>, HashMap<String, Vec<f64>>), IoError> {
    let rows: Vec<ThermalRampRow> = read_csv(path.as_ref().to_path_buf())?;
    let mut startup_steps = BTreeMap::<String, Vec<(usize, f64)>>::new();
    let mut shutdown_steps = BTreeMap::<String, Vec<(usize, f64)>>::new();

    for row in rows {
        let key = format!("{}-{}", row.nome_ute.trim(), row.unidade);
        let target = match row.traj.trim().to_ascii_uppercase().as_str() {
            "A" => &mut startup_steps,
            "D" => &mut shutdown_steps,
            other => {
                return Err(IoError::invalid_data(format!(
                    "unknown thermal ramp trajectory '{other}' for {key}; expected A or D"
                )));
            }
        };
        target
            .entry(key)
            .or_default()
            .push((row.indice_passo, row.passo));
    }

    Ok((
        build_ordered_trajectory_map(startup_steps),
        build_ordered_trajectory_map(shutdown_steps),
    ))
}

fn build_ordered_trajectory_map(
    steps_by_unit: BTreeMap<String, Vec<(usize, f64)>>,
) -> HashMap<String, Vec<f64>> {
    steps_by_unit
        .into_iter()
        .map(|(unit, mut steps)| {
            steps.sort_by_key(|(step_idx, _)| *step_idx);
            (unit, steps.into_iter().map(|(_, value)| value).collect())
        })
        .collect()
}

fn build_thermal_plants(
    rows: &[ThermalUnitRow],
    startup_trajectories: &HashMap<String, Vec<f64>>,
    shutdown_trajectories: &HashMap<String, Vec<f64>>,
    submarkets: &[Submarket],
    period_duration_hours: f64,
    network_enabled: bool,
    uct_enabled: bool,
) -> Result<Vec<ThermalPlant>, IoError> {
    let mut grouped = BTreeMap::<usize, Vec<&ThermalUnitRow>>::new();
    for row in rows {
        grouped.entry(row.codigo_ute).or_default().push(row);
    }

    grouped
        .into_iter()
        .map(|(plant_code, plant_rows)| {
            let first = plant_rows[0];
            let mut units = Vec::with_capacity(plant_rows.len());

            for row in plant_rows {
                let plant_name = row.nome_ute.trim();
                let unit_trajectory_key = format!("{}-{}", plant_name, row.unidade);
                let max_generation_mw =
                    normalize_max_bound(row.pmin, row.pmax, "Pmax", &unit_trajectory_key)?;
                let startup = if uct_enabled {
                    trajectory_for(startup_trajectories, &unit_trajectory_key)?
                } else {
                    Vec::new()
                };
                let shutdown = if uct_enabled {
                    trajectory_for(shutdown_trajectories, &unit_trajectory_key)?
                } else {
                    Vec::new()
                };
                let initial_condition = if uct_enabled {
                    ThermalInitialCondition {
                        is_on: row.status_inic != 0,
                        generation_mw: row.ger_inic,
                        time_in_state: hours_to_periods(
                            row.tinic,
                            period_duration_hours,
                            "Tinic",
                            plant_name,
                        )?,
                        time_in_ramp: hours_to_periods(
                            row.trampa_inic,
                            period_duration_hours,
                            "TRampaInic",
                            plant_name,
                        )?,
                        is_ramping_up: row.rup_inic != 0,
                        is_ramping_down: row.rdown_inic != 0,
                    }
                } else {
                    ThermalInitialCondition {
                        is_on: false,
                        generation_mw: 0.0,
                        time_in_state: 0,
                        time_in_ramp: 0,
                        is_ramping_up: false,
                        is_ramping_down: false,
                    }
                };

                units.push(ThermalUnit {
                    id: ThermalUnitId(row.unidade),
                    name: format!("{}-{}", plant_name, row.unidade),
                    min_generation_mw: row.pmin,
                    max_generation_mw,
                    startup_trajectory_mw: startup,
                    shutdown_trajectory_mw: shutdown,
                    min_up_time: hours_to_periods(
                        row.ton,
                        period_duration_hours,
                        "Ton",
                        plant_name,
                    )?,
                    min_down_time: hours_to_periods(
                        row.toff,
                        period_duration_hours,
                        "Toff",
                        plant_name,
                    )?,
                    startup_cost: row.custo_partida,
                    shutdown_cost: row.custo_desliga,
                    variable_cost_per_mwh: row.cvu,
                    initial_condition,
                });
            }

            let submarket_id = submarket_id_by_name(submarkets, &first.submercado)?;
            Ok(ThermalPlant {
                id: ThermalPlantId(plant_code),
                name: first.nome_ute.trim().to_string(),
                submarket_id,
                bus_id: BusId(bus_id_or_dummy(
                    first.barra,
                    network_enabled,
                    first.nome_ute.trim(),
                    submarket_id,
                )?),
                units,
            })
        })
        .collect()
}

#[derive(Debug, Deserialize)]
struct ThermalUnitRow {
    #[serde(rename = "CodigoUTE")]
    codigo_ute: usize,
    #[serde(rename = "NomeUTE")]
    nome_ute: String,
    #[serde(rename = "Unidade")]
    unidade: usize,
    #[serde(rename = "Pmin")]
    pmin: f64,
    #[serde(rename = "Pmax")]
    pmax: f64,
    #[serde(rename = "Ton")]
    ton: f64,
    #[serde(rename = "Toff")]
    toff: f64,
    #[serde(rename = "GerInic")]
    ger_inic: f64,
    #[serde(rename = "StatusInic")]
    status_inic: usize,
    #[serde(rename = "Tinic")]
    tinic: f64,
    #[serde(rename = "TRampaInic")]
    trampa_inic: f64,
    #[serde(rename = "RupInic")]
    rup_inic: usize,
    #[serde(rename = "RdownInic")]
    rdown_inic: usize,
    #[serde(rename = "CVU")]
    cvu: f64,
    #[serde(rename = "CustoPartida")]
    custo_partida: f64,
    #[serde(rename = "CustoDesliga")]
    custo_desliga: f64,
    #[serde(rename = "Barra")]
    barra: Option<usize>,
    #[serde(rename = "Submercado")]
    submercado: String,
}

#[derive(Debug, Deserialize)]
struct ThermalRampRow {
    #[serde(rename = "CodigoUTE")]
    _codigo_ute: usize,
    #[serde(rename = "NomeUTE")]
    nome_ute: String,
    #[serde(rename = "Unidade")]
    unidade: usize,
    #[serde(rename = "Traj")]
    traj: String,
    #[serde(rename = "Passo")]
    passo: f64,
    #[serde(rename = " Ipasso")]
    indice_passo: usize,
}
