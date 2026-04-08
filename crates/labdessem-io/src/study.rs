use std::{
    collections::{BTreeMap, HashMap},
    fs,
    fs::File,
    path::{Path, PathBuf},
};

use csv::{ReaderBuilder, StringRecord};
use labdessem_core::{
    hydro::{HydroGroup, HydroInitialCondition, HydroPlant, HydroUnit, Reservoir},
    ids::{
        BranchId, BusId, HydroGroupId, HydroPlantId, HydroUnitId, SubmarketId, ThermalPlantId,
        ThermalUnitId,
    },
    renewable::{SolarPlant, WindPlant},
    system::{Branch, Bus, InterchangeLimit, StudyHorizon, Submarket, System},
    thermal::{ThermalInitialCondition, ThermalPlant, ThermalUnit},
};
use serde::{Deserialize, Deserializer};

use crate::error::IoError;

#[derive(Debug, Clone, Deserialize, PartialEq, Eq)]
pub struct StudyConfig {
    pub case_path: PathBuf,
}

pub fn read_study_config(config_path: impl AsRef<Path>) -> Result<StudyConfig, IoError> {
    let config_path = config_path.as_ref();
    let contents = fs::read_to_string(config_path).map_err(|error| {
        IoError::invalid_data(format!(
            "failed to read study config {}: {error}",
            config_path.display()
        ))
    })?;

    let mut config: StudyConfig = serde_json::from_str(&contents)?;
    if config.case_path.is_relative() {
        let config_dir = config_path.parent().ok_or_else(|| {
            IoError::invalid_data(format!(
                "study config path {} does not have a parent directory",
                config_path.display()
            ))
        })?;
        config.case_path = config_dir.join(&config.case_path);
    }

    Ok(config)
}

pub fn read_study_from_config(config_path: impl AsRef<Path>) -> Result<System, IoError> {
    let config = read_study_config(config_path)?;
    read_study_from_path(config.case_path)
}

pub fn read_study_from_path(base_path: impl AsRef<Path>) -> Result<System, IoError> {
    let base_path = base_path.as_ref();
    let cad_path = base_path.join("CAD");
    let oper_path = base_path.join("OPER");

    let submarket_catalog: Vec<SubmarketCatalogRow> = read_csv(cad_path.join("CAD_SBM.csv"))?;
    let duration_rows: Vec<DurationRow> = read_csv(oper_path.join("OPER_DURACAO.csv"))?;
    let submarket_operation_rows: Vec<SubmarketOperationRow> =
        read_csv(oper_path.join("OPER_SBM.csv"))?;
    let bus_rows: Vec<BusOperationRow> = read_csv(oper_path.join("OPER_CARGA_BARRA.csv"))?;
    let branch_rows: Vec<BranchRow> = read_csv(oper_path.join("OPER_LINHA.csv"))?;
    let interchange_rows: Vec<InterchangeRow> =
        read_csv(oper_path.join("OPER_LIMITE_INTERCAMBIO.csv"))?;
    let thermal_rows: Vec<ThermalUnitRow> = read_csv(cad_path.join("CAD_UNID_UTE.csv"))?;
    let hydro_rows: Vec<HydroPlantRow> = read_csv(cad_path.join("CAD_UHE.csv"))?;
    let hydro_unit_rows: Vec<HydroUnitRow> = read_csv(cad_path.join("CAD_CONJ_UHE.csv"))?;
    let hydro_inflow_rows: Vec<HydroInflowRow> = read_csv(oper_path.join("OPER_VAZAO.csv"))?;
    let renewable_catalog_rows: Vec<RenewableCatalogRow> = read_csv(cad_path.join("CAD_REN.csv"))?;
    let renewable_operation_rows: Vec<RenewableOperationRow> =
        read_csv(oper_path.join("OPER_REN.csv"))?;

    let thermal_startup_trajectories =
        read_trajectory_table(cad_path.join("CAD_RAMPAS_UP_UTE.csv"))?;
    let thermal_shutdown_trajectories =
        read_trajectory_table(cad_path.join("CAD_RAMPAS_DOWN_UTE.csv"))?;
    let hydro_startup_trajectories = read_trajectory_table(cad_path.join("CAD_RAMPAS_UP_UHE.csv"))?;
    let hydro_shutdown_trajectories =
        read_trajectory_table(cad_path.join("CAD_RAMPAS_DOWN_UHE.csv"))?;

    let horizon = build_horizon(&duration_rows)?;
    let submarkets = build_submarkets(
        &submarket_catalog,
        &submarket_operation_rows,
        horizon.periods,
    )?;
    let buses = build_buses(&bus_rows, horizon.periods)?;
    let branches = build_branches(&branch_rows);
    let interchange_limits = build_interchange_limits(&interchange_rows);
    let thermal_plants = build_thermal_plants(
        &thermal_rows,
        &thermal_startup_trajectories,
        &thermal_shutdown_trajectories,
    )?;
    let hydro_plants = build_hydro_plants(
        &hydro_rows,
        &hydro_unit_rows,
        &hydro_inflow_rows,
        &hydro_startup_trajectories,
        &hydro_shutdown_trajectories,
        horizon.periods,
    )?;
    let (wind_plants, solar_plants) = build_renewables(
        &renewable_catalog_rows,
        &renewable_operation_rows,
        horizon.periods,
    )?;

    let system = System {
        horizon,
        submarkets,
        interchange_limits,
        buses,
        branches,
        thermal_plants,
        hydro_plants,
        wind_plants,
        solar_plants,
    };

    system.validate()?;

    Ok(system)
}

fn build_horizon(duration_rows: &[DurationRow]) -> Result<StudyHorizon, IoError> {
    if duration_rows.is_empty() {
        return Err(IoError::invalid_data("OPER_DURACAO.csv cannot be empty"));
    }

    let mut periods = BTreeMap::new();
    for row in duration_rows {
        periods.insert(row.periodo, row.duracao);
    }

    let expected_periods = periods.len();
    for period in 1..=expected_periods {
        if !periods.contains_key(&period) {
            return Err(IoError::invalid_data(format!(
                "missing period {period} in OPER_DURACAO.csv"
            )));
        }
    }

    let first_duration = *periods
        .values()
        .next()
        .expect("duration map should not be empty");
    if periods.values().any(|duration| *duration != first_duration) {
        return Err(IoError::invalid_data(
            "all periods must have the same duration in OPER_DURACAO.csv",
        ));
    }

    Ok(StudyHorizon {
        periods: expected_periods,
        period_duration_hours: first_duration,
    })
}

fn build_submarkets(
    catalog_rows: &[SubmarketCatalogRow],
    operation_rows: &[SubmarketOperationRow],
    horizon: usize,
) -> Result<Vec<Submarket>, IoError> {
    let mut demand_by_submarket: HashMap<usize, Vec<f64>> = HashMap::new();
    let mut deficit_cost_by_submarket: HashMap<usize, f64> = HashMap::new();

    for row in operation_rows {
        let period_idx = validate_period(row.periodo, horizon, "OPER_SBM.csv")?;
        let demand_series = demand_by_submarket
            .entry(row.codigo_submercado)
            .or_insert_with(|| vec![0.0; horizon]);
        demand_series[period_idx] = row.demanda;

        match deficit_cost_by_submarket.get(&row.codigo_submercado) {
            Some(cost) if (*cost - row.custo_deficit).abs() > f64::EPSILON => {
                return Err(IoError::invalid_data(format!(
                    "submarket {} has inconsistent deficit cost across periods",
                    row.codigo_submercado
                )));
            }
            Some(_) => {}
            None => {
                deficit_cost_by_submarket.insert(row.codigo_submercado, row.custo_deficit);
            }
        }
    }

    catalog_rows
        .iter()
        .map(|row| {
            let demand_mw = demand_by_submarket
                .get(&row.codigo)
                .cloned()
                .ok_or_else(|| {
                    IoError::invalid_data(format!(
                        "missing demand data for submarket {}",
                        row.codigo
                    ))
                })?;

            let deficit_cost_per_mwh =
                *deficit_cost_by_submarket.get(&row.codigo).ok_or_else(|| {
                    IoError::invalid_data(format!(
                        "missing deficit cost for submarket {}",
                        row.codigo
                    ))
                })?;

            Ok(Submarket {
                id: SubmarketId(row.codigo),
                name: row.nome.clone(),
                demand_mw,
                deficit_cost_per_mwh,
            })
        })
        .collect()
}

fn build_buses(rows: &[BusOperationRow], horizon: usize) -> Result<Vec<Bus>, IoError> {
    let mut buses = BTreeMap::<usize, BusAccumulator>::new();

    for row in rows {
        let period_idx = validate_period(row.periodo, horizon, "OPER_CARGA_BARRA.csv")?;
        let entry = buses
            .entry(row.codigo_barra)
            .or_insert_with(|| BusAccumulator {
                name: row.nome_barra.clone(),
                submarket_id: row.codigo_submercado,
                angle_reference: row.swing,
                demand_mw: vec![0.0; horizon],
            });

        if entry.name != row.nome_barra {
            return Err(IoError::invalid_data(format!(
                "bus {} has inconsistent name across periods",
                row.codigo_barra
            )));
        }
        if entry.submarket_id != row.codigo_submercado {
            return Err(IoError::invalid_data(format!(
                "bus {} has inconsistent submarket across periods",
                row.codigo_barra
            )));
        }
        if entry.angle_reference != row.swing {
            return Err(IoError::invalid_data(format!(
                "bus {} has inconsistent swing flag across periods",
                row.codigo_barra
            )));
        }

        entry.demand_mw[period_idx] = row.carga;
    }

    Ok(buses
        .into_iter()
        .map(|(id, bus)| Bus {
            id: BusId(id),
            name: bus.name,
            submarket_id: SubmarketId(bus.submarket_id),
            angle_reference: bus.angle_reference,
            demand_mw: bus.demand_mw,
        })
        .collect())
}

fn build_branches(rows: &[BranchRow]) -> Vec<Branch> {
    rows.iter()
        .map(|row| Branch {
            id: BranchId(row.codigo_linha),
            name: format!("LINHA-{}", row.codigo_linha),
            from_bus_id: BusId(row.de),
            to_bus_id: BusId(row.para),
            reactance_pu: row.reatancia,
            thermal_limit_mw: row.capacidade,
        })
        .collect()
}

fn build_interchange_limits(rows: &[InterchangeRow]) -> Vec<InterchangeLimit> {
    rows.iter()
        .map(|row| InterchangeLimit {
            from_submarket_id: SubmarketId(row.sbm_de),
            to_submarket_id: SubmarketId(row.sbm_para),
            max_flow_mw: row.limite,
            penalty_cost_per_mwh: row.penalidade,
        })
        .collect()
}

fn build_thermal_plants(
    rows: &[ThermalUnitRow],
    startup_trajectories: &HashMap<String, Vec<f64>>,
    shutdown_trajectories: &HashMap<String, Vec<f64>>,
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
                let startup = trajectory_for(startup_trajectories, &row.nome_ute)?;
                let shutdown = trajectory_for(shutdown_trajectories, &row.nome_ute)?;

                units.push(ThermalUnit {
                    id: ThermalUnitId(row.unidade),
                    name: format!("{}-{}", row.nome_ute, row.unidade),
                    min_generation_mw: row.pmin,
                    max_generation_mw: row.pmax,
                    startup_trajectory_mw: startup,
                    shutdown_trajectory_mw: shutdown,
                    min_up_time: row.ton,
                    min_down_time: row.toff,
                    startup_cost: row.custo_partida,
                    shutdown_cost: row.custo_desliga,
                    variable_cost_per_mwh: row.cvu,
                    initial_condition: ThermalInitialCondition {
                        is_on: row.status_inic != 0,
                        generation_mw: row.ger_inic,
                        time_in_state: row.tinic,
                    },
                });
            }

            Ok(ThermalPlant {
                id: ThermalPlantId(plant_code),
                name: first.nome_ute.clone(),
                submarket_id: SubmarketId(first.submercado),
                bus_id: BusId(first.barra),
                units,
            })
        })
        .collect()
}

fn build_hydro_plants(
    plant_rows: &[HydroPlantRow],
    unit_rows: &[HydroUnitRow],
    inflow_rows: &[HydroInflowRow],
    startup_trajectories: &HashMap<String, Vec<f64>>,
    shutdown_trajectories: &HashMap<String, Vec<f64>>,
    horizon: usize,
) -> Result<Vec<HydroPlant>, IoError> {
    let hydro_name_to_id: HashMap<_, _> = plant_rows
        .iter()
        .map(|row| (row.nome.clone(), HydroPlantId(row.codigo)))
        .collect();

    let mut inflows_by_plant = HashMap::<usize, Vec<f64>>::new();
    for row in inflow_rows {
        let period_idx = validate_period(row.periodo, horizon, "OPER_VAZAO.csv")?;
        let series = inflows_by_plant
            .entry(row.codigo)
            .or_insert_with(|| vec![0.0; horizon]);
        series[period_idx] = row.afluencia;
    }

    let mut units_by_plant = BTreeMap::<usize, Vec<&HydroUnitRow>>::new();
    for row in unit_rows {
        units_by_plant.entry(row.codigo).or_default().push(row);
    }

    plant_rows
        .iter()
        .map(|row| {
            let plant_unit_rows = units_by_plant.get(&row.codigo).ok_or_else(|| {
                IoError::invalid_data(format!("missing hydro units for plant {}", row.codigo))
            })?;
            let first_unit = plant_unit_rows[0];

            let mut groups_by_id = BTreeMap::<usize, Vec<&HydroUnitRow>>::new();
            for unit_row in plant_unit_rows {
                groups_by_id
                    .entry(unit_row.conjunto)
                    .or_default()
                    .push(unit_row);
            }

            let mut groups = Vec::with_capacity(groups_by_id.len());
            for (group_id, group_rows) in groups_by_id {
                let mut units = Vec::with_capacity(group_rows.len());
                for unit_row in group_rows {
                    let startup = trajectory_for(startup_trajectories, &row.nome)?;
                    let shutdown = trajectory_for(shutdown_trajectories, &row.nome)?;
                    let is_on = unit_row.status_inic != 0;
                    units.push(HydroUnit {
                        id: HydroUnitId(unit_row.unidade),
                        name: format!("{}-{}", row.nome, unit_row.unidade),
                        min_generation_mw: unit_row.pmin,
                        max_generation_mw: unit_row.pmax,
                        max_turbining_hm3: unit_row.max_turb,
                        productivity_mw_per_hm3: unit_row.prod,
                        startup_trajectory_mw: startup,
                        shutdown_trajectory_mw: shutdown,
                        min_up_time: unit_row.ton,
                        min_down_time: unit_row.toff,
                        startup_cost: unit_row.custo_partida,
                        shutdown_cost: unit_row.custo_desliga,
                        initial_condition: HydroInitialCondition {
                            is_on,
                            generation_mw: if is_on { unit_row.pmin } else { 0.0 },
                            time_in_state: unit_row.tinic,
                        },
                    });
                }

                groups.push(HydroGroup {
                    id: HydroGroupId(group_id),
                    name: format!("{}-{}", row.nome, group_id),
                    units,
                });
            }

            let downstream_plant_id = optional_hydro_reference(&row.jusante, &hydro_name_to_id)?;
            let upstream_plant_ids = optional_hydro_references(&row.montante, &hydro_name_to_id)?;
            let natural_inflow_hm3 =
                inflows_by_plant.get(&row.codigo).cloned().ok_or_else(|| {
                    IoError::invalid_data(format!(
                        "missing hydro inflow data for plant {}",
                        row.codigo
                    ))
                })?;

            Ok(HydroPlant {
                id: HydroPlantId(row.codigo),
                name: row.nome.clone(),
                submarket_id: SubmarketId(row.submercado),
                bus_id: BusId(first_unit.barra),
                upstream_plant_ids,
                downstream_plant_id,
                reservoir: Reservoir {
                    min_volume_hm3: row.vmin,
                    max_volume_hm3: row.vmax,
                    initial_volume_hm3: row.vol_inic,
                },
                natural_inflow_hm3,
                spillage_cost_per_hm3: row.penal_vert,
                groups,
            })
        })
        .collect()
}

fn build_renewables(
    catalog_rows: &[RenewableCatalogRow],
    operation_rows: &[RenewableOperationRow],
    _horizon: usize,
) -> Result<(Vec<WindPlant>, Vec<SolarPlant>), IoError> {
    if catalog_rows.is_empty() && operation_rows.is_empty() {
        return Ok((Vec::new(), Vec::new()));
    }

    Err(IoError::invalid_data(format!(
        "renewable parsing is not implemented yet; found {} catalog rows and {} operation rows",
        catalog_rows.len(),
        operation_rows.len()
    )))
}

fn optional_hydro_reference(
    value: &str,
    hydro_name_to_id: &HashMap<String, HydroPlantId>,
) -> Result<Option<HydroPlantId>, IoError> {
    let trimmed = value.trim();
    if trimmed.is_empty() || trimmed == "-" {
        return Ok(None);
    }

    hydro_name_to_id
        .get(trimmed)
        .copied()
        .map(Some)
        .ok_or_else(|| IoError::invalid_data(format!("unknown hydro reference '{trimmed}'")))
}

fn optional_hydro_references(
    value: &str,
    hydro_name_to_id: &HashMap<String, HydroPlantId>,
) -> Result<Vec<HydroPlantId>, IoError> {
    let trimmed = value.trim();
    if trimmed.is_empty() || trimmed == "-" {
        return Ok(Vec::new());
    }

    trimmed
        .split(',')
        .map(|item| item.trim())
        .filter(|item| !item.is_empty())
        .map(|item| {
            hydro_name_to_id
                .get(item)
                .copied()
                .ok_or_else(|| IoError::invalid_data(format!("unknown hydro reference '{item}'")))
        })
        .collect()
}

fn trajectory_for(
    trajectories: &HashMap<String, Vec<f64>>,
    asset_name: &str,
) -> Result<Vec<f64>, IoError> {
    Ok(trajectories.get(asset_name).cloned().unwrap_or_default())
}

fn validate_period(period: usize, horizon: usize, file_name: &str) -> Result<usize, IoError> {
    if !(1..=horizon).contains(&period) {
        return Err(IoError::invalid_data(format!(
            "invalid period {period} in {file_name}; expected values between 1 and {horizon}"
        )));
    }

    Ok(period - 1)
}

fn read_csv<T: for<'de> Deserialize<'de>>(path: PathBuf) -> Result<Vec<T>, IoError> {
    let mut reader = ReaderBuilder::new()
        .delimiter(b';')
        .has_headers(true)
        .from_path(path)?;

    let mut rows = Vec::new();
    for record in reader.deserialize() {
        rows.push(record?);
    }

    Ok(rows)
}

fn read_trajectory_table(path: PathBuf) -> Result<HashMap<String, Vec<f64>>, IoError> {
    let file = File::open(&path).map_err(|error| {
        IoError::invalid_data(format!("failed to open {}: {error}", path.display()))
    })?;
    let mut reader = ReaderBuilder::new()
        .delimiter(b';')
        .has_headers(true)
        .flexible(true)
        .from_reader(file);

    let headers = reader
        .headers()
        .map_err(IoError::from)?
        .iter()
        .map(|value| value.trim().to_string())
        .collect::<Vec<_>>();

    let mut trajectories: HashMap<String, Vec<f64>> = headers
        .iter()
        .filter(|header| !header.is_empty())
        .map(|header| (header.clone(), Vec::new()))
        .collect();

    for row in reader.records() {
        let row = row?;
        append_trajectory_row(&headers, &row, &mut trajectories);
    }

    Ok(trajectories)
}

fn append_trajectory_row(
    headers: &[String],
    row: &StringRecord,
    trajectories: &mut HashMap<String, Vec<f64>>,
) {
    for (column_idx, header) in headers.iter().enumerate() {
        if header.is_empty() {
            continue;
        }
        let value = row.get(column_idx).unwrap_or("").trim();
        if value.is_empty() {
            continue;
        }
        if let Ok(parsed) = value.parse::<f64>() {
            trajectories.entry(header.clone()).or_default().push(parsed);
        }
    }
}

#[derive(Debug, Deserialize)]
struct DurationRow {
    #[serde(rename = "Periodo")]
    periodo: usize,
    #[serde(rename = "Duracao")]
    duracao: f64,
}

#[derive(Debug, Deserialize)]
struct SubmarketCatalogRow {
    #[serde(rename = "Codigo")]
    codigo: usize,
    #[serde(rename = "Nome")]
    nome: String,
}

#[derive(Debug, Deserialize)]
struct SubmarketOperationRow {
    #[serde(rename = "CodigoSubmercado")]
    codigo_submercado: usize,
    #[serde(rename = "Periodo")]
    periodo: usize,
    #[serde(rename = "Demanda")]
    demanda: f64,
    #[serde(rename = "CustoDeficit")]
    custo_deficit: f64,
}

#[derive(Debug, Deserialize)]
struct BusOperationRow {
    #[serde(rename = "CodigoBarra")]
    codigo_barra: usize,
    #[serde(rename = "NomeBarra")]
    nome_barra: String,
    #[serde(rename = "Periodo")]
    periodo: usize,
    #[serde(rename = "Carga")]
    carga: f64,
    #[serde(rename = "CodigoSubmercado")]
    codigo_submercado: usize,
    #[serde(rename = "Swing")]
    #[serde(deserialize_with = "deserialize_bool_flag")]
    swing: bool,
}

#[derive(Debug, Deserialize)]
struct BranchRow {
    #[serde(rename = "De")]
    de: usize,
    #[serde(rename = "Para")]
    para: usize,
    #[serde(rename = "Capacidade")]
    capacidade: f64,
    #[serde(rename = "Reatancia")]
    reatancia: f64,
    #[serde(rename = "CodigoLinha")]
    codigo_linha: usize,
}

#[derive(Debug, Deserialize)]
struct InterchangeRow {
    #[serde(rename = "SbmDe")]
    sbm_de: usize,
    #[serde(rename = "SbmPara")]
    sbm_para: usize,
    #[serde(rename = "Limite")]
    limite: f64,
    #[serde(rename = "Penalidade")]
    penalidade: f64,
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
    ton: usize,
    #[serde(rename = "Toff")]
    toff: usize,
    #[serde(rename = "GerInic")]
    ger_inic: f64,
    #[serde(rename = "StatusInic")]
    status_inic: usize,
    #[serde(rename = "Tinic")]
    tinic: usize,
    #[serde(rename = "CVU")]
    cvu: f64,
    #[serde(rename = "CustoPartida")]
    custo_partida: f64,
    #[serde(rename = "CustoDesliga")]
    custo_desliga: f64,
    #[serde(rename = "Barra")]
    barra: usize,
    #[serde(rename = "Submercado")]
    submercado: usize,
}

#[derive(Debug, Deserialize)]
struct HydroPlantRow {
    #[serde(rename = "Codigo")]
    codigo: usize,
    #[serde(rename = "Nome")]
    nome: String,
    #[serde(rename = "VolInic")]
    vol_inic: f64,
    #[serde(rename = "Vmin")]
    vmin: f64,
    #[serde(rename = "Vmax")]
    vmax: f64,
    #[serde(rename = "Jusante")]
    jusante: String,
    #[serde(rename = "Montante")]
    montante: String,
    #[serde(rename = "Submercado")]
    submercado: usize,
    #[serde(rename = "PenalVert")]
    penal_vert: f64,
}

#[derive(Debug, Deserialize)]
struct HydroUnitRow {
    #[serde(rename = "Codigo")]
    codigo: usize,
    #[serde(rename = "Conjunto")]
    conjunto: usize,
    #[serde(rename = "Unidade")]
    unidade: usize,
    #[serde(rename = "Pmin")]
    pmin: f64,
    #[serde(rename = "Pmax")]
    pmax: f64,
    #[serde(rename = "Ton")]
    ton: usize,
    #[serde(rename = "Toff")]
    toff: usize,
    #[serde(rename = "StatusInic")]
    status_inic: usize,
    #[serde(rename = "Tinic")]
    tinic: usize,
    #[serde(rename = "Barra")]
    barra: usize,
    #[serde(rename = "MaxTurb")]
    max_turb: f64,
    #[serde(rename = "Prod")]
    prod: f64,
    #[serde(rename = "CustoPartida")]
    custo_partida: f64,
    #[serde(rename = "CustoDesliga")]
    custo_desliga: f64,
}

#[derive(Debug, Deserialize)]
struct HydroInflowRow {
    #[serde(rename = "Codigo")]
    codigo: usize,
    #[serde(rename = "Periodo")]
    periodo: usize,
    #[serde(rename = "Afluencia")]
    afluencia: f64,
}

#[derive(Debug, Deserialize)]
struct RenewableCatalogRow {
    #[serde(rename = "Codigo")]
    _codigo: usize,
    #[serde(rename = "Nome")]
    _nome: String,
    #[serde(rename = "Submercado")]
    _submercado: usize,
}

#[derive(Debug, Deserialize)]
struct RenewableOperationRow {
    #[serde(rename = "Posto")]
    _posto: Option<usize>,
    #[serde(rename = "Codigo")]
    _codigo: Option<usize>,
    #[serde(rename = "Nome")]
    _nome: Option<String>,
    #[serde(rename = "Periodo")]
    _periodo: Option<usize>,
    #[serde(rename = "GerProg")]
    _ger_prog: Option<f64>,
}

#[derive(Debug)]
struct BusAccumulator {
    name: String,
    submarket_id: usize,
    angle_reference: bool,
    demand_mw: Vec<f64>,
}

fn deserialize_bool_flag<'de, D>(deserializer: D) -> Result<bool, D::Error>
where
    D: Deserializer<'de>,
{
    let value = String::deserialize(deserializer)?;
    match value.trim().to_ascii_lowercase().as_str() {
        "true" | "1" => Ok(true),
        "false" | "0" => Ok(false),
        other => Err(serde::de::Error::custom(format!(
            "invalid boolean flag '{other}'"
        ))),
    }
}

#[cfg(test)]
mod tests {
    use super::{read_study_config, read_study_from_config, read_study_from_path};
    use std::path::PathBuf;

    #[test]
    fn reads_example_case_into_a_valid_system() {
        let example_path = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../examples/3Barras");

        let system =
            read_study_from_path(example_path).expect("3Barras example should be readable");

        assert_eq!(system.horizon.periods, 3);
        assert_eq!(system.horizon.period_duration_hours, 1.0);
        assert_eq!(system.submarkets.len(), 2);
        assert_eq!(system.buses.len(), 3);
        assert_eq!(system.branches.len(), 3);
        assert_eq!(system.interchange_limits.len(), 2);
        assert_eq!(system.thermal_plants.len(), 1);
        assert_eq!(system.hydro_plants.len(), 2);
        assert!(system.wind_plants.is_empty());
        assert!(system.solar_plants.is_empty());

        let uhe1 = system
            .hydro_plants
            .iter()
            .find(|plant| plant.name == "UHE1")
            .expect("UHE1 should exist");
        assert_eq!(uhe1.natural_inflow_hm3, vec![100.0, 100.0, 100.0]);
        assert_eq!(uhe1.spillage_cost_per_hm3, 0.1);
        assert_eq!(
            uhe1.downstream_plant_id,
            Some(labdessem_core::ids::HydroPlantId(2))
        );
    }

    #[test]
    fn reads_study_from_json_config() {
        let config_path = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("study_config.json");

        let config = read_study_config(&config_path).expect("study config should be readable");
        assert!(config.case_path.ends_with("examples/3Barras"));

        let system =
            read_study_from_config(config_path).expect("study config should build a valid system");
        assert_eq!(system.submarkets.len(), 2);
        assert_eq!(system.thermal_plants.len(), 1);
        assert_eq!(system.hydro_plants.len(), 2);
    }
}
