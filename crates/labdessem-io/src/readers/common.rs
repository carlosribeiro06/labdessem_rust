//! Common utilities for CSV readers.

use std::io::{self, Write};
use std::path::{Path, PathBuf};

use csv::ReaderBuilder;
use serde::de::DeserializeOwned;

use crate::error::IoError;

/// Read a CSV file with semicolon delimiter into a vector of deserialized rows.
pub fn read_csv<T: DeserializeOwned>(path: PathBuf) -> Result<Vec<T>, IoError> {
    log_read_file(&path);
    let mut reader = ReaderBuilder::new()
        .delimiter(b';')
        .has_headers(true)
        .from_path(&path)?;

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
    path.file_name()
        .map(|name| name.to_string_lossy().into_owned())
        .unwrap_or_else(|| path.display().to_string())
}
