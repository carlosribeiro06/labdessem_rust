use std::{
    collections::{BTreeMap, HashMap},
    fs,
    path::{Path, PathBuf},
};

use csv::{ReaderBuilder, StringRecord};
use labdessem_core::{
    entities::{HydroPlant, ThermalPlant},
    ids::{BranchId, BusId, SubmarketId},
    system::{
        Branch, Bus, FutureCostCut, InterchangeLimit, OperationalLimit, OperationalLimitTarget,
        OperationalLimitVariable, PumpingPlant, ResidualCost, StudyHorizon, Submarket, System,
    },
};
use serde::{Deserialize, Deserializer};

use crate::error::IoError;
use crate::readers::{
    read_csv, read_hydro_plants, read_pumping_plants, read_renewable_plants, read_thermal_plants,
};

#[derive(Debug, Clone, Deserialize, PartialEq, Eq)]
pub struct StudyConfig {
    pub opcao_execucao: u8,
    pub rede: u8,
    #[serde(rename = "UCT")]
    pub uct: u8,
    #[serde(rename = "UCH")]
    pub uch: u8,
    #[serde(rename = "FPHA")]
    pub fpha: u8,
    #[serde(rename = "FCF", default)]
    pub fcf: u8,
    #[serde(rename = "TON_Residual")]
    pub ton_residual: u8,
}

pub fn read_study_config(config_path: impl AsRef<Path>) -> Result<StudyConfig, IoError> {
    let config_path = config_path.as_ref();
    let contents = fs::read_to_string(config_path).map_err(|error| {
        IoError::invalid_data(format!(
            "failed to read study config {}: {error}",
            config_path.display()
        ))
    })?;

    let config: StudyConfig = serde_json::from_str(&contents)?;
    Ok(config)
}

pub fn read_study_from_path(case_path: impl AsRef<Path>) -> Result<System, IoError> {
    let case_path = case_path.as_ref();
    let config_path = case_path.join("study_config.json");

    if !config_path.exists() {
        return Err(IoError::invalid_data(format!(
            "Configuration file not found.\n\
             Expected: {}\n\n\
             Each study case must contain a study_config.json file in its root directory.",
            config_path.display()
        )));
    }

    let config = read_study_config(&config_path)?;
    read_study_from_path_with_options(
        case_path,
        config.ton_residual != 0,
        config.rede != 0,
        config.uct != 0,
        config.uch != 0,
        config.fpha != 0,
        config.fcf != 0,
    )
}

fn read_study_from_path_with_options(
    base_path: impl AsRef<Path>,
    ton_residual_enabled: bool,
    network_enabled: bool,
    thermal_unit_commitment_enabled: bool,
    hydro_unit_commitment_enabled: bool,
    fpha_enabled: bool,
    future_cost_enabled: bool,
) -> Result<System, IoError> {
    let base_path = base_path.as_ref();
    let cad_path = base_path.join("CAD");
    let oper_path = base_path.join("OPER");

    let submarket_catalog: Vec<SubmarketCatalogRow> = read_csv(cad_path.join("CAD_SBM.csv"))?;
    let duration_rows: Vec<DurationRow> = read_csv(oper_path.join("OPER_DURACAO.csv"))?;
    let submarket_operation_rows: Vec<SubmarketOperationRow> =
        read_csv(oper_path.join("OPER_SBM.csv"))?;
    let bus_rows: Vec<BusOperationRow> = if network_enabled {
        read_csv(oper_path.join("OPER_CARGA_BARRA.csv"))?
    } else {
        Vec::new()
    };
    let branch_rows: Vec<BranchRow> = if network_enabled {
        read_csv(oper_path.join("OPER_LINHA.csv"))?
    } else {
        Vec::new()
    };
    let interchange_rows: Vec<InterchangeRow> =
        read_csv(oper_path.join("OPER_LIMITE_INTERCAMBIO.csv"))?;
    let residual_cost_rows: Vec<ResidualCostRow> = if ton_residual_enabled {
        read_csv(oper_path.join("OPER_CUSTO_RESIDUAL.csv"))?
    } else {
        Vec::new()
    };
    let future_cost_cuts = if future_cost_enabled {
        read_future_cost_cuts(oper_path.join("OPER_FCF.bin"))?
    } else {
        Vec::new()
    };

    let horizon = build_horizon(&duration_rows)?;
    let submarkets = build_submarkets(&submarket_catalog, &submarket_operation_rows, &horizon)?;
    let buses = if network_enabled {
        build_buses(&bus_rows, &horizon)?
    } else {
        build_dummy_buses(&submarkets, horizon.periods)
    };
    let branches = if network_enabled {
        build_branches(&branch_rows)
    } else {
        Vec::new()
    };
    let interchange_limits = build_interchange_limits(&interchange_rows);
    let residual_costs = build_residual_costs(&residual_cost_rows);
    let operational_limit_rows: Vec<OperationalLimitRow> =
        read_csv(oper_path.join("OPER_REST_LIM.csv"))?;
    let thermal_plants = read_thermal_plants(
        &cad_path,
        &submarkets,
        horizon.period_duration_hours,
        network_enabled,
        thermal_unit_commitment_enabled,
    )?;
    let hydro_plants = read_hydro_plants(
        &cad_path,
        &oper_path,
        &submarkets,
        &horizon,
        network_enabled,
        fpha_enabled,
    )?;
    let pumping_plants = read_pumping_plants(
        &cad_path,
        &hydro_plants,
        &submarkets,
        horizon.period_duration_hours,
        network_enabled,
    )?;
    let renewable_plants = read_renewable_plants(
        &cad_path,
        &oper_path,
        &submarkets,
        &horizon,
        network_enabled,
    )?;
    let operational_limits = build_operational_limits(
        &operational_limit_rows,
        &thermal_plants,
        &hydro_plants,
        &pumping_plants,
        &horizon,
    )?;

    let system = System {
        horizon,
        thermal_unit_commitment_enabled,
        hydro_unit_commitment_enabled,
        ton_residual_enabled,
        fpha_enabled,
        future_cost_enabled,
        residual_costs,
        future_cost_cuts,
        submarkets,
        interchange_limits,
        operational_limits,
        buses,
        branches,
        thermal_plants,
        hydro_plants,
        pumping_plants,
        renewable_plants,
    };

    system.validate()?;

    Ok(system)
}

const INTERNAL_PERIOD_DURATION_HOURS: f64 = 0.5;

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

    let mut original_period_durations_hours = Vec::with_capacity(expected_periods);
    let mut internal_to_original_period = Vec::new();
    let mut internal_subperiod_index = Vec::new();

    for original_period in 1..=expected_periods {
        let duration = *periods
            .get(&original_period)
            .expect("duration should exist after sequence validation");
        if duration <= 0.0 {
            return Err(IoError::invalid_data(format!(
                "duration for period {original_period} must be positive"
            )));
        }
        let subperiods = duration / INTERNAL_PERIOD_DURATION_HOURS;
        let rounded_subperiods = subperiods.round();
        if (subperiods - rounded_subperiods).abs() > 1e-8 {
            return Err(IoError::invalid_data(format!(
                "duration for period {original_period} must be a multiple of {INTERNAL_PERIOD_DURATION_HOURS} hour"
            )));
        }
        let subperiod_count = rounded_subperiods as usize;
        if subperiod_count == 0 {
            return Err(IoError::invalid_data(format!(
                "duration for period {original_period} is below the internal time step"
            )));
        }

        original_period_durations_hours.push(duration);
        for subperiod in 1..=subperiod_count {
            internal_to_original_period.push(original_period);
            internal_subperiod_index.push(subperiod);
        }
    }

    Ok(StudyHorizon {
        periods: internal_to_original_period.len(),
        period_duration_hours: INTERNAL_PERIOD_DURATION_HOURS,
        original_periods: expected_periods,
        original_period_durations_hours,
        internal_to_original_period,
        internal_subperiod_index,
    })
}

fn build_submarkets(
    catalog_rows: &[SubmarketCatalogRow],
    operation_rows: &[SubmarketOperationRow],
    horizon: &StudyHorizon,
) -> Result<Vec<Submarket>, IoError> {
    let mut demand_by_submarket: HashMap<usize, Vec<f64>> = HashMap::new();
    let mut deficit_cost_by_submarket: HashMap<usize, f64> = HashMap::new();

    for row in operation_rows {
        let demand_series = demand_by_submarket
            .entry(row.codigo_submercado)
            .or_insert_with(|| vec![0.0; horizon.periods]);
        for period_idx in internal_periods_for_input_period(row.periodo, horizon, "OPER_SBM.csv")? {
            demand_series[period_idx] = row.demanda;
        }

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
                .unwrap_or_else(|| vec![0.0; horizon.periods]);

            let deficit_cost_per_mwh =
                *deficit_cost_by_submarket
                    .get(&row.codigo)
                    .unwrap_or_else(|| {
                        deficit_cost_by_submarket
                            .values()
                            .next()
                            .expect("OPER_SBM.csv must contain at least one deficit cost")
                    });

            Ok(Submarket {
                id: SubmarketId(row.codigo),
                name: row.nome.clone(),
                demand_mw,
                deficit_cost_per_mwh,
            })
        })
        .collect()
}

fn build_buses(rows: &[BusOperationRow], horizon: &StudyHorizon) -> Result<Vec<Bus>, IoError> {
    let mut buses = BTreeMap::<usize, BusAccumulator>::new();

    for row in rows {
        let entry = buses
            .entry(row.codigo_barra)
            .or_insert_with(|| BusAccumulator {
                name: row.nome_barra.clone(),
                submarket_id: row.codigo_submercado,
                angle_reference: row.swing,
                demand_mw: vec![0.0; horizon.periods],
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

        for period_idx in
            internal_periods_for_input_period(row.periodo, horizon, "OPER_CARGA_BARRA.csv")?
        {
            entry.demand_mw[period_idx] = row.carga;
        }
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

fn internal_periods_for_input_period(
    period: usize,
    horizon: &StudyHorizon,
    file_name: &str,
) -> Result<Vec<usize>, IoError> {
    if !(1..=horizon.original_periods).contains(&period) {
        return Err(IoError::invalid_data(format!(
            "invalid period {period} in {file_name}; expected values between 1 and {}",
            horizon.original_periods
        )));
    }

    Ok(horizon.internal_periods_for_original(period))
}

fn build_dummy_buses(submarkets: &[Submarket], horizon: usize) -> Vec<Bus> {
    submarkets
        .iter()
        .map(|submarket| Bus {
            id: BusId(submarket.id.0),
            name: format!("DUMMY-BUS-{}", submarket.name),
            submarket_id: submarket.id,
            angle_reference: submarket.id == SubmarketId(1),
            demand_mw: vec![0.0; horizon],
        })
        .collect()
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

fn build_residual_costs(rows: &[ResidualCostRow]) -> Vec<ResidualCost> {
    rows.iter()
        .map(|row| ResidualCost {
            submarket_id: SubmarketId(row.submercado),
            cmo_per_mwh: row.cmo,
        })
        .collect()
}

fn build_operational_limits(
    rows: &[OperationalLimitRow],
    thermal_plants: &[ThermalPlant],
    hydro_plants: &[HydroPlant],
    pumping_plants: &[PumpingPlant],
    horizon: &StudyHorizon,
) -> Result<Vec<OperationalLimit>, IoError> {
    rows.iter()
        .map(|row| {
            let start_original =
                parse_restriction_period(&row.periodo_inicial, horizon.original_periods, true)?;
            let end_original =
                parse_restriction_period(&row.periodo_final, horizon.original_periods, false)?;
            if start_original > end_original {
                return Err(IoError::invalid_data(format!(
                    "operational limit for {} has initial period after final period",
                    row.nome_usina
                )));
            }
            let start_period = horizon
                .internal_periods_for_original(start_original)
                .first()
                .copied()
                .map(|period| period + 1)
                .ok_or_else(|| {
                    IoError::invalid_data(format!(
                        "period {start_original} has no internal representation"
                    ))
                })?;
            let end_period = horizon
                .internal_periods_for_original(end_original)
                .last()
                .copied()
                .map(|period| period + 1)
                .ok_or_else(|| {
                    IoError::invalid_data(format!(
                        "period {end_original} has no internal representation"
                    ))
                })?;
            let variable = parse_operational_limit_variable(&row.variavel)?;
            let lower_bound = parse_optional_bound(&row.linf, "Linf")?;
            let upper_bound = parse_optional_bound(&row.lsup, "Lsup")?;
            let plant_name = row.nome_usina.trim();

            let thermal_match = thermal_plants
                .iter()
                .find(|plant| plant.id.0 == row.codigo_usina && plant.name == plant_name);
            let hydro_match = hydro_plants
                .iter()
                .find(|plant| plant.id.0 == row.codigo_usina && plant.name == plant_name);
            let pumping_match = pumping_plants
                .iter()
                .find(|plant| plant.id.0 == row.codigo_usina && plant.name == plant_name);

            let target = match variable {
                OperationalLimitVariable::Generation => {
                    match (thermal_match, hydro_match) {
                        (Some(plant), None) => {
                            OperationalLimitTarget::ThermalPlant(plant.id)
                        }
                        (None, Some(plant)) => {
                            OperationalLimitTarget::HydroPlant(plant.id)
                        }
                        (Some(_), Some(_)) => {
                            return Err(IoError::invalid_data(format!(
                                "operational limit for {} is ambiguous between thermal and hydro generation",
                                plant_name
                            )));
                        }
                        (None, None) => {
                            return Err(IoError::invalid_data(format!(
                                "unknown plant {} ({}) in OPER_REST_LIM.csv",
                                plant_name, row.codigo_usina
                            )));
                        }
                    }
                }
                OperationalLimitVariable::Pumping => {
                    if let Some(thermal) = thermal_match {
                        return Err(IoError::invalid_data(format!(
                            "thermal plant {} cannot define restriction for variable {}",
                            thermal.name, row.variavel
                        )));
                    }
                    if let Some(hydro) = hydro_match {
                        return Err(IoError::invalid_data(format!(
                            "hydro plant {} cannot define restriction for variable {}",
                            hydro.name, row.variavel
                        )));
                    }

                    let pumping = pumping_match.ok_or_else(|| {
                        IoError::invalid_data(format!(
                            "unknown pumping plant {} ({}) in OPER_REST_LIM.csv",
                            plant_name, row.codigo_usina
                        ))
                    })?;
                    OperationalLimitTarget::PumpingPlant(pumping.id)
                }
                _ => {
                    if let Some(thermal) = thermal_match {
                        return Err(IoError::invalid_data(format!(
                            "thermal plant {} cannot define restriction for variable {}",
                            thermal.name, row.variavel
                        )));
                    }

                    let hydro = hydro_match.ok_or_else(|| {
                        IoError::invalid_data(format!(
                            "unknown hydro plant {} ({}) in OPER_REST_LIM.csv",
                            plant_name, row.codigo_usina
                        ))
                    })?;
                    OperationalLimitTarget::HydroPlant(hydro.id)
                }
            };

            Ok(OperationalLimit {
                target,
                plant_name: plant_name.to_string(),
                variable,
                start_period,
                end_period,
                lower_bound,
                upper_bound,
            })
        })
        .collect()
}

fn parse_restriction_period(value: &str, horizon: usize, is_start: bool) -> Result<usize, IoError> {
    let trimmed = value.trim();
    if trimmed.eq_ignore_ascii_case("I") {
        return Ok(1);
    }
    if trimmed.eq_ignore_ascii_case("F") {
        return Ok(horizon);
    }

    let period = trimmed.parse::<usize>().map_err(|_| {
        IoError::invalid_data(format!("invalid restriction period value '{trimmed}'"))
    })?;

    if !(1..=horizon).contains(&period) {
        return Err(IoError::invalid_data(format!(
            "restriction {} period {} is outside study horizon 1..={}",
            if is_start { "initial" } else { "final" },
            period,
            horizon
        )));
    }

    Ok(period)
}

fn parse_optional_bound(value: &str, field_name: &str) -> Result<Option<f64>, IoError> {
    let trimmed = value.trim();
    if trimmed.is_empty() || trimmed == "-" {
        return Ok(None);
    }

    let parsed = trimmed.parse::<f64>().map_err(|_| {
        IoError::invalid_data(format!(
            "invalid {field_name} value '{trimmed}' in OPER_REST_LIM.csv"
        ))
    })?;
    Ok(Some(parsed))
}

fn parse_operational_limit_variable(value: &str) -> Result<OperationalLimitVariable, IoError> {
    match value.trim().to_ascii_uppercase().as_str() {
        "GER" => Ok(OperationalLimitVariable::Generation),
        "VERT" => Ok(OperationalLimitVariable::Spillage),
        "VOL" => Ok(OperationalLimitVariable::Volume),
        "DEFLU" => Ok(OperationalLimitVariable::Defluence),
        "TURB" => Ok(OperationalLimitVariable::Turbining),
        "QDES" => Ok(OperationalLimitVariable::Diversion),
        "QBOM" => Ok(OperationalLimitVariable::Pumping),
        other => Err(IoError::invalid_data(format!(
            "unknown operational limit variable '{other}' in OPER_REST_LIM.csv"
        ))),
    }
}

fn read_future_cost_cuts(path: PathBuf) -> Result<Vec<FutureCostCut>, IoError> {
    let bytes = fs::read(&path).map_err(|error| {
        IoError::invalid_data(format!(
            "failed to read future cost binary {}: {error}",
            path.display()
        ))
    })?;
    if bytes.len() < 8 {
        return Err(IoError::invalid_data(format!(
            "future cost binary {} is too small to be a valid FlatBuffers buffer",
            path.display()
        )));
    }

    let root_pos = read_fb_u32(&bytes, 0)? as usize;
    let stage_table = fb_table(&bytes, root_pos, "StageCuts")?;
    let stage_id = fb_get_u32(&bytes, &stage_table, 0)?.unwrap_or(0) as usize;
    let cuts_vector_pos = fb_get_vector_pos(&bytes, &stage_table, 4)?.ok_or_else(|| {
        IoError::invalid_data(format!(
            "future cost binary {} does not contain a cuts vector",
            path.display()
        ))
    })?;
    let cut_count = fb_vector_len(&bytes, cuts_vector_pos)?;
    let mut cuts = Vec::with_capacity(cut_count);

    for cut_idx in 0..cut_count {
        let cut_table_pos = fb_get_table_from_vector(&bytes, cuts_vector_pos, cut_idx)?;
        let cut_table = fb_table(&bytes, cut_table_pos, "Cut")?;
        let coefficients = if let Some(coeff_vector_pos) = fb_get_vector_pos(&bytes, &cut_table, 6)?
        {
            let coefficient_count = fb_vector_len(&bytes, coeff_vector_pos)?;
            (0..coefficient_count)
                .map(|coefficient_idx| {
                    fb_get_f64_from_vector(&bytes, coeff_vector_pos, coefficient_idx)
                })
                .collect::<Result<Vec<_>, IoError>>()?
        } else {
            Vec::new()
        };

        cuts.push(FutureCostCut {
            stage_id,
            cut_id: fb_get_u64(&bytes, &cut_table, 0)?.unwrap_or(0),
            slot_index: fb_get_u32(&bytes, &cut_table, 1)?.unwrap_or(cut_idx as u32) as usize,
            iteration: fb_get_u32(&bytes, &cut_table, 2)?.unwrap_or(0) as usize,
            forward_pass_index: fb_get_u32(&bytes, &cut_table, 3)?.unwrap_or(0) as usize,
            intercept: fb_get_f64(&bytes, &cut_table, 5)?.unwrap_or(0.0),
            coefficients,
            is_active: fb_get_bool(&bytes, &cut_table, 8)?.unwrap_or(false),
        });
    }

    Ok(cuts)
}

#[derive(Debug, Clone, Copy)]
struct FbTable {
    table_pos: usize,
    vtable_pos: usize,
}

fn fb_table(bytes: &[u8], table_pos: usize, label: &str) -> Result<FbTable, IoError> {
    let vtable_distance = read_fb_i32(bytes, table_pos)?;
    let vtable_pos = if vtable_distance >= 0 {
        table_pos
            .checked_sub(vtable_distance as usize)
            .ok_or_else(|| {
                IoError::invalid_data(format!(
                    "{label} table at position {table_pos} points to an invalid vtable"
                ))
            })?
    } else {
        table_pos
            .checked_add(vtable_distance.unsigned_abs() as usize)
            .ok_or_else(|| {
                IoError::invalid_data(format!(
                    "{label} table at position {table_pos} points to an invalid vtable"
                ))
            })?
    };
    let _ = read_fb_u16(bytes, vtable_pos)?;
    let _ = read_fb_u16(bytes, vtable_pos + 2)?;

    Ok(FbTable {
        table_pos,
        vtable_pos,
    })
}

fn fb_field_offset(
    bytes: &[u8],
    table: &FbTable,
    field_id: usize,
) -> Result<Option<usize>, IoError> {
    let field_pos = table.vtable_pos + 4 + field_id * 2;
    if field_pos + 2 > bytes.len() {
        return Ok(None);
    }
    let offset = read_fb_u16(bytes, field_pos)? as usize;
    if offset == 0 {
        Ok(None)
    } else {
        Ok(Some(table.table_pos + offset))
    }
}

fn fb_get_u32(bytes: &[u8], table: &FbTable, field_id: usize) -> Result<Option<u32>, IoError> {
    fb_field_offset(bytes, table, field_id)?
        .map(|pos| read_fb_u32(bytes, pos))
        .transpose()
}

fn fb_get_u64(bytes: &[u8], table: &FbTable, field_id: usize) -> Result<Option<u64>, IoError> {
    fb_field_offset(bytes, table, field_id)?
        .map(|pos| read_fb_u64(bytes, pos))
        .transpose()
}

fn fb_get_f64(bytes: &[u8], table: &FbTable, field_id: usize) -> Result<Option<f64>, IoError> {
    fb_field_offset(bytes, table, field_id)?
        .map(|pos| read_fb_f64(bytes, pos))
        .transpose()
}

fn fb_get_bool(bytes: &[u8], table: &FbTable, field_id: usize) -> Result<Option<bool>, IoError> {
    fb_field_offset(bytes, table, field_id)?
        .map(|pos| read_fb_bool(bytes, pos))
        .transpose()
}

fn fb_get_vector_pos(
    bytes: &[u8],
    table: &FbTable,
    field_id: usize,
) -> Result<Option<usize>, IoError> {
    let Some(field_pos) = fb_field_offset(bytes, table, field_id)? else {
        return Ok(None);
    };
    let vector_offset = read_fb_u32(bytes, field_pos)? as usize;
    Ok(Some(field_pos + vector_offset))
}

fn fb_vector_len(bytes: &[u8], vector_pos: usize) -> Result<usize, IoError> {
    Ok(read_fb_u32(bytes, vector_pos)? as usize)
}

fn fb_get_table_from_vector(
    bytes: &[u8],
    vector_pos: usize,
    index: usize,
) -> Result<usize, IoError> {
    let element_pos = vector_pos + 4 + index * 4;
    let relative = read_fb_u32(bytes, element_pos)? as usize;
    Ok(element_pos + relative)
}

fn fb_get_f64_from_vector(bytes: &[u8], vector_pos: usize, index: usize) -> Result<f64, IoError> {
    let element_pos = vector_pos + 4 + index * 8;
    read_fb_f64(bytes, element_pos)
}

fn read_fb_u16(bytes: &[u8], pos: usize) -> Result<u16, IoError> {
    let slice = bytes.get(pos..pos + 2).ok_or_else(|| {
        IoError::invalid_data(format!(
            "FlatBuffers read out of bounds at byte range [{pos}, {})",
            pos + 2
        ))
    })?;
    Ok(u16::from_le_bytes([slice[0], slice[1]]))
}

fn read_fb_u32(bytes: &[u8], pos: usize) -> Result<u32, IoError> {
    let slice = bytes.get(pos..pos + 4).ok_or_else(|| {
        IoError::invalid_data(format!(
            "FlatBuffers read out of bounds at byte range [{pos}, {})",
            pos + 4
        ))
    })?;
    Ok(u32::from_le_bytes([slice[0], slice[1], slice[2], slice[3]]))
}

fn read_fb_i32(bytes: &[u8], pos: usize) -> Result<i32, IoError> {
    let slice = bytes.get(pos..pos + 4).ok_or_else(|| {
        IoError::invalid_data(format!(
            "FlatBuffers read out of bounds at byte range [{pos}, {})",
            pos + 4
        ))
    })?;
    Ok(i32::from_le_bytes([slice[0], slice[1], slice[2], slice[3]]))
}

fn read_fb_u64(bytes: &[u8], pos: usize) -> Result<u64, IoError> {
    let slice = bytes.get(pos..pos + 8).ok_or_else(|| {
        IoError::invalid_data(format!(
            "FlatBuffers read out of bounds at byte range [{pos}, {})",
            pos + 8
        ))
    })?;
    Ok(u64::from_le_bytes([
        slice[0], slice[1], slice[2], slice[3], slice[4], slice[5], slice[6], slice[7],
    ]))
}

fn read_fb_f64(bytes: &[u8], pos: usize) -> Result<f64, IoError> {
    let slice = bytes.get(pos..pos + 8).ok_or_else(|| {
        IoError::invalid_data(format!(
            "FlatBuffers read out of bounds at byte range [{pos}, {})",
            pos + 8
        ))
    })?;
    Ok(f64::from_le_bytes([
        slice[0], slice[1], slice[2], slice[3], slice[4], slice[5], slice[6], slice[7],
    ]))
}

fn read_fb_bool(bytes: &[u8], pos: usize) -> Result<bool, IoError> {
    let value = *bytes.get(pos).ok_or_else(|| {
        IoError::invalid_data(format!(
            "FlatBuffers read out of bounds at byte index {pos}"
        ))
    })?;
    Ok(value != 0)
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
struct ResidualCostRow {
    #[serde(rename = "Submercado")]
    submercado: usize,
    #[serde(rename = "CMO")]
    cmo: f64,
}

fn log_read_file(path: &Path) {
    crate::readers::log_read_file(path);
}

#[derive(Debug, Deserialize)]
struct OperationalLimitRow {
    #[serde(rename = "PeriodoInicial")]
    periodo_inicial: String,
    #[serde(rename = "PeriodoFinal")]
    periodo_final: String,
    #[serde(rename = "CodigoUsina")]
    codigo_usina: usize,
    #[serde(rename = "NomeUsina")]
    nome_usina: String,
    #[serde(rename = "Variavel")]
    variavel: String,
    #[serde(rename = "Linf")]
    linf: String,
    #[serde(rename = "Lsup")]
    lsup: String,
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
    use super::{read_study_config, read_study_from_path};
    use std::path::PathBuf;

    fn examples_24barras_path() -> PathBuf {
        PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../examples/24Barras")
    }

    #[test]
    fn reads_current_case() {
        let case_path = examples_24barras_path();

        let system = read_study_from_path(&case_path).expect("24Barras should be readable");
        assert_eq!(system.submarkets.len(), 2);
        assert_eq!(system.buses.len(), 2);
        assert!(system.branches.is_empty());
        assert_eq!(system.thermal_plants.len(), 3);
        assert_eq!(system.hydro_plants.len(), 4);
        assert!(!system.hydro_unit_commitment_enabled);
    }

    #[test]
    fn reads_study_config_without_case_path() {
        let config_path = examples_24barras_path().join("study_config.json");

        let config = read_study_config(&config_path).expect("study config should be readable");
        assert_eq!(config.rede, 0);
        assert!(config.uct <= 1);
        assert!(config.uch <= 1);
        assert!(config.fcf <= 1);
        assert!(config.ton_residual <= 1);
    }

    #[test]
    fn reads_study_from_case_directory() {
        let case_path = examples_24barras_path();

        let system =
            read_study_from_path(&case_path).expect("study config should build a valid system");
        assert_eq!(system.submarkets.len(), 2);
        assert_eq!(system.buses.len(), 2);
    }

    #[test]
    fn returns_error_when_config_missing() {
        let case_path = PathBuf::from("/nonexistent/path");

        let result = read_study_from_path(&case_path);
        assert!(result.is_err());
        let error_message = result.unwrap_err().to_string();
        assert!(error_message.contains("Configuration file not found"));
        assert!(error_message.contains("study_config.json"));
    }

    #[test]
    fn reads_thermal_plants_with_trajectories() {
        let case_path = examples_24barras_path();

        let system = read_study_from_path(&case_path).expect("24Barras should be readable");

        let termica1 = system
            .thermal_plants
            .iter()
            .find(|p| p.name == "Termica-1")
            .expect("Termica-1 should exist");

        let unit1 = termica1
            .units
            .iter()
            .find(|u| u.id.0 == 1)
            .expect("Termica-1 unit 1 should exist");

        assert_eq!(
            unit1.startup_trajectory_mw,
            vec![0.0, 0.0, 20.0, 20.0, 60.0, 80.0, 120.0, 160.0, 180.0, 220.0, 280.0, 410.0, 450.0, 523.0]
        );
        assert_eq!(unit1.shutdown_trajectory_mw, vec![255.0, 0.0]);
    }
}
