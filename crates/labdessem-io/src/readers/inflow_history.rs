use std::path::Path;

use arrow::temporal_conversions::date32_to_datetime;
use chrono::NaiveDate;
use labdessem_core::entities::InflowHistoryRow;

use super::parquet_helpers::{
    extract_required_date32, extract_required_float64, extract_required_int32,
    open_record_batch_reader,
};
use crate::IoError;

pub fn read_inflow_history(oper_path: &Path) -> Result<Vec<InflowHistoryRow>, IoError> {
    let file_path = oper_path.join("inflow_history.parquet");

    if !file_path.exists() {
        return Ok(Vec::new());
    }

    let reader = open_record_batch_reader(&file_path)?;
    let mut rows: Vec<InflowHistoryRow> = Vec::new();

    for batch_result in reader {
        let batch = batch_result
            .map_err(|e| IoError::invalid_data(format!("failed to read batch: {e}")))?;

        let hydro_ids = extract_required_int32(&batch, "hydro_id", &file_path)?;
        let start_dates = extract_required_date32(&batch, "start_date", &file_path)?;
        let end_dates = extract_required_date32(&batch, "end_date", &file_path)?;
        let values = extract_required_float64(&batch, "value_m3s", &file_path)?;

        let n = batch.num_rows();
        rows.reserve(n);

        for i in 0..n {
            let value_m3s = values.value(i);

            if !value_m3s.is_finite() {
                return Err(IoError::invalid_data(format!(
                    "{}: row {i} has non-finite value_m3s",
                    file_path.display()
                )));
            }

            rows.push(InflowHistoryRow {
                hydro_id: hydro_ids.value(i),
                start_date: parse_date32(start_dates.value(i))?,
                end_date: parse_date32(end_dates.value(i))?,
                value_m3s,
            });
        }
    }

    rows.sort_by(|a, b| {
        a.hydro_id
            .cmp(&b.hydro_id)
            .then_with(|| a.start_date.cmp(&b.start_date))
    });

    Ok(rows)
}

fn parse_date32(raw: i32) -> Result<NaiveDate, IoError> {
    date32_to_datetime(raw)
        .map(|dt| dt.date())
        .ok_or_else(|| IoError::invalid_data(format!("invalid date32 value {raw}")))
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;

    #[test]
    fn returns_empty_when_file_absent() {
        let result = read_inflow_history(Path::new("/nonexistent/path"));
        assert!(result.is_ok());
        assert!(result.unwrap().is_empty());
    }

    #[test]
    fn parses_example_file() {
        let oper_path = PathBuf::from("../../examples/24Barras/OPER");
        if !oper_path.join("inflow_history.parquet").exists() {
            return; // skip if example file not available
        }

        let result = read_inflow_history(&oper_path);
        assert!(result.is_ok());

        let rows = result.unwrap();
        assert!(!rows.is_empty());

        // Verify sorting: each subsequent row has >= hydro_id, and within same hydro_id, >= start_date
        for window in rows.windows(2) {
            let a = &window[0];
            let b = &window[1];
            assert!(
                (a.hydro_id < b.hydro_id)
                    || (a.hydro_id == b.hydro_id && a.start_date <= b.start_date)
            );
        }
    }
}
