//! Readers for FPHA geometry data (VHA table and tailrace polynomial).

use std::collections::HashMap;
use std::path::Path;

use csv::ReaderBuilder;
use serde::Deserialize;

use super::common::log_read_file;
use crate::error::IoError;

/// A single point in the Volume-Height-Area table.
#[derive(Debug, Clone)]
pub struct VhaPoint {
    pub volume_hm3: f64,
    #[allow(dead_code)]
    pub height_m: f64,
    #[allow(dead_code)]
    pub area_km2: f64,
}

/// VHA geometry data for a single hydro plant.
#[derive(Debug, Clone)]
pub struct VhaGeometryData {
    #[allow(dead_code)]
    pub plant_code: u32,
    pub points: Vec<VhaPoint>,
}

/// Tailrace polynomial coefficients for a single hydro plant.
#[derive(Debug, Clone)]
pub struct TailraceData {
    #[allow(dead_code)]
    pub plant_code: u32,
    pub coefficients: [f64; 5],
}

#[derive(Debug, Deserialize)]
struct VhaRow {
    #[serde(alias = "codigo_usina", alias = "Codigo", alias = "CODIGO", alias = "codigo")]
    codigo: u32,
    #[serde(alias = "Volume_hm3", alias = "VOLUME_HM3", alias = "volume_hm3")]
    volume_hm3: f64,
    #[serde(alias = "Cota_m", alias = "COTA_M", alias = "cota_m")]
    cota_m: f64,
    #[serde(alias = "Area_km2", alias = "AREA_KM2", alias = "area_km2")]
    area_km2: f64,
}

#[derive(Debug, Deserialize)]
struct TailraceRow {
    #[serde(alias = "codigo_usina", alias = "Codigo", alias = "CODIGO", alias = "codigo")]
    codigo: u32,
    #[serde(alias = "A0", alias = "Coef0", alias = "COEF0", alias = "coef0")]
    a0: f64,
    #[serde(alias = "A1", alias = "Coef1", alias = "COEF1", alias = "coef1", default)]
    a1: f64,
    #[serde(alias = "A2", alias = "Coef2", alias = "COEF2", alias = "coef2", default)]
    a2: f64,
    #[serde(alias = "A3", alias = "Coef3", alias = "COEF3", alias = "coef3", default)]
    a3: f64,
    #[serde(alias = "A4", alias = "Coef4", alias = "COEF4", alias = "coef4", default)]
    a4: f64,
}

/// Read VHA geometry data from `CAD/CAD_GEOMETRIA_UHE.csv`.
///
/// Returns a HashMap mapping plant codes to their VHA geometry data.
/// Points are sorted by volume ascending within each plant.
///
/// Returns an empty HashMap if the file does not exist (file is optional).
pub fn read_vha_geometry(case_path: &Path) -> Result<HashMap<u32, VhaGeometryData>, IoError> {
    let file_path = case_path.join("CAD").join("CAD_GEOMETRIA_UHE.csv");

    if !file_path.exists() {
        return Ok(HashMap::new());
    }

    log_read_file(&file_path);

    let mut reader = ReaderBuilder::new()
        .delimiter(b';')
        .has_headers(true)
        .from_path(&file_path)?;

    let mut grouped: HashMap<u32, Vec<VhaPoint>> = HashMap::new();

    for (line_idx, record) in reader.deserialize().enumerate() {
        let row: VhaRow = record.map_err(|e| {
            IoError::invalid_data(format!(
                "failed to parse CAD_GEOMETRIA_UHE.csv at line {}: {e}",
                line_idx + 2
            ))
        })?;

        if row.volume_hm3 < 0.0 {
            return Err(IoError::invalid_data(format!(
                "plant {} has negative volume {} at line {} in CAD_GEOMETRIA_UHE.csv",
                row.codigo,
                row.volume_hm3,
                line_idx + 2
            )));
        }

        if row.cota_m < 0.0 {
            return Err(IoError::invalid_data(format!(
                "plant {} has negative height {} at line {} in CAD_GEOMETRIA_UHE.csv",
                row.codigo,
                row.cota_m,
                line_idx + 2
            )));
        }

        if row.area_km2 < 0.0 {
            return Err(IoError::invalid_data(format!(
                "plant {} has negative area {} at line {} in CAD_GEOMETRIA_UHE.csv",
                row.codigo,
                row.area_km2,
                line_idx + 2
            )));
        }

        grouped.entry(row.codigo).or_default().push(VhaPoint {
            volume_hm3: row.volume_hm3,
            height_m: row.cota_m,
            area_km2: row.area_km2,
        });
    }

    let result = grouped
        .into_iter()
        .map(|(code, mut points)| {
            points.sort_by(|a, b| {
                a.volume_hm3
                    .partial_cmp(&b.volume_hm3)
                    .unwrap_or(std::cmp::Ordering::Equal)
            });
            (
                code,
                VhaGeometryData {
                    plant_code: code,
                    points,
                },
            )
        })
        .collect();

    Ok(result)
}

/// Read tailrace polynomial coefficients from `CAD/CAD_TAILRACE.csv`.
///
/// Returns a HashMap mapping plant codes to their tailrace data.
///
/// Returns an empty HashMap if the file does not exist (file is optional).
pub fn read_tailrace(case_path: &Path) -> Result<HashMap<u32, TailraceData>, IoError> {
    let file_path = case_path.join("CAD").join("CAD_TAILRACE.csv");

    if !file_path.exists() {
        return Ok(HashMap::new());
    }

    log_read_file(&file_path);

    let mut reader = ReaderBuilder::new()
        .delimiter(b';')
        .has_headers(true)
        .flexible(true)
        .from_path(&file_path)?;

    let mut result = HashMap::new();

    for (line_idx, record) in reader.deserialize().enumerate() {
        let row: TailraceRow = record.map_err(|e| {
            IoError::invalid_data(format!(
                "failed to parse CAD_TAILRACE.csv at line {}: {e}",
                line_idx + 2
            ))
        })?;

        result.insert(
            row.codigo,
            TailraceData {
                plant_code: row.codigo,
                coefficients: [row.a0, row.a1, row.a2, row.a3, row.a4],
            },
        );
    }

    Ok(result)
}

/// FPHA fitting configuration overrides for a single plant.
#[derive(Debug, Clone, Default)]
pub struct FphaPlantConfig {
    #[allow(dead_code)]
    pub num_planes: Option<usize>,
    #[allow(dead_code)]
    pub tolerance: Option<f64>,
    #[allow(dead_code)]
    pub min_volume_pct: Option<f64>,
    #[allow(dead_code)]
    pub max_volume_pct: Option<f64>,
}

#[derive(Debug, Deserialize)]
struct FphaConfigRow {
    #[serde(alias = "codigo_usina", alias = "Codigo", alias = "CODIGO", alias = "codigo")]
    codigo: u32,
    #[serde(default, alias = "MaxPlanes", alias = "max_planes")]
    num_planes: Option<usize>,
    #[serde(default, alias = "ReductionTolerance", alias = "Tolerance")]
    tolerance: Option<f64>,
    #[serde(default, alias = "MinVolumePct")]
    min_volume_pct: Option<f64>,
    #[serde(default, alias = "MaxVolumePct")]
    max_volume_pct: Option<f64>,
}

/// Read FPHA fitting configuration overrides from `CAD/CAD_FPHA_CONFIG.csv`.
///
/// Returns a HashMap mapping plant codes to their configuration overrides.
///
/// Returns an empty HashMap if the file does not exist (file is optional).
pub fn read_fpha_config(case_path: &Path) -> Result<HashMap<u32, FphaPlantConfig>, IoError> {
    let file_path = case_path.join("CAD").join("CAD_FPHA_CONFIG.csv");

    if !file_path.exists() {
        return Ok(HashMap::new());
    }

    log_read_file(&file_path);

    let mut reader = ReaderBuilder::new()
        .delimiter(b';')
        .has_headers(true)
        .flexible(true)
        .from_path(&file_path)?;

    let mut result = HashMap::new();

    for (line_idx, record) in reader.deserialize().enumerate() {
        let row: FphaConfigRow = record.map_err(|e| {
            IoError::invalid_data(format!(
                "failed to parse CAD_FPHA_CONFIG.csv at line {}: {e}",
                line_idx + 2
            ))
        })?;

        if let Some(tol) = row.tolerance {
            if tol < 0.0 {
                return Err(IoError::invalid_data(format!(
                    "plant {} has negative tolerance {} at line {} in CAD_FPHA_CONFIG.csv",
                    row.codigo,
                    tol,
                    line_idx + 2
                )));
            }
        }

        if let Some(pct) = row.min_volume_pct {
            if !(0.0..=1.0).contains(&pct) {
                return Err(IoError::invalid_data(format!(
                    "plant {} has invalid min_volume_pct {} (must be 0-1) at line {} in CAD_FPHA_CONFIG.csv",
                    row.codigo,
                    pct,
                    line_idx + 2
                )));
            }
        }

        if let Some(pct) = row.max_volume_pct {
            if !(0.0..=1.0).contains(&pct) {
                return Err(IoError::invalid_data(format!(
                    "plant {} has invalid max_volume_pct {} (must be 0-1) at line {} in CAD_FPHA_CONFIG.csv",
                    row.codigo,
                    pct,
                    line_idx + 2
                )));
            }
        }

        result.insert(
            row.codigo,
            FphaPlantConfig {
                num_planes: row.num_planes,
                tolerance: row.tolerance,
                min_volume_pct: row.min_volume_pct,
                max_volume_pct: row.max_volume_pct,
            },
        );
    }

    Ok(result)
}

#[cfg(test)]
#[allow(clippy::unwrap_used, clippy::expect_used)]
mod tests {
    use super::*;
    use std::io::Write;
    use tempfile::TempDir;

    fn create_test_case(vha_content: Option<&str>, tailrace_content: Option<&str>) -> TempDir {
        let dir = TempDir::new().unwrap();
        let cad_dir = dir.path().join("CAD");
        std::fs::create_dir(&cad_dir).unwrap();

        if let Some(content) = vha_content {
            let mut file =
                std::fs::File::create(cad_dir.join("CAD_GEOMETRIA_UHE.csv")).unwrap();
            file.write_all(content.as_bytes()).unwrap();
        }
        if let Some(content) = tailrace_content {
            let mut file = std::fs::File::create(cad_dir.join("CAD_TAILRACE.csv")).unwrap();
            file.write_all(content.as_bytes()).unwrap();
        }
        dir
    }

    #[test]
    fn read_vha_geometry_valid_single_plant() {
        let content =
            "codigo_usina;volume_hm3;cota_m;area_km2\n1;1000.0;250.0;50.0\n1;2000.0;260.0;75.0";
        let dir = create_test_case(Some(content), None);

        let result = read_vha_geometry(dir.path()).unwrap();
        assert_eq!(result.len(), 1);
        assert_eq!(result[&1].points.len(), 2);
        assert_eq!(result[&1].points[0].volume_hm3, 1000.0);
        assert_eq!(result[&1].points[1].volume_hm3, 2000.0);
    }

    #[test]
    fn read_vha_geometry_sorts_by_volume() {
        let content = "codigo_usina;volume_hm3;cota_m;area_km2\n1;3000.0;270.0;100.0\n1;1000.0;250.0;50.0\n1;2000.0;260.0;75.0";
        let dir = create_test_case(Some(content), None);

        let result = read_vha_geometry(dir.path()).unwrap();
        assert_eq!(result[&1].points[0].volume_hm3, 1000.0);
        assert_eq!(result[&1].points[1].volume_hm3, 2000.0);
        assert_eq!(result[&1].points[2].volume_hm3, 3000.0);
    }

    #[test]
    fn read_vha_geometry_multiple_plants() {
        let content = "codigo_usina;volume_hm3;cota_m;area_km2\n1;1000.0;250.0;50.0\n2;500.0;180.0;25.0\n1;2000.0;260.0;75.0";
        let dir = create_test_case(Some(content), None);

        let result = read_vha_geometry(dir.path()).unwrap();
        assert_eq!(result.len(), 2);
        assert_eq!(result[&1].points.len(), 2);
        assert_eq!(result[&2].points.len(), 1);
    }

    #[test]
    fn read_vha_geometry_missing_file_returns_empty() {
        let dir = create_test_case(None, None);
        let result = read_vha_geometry(dir.path()).unwrap();
        assert!(result.is_empty());
    }

    #[test]
    fn read_vha_geometry_negative_volume_error() {
        let content = "codigo_usina;volume_hm3;cota_m;area_km2\n1;-100.0;250.0;50.0";
        let dir = create_test_case(Some(content), None);

        let result = read_vha_geometry(dir.path());
        assert!(result.is_err());
        let err_msg = result.unwrap_err().to_string();
        assert!(err_msg.contains("negative volume"));
        assert!(err_msg.contains("plant 1"));
    }

    #[test]
    fn read_vha_geometry_negative_height_error() {
        let content = "codigo_usina;volume_hm3;cota_m;area_km2\n1;1000.0;-50.0;50.0";
        let dir = create_test_case(Some(content), None);

        let result = read_vha_geometry(dir.path());
        assert!(result.is_err());
        let err_msg = result.unwrap_err().to_string();
        assert!(err_msg.contains("negative height"));
    }

    #[test]
    fn read_vha_geometry_negative_area_error() {
        let content = "codigo_usina;volume_hm3;cota_m;area_km2\n1;1000.0;250.0;-50.0";
        let dir = create_test_case(Some(content), None);

        let result = read_vha_geometry(dir.path());
        assert!(result.is_err());
        let err_msg = result.unwrap_err().to_string();
        assert!(err_msg.contains("negative area"));
    }

    #[test]
    fn read_tailrace_valid() {
        let content = "codigo_usina;a0;a1;a2;a3;a4\n1;120.5;0.001;-0.00001;0.0;0.0";
        let dir = create_test_case(None, Some(content));

        let result = read_tailrace(dir.path()).unwrap();
        assert_eq!(result.len(), 1);
        assert_eq!(result[&1].coefficients[0], 120.5);
        assert_eq!(result[&1].coefficients[1], 0.001);
        assert_eq!(result[&1].coefficients[2], -0.00001);
        assert_eq!(result[&1].coefficients[3], 0.0);
        assert_eq!(result[&1].coefficients[4], 0.0);
    }

    #[test]
    fn read_tailrace_multiple_plants() {
        let content = "codigo_usina;a0;a1;a2;a3;a4\n1;120.5;0.001;-0.00001;0.0;0.0\n2;95.2;0.002;0.0;0.0;0.0";
        let dir = create_test_case(None, Some(content));

        let result = read_tailrace(dir.path()).unwrap();
        assert_eq!(result.len(), 2);
        assert_eq!(result[&1].coefficients[0], 120.5);
        assert_eq!(result[&2].coefficients[0], 95.2);
    }

    #[test]
    fn read_tailrace_missing_file_returns_empty() {
        let dir = create_test_case(None, None);
        let result = read_tailrace(dir.path()).unwrap();
        assert!(result.is_empty());
    }

    #[test]
    fn read_tailrace_malformed_csv_error() {
        let content = "codigo_usina;a0;a1;a2;a3;a4\n1;120.5;invalid;-0.00001;0.0;0.0";
        let dir = create_test_case(None, Some(content));

        let result = read_tailrace(dir.path());
        assert!(result.is_err());
        let err_msg = result.unwrap_err().to_string();
        assert!(err_msg.contains("CAD_TAILRACE.csv"));
    }

    // Tailrace coefficient alias tests

    #[test]
    fn read_tailrace_with_coef_aliases() {
        // Uses Coef0..Coef4 naming convention (labdessem-fpha style)
        let content = "Codigo;Coef0;Coef1;Coef2;Coef3;Coef4\n1;120.5;0.001;-0.00001;0.0;0.0";
        let dir = create_test_case(None, Some(content));

        let result = read_tailrace(dir.path()).unwrap();
        assert_eq!(result.len(), 1);
        assert_eq!(result[&1].coefficients[0], 120.5);
        assert_eq!(result[&1].coefficients[1], 0.001);
        assert_eq!(result[&1].coefficients[2], -0.00001);
    }

    #[test]
    fn read_tailrace_with_uppercase_coef_aliases() {
        // Uses COEF0..COEF4 naming convention
        let content = "CODIGO;COEF0;COEF1;COEF2;COEF3;COEF4\n2;95.2;0.002;0.0;0.0;0.0";
        let dir = create_test_case(None, Some(content));

        let result = read_tailrace(dir.path()).unwrap();
        assert_eq!(result.len(), 1);
        assert_eq!(result[&2].coefficients[0], 95.2);
        assert_eq!(result[&2].coefficients[1], 0.002);
    }

    #[test]
    fn read_tailrace_with_partial_coefficients() {
        // Only Coef0, Coef1, Coef2 present - higher order coefficients default to 0.0
        let content = "Codigo;Coef0;Coef1;Coef2\n3;100.0;0.005;-1e-8";
        let dir = create_test_case(None, Some(content));

        let result = read_tailrace(dir.path()).unwrap();
        assert_eq!(result.len(), 1);
        assert_eq!(result[&3].coefficients[0], 100.0);
        assert_eq!(result[&3].coefficients[1], 0.005);
        assert_eq!(result[&3].coefficients[2], -1e-8);
        assert_eq!(result[&3].coefficients[3], 0.0);
        assert_eq!(result[&3].coefficients[4], 0.0);
    }

    #[test]
    fn read_tailrace_with_constant_coefficient_only() {
        // Only Coef0 present - constant tailrace
        let content = "Codigo;Coef0\n4;50.0";
        let dir = create_test_case(None, Some(content));

        let result = read_tailrace(dir.path()).unwrap();
        assert_eq!(result.len(), 1);
        assert_eq!(result[&4].coefficients[0], 50.0);
        assert_eq!(result[&4].coefficients[1], 0.0);
        assert_eq!(result[&4].coefficients[2], 0.0);
        assert_eq!(result[&4].coefficients[3], 0.0);
        assert_eq!(result[&4].coefficients[4], 0.0);
    }

    fn create_config_test_case(config_content: Option<&str>) -> TempDir {
        let dir = TempDir::new().unwrap();
        let cad_dir = dir.path().join("CAD");
        std::fs::create_dir(&cad_dir).unwrap();

        if let Some(content) = config_content {
            let mut file = std::fs::File::create(cad_dir.join("CAD_FPHA_CONFIG.csv")).unwrap();
            file.write_all(content.as_bytes()).unwrap();
        }
        dir
    }

    #[test]
    fn read_fpha_config_full_row() {
        let content =
            "codigo_usina;num_planes;tolerance;min_volume_pct;max_volume_pct\n1;5;0.01;0.05;1.0";
        let dir = create_config_test_case(Some(content));

        let result = read_fpha_config(dir.path()).unwrap();
        let config = &result[&1];
        assert_eq!(config.num_planes, Some(5));
        assert_eq!(config.tolerance, Some(0.01));
        assert_eq!(config.min_volume_pct, Some(0.05));
        assert_eq!(config.max_volume_pct, Some(1.0));
    }

    #[test]
    fn read_fpha_config_partial_row() {
        let content = "codigo_usina;num_planes\n1;7";
        let dir = create_config_test_case(Some(content));

        let result = read_fpha_config(dir.path()).unwrap();
        let config = &result[&1];
        assert_eq!(config.num_planes, Some(7));
        assert_eq!(config.tolerance, None);
        assert_eq!(config.min_volume_pct, None);
        assert_eq!(config.max_volume_pct, None);
    }

    #[test]
    fn read_fpha_config_multiple_plants() {
        let content =
            "codigo_usina;num_planes;tolerance\n1;5;0.01\n3;7;0.005";
        let dir = create_config_test_case(Some(content));

        let result = read_fpha_config(dir.path()).unwrap();
        assert_eq!(result.len(), 2);
        assert_eq!(result[&1].num_planes, Some(5));
        assert_eq!(result[&3].num_planes, Some(7));
    }

    #[test]
    fn read_fpha_config_missing_file_returns_empty() {
        let dir = create_config_test_case(None);
        let result = read_fpha_config(dir.path()).unwrap();
        assert!(result.is_empty());
    }

    #[test]
    fn read_fpha_config_negative_tolerance_error() {
        let content = "codigo_usina;num_planes;tolerance\n1;5;-0.01";
        let dir = create_config_test_case(Some(content));

        let result = read_fpha_config(dir.path());
        assert!(result.is_err());
        let err_msg = result.unwrap_err().to_string();
        assert!(err_msg.contains("negative tolerance"));
    }

    #[test]
    fn read_fpha_config_invalid_min_volume_pct_error() {
        let content = "codigo_usina;num_planes;tolerance;min_volume_pct\n1;5;0.01;1.5";
        let dir = create_config_test_case(Some(content));

        let result = read_fpha_config(dir.path());
        assert!(result.is_err());
        let err_msg = result.unwrap_err().to_string();
        assert!(err_msg.contains("min_volume_pct"));
        assert!(err_msg.contains("must be 0-1"));
    }

    #[test]
    fn read_fpha_config_invalid_max_volume_pct_error() {
        let content = "codigo_usina;num_planes;tolerance;min_volume_pct;max_volume_pct\n1;5;0.01;0.05;-0.5";
        let dir = create_config_test_case(Some(content));

        let result = read_fpha_config(dir.path());
        assert!(result.is_err());
        let err_msg = result.unwrap_err().to_string();
        assert!(err_msg.contains("max_volume_pct"));
    }

    // Column alias tests for VHA geometry

    #[test]
    fn read_vha_geometry_with_codigo_alias() {
        // Uses 'Codigo' instead of 'codigo_usina'
        let content = "Codigo;Volume_hm3;Cota_m;Area_km2\n1;1000.0;250.0;50.0";
        let dir = create_test_case(Some(content), None);

        let result = read_vha_geometry(dir.path()).unwrap();
        assert_eq!(result.len(), 1);
        assert_eq!(result[&1].plant_code, 1);
        assert_eq!(result[&1].points[0].volume_hm3, 1000.0);
    }

    #[test]
    fn read_vha_geometry_with_all_uppercase_columns() {
        // Uses all uppercase column names
        let content = "CODIGO;VOLUME_HM3;COTA_M;AREA_KM2\n2;500.0;180.0;25.0";
        let dir = create_test_case(Some(content), None);

        let result = read_vha_geometry(dir.path()).unwrap();
        assert_eq!(result.len(), 1);
        assert_eq!(result[&2].plant_code, 2);
        assert_eq!(result[&2].points[0].volume_hm3, 500.0);
        assert_eq!(result[&2].points[0].height_m, 180.0);
        assert_eq!(result[&2].points[0].area_km2, 25.0);
    }

    #[test]
    fn read_vha_geometry_with_all_lowercase_columns() {
        // Uses all lowercase column names
        let content = "codigo;volume_hm3;cota_m;area_km2\n3;750.0;200.0;35.0";
        let dir = create_test_case(Some(content), None);

        let result = read_vha_geometry(dir.path()).unwrap();
        assert_eq!(result.len(), 1);
        assert_eq!(result[&3].plant_code, 3);
        assert_eq!(result[&3].points[0].volume_hm3, 750.0);
    }

    #[test]
    fn read_vha_geometry_with_mixed_case_columns() {
        // Uses MixedCase column names matching labdessem-fpha convention
        let content = "Codigo;Volume_hm3;Cota_m;Area_km2\n4;1200.0;220.0;45.0\n4;2400.0;240.0;60.0";
        let dir = create_test_case(Some(content), None);

        let result = read_vha_geometry(dir.path()).unwrap();
        assert_eq!(result.len(), 1);
        assert_eq!(result[&4].points.len(), 2);
        assert_eq!(result[&4].points[0].volume_hm3, 1200.0);
        assert_eq!(result[&4].points[1].volume_hm3, 2400.0);
    }
}
