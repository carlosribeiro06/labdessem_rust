//! Renewable plant CSV reader.

use std::collections::HashMap;
use std::path::Path;

use csv::{ReaderBuilder, StringRecord};
use labdessem_core::entities::{RenewablePlant, Submarket};
use labdessem_core::ids::{BusId, RenewablePlantId};
use labdessem_core::system::StudyHorizon;

use crate::error::IoError;
use crate::readers::common::{bus_id_or_dummy, log_read_file, submarket_id_by_name};

/// Read renewable plants from CAD_REN.csv and OPER_REN.csv.
pub fn read_renewable_plants(
    cad_path: &Path,
    oper_path: &Path,
    submarkets: &[Submarket],
    horizon: &StudyHorizon,
    network_enabled: bool,
) -> Result<Vec<RenewablePlant>, IoError> {
    let catalog_rows = read_renewable_catalog_table(cad_path.join("CAD_REN.csv"))?;
    let operation_rows = read_renewable_operation_table(oper_path.join("OPER_REN.csv"))?;

    build_renewables(
        &catalog_rows,
        &operation_rows,
        submarkets,
        horizon,
        network_enabled,
    )
}

fn build_renewables(
    catalog_rows: &[RenewableCatalogRow],
    operation_rows: &[RenewableOperationRow],
    submarkets: &[Submarket],
    horizon: &StudyHorizon,
    network_enabled: bool,
) -> Result<Vec<RenewablePlant>, IoError> {
    let mut generation_by_plant = HashMap::<usize, Vec<f64>>::new();

    for row in operation_rows {
        let series = generation_by_plant
            .entry(row.codigo)
            .or_insert_with(|| vec![0.0; horizon.periods]);
        for period_idx in internal_periods_for_input_period(row.periodo, horizon, "OPER_REN.csv")? {
            series[period_idx] = row.ger_prog;
        }
    }

    catalog_rows
        .iter()
        .map(|row| {
            let name = row.nome.trim().to_string();
            let submarket_id = submarket_id_by_name(submarkets, &row.submercado)?;
            Ok(RenewablePlant {
                id: RenewablePlantId(row.codigo),
                name: name.clone(),
                submarket_id,
                bus_id: BusId(bus_id_or_dummy(
                    row.barra,
                    network_enabled,
                    name.as_str(),
                    submarket_id,
                )?),
                available_generation_mw: generation_by_plant
                    .get(&row.codigo)
                    .cloned()
                    .unwrap_or_else(|| vec![0.0; horizon.periods]),
            })
        })
        .collect()
}

fn read_renewable_catalog_table(
    path: impl AsRef<Path>,
) -> Result<Vec<RenewableCatalogRow>, IoError> {
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
        .map(|header| header.trim().to_ascii_uppercase())
        .collect::<Vec<_>>();

    let column = |name: &str| -> Result<usize, IoError> {
        headers
            .iter()
            .position(|header| header == name)
            .ok_or_else(|| IoError::invalid_data(format!("missing column '{name}' in CAD_REN.csv")))
    };

    let codigo_idx = column("CODIGO")?;
    let nome_idx = column("NOME")?;
    let barra_idx = column("BARRA")?;
    let submercado_idx = column("SUBMERCADO")?;

    let mut rows = Vec::new();
    for record in reader.records() {
        let record = record?;
        let first_field = record.get(codigo_idx).unwrap_or_default().trim();
        if first_field.is_empty() || first_field.starts_with('-') {
            continue;
        }

        let barra_text = record.get(barra_idx).unwrap_or_default().trim();
        rows.push(RenewableCatalogRow {
            codigo: parse_record_usize(&record, codigo_idx, "Codigo", "CAD_REN.csv")?,
            nome: record.get(nome_idx).unwrap_or_default().trim().to_string(),
            barra: if barra_text.is_empty() || barra_text == "-" {
                None
            } else {
                Some(parse_record_usize(
                    &record,
                    barra_idx,
                    "Barra",
                    "CAD_REN.csv",
                )?)
            },
            submercado: record
                .get(submercado_idx)
                .unwrap_or_default()
                .trim()
                .to_string(),
        });
    }

    Ok(rows)
}

fn read_renewable_operation_table(
    path: impl AsRef<Path>,
) -> Result<Vec<RenewableOperationRow>, IoError> {
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
        .map(|header| header.trim().to_ascii_uppercase())
        .collect::<Vec<_>>();

    let column = |name: &str| -> Result<usize, IoError> {
        headers
            .iter()
            .position(|header| header == name)
            .ok_or_else(|| {
                IoError::invalid_data(format!("missing column '{name}' in OPER_REN.csv"))
            })
    };

    let periodo_idx = column("PERIODO")?;
    let codigo_idx = column("CODIGO")?;
    let nome_idx = column("NOME")?;
    let ger_prog_idx = column("GERPROG.MW")?;

    let mut rows = Vec::new();
    for record in reader.records() {
        let record = record?;
        let first_field = record.get(periodo_idx).unwrap_or_default().trim();
        if first_field.is_empty() || first_field.starts_with('-') {
            continue;
        }

        rows.push(RenewableOperationRow {
            periodo: parse_record_usize(&record, periodo_idx, "Periodo", "OPER_REN.csv")?,
            codigo: parse_record_usize(&record, codigo_idx, "Codigo", "OPER_REN.csv")?,
            _nome: record.get(nome_idx).unwrap_or_default().trim().to_string(),
            ger_prog: parse_record_f64(&record, ger_prog_idx, "GerProg", "OPER_REN.csv")?,
        });
    }

    Ok(rows)
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

#[derive(Debug)]
struct RenewableCatalogRow {
    codigo: usize,
    nome: String,
    barra: Option<usize>,
    submercado: String,
}

#[derive(Debug)]
struct RenewableOperationRow {
    periodo: usize,
    codigo: usize,
    _nome: String,
    ger_prog: f64,
}
