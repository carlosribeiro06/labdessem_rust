use std::fs::File;
use std::path::Path;

use arrow::array::{Array, Date32Array, Float64Array, Int32Array};
use arrow::record_batch::RecordBatch;
use parquet::arrow::arrow_reader::{ParquetRecordBatchReader, ParquetRecordBatchReaderBuilder};

use crate::IoError;

pub(crate) fn open_record_batch_reader(path: &Path) -> Result<ParquetRecordBatchReader, IoError> {
    let file = File::open(path).map_err(|e| {
        IoError::invalid_data(format!("failed to open {}: {e}", path.display()))
    })?;

    let builder = ParquetRecordBatchReaderBuilder::try_new(file).map_err(|e| {
        IoError::invalid_data(format!("failed to read parquet {}: {e}", path.display()))
    })?;

    builder.build().map_err(|e| {
        IoError::invalid_data(format!(
            "failed to build parquet reader for {}: {e}",
            path.display()
        ))
    })
}

pub(crate) fn extract_required_int32<'a>(
    batch: &'a RecordBatch,
    name: &str,
    path: &Path,
) -> Result<&'a Int32Array, IoError> {
    let col = batch.column_by_name(name).ok_or_else(|| {
        IoError::invalid_data(format!("{}: missing column {name}", path.display()))
    })?;
    col.as_any()
        .downcast_ref::<Int32Array>()
        .ok_or_else(|| IoError::invalid_data(format!("{}: {name} is not Int32", path.display())))
}

pub(crate) fn extract_required_float64<'a>(
    batch: &'a RecordBatch,
    name: &str,
    path: &Path,
) -> Result<&'a Float64Array, IoError> {
    let col = batch.column_by_name(name).ok_or_else(|| {
        IoError::invalid_data(format!("{}: missing column {name}", path.display()))
    })?;
    col.as_any()
        .downcast_ref::<Float64Array>()
        .ok_or_else(|| IoError::invalid_data(format!("{}: {name} is not Float64", path.display())))
}

pub(crate) fn extract_required_date32<'a>(
    batch: &'a RecordBatch,
    name: &str,
    path: &Path,
) -> Result<&'a Date32Array, IoError> {
    let col = batch.column_by_name(name).ok_or_else(|| {
        IoError::invalid_data(format!("{}: missing column {name}", path.display()))
    })?;
    col.as_any()
        .downcast_ref::<Date32Array>()
        .ok_or_else(|| IoError::invalid_data(format!("{}: {name} is not Date32", path.display())))
}
