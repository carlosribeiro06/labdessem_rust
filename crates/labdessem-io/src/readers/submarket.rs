//! Submarket CSV reader.

use std::collections::HashMap;
use std::path::Path;

use labdessem_core::entities::Submarket;
use labdessem_core::ids::SubmarketId;
use labdessem_core::system::StudyHorizon;
use serde::Deserialize;

use crate::error::IoError;
use crate::readers::common::read_csv;

/// Read submarkets from CAD_SBM.csv and OPER_SBM.csv.
pub fn read_submarkets(
    cad_path: &Path,
    oper_path: &Path,
    horizon: &StudyHorizon,
) -> Result<Vec<Submarket>, IoError> {
    let catalog_rows: Vec<SubmarketCatalogRow> = read_csv(&cad_path.join("CAD_SBM.csv"))?;
    let operation_rows: Vec<SubmarketOperationRow> = read_csv(&oper_path.join("OPER_SBM.csv"))?;

    build_submarkets(&catalog_rows, &operation_rows, horizon)
}

fn build_submarkets(
    catalog_rows: &[SubmarketCatalogRow],
    operation_rows: &[SubmarketOperationRow],
    horizon: &StudyHorizon,
) -> Result<Vec<Submarket>, IoError> {
    let mut demand_by_submarket: HashMap<usize, Vec<f64>> = HashMap::new();

    for row in operation_rows {
        let demand_series = demand_by_submarket
            .entry(row.codigo_submercado)
            .or_insert_with(|| vec![0.0; horizon.periods]);
        for period_idx in internal_periods_for_input_period(row.periodo, horizon, "OPER_SBM.csv")? {
            demand_series[period_idx] = row.demanda;
        }
    }

    catalog_rows
        .iter()
        .map(|row| {
            let demand_mw = demand_by_submarket
                .get(&row.codigo)
                .cloned()
                .unwrap_or_else(|| vec![0.0; horizon.periods]);

            Ok(Submarket {
                id: SubmarketId(row.codigo),
                name: row.nome.clone(),
                demand_mw,
            })
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
}
