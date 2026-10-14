//! Pumping plant CSV reader.

use std::path::Path;

use csv::{ReaderBuilder, StringRecord};
use labdessem_core::entities::{HydroPlant, Submarket};
use labdessem_core::ids::{BusId, PumpingPlantId};
use labdessem_core::system::PumpingPlant;

use crate::error::IoError;
use crate::readers::common::{bus_id_or_dummy, flow_m3s_to_hm3, log_read_file, submarket_id_by_name};

/// Read pumping plants from CAD_USIE.csv.
pub fn read_pumping_plants(
    cad_path: &Path,
    hydro_plants: &[HydroPlant],
    submarkets: &[Submarket],
    period_duration_hours: f64,
    network_enabled: bool,
) -> Result<Vec<PumpingPlant>, IoError> {
    let rows = read_pumping_table(cad_path.join("CAD_USIE.csv"))?;

    build_pumping_plants(
        &rows,
        hydro_plants,
        submarkets,
        period_duration_hours,
        network_enabled,
    )
}

fn build_pumping_plants(
    rows: &[PumpingPlantRow],
    hydro_plants: &[HydroPlant],
    submarkets: &[Submarket],
    period_duration_hours: f64,
    network_enabled: bool,
) -> Result<Vec<PumpingPlant>, IoError> {
    rows.iter()
        .map(|row| {
            let name = row.nome.trim().to_string();
            let submarket_id = submarket_id_by_name(submarkets, &row.submercado)?;
            let downstream_hydro_id = hydro_plants
                .iter()
                .find(|plant| plant.id.0 == row.jusante)
                .map(|plant| plant.id)
                .ok_or_else(|| {
                    IoError::invalid_data(format!(
                        "pumping plant {} references unknown downstream hydro plant {}",
                        name, row.jusante
                    ))
                })?;
            let upstream_hydro_id = hydro_plants
                .iter()
                .find(|plant| plant.id.0 == row.montante)
                .map(|plant| plant.id)
                .ok_or_else(|| {
                    IoError::invalid_data(format!(
                        "pumping plant {} references unknown upstream hydro plant {}",
                        name, row.montante
                    ))
                })?;

            Ok(PumpingPlant {
                id: PumpingPlantId(row.codigo),
                name: name.clone(),
                submarket_id,
                bus_id: BusId(bus_id_or_dummy(
                    row.barra,
                    network_enabled,
                    name.as_str(),
                    submarket_id,
                )?),
                downstream_hydro_id,
                upstream_hydro_id,
                min_pumping_hm3: flow_m3s_to_hm3(row.qbomb_min, period_duration_hours),
                max_pumping_hm3: flow_m3s_to_hm3(row.qbomb_max, period_duration_hours),
                specific_consumption_mw_per_m3s: row.consumo_especifico,
            })
        })
        .collect()
}

fn read_pumping_table(path: impl AsRef<Path>) -> Result<Vec<PumpingPlantRow>, IoError> {
    let path = path.as_ref();
    log_read_file(path);
    let mut reader = ReaderBuilder::new()
        .delimiter(b';')
        .has_headers(true)
        .from_path(path)?;

    let headers = reader
        .headers()
        .map_err(IoError::from)?
        .iter()
        .map(normalize_column_name)
        .collect::<Vec<_>>();

    let column = |names: &[&str]| -> Result<usize, IoError> {
        headers
            .iter()
            .position(|header| names.iter().any(|name| header == name))
            .ok_or_else(|| {
                IoError::invalid_data(format!(
                    "missing column '{}' in CAD_USIE.csv",
                    names.join("' or '")
                ))
            })
    };

    let codigo_idx = column(&["CODIGO", "USIE"])?;
    let nome_idx = column(&["NOME", "NOMEUSIE"])?;
    let submarket_idx = column(&["SUBMERCADO", "NOMESIST"])?;
    let downstream_idx = column(&["JUSANTE", "USIHJUS"])?;
    let upstream_idx = column(&["MONTANTE", "MONTATE", "USIHMON"])?;
    let min_idx = column(&["QBOMBMIN"])?;
    let max_idx = column(&["QBOMBMAX"])?;
    let consumption_idx = column(&["CONSUMO", "CONSESPEC"])?;
    let bus_idx = headers.iter().position(|header| header == "BARRA");

    let mut rows = Vec::new();
    for record in reader.records() {
        let record = record?;
        let first_field = record.get(codigo_idx).unwrap_or_default().trim();
        if first_field.is_empty() || first_field.starts_with('-') {
            continue;
        }

        rows.push(PumpingPlantRow {
            codigo: parse_record_usize(&record, codigo_idx, "Codigo", "CAD_USIE.csv")?,
            nome: record.get(nome_idx).unwrap_or_default().trim().to_string(),
            submercado: record
                .get(submarket_idx)
                .unwrap_or_default()
                .trim()
                .to_string(),
            jusante: parse_record_usize(&record, downstream_idx, "Jusante", "CAD_USIE.csv")?,
            montante: parse_record_usize(&record, upstream_idx, "Montante", "CAD_USIE.csv")?,
            qbomb_min: parse_record_f64(&record, min_idx, "QbombMin", "CAD_USIE.csv")?,
            qbomb_max: parse_record_f64(&record, max_idx, "QbombMax", "CAD_USIE.csv")?,
            consumo_especifico: parse_record_f64(
                &record,
                consumption_idx,
                "Consumo",
                "CAD_USIE.csv",
            )?,
            barra: match bus_idx {
                Some(idx) => parse_optional_record_usize(&record, idx, "Barra", "CAD_USIE.csv")?,
                None => None,
            },
        });
    }

    Ok(rows)
}

fn normalize_column_name(value: &str) -> String {
    value
        .chars()
        .filter(|character| !character.is_whitespace())
        .collect::<String>()
        .to_ascii_uppercase()
}

fn parse_record_usize(
    record: &StringRecord,
    index: usize,
    field_name: &str,
    file_name: &str,
) -> Result<usize, IoError> {
    record
        .get(index)
        .unwrap_or_default()
        .trim()
        .parse::<usize>()
        .map_err(|_| IoError::invalid_data(format!("invalid {field_name} in {file_name}")))
}

fn parse_record_f64(
    record: &StringRecord,
    index: usize,
    field_name: &str,
    file_name: &str,
) -> Result<f64, IoError> {
    record
        .get(index)
        .unwrap_or_default()
        .trim()
        .parse::<f64>()
        .map_err(|_| IoError::invalid_data(format!("invalid {field_name} in {file_name}")))
}

fn parse_optional_record_usize(
    record: &StringRecord,
    index: usize,
    field_name: &str,
    file_name: &str,
) -> Result<Option<usize>, IoError> {
    let value = record.get(index).unwrap_or_default().trim();
    if value.is_empty() || value == "-" {
        return Ok(None);
    }

    value
        .parse::<usize>()
        .map(Some)
        .map_err(|_| IoError::invalid_data(format!("invalid {field_name} in {file_name}")))
}

#[derive(Debug)]
struct PumpingPlantRow {
    codigo: usize,
    nome: String,
    submercado: String,
    jusante: usize,
    montante: usize,
    qbomb_min: f64,
    qbomb_max: f64,
    consumo_especifico: f64,
    barra: Option<usize>,
}
