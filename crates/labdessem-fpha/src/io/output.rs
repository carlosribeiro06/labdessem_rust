//! resultado_fpha_computed.csv writer.

use std::io::Write;
use std::path::Path;

use crate::error::FphaFittingError;
use crate::fitting::{FphaFitDeviation, RawPlane};

/// A fitted plant's FPHA result for output.
#[derive(Debug, Clone)]
pub struct FphaPlantResult {
    pub codigo: i32,
    pub nome: String,
    pub planes: Vec<RawPlane>,
    pub alpha: f64,
    pub deviation: FphaFitDeviation,
}

/// Write computed FPHA results to CSV.
pub fn write_fpha_results(
    results: &[FphaPlantResult],
    path: impl AsRef<Path>,
) -> Result<(), FphaFittingError> {
    let path = path.as_ref();
    let file = std::fs::File::create(path).map_err(|e| FphaFittingError::IoError {
        path: path.display().to_string(),
        detail: e.to_string(),
    })?;
    let mut writer = std::io::BufWriter::new(file);

    writeln!(
        writer,
        "Codigo;Nome;Segmento;Gamma0;GammaV;GammaQ;GammaS;Alpha;MaxDeviation_MW;MeanDeviation_MW;RMSDeviation_MW"
    )
    .map_err(|e| FphaFittingError::IoError {
        path: path.display().to_string(),
        detail: e.to_string(),
    })?;

    for result in results {
        for (idx, plane) in result.planes.iter().enumerate() {
            writeln!(
                writer,
                "{};{};{};{:.6};{:.9};{:.9};{:.9};{:.6};{:.3};{:.3};{:.3}",
                result.codigo,
                result.nome,
                idx + 1,
                plane.gamma_0,
                plane.gamma_v,
                plane.gamma_q,
                plane.gamma_s,
                result.alpha,
                result.deviation.max_abs_mw,
                result.deviation.mean_abs_mw,
                result.deviation.rms_mw,
            )
            .map_err(|e| FphaFittingError::IoError {
                path: path.display().to_string(),
                detail: e.to_string(),
            })?;
        }
    }

    writer.flush().map_err(|e| FphaFittingError::IoError {
        path: path.display().to_string(),
        detail: e.to_string(),
    })?;

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    fn test_result() -> FphaPlantResult {
        FphaPlantResult {
            codigo: 1,
            nome: "FURNAS".to_string(),
            planes: vec![
                RawPlane {
                    gamma_0: 123.456,
                    gamma_v: 0.00123,
                    gamma_q: 0.45678,
                    gamma_s: -0.00012,
                },
                RawPlane {
                    gamma_0: 234.567,
                    gamma_v: 0.00234,
                    gamma_q: 0.56789,
                    gamma_s: -0.00023,
                },
            ],
            alpha: 0.9876,
            deviation: FphaFitDeviation {
                max_abs_mw: 1.234,
                mean_abs_mw: 0.567,
                rms_mw: 0.789,
                mean_signed_mw: 0.123,
                relative: 0.01,
            },
        }
    }

    #[test]
    fn writes_valid_csv() {
        let dir = TempDir::new().unwrap();
        let path = dir.path().join("resultado_fpha_computed.csv");

        let results = vec![test_result()];
        write_fpha_results(&results, &path).unwrap();

        let content = std::fs::read_to_string(&path).unwrap();
        assert!(content.contains("Codigo;Nome;Segmento"));
        assert!(content.contains("FURNAS"));
        assert!(content.contains("123.456"));
    }

    #[test]
    fn writes_one_row_per_plane() {
        let dir = TempDir::new().unwrap();
        let path = dir.path().join("resultado_fpha_computed.csv");

        let results = vec![test_result()];
        write_fpha_results(&results, &path).unwrap();

        let content = std::fs::read_to_string(&path).unwrap();
        let lines: Vec<&str> = content.lines().collect();
        assert_eq!(lines.len(), 3); // header + 2 planes
    }

    #[test]
    fn handles_multiple_plants() {
        let dir = TempDir::new().unwrap();
        let path = dir.path().join("resultado_fpha_computed.csv");

        let mut result2 = test_result();
        result2.codigo = 2;
        result2.nome = "ITAIPU".to_string();

        let results = vec![test_result(), result2];
        write_fpha_results(&results, &path).unwrap();

        let content = std::fs::read_to_string(&path).unwrap();
        assert!(content.contains("FURNAS"));
        assert!(content.contains("ITAIPU"));
    }

    #[test]
    fn handles_empty_results() {
        let dir = TempDir::new().unwrap();
        let path = dir.path().join("resultado_fpha_computed.csv");

        write_fpha_results(&[], &path).unwrap();

        let content = std::fs::read_to_string(&path).unwrap();
        let lines: Vec<&str> = content.lines().collect();
        assert_eq!(lines.len(), 1); // header only
    }
}
