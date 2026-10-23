//! Writer for computed FPHA results.

use std::path::Path;

use crate::error::IoError;
use crate::output::{csv_with_header, write_csv_file};

/// A single FPHA plane record for output.
#[derive(Debug, Clone)]
pub struct ComputedFphaRecord {
    pub codigo_usina: u32,
    pub nome_usina: String,
    pub plane_index: usize,
    pub gamma_0: f64,
    pub gamma_v: f64,
    pub gamma_q: f64,
    pub gamma_s: f64,
    pub alpha: f64,
    pub max_deviation_mw: Option<f64>,
    pub mean_deviation_mw: Option<f64>,
    pub rms_deviation_mw: Option<f64>,
}

/// Write computed FPHA planes to `RESULTADOS/resultado_fpha_computed.csv`.
///
/// Creates the `RESULTADOS` directory if it does not exist.
/// Overwrites the file if it already exists.
pub fn write_fpha_computed(case_path: &Path, records: &[ComputedFphaRecord]) -> Result<(), IoError> {
    let results_dir = case_path.join("RESULTADOS");
    std::fs::create_dir_all(&results_dir).map_err(|e| {
        IoError::invalid_data(format!(
            "failed to create RESULTADOS directory at {}: {e}",
            results_dir.display()
        ))
    })?;

    let mut csv = csv_with_header(
        "codigo_usina;nome_usina;plane_index;gamma_0;gamma_v;gamma_q;gamma_s;alpha;max_deviation_mw;mean_deviation_mw;rms_deviation_mw",
        &[
            ("codigo_usina", "codigo identificador da usina hidroeletrica"),
            ("nome_usina", "nome da usina hidroeletrica"),
            ("plane_index", "indice do plano FPHA (0-indexed)"),
            (
                "gamma_0",
                "coeficiente independente do hiperplano (RHS em MW)",
            ),
            (
                "gamma_v",
                "coeficiente do volume armazenado (MW/hm3)",
            ),
            (
                "gamma_q",
                "coeficiente da vazao turbinada (MW/m3/s)",
            ),
            (
                "gamma_s",
                "coeficiente do vertimento (MW/m3/s)",
            ),
            ("alpha", "fator de correcao aplicado aos coeficientes"),
            ("max_deviation_mw", "desvio maximo absoluto da aproximacao (MW)"),
            ("mean_deviation_mw", "desvio medio absoluto da aproximacao (MW)"),
            ("rms_deviation_mw", "raiz do erro quadratico medio da aproximacao (MW)"),
        ],
    );

    for record in records {
        let max_dev = record.max_deviation_mw.map_or(String::new(), |v| format!("{:.3}", v));
        let mean_dev = record.mean_deviation_mw.map_or(String::new(), |v| format!("{:.3}", v));
        let rms_dev = record.rms_deviation_mw.map_or(String::new(), |v| format!("{:.3}", v));

        csv.push_str(&format!(
            "{};{};{};{:.6};{:.9};{:.6};{:.9};{:.6};{};{};{}\n",
            record.codigo_usina,
            record.nome_usina,
            record.plane_index,
            record.gamma_0,
            record.gamma_v,
            record.gamma_q,
            record.gamma_s,
            record.alpha,
            max_dev,
            mean_dev,
            rms_dev,
        ));
    }

    let file_path = results_dir.join("resultado_fpha_computed.csv");
    write_csv_file(&file_path, csv).map_err(|e| {
        IoError::invalid_data(format!(
            "failed to write {}: {e}",
            file_path.display()
        ))
    })?;

    Ok(())
}

#[cfg(test)]
#[allow(clippy::unwrap_used, clippy::expect_used)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    #[test]
    fn write_fpha_computed_creates_file() {
        let dir = TempDir::new().unwrap();
        let records = vec![ComputedFphaRecord {
            codigo_usina: 1,
            nome_usina: "FURNAS".to_string(),
            plane_index: 0,
            gamma_0: 1234.5,
            gamma_v: -0.0023,
            gamma_q: 0.045,
            gamma_s: 0.001,
            alpha: 0.95,
            max_deviation_mw: Some(1.5),
            mean_deviation_mw: Some(0.8),
            rms_deviation_mw: Some(0.9),
        }];

        write_fpha_computed(dir.path(), &records).unwrap();

        let file_path = dir
            .path()
            .join("RESULTADOS")
            .join("resultado_fpha_computed.csv");
        assert!(file_path.exists());

        let content = std::fs::read_to_string(&file_path).unwrap();
        assert!(content.contains("codigo_usina"));
        assert!(content.contains("FURNAS"));
        assert!(content.contains("1234.5"));
    }

    #[test]
    fn write_fpha_computed_creates_directory() {
        let dir = TempDir::new().unwrap();
        assert!(!dir.path().join("RESULTADOS").exists());

        write_fpha_computed(dir.path(), &[]).unwrap();

        assert!(dir.path().join("RESULTADOS").exists());
    }

    #[test]
    fn write_fpha_computed_multiple_plants() {
        let dir = TempDir::new().unwrap();
        let records = vec![
            ComputedFphaRecord {
                codigo_usina: 1,
                nome_usina: "PLANT_A".to_string(),
                plane_index: 0,
                gamma_0: 100.0,
                gamma_v: 0.1,
                gamma_q: 0.2,
                gamma_s: 0.3,
                alpha: 0.9,
                max_deviation_mw: Some(1.0),
                mean_deviation_mw: Some(0.5),
                rms_deviation_mw: Some(0.6),
            },
            ComputedFphaRecord {
                codigo_usina: 1,
                nome_usina: "PLANT_A".to_string(),
                plane_index: 1,
                gamma_0: 90.0,
                gamma_v: 0.08,
                gamma_q: 0.18,
                gamma_s: 0.25,
                alpha: 0.9,
                max_deviation_mw: Some(1.0),
                mean_deviation_mw: Some(0.5),
                rms_deviation_mw: Some(0.6),
            },
            ComputedFphaRecord {
                codigo_usina: 2,
                nome_usina: "PLANT_B".to_string(),
                plane_index: 0,
                gamma_0: 200.0,
                gamma_v: 0.4,
                gamma_q: 0.5,
                gamma_s: 0.6,
                alpha: 0.8,
                max_deviation_mw: Some(2.0),
                mean_deviation_mw: Some(1.2),
                rms_deviation_mw: Some(1.4),
            },
        ];

        write_fpha_computed(dir.path(), &records).unwrap();

        let file_path = dir
            .path()
            .join("RESULTADOS")
            .join("resultado_fpha_computed.csv");
        let content = std::fs::read_to_string(&file_path).unwrap();
        assert!(content.contains("PLANT_A"));
        assert!(content.contains("PLANT_B"));

        let lines: Vec<&str> = content.lines().collect();
        let data_lines: Vec<&str> = lines
            .iter()
            .filter(|l| l.starts_with('1') || l.starts_with('2'))
            .copied()
            .collect();
        assert_eq!(data_lines.len(), 3);
    }

    #[test]
    fn write_fpha_computed_overwrites() {
        let dir = TempDir::new().unwrap();

        let records1 = vec![ComputedFphaRecord {
            codigo_usina: 1,
            nome_usina: "FIRST".to_string(),
            plane_index: 0,
            gamma_0: 1.0,
            gamma_v: 0.0,
            gamma_q: 0.0,
            gamma_s: 0.0,
            alpha: 1.0,
            max_deviation_mw: None,
            mean_deviation_mw: None,
            rms_deviation_mw: None,
        }];
        write_fpha_computed(dir.path(), &records1).unwrap();

        let records2 = vec![ComputedFphaRecord {
            codigo_usina: 2,
            nome_usina: "SECOND".to_string(),
            plane_index: 0,
            gamma_0: 2.0,
            gamma_v: 0.0,
            gamma_q: 0.0,
            gamma_s: 0.0,
            alpha: 1.0,
            max_deviation_mw: None,
            mean_deviation_mw: None,
            rms_deviation_mw: None,
        }];
        write_fpha_computed(dir.path(), &records2).unwrap();

        let file_path = dir
            .path()
            .join("RESULTADOS")
            .join("resultado_fpha_computed.csv");
        let content = std::fs::read_to_string(&file_path).unwrap();
        assert!(!content.contains("FIRST"));
        assert!(content.contains("SECOND"));
    }

    #[test]
    fn write_fpha_computed_empty_records() {
        let dir = TempDir::new().unwrap();

        write_fpha_computed(dir.path(), &[]).unwrap();

        let file_path = dir
            .path()
            .join("RESULTADOS")
            .join("resultado_fpha_computed.csv");
        let content = std::fs::read_to_string(&file_path).unwrap();
        assert!(content.contains("codigo_usina"));
        assert!(!content.contains("FURNAS"));
    }

    #[test]
    fn write_fpha_computed_with_populated_deviations() {
        let dir = TempDir::new().unwrap();
        let records = vec![ComputedFphaRecord {
            codigo_usina: 1,
            nome_usina: "TEST".to_string(),
            plane_index: 0,
            gamma_0: 100.0,
            gamma_v: 0.1,
            gamma_q: 0.2,
            gamma_s: 0.3,
            alpha: 0.9,
            max_deviation_mw: Some(1.234),
            mean_deviation_mw: Some(0.567),
            rms_deviation_mw: Some(0.789),
        }];

        write_fpha_computed(dir.path(), &records).unwrap();

        let file_path = dir
            .path()
            .join("RESULTADOS")
            .join("resultado_fpha_computed.csv");
        let content = std::fs::read_to_string(&file_path).unwrap();
        assert!(content.contains("max_deviation_mw"));
        assert!(content.contains("mean_deviation_mw"));
        assert!(content.contains("rms_deviation_mw"));
        assert!(content.contains("1.234"));
        assert!(content.contains("0.567"));
        assert!(content.contains("0.789"));
    }

    #[test]
    fn write_fpha_computed_with_none_deviations() {
        let dir = TempDir::new().unwrap();
        let records = vec![ComputedFphaRecord {
            codigo_usina: 1,
            nome_usina: "TEST".to_string(),
            plane_index: 0,
            gamma_0: 100.0,
            gamma_v: 0.1,
            gamma_q: 0.2,
            gamma_s: 0.3,
            alpha: 0.9,
            max_deviation_mw: None,
            mean_deviation_mw: None,
            rms_deviation_mw: None,
        }];

        write_fpha_computed(dir.path(), &records).unwrap();

        let file_path = dir
            .path()
            .join("RESULTADOS")
            .join("resultado_fpha_computed.csv");
        let content = std::fs::read_to_string(&file_path).unwrap();

        // Find the column header line (contains codigo_usina;nome_usina)
        let header_line = content
            .lines()
            .find(|l| l.starts_with("codigo_usina;"))
            .expect("Column header line not found");
        assert_eq!(header_line.split(';').count(), 11);

        // Data line should have 11 fields, with empty strings for None deviations
        let data_line = content
            .lines()
            .find(|l| l.starts_with("1;"))
            .expect("Data line not found");
        let fields: Vec<&str> = data_line.split(';').collect();
        assert_eq!(fields.len(), 11);
        // Last 3 fields should be empty
        assert_eq!(fields[8], "");
        assert_eq!(fields[9], "");
        assert_eq!(fields[10], "");
    }

    #[test]
    fn write_fpha_computed_header_matches_expected_columns() {
        let dir = TempDir::new().unwrap();

        write_fpha_computed(dir.path(), &[]).unwrap();

        let file_path = dir
            .path()
            .join("RESULTADOS")
            .join("resultado_fpha_computed.csv");
        let content = std::fs::read_to_string(&file_path).unwrap();

        // Find the column header line (contains codigo_usina;nome_usina)
        let header = content
            .lines()
            .find(|l| l.starts_with("codigo_usina;"))
            .expect("Column header line not found");

        let expected_columns = [
            "codigo_usina",
            "nome_usina",
            "plane_index",
            "gamma_0",
            "gamma_v",
            "gamma_q",
            "gamma_s",
            "alpha",
            "max_deviation_mw",
            "mean_deviation_mw",
            "rms_deviation_mw",
        ];
        let actual_columns: Vec<&str> = header.split(';').collect();
        assert_eq!(actual_columns, expected_columns);
    }
}
