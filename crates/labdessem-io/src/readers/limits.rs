//! Operational limits, residual costs, and interchange limits readers.

use std::path::Path;

use labdessem_core::entities::{HydroPlant, ThermalPlant};
use labdessem_core::ids::SubmarketId;
use labdessem_core::system::{
    InterchangeLimit, OperationalLimit, OperationalLimitTarget, OperationalLimitVariable,
    PumpingPlant, ResidualCost, StudyHorizon,
};
use serde::Deserialize;

use crate::error::IoError;
use crate::readers::common::read_csv;

/// Read interchange limits from OPER_LIMITE_INTERCAMBIO.csv.
pub fn read_interchange_limits(oper_path: &Path) -> Result<Vec<InterchangeLimit>, IoError> {
    let rows: Vec<InterchangeRow> = read_csv(&oper_path.join("OPER_LIMITE_INTERCAMBIO.csv"))?;
    Ok(build_interchange_limits(&rows))
}

/// Read residual costs from OPER_CUSTO_RESIDUAL.csv.
pub fn read_residual_costs(oper_path: &Path, enabled: bool) -> Result<Vec<ResidualCost>, IoError> {
    if !enabled {
        return Ok(Vec::new());
    }
    let rows: Vec<ResidualCostRow> = read_csv(&oper_path.join("OPER_CUSTO_RESIDUAL.csv"))?;
    Ok(build_residual_costs(&rows))
}

/// Read operational limits from OPER_REST_LIM.csv.
pub fn read_operational_limits(
    oper_path: &Path,
    thermal_plants: &[ThermalPlant],
    hydro_plants: &[HydroPlant],
    pumping_plants: &[PumpingPlant],
    horizon: &StudyHorizon,
) -> Result<Vec<OperationalLimit>, IoError> {
    let rows: Vec<OperationalLimitRow> = read_csv(&oper_path.join("OPER_REST_LIM.csv"))?;
    build_operational_limits(&rows, thermal_plants, hydro_plants, pumping_plants, horizon)
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
                        (Some(plant), None) => OperationalLimitTarget::ThermalPlant(plant.id),
                        (None, Some(plant)) => OperationalLimitTarget::HydroPlant(plant.id),
                        (Some(_), Some(_)) => {
                            return Err(IoError::invalid_data(format!(
                                "operational limit for {plant_name} is ambiguous between thermal and hydro generation"
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
