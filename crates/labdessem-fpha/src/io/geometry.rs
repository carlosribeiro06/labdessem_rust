//! CAD_GEOMETRIA_UHE.csv reader.

use std::collections::HashMap;
use std::path::Path;

use csv::ReaderBuilder;
use serde::Deserialize;

use crate::error::FphaFittingError;
use crate::fitting::{ForebayTable, VhaRow};

/// Raw CSV row from CAD_GEOMETRIA_UHE.csv.
#[derive(Debug, Clone, Deserialize)]
pub struct GeometryRow {
    #[serde(alias = "Codigo", alias = "CODIGO", alias = "codigo")]
    pub codigo: i32,
    #[serde(alias = "Nome", alias = "NOME", alias = "nome")]
    pub nome: String,
    #[serde(alias = "Volume_hm3", alias = "VOLUME_HM3", alias = "volume_hm3")]
    pub volume_hm3: f64,
    #[serde(alias = "Cota_m", alias = "COTA_M", alias = "cota_m")]
    pub cota_m: f64,
    #[serde(alias = "Area_km2", alias = "AREA_KM2", alias = "area_km2")]
    pub area_km2: f64,
}

/// Read CAD_GEOMETRIA_UHE.csv and group into per-plant ForebayTables.
pub fn read_geometry_table(
    path: impl AsRef<Path>,
) -> Result<HashMap<i32, ForebayTable>, FphaFittingError> {
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

    let mut rows_by_plant: HashMap<i32, (String, Vec<VhaRow>)> = HashMap::new();

    for result in reader.deserialize() {
        let row: GeometryRow = result.map_err(|e| FphaFittingError::IoError {
            path: path.display().to_string(),
            detail: e.to_string(),
        })?;

        let entry = rows_by_plant
            .entry(row.codigo)
            .or_insert_with(|| (row.nome.clone(), Vec::new()));
        entry.1.push(VhaRow {
            volume_hm3: row.volume_hm3,
            height_m: row.cota_m,
            area_km2: row.area_km2,
        });
    }

    let mut tables = HashMap::new();
    for (codigo, (nome, rows)) in rows_by_plant {
        let table = ForebayTable::new(&rows, &nome)?;
        tables.insert(codigo, table);
    }

    Ok(tables)
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
    fn reads_valid_geometry_file() {
        let csv = "Codigo;Nome;Volume_hm3;Cota_m;Area_km2\n\
                   1;FURNAS;5733.00;762.00;1442.50\n\
                   1;FURNAS;17217.00;780.00;1406.00\n\
                   2;ITAIPU;12500.00;195.00;850.00\n\
                   2;ITAIPU;29000.00;220.00;1350.00";
        let file = write_test_csv(csv);
        let tables = read_geometry_table(file.path()).unwrap();

        assert_eq!(tables.len(), 2);
        assert!(tables.contains_key(&1));
        assert!(tables.contains_key(&2));
    }

    #[test]
    fn groups_rows_by_codigo() {
        let csv = "Codigo;Nome;Volume_hm3;Cota_m;Area_km2\n\
                   1;FURNAS;0.00;750.00;1000.00\n\
                   1;FURNAS;5000.00;760.00;1200.00\n\
                   1;FURNAS;10000.00;770.00;1400.00";
        let file = write_test_csv(csv);
        let tables = read_geometry_table(file.path()).unwrap();

        assert_eq!(tables.len(), 1);
        let furnas = tables.get(&1).unwrap();
        assert!((furnas.height(5000.0) - 760.0).abs() < 1e-6);
    }

    #[test]
    fn accepts_single_row_plant() {
        let csv = "Codigo;Nome;Volume_hm3;Cota_m;Area_km2\n\
                   1;SINGLE;5000.00;760.00;1200.00";
        let file = write_test_csv(csv);
        let tables = read_geometry_table(file.path()).unwrap();

        assert_eq!(tables.len(), 1);
        let single = tables.get(&1).unwrap();
        // Single-row table returns constant height
        assert_eq!(single.height(0.0), 760.0);
        assert_eq!(single.height(9999.0), 760.0);
    }
}
