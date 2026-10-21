//! CAD_TAILRACE.csv reader.

use std::collections::HashMap;
use std::path::Path;

use csv::ReaderBuilder;
use serde::Deserialize;

use crate::error::FphaFittingError;
use crate::types::TailraceModel;

/// Raw CSV row from CAD_TAILRACE.csv.
#[derive(Debug, Clone, Deserialize)]
pub struct TailraceRow {
    #[serde(alias = "Codigo", alias = "CODIGO", alias = "codigo")]
    pub codigo: i32,
    #[serde(alias = "Nome", alias = "NOME", alias = "nome")]
    pub nome: String,
    #[serde(alias = "Coef0", alias = "COEF0", alias = "coef0")]
    pub coef0: f64,
    #[serde(alias = "Coef1", alias = "COEF1", alias = "coef1", default)]
    pub coef1: f64,
    #[serde(alias = "Coef2", alias = "COEF2", alias = "coef2", default)]
    pub coef2: f64,
    #[serde(alias = "Coef3", alias = "COEF3", alias = "coef3", default)]
    pub coef3: f64,
    #[serde(alias = "Coef4", alias = "COEF4", alias = "coef4", default)]
    pub coef4: f64,
}

impl TailraceRow {
    /// Build polynomial coefficients, trimming trailing zeros.
    pub fn to_coefficients(&self) -> Vec<f64> {
        let mut coeffs = vec![self.coef0, self.coef1, self.coef2, self.coef3, self.coef4];
        while coeffs.len() > 1 && coeffs.last() == Some(&0.0) {
            coeffs.pop();
        }
        coeffs
    }
}

/// Read CAD_TAILRACE.csv and build TailraceModel per plant.
pub fn read_tailrace_table(
    path: impl AsRef<Path>,
) -> Result<HashMap<i32, TailraceModel>, FphaFittingError> {
    let path = path.as_ref();
    let mut reader = ReaderBuilder::new()
        .delimiter(b';')
        .has_headers(true)
        .flexible(true)
        .from_path(path)
        .map_err(|e| FphaFittingError::IoError {
            path: path.display().to_string(),
            detail: e.to_string(),
        })?;

    let mut models = HashMap::new();

    for result in reader.deserialize() {
        let row: TailraceRow = result.map_err(|e| FphaFittingError::IoError {
            path: path.display().to_string(),
            detail: e.to_string(),
        })?;

        let coefficients = row.to_coefficients();
        let model = TailraceModel::Polynomial { coefficients };
        models.insert(row.codigo, model);
    }

    Ok(models)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Write;
    use tempfile::NamedTempFile;

    fn write_test_csv(content: &str) -> NamedTempFile {
        let mut file = NamedTempFile::new().unwrap();
        file.write_all(content.as_bytes()).unwrap();
        file
    }

    #[test]
    fn reads_valid_tailrace_file() {
        let csv = "Codigo;Nome;Coef0;Coef1;Coef2;Coef3;Coef4\n\
                   1;FURNAS;660.5;0.0008;-1.5e-8;0;0\n\
                   2;ITAIPU;100.0;0.0012;-2e-8;1e-12;0";
        let file = write_test_csv(csv);
        let models = read_tailrace_table(file.path()).unwrap();

        assert_eq!(models.len(), 2);
        assert!(models.contains_key(&1));
        assert!(models.contains_key(&2));
    }

    #[test]
    fn trims_trailing_zero_coefficients() {
        let csv = "Codigo;Nome;Coef0;Coef1;Coef2;Coef3;Coef4\n\
                   1;TEST;100.0;0.001;0;0;0";
        let file = write_test_csv(csv);
        let models = read_tailrace_table(file.path()).unwrap();

        if let TailraceModel::Polynomial { coefficients } = &models[&1] {
            assert_eq!(coefficients.len(), 2);
            assert!((coefficients[0] - 100.0).abs() < 1e-9);
            assert!((coefficients[1] - 0.001).abs() < 1e-9);
        } else {
            panic!("expected Polynomial");
        }
    }

    #[test]
    fn handles_constant_tailrace() {
        let csv = "Codigo;Nome;Coef0;Coef1;Coef2;Coef3;Coef4\n\
                   1;CONSTANT;50.0;0;0;0;0";
        let file = write_test_csv(csv);
        let models = read_tailrace_table(file.path()).unwrap();

        if let TailraceModel::Polynomial { coefficients } = &models[&1] {
            assert_eq!(coefficients.len(), 1);
            assert!((coefficients[0] - 50.0).abs() < 1e-9);
        } else {
            panic!("expected Polynomial");
        }
    }
}
