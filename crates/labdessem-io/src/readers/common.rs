//! Common utilities for CSV readers.

use std::collections::HashMap;
use std::io::{self, Write};
use std::path::Path;

use csv::ReaderBuilder;
use labdessem_core::entities::Submarket;
use labdessem_core::ids::SubmarketId;
use serde::de::DeserializeOwned;

use crate::error::IoError;

/// Read a CSV file with semicolon delimiter into a vector of deserialized rows.
pub fn read_csv<T: DeserializeOwned>(path: &Path) -> Result<Vec<T>, IoError> {
    log_read_file(path);
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

/// Log the file being read to stdout.
pub fn log_read_file(path: &Path) {
    println!("Lendo arquivo: {}", file_label(path));
    io::stdout().flush().ok();
}

/// Extract the file name from a path for display purposes.
pub fn file_label(path: &Path) -> String {
    path.file_name().map_or_else(
        || path.display().to_string(),
        |name| name.to_string_lossy().into_owned(),
    )
}

/// Convert a duration in hours to a number of periods, rounding up.
pub fn hours_to_periods(
    hours: f64,
    period_duration_hours: f64,
    field_name: &str,
    asset_name: &str,
) -> Result<usize, IoError> {
    if hours < 0.0 {
        return Err(IoError::invalid_data(format!(
            "{field_name} for {asset_name} cannot be negative"
        )));
    }

    let periods = (hours / period_duration_hours).ceil();
    if !periods.is_finite() {
        return Err(IoError::invalid_data(format!(
            "failed to convert {field_name} for {asset_name} from hours to periods"
        )));
    }

    Ok(periods as usize)
}

/// Find a submarket by name and return its ID.
pub fn submarket_id_by_name(submarkets: &[Submarket], value: &str) -> Result<SubmarketId, IoError> {
    let submarket_name = value.trim();
    submarkets
        .iter()
        .find(|submarket| submarket.name == submarket_name)
        .map(|submarket| submarket.id)
        .ok_or_else(|| IoError::invalid_data(format!("unknown submarket '{submarket_name}'")))
}

/// Validate that min <= max with tolerance, returning the effective max.
pub fn normalize_max_bound(
    min_value: f64,
    max_value: f64,
    field_name: &str,
    asset_name: &str,
) -> Result<f64, IoError> {
    if min_value <= max_value {
        return Ok(max_value);
    }

    if (min_value - max_value).abs() <= 1e-2 {
        return Ok(min_value);
    }

    Err(IoError::invalid_data(format!(
        "{field_name} for {asset_name} is below the minimum value"
    )))
}

/// Get a trajectory from the map, returning an error if not found.
pub fn trajectory_for(
    trajectories: &HashMap<String, Vec<f64>>,
    asset_name: &str,
) -> Result<Vec<f64>, IoError> {
    trajectories
        .get(asset_name)
        .cloned()
        .ok_or_else(|| IoError::invalid_data(format!("missing trajectory for '{asset_name}'")))
}

/// Get bus ID or a dummy value based on network mode.
pub fn bus_id_or_dummy(
    bus_id: Option<usize>,
    network_enabled: bool,
    asset_name: &str,
    dummy_submarket_id: SubmarketId,
) -> Result<usize, IoError> {
    if network_enabled {
        bus_id.ok_or_else(|| {
            IoError::invalid_data(format!(
                "missing Barra for {asset_name}; Barra is required when rede = 1"
            ))
        })
    } else {
        Ok(dummy_submarket_id.0)
    }
}

/// Convert flow from m3/s to hm3 over a period.
pub fn flow_m3s_to_hm3(flow_m3s: f64, period_duration_hours: f64) -> f64 {
    flow_m3s * period_duration_hours * 0.0036
}
