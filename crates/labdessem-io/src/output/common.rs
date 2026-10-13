//! Common utilities for CSV output generation.

use std::{fs, io, path::Path};

/// Generate a CSV header with ONS branding and column descriptions.
pub fn csv_with_header(columns: &str, column_descriptions: &[(&str, &str)]) -> String {
    let mut header = String::from(
        "ONS - Operador Nacional do Sistema Eletrico\nLabDessem\n\nDescricao das colunas:\n",
    );
    for (column, description) in column_descriptions {
        header.push_str(&format!("{column}: {description}\n"));
    }
    header.push('\n');
    header.push_str(columns);
    header.push('\n');
    header
}

/// Write CSV contents to a file.
pub fn write_csv_file(path: impl AsRef<Path>, contents: String) -> io::Result<()> {
    fs::write(path, contents)
}

/// Convert volume in hm3 to flow in m3/s given the period duration in hours.
pub fn volume_hm3_to_flow_m3s(volume_hm3: f64, duration_hours: f64) -> f64 {
    if duration_hours.abs() < 1e-12 {
        return 0.0;
    }
    volume_hm3 * 1_000_000.0 / (duration_hours * 3600.0)
}
