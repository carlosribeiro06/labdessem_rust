//! Network (buses, branches) CSV reader.

use std::collections::BTreeMap;
use std::path::Path;

use labdessem_core::entities::Submarket;
use labdessem_core::ids::{BranchId, BusId, SubmarketId};
use labdessem_core::system::{Branch, Bus, StudyHorizon};
use serde::{Deserialize, Deserializer};

use crate::error::IoError;
use crate::readers::common::read_csv;

/// Read buses from OPER_CARGA_BARRA.csv for network-enabled cases.
pub fn read_buses(oper_path: &Path, horizon: &StudyHorizon) -> Result<Vec<Bus>, IoError> {
    let rows: Vec<BusOperationRow> = read_csv(&oper_path.join("OPER_CARGA_BARRA.csv"))?;
    build_buses(&rows, horizon)
}

/// Read branches from OPER_LINHA.csv for network-enabled cases.
pub fn read_branches(oper_path: &Path) -> Result<Vec<Branch>, IoError> {
    let rows: Vec<BranchRow> = read_csv(&oper_path.join("OPER_LINHA.csv"))?;
    Ok(build_branches(&rows))
}

/// Create dummy buses for non-network cases (one per submarket).
pub fn build_dummy_buses(submarkets: &[Submarket], horizon: usize) -> Vec<Bus> {
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

#[derive(Debug)]
struct BusAccumulator {
    name: String,
    submarket_id: usize,
    angle_reference: bool,
    demand_mw: Vec<f64>,
}

#[derive(Debug, Deserialize)]
struct BusOperationRow {
    #[serde(rename = "CodigoBarra")]
    codigo_barra: usize,
    #[serde(rename = "NomeBarra")]
    nome_barra: String,
    #[serde(rename = "Periodo")]
    periodo: usize,
    #[serde(rename = "Carga.mw")]
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
    #[serde(rename = "Capacidade.mw")]
    capacidade: f64,
    #[serde(rename = "Reatancia.pu")]
    reatancia: f64,
    #[serde(rename = "CodigoLinha")]
    codigo_linha: usize,
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
