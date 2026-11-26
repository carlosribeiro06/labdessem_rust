//! Readers for FPHA geometry data (VHA table and tailrace polynomial).

use std::collections::{BTreeMap, HashMap};
use std::path::Path;

use csv::ReaderBuilder;
use labdessem_core::ids::HydroPlantId;
use labdessem_model::production::FphaConfigOverrides;
use serde::Deserialize;

use super::common::log_read_file;
use crate::error::IoError;

/// A single point in the Volume-Height-Area table.
#[derive(Debug, Clone)]
pub(crate) struct VhaPoint {
    pub(crate) volume_hm3: f64,
    pub(crate) height_m: f64,
}

/// VHA geometry data for a single hydro plant.
#[derive(Debug, Clone)]
pub(crate) struct VhaGeometryData {
    pub(crate) points: Vec<VhaPoint>,
}

/// Tailrace polynomial coefficients for a single hydro plant.
#[derive(Debug, Clone)]
pub(crate) struct TailraceData {
    pub(crate) coefficients: [f64; 5],
}

#[derive(Debug, Deserialize)]
struct VhaRow {
    #[serde(rename = "codigo_usina")]
    codigo: u32,
    #[serde(rename = "volume.hm3")]
    volume_hm3: f64,
    #[serde(rename = "cota.m")]
    cota_m: f64,
    #[serde(rename = "area.km2")]
    area_km2: f64,
}

#[derive(Debug, Deserialize)]
struct TailraceRow {
    #[serde(rename = "codigo_usina")]
    codigo: u32,
    #[serde(rename = "a0.m")]
    a0: f64,
    #[serde(rename = "a1.m/m3s", default)]
    a1: f64,
    #[serde(rename = "a2.m/m3s2", default)]
    a2: f64,
    #[serde(rename = "a3.m/m3s3", default)]
    a3: f64,
    #[serde(rename = "a4.m/m3s4", default)]
    a4: f64,
}

const TAILRACE_COLUMNS: [&str; 5] = ["a0.m", "a1.m/m3s", "a2.m/m3s2", "a3.m/m3s3", "a4.m/m3s4"];

fn check_finite(
    file: &str,
    plant: u32,
    line: usize,
    column: &str,
    value: f64,
) -> Result<(), IoError> {
    if value.is_finite() {
        return Ok(());
    }
    Err(IoError::invalid_data(format!(
        "plant {plant} has non-finite {column} {value} at line {line} in {file}"
    )))
}

/// Read VHA geometry data from `CAD/CAD_GEOMETRIA_UHE.csv`.
///
/// Returns a HashMap mapping plant codes to their VHA geometry data.
/// Points are sorted by volume ascending within each plant.
///
/// Returns an empty HashMap if the file does not exist (file is optional).
/// Fails on a non-finite value or on two points of one plant with the same volume, reporting the
/// lowest plant code first.
pub(crate) fn read_vha_geometry(
    case_path: &Path,
) -> Result<HashMap<u32, VhaGeometryData>, IoError> {
    let file_path = case_path.join("CAD").join("CAD_GEOMETRIA_UHE.csv");

    if !file_path.exists() {
        return Ok(HashMap::new());
    }

    log_read_file(&file_path);

    let mut reader = ReaderBuilder::new()
        .delimiter(b';')
        .has_headers(true)
        .from_path(&file_path)?;

    let mut grouped: BTreeMap<u32, Vec<(usize, VhaPoint)>> = BTreeMap::new();

    for (line_idx, record) in reader.deserialize().enumerate() {
        let row: VhaRow = record.map_err(|e| {
            IoError::invalid_data(format!(
                "failed to parse CAD_GEOMETRIA_UHE.csv at line {}: {e}",
                line_idx + 2
            ))
        })?;

        for (column, value) in [
            ("volume.hm3", row.volume_hm3),
            ("cota.m", row.cota_m),
            ("area.km2", row.area_km2),
        ] {
            check_finite(
                "CAD_GEOMETRIA_UHE.csv",
                row.codigo,
                line_idx + 2,
                column,
                value,
            )?;
        }

        for (quantity, value) in [
            ("volume", row.volume_hm3),
            ("height", row.cota_m),
            ("area", row.area_km2),
        ] {
            if value < 0.0 {
                return Err(IoError::invalid_data(format!(
                    "plant {} has negative {quantity} {value} at line {} in CAD_GEOMETRIA_UHE.csv",
                    row.codigo,
                    line_idx + 2
                )));
            }
        }

        grouped.entry(row.codigo).or_default().push((
            line_idx + 2,
            VhaPoint {
                volume_hm3: row.volume_hm3,
                height_m: row.cota_m,
            },
        ));
    }

    grouped
        .into_iter()
        .map(|(code, mut points)| {
            points.sort_by(|(_, a), (_, b)| a.volume_hm3.total_cmp(&b.volume_hm3));
            if let Some(pair) = points
                .windows(2)
                .find(|pair| pair[0].1.volume_hm3 == pair[1].1.volume_hm3)
            {
                return Err(IoError::invalid_data(format!(
                    "plant {code} has duplicate volume {} at lines {} and {} in CAD_GEOMETRIA_UHE.csv",
                    pair[0].1.volume_hm3, pair[0].0, pair[1].0
                )));
            }
            Ok((
                code,
                VhaGeometryData {
                    points: points.into_iter().map(|(_, point)| point).collect(),
                },
            ))
        })
        .collect()
}

/// Read tailrace polynomial coefficients from `CAD/CAD_TAILRACE.csv`.
///
/// Returns a HashMap mapping plant codes to their tailrace data.
///
/// Returns an empty HashMap if the file does not exist (file is optional).
/// Fails on a non-finite coefficient or on a second row for the same plant.
pub(crate) fn read_tailrace(case_path: &Path) -> Result<HashMap<u32, TailraceData>, IoError> {
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
    let mut first_lines: HashMap<u32, usize> = HashMap::new();

    for (line_idx, record) in reader.deserialize().enumerate() {
        let row: TailraceRow = record.map_err(|e| {
            IoError::invalid_data(format!(
                "failed to parse CAD_TAILRACE.csv at line {}: {e}",
                line_idx + 2
            ))
        })?;

        let coefficients = [row.a0, row.a1, row.a2, row.a3, row.a4];
        for (column, value) in TAILRACE_COLUMNS.into_iter().zip(coefficients) {
            check_finite("CAD_TAILRACE.csv", row.codigo, line_idx + 2, column, value)?;
        }

        if let Some(first_line) = first_lines.insert(row.codigo, line_idx + 2) {
            return Err(IoError::invalid_data(format!(
                "plant {} has duplicate rows at lines {} and {} in CAD_TAILRACE.csv",
                row.codigo,
                first_line,
                line_idx + 2
            )));
        }

        result.insert(row.codigo, TailraceData { coefficients });
    }

    Ok(result)
}

#[derive(Debug, Deserialize)]
struct FphaConfigRow {
    #[serde(
        alias = "codigo_usina",
        alias = "Codigo",
        alias = "CODIGO",
        alias = "codigo"
    )]
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
/// Returns a HashMap mapping plant ids to their configuration overrides.
///
/// Returns an empty HashMap if the file does not exist (file is optional).
/// Fails on a non-finite value, `num_planes = 0`, `min_volume_pct >= max_volume_pct` when both are
/// given, or a second row for the same plant.
pub fn read_fpha_config(
    case_path: &Path,
) -> Result<HashMap<HydroPlantId, FphaConfigOverrides>, IoError> {
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
    let mut first_lines: HashMap<u32, usize> = HashMap::new();

    for (line_idx, record) in reader.deserialize().enumerate() {
        let row: FphaConfigRow = record.map_err(|e| {
            IoError::invalid_data(format!(
                "failed to parse CAD_FPHA_CONFIG.csv at line {}: {e}",
                line_idx + 2
            ))
        })?;

        for (column, value) in [
            ("tolerance", row.tolerance),
            ("min_volume_pct", row.min_volume_pct),
            ("max_volume_pct", row.max_volume_pct),
        ] {
            if let Some(value) = value {
                check_finite(
                    "CAD_FPHA_CONFIG.csv",
                    row.codigo,
                    line_idx + 2,
                    column,
                    value,
                )?;
            }
        }

        if let Some(tol) = row.tolerance
            && tol < 0.0
        {
            return Err(IoError::invalid_data(format!(
                "plant {} has negative tolerance {} at line {} in CAD_FPHA_CONFIG.csv",
                row.codigo,
                tol,
                line_idx + 2
            )));
        }

        if row.num_planes == Some(0) {
            return Err(IoError::invalid_data(format!(
                "plant {} has num_planes 0 (must be at least 1) at line {} in CAD_FPHA_CONFIG.csv",
                row.codigo,
                line_idx + 2
            )));
        }

        if let Some(pct) = row.min_volume_pct
            && !(0.0..=1.0).contains(&pct)
        {
            return Err(IoError::invalid_data(format!(
                "plant {} has invalid min_volume_pct {} (must be 0-1) at line {} in CAD_FPHA_CONFIG.csv",
                row.codigo,
                pct,
                line_idx + 2
            )));
        }

        if let Some(pct) = row.max_volume_pct
            && !(0.0..=1.0).contains(&pct)
        {
            return Err(IoError::invalid_data(format!(
                "plant {} has invalid max_volume_pct {} (must be 0-1) at line {} in CAD_FPHA_CONFIG.csv",
                row.codigo,
                pct,
                line_idx + 2
            )));
        }

        if let (Some(min), Some(max)) = (row.min_volume_pct, row.max_volume_pct)
            && min >= max
        {
            return Err(IoError::invalid_data(format!(
                "plant {} has min_volume_pct {} not below max_volume_pct {} at line {} in CAD_FPHA_CONFIG.csv",
                row.codigo,
                min,
                max,
                line_idx + 2
            )));
        }

        if let Some(first_line) = first_lines.insert(row.codigo, line_idx + 2) {
            return Err(IoError::invalid_data(format!(
                "plant {} has duplicate rows at lines {} and {} in CAD_FPHA_CONFIG.csv",
                row.codigo,
                first_line,
                line_idx + 2
            )));
        }

        result.insert(
            HydroPlantId(row.codigo as usize),
            FphaConfigOverrides {
                max_planes: row.num_planes,
                tolerance_deg: row.tolerance,
                min_volume_fraction: row.min_volume_pct,
                max_volume_fraction: row.max_volume_pct,
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
            let mut file = std::fs::File::create(cad_dir.join("CAD_GEOMETRIA_UHE.csv")).unwrap();
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
            "codigo_usina;volume.hm3;cota.m;area.km2\n1;1000.0;250.0;50.0\n1;2000.0;260.0;75.0";
        let dir = create_test_case(Some(content), None);

        let result = read_vha_geometry(dir.path()).unwrap();
        assert_eq!(result.len(), 1);
        assert_eq!(result[&1].points.len(), 2);
        assert_eq!(result[&1].points[0].volume_hm3, 1000.0);
        assert_eq!(result[&1].points[1].volume_hm3, 2000.0);
    }

    #[test]
    fn read_vha_geometry_sorts_by_volume() {
        let content = "codigo_usina;volume.hm3;cota.m;area.km2\n1;3000.0;270.0;100.0\n1;1000.0;250.0;50.0\n1;2000.0;260.0;75.0";
        let dir = create_test_case(Some(content), None);

        let result = read_vha_geometry(dir.path()).unwrap();
        assert_eq!(result[&1].points[0].volume_hm3, 1000.0);
        assert_eq!(result[&1].points[1].volume_hm3, 2000.0);
        assert_eq!(result[&1].points[2].volume_hm3, 3000.0);
    }

    #[test]
    fn read_vha_geometry_multiple_plants() {
        let content = "codigo_usina;volume.hm3;cota.m;area.km2\n1;1000.0;250.0;50.0\n2;500.0;180.0;25.0\n1;2000.0;260.0;75.0";
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
        let content = "codigo_usina;volume.hm3;cota.m;area.km2\n1;-100.0;250.0;50.0";
        let dir = create_test_case(Some(content), None);

        let result = read_vha_geometry(dir.path());
        assert!(result.is_err());
        let err_msg = result.unwrap_err().to_string();
        assert!(err_msg.contains("negative volume"));
        assert!(err_msg.contains("plant 1"));
    }

    #[test]
    fn read_vha_geometry_negative_height_error() {
        let content = "codigo_usina;volume.hm3;cota.m;area.km2\n1;1000.0;-50.0;50.0";
        let dir = create_test_case(Some(content), None);

        let result = read_vha_geometry(dir.path());
        assert!(result.is_err());
        let err_msg = result.unwrap_err().to_string();
        assert!(err_msg.contains("negative height"));
    }

    #[test]
    fn read_vha_geometry_negative_area_error() {
        let content = "codigo_usina;volume.hm3;cota.m;area.km2\n1;1000.0;250.0;-50.0";
        let dir = create_test_case(Some(content), None);

        let result = read_vha_geometry(dir.path());
        assert!(result.is_err());
        let err_msg = result.unwrap_err().to_string();
        assert!(err_msg.contains("negative area"));
    }

    #[test]
    fn read_tailrace_valid() {
        let content = "codigo_usina;a0.m;a1.m/m3s;a2.m/m3s2;a3.m/m3s3;a4.m/m3s4\n1;120.5;0.001;-0.00001;0.0;0.0";
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
        let content = "codigo_usina;a0.m;a1.m/m3s;a2.m/m3s2;a3.m/m3s3;a4.m/m3s4\n1;120.5;0.001;-0.00001;0.0;0.0\n2;95.2;0.002;0.0;0.0;0.0";
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
        let content = "codigo_usina;a0.m;a1.m/m3s;a2.m/m3s2;a3.m/m3s3;a4.m/m3s4\n1;120.5;invalid;-0.00001;0.0;0.0";
        let dir = create_test_case(None, Some(content));

        let result = read_tailrace(dir.path());
        assert!(result.is_err());
        let err_msg = result.unwrap_err().to_string();
        assert!(err_msg.contains("CAD_TAILRACE.csv"));
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
        let config = &result[&HydroPlantId(1)];
        assert_eq!(config.max_planes, Some(5));
        assert_eq!(config.tolerance_deg, Some(0.01));
        assert_eq!(config.min_volume_fraction, Some(0.05));
        assert_eq!(config.max_volume_fraction, Some(1.0));
    }

    #[test]
    fn read_fpha_config_partial_row() {
        let content = "codigo_usina;num_planes\n1;7";
        let dir = create_config_test_case(Some(content));

        let result = read_fpha_config(dir.path()).unwrap();
        let config = &result[&HydroPlantId(1)];
        assert_eq!(config.max_planes, Some(7));
        assert_eq!(config.tolerance_deg, None);
        assert_eq!(config.min_volume_fraction, None);
        assert_eq!(config.max_volume_fraction, None);
    }

    #[test]
    fn read_fpha_config_multiple_plants() {
        let content = "codigo_usina;num_planes;tolerance\n1;5;0.01\n3;7;0.005";
        let dir = create_config_test_case(Some(content));

        let result = read_fpha_config(dir.path()).unwrap();
        assert_eq!(result.len(), 2);
        assert_eq!(result[&HydroPlantId(1)].max_planes, Some(5));
        assert_eq!(result[&HydroPlantId(3)].max_planes, Some(7));
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
        let content =
            "codigo_usina;num_planes;tolerance;min_volume_pct;max_volume_pct\n1;5;0.01;0.05;-0.5";
        let dir = create_config_test_case(Some(content));

        let result = read_fpha_config(dir.path());
        assert!(result.is_err());
        let err_msg = result.unwrap_err().to_string();
        assert!(err_msg.contains("max_volume_pct"));
    }

    fn error_message<T>(result: Result<T, IoError>) -> Result<String, Box<dyn std::error::Error>> {
        Ok(result.err().ok_or("expected an error")?.to_string())
    }

    const NON_FINITE_SPELLINGS: [&str; 3] = ["NaN", "inf", "-inf"];

    #[test]
    fn read_vha_geometry_non_finite_value_error() -> Result<(), Box<dyn std::error::Error>> {
        for (index, column) in ["volume.hm3", "cota.m", "area.km2"].into_iter().enumerate() {
            for spelling in NON_FINITE_SPELLINGS {
                let mut cells = ["2000.0", "260.0", "75.0"];
                cells[index] = spelling;
                let content = format!(
                    "codigo_usina;volume.hm3;cota.m;area.km2\n1;1000.0;250.0;50.0\n1;{}",
                    cells.join(";")
                );
                let dir = create_test_case(Some(&content), None);

                let message = error_message(read_vha_geometry(dir.path()))?;

                assert_eq!(
                    message,
                    format!(
                        "plant 1 has non-finite {column} {spelling} at line 3 in CAD_GEOMETRIA_UHE.csv"
                    )
                );
            }
        }
        Ok(())
    }

    #[test]
    fn read_vha_geometry_duplicate_volume_error() -> Result<(), Box<dyn std::error::Error>> {
        let content = "codigo_usina;volume.hm3;cota.m;area.km2\n1;1000.0;250.0;50.0\n1;2000.0;260.0;75.0\n1;1000.0;251.0;51.0";
        let dir = create_test_case(Some(content), None);

        let message = error_message(read_vha_geometry(dir.path()))?;

        assert_eq!(
            message,
            "plant 1 has duplicate volume 1000 at lines 2 and 4 in CAD_GEOMETRIA_UHE.csv"
        );
        Ok(())
    }

    #[test]
    fn read_vha_geometry_reports_duplicates_in_code_order() -> Result<(), Box<dyn std::error::Error>>
    {
        let content = "codigo_usina;volume.hm3;cota.m;area.km2\n2;1000.0;250.0;50.0\n2;1000.0;251.0;51.0\n1;500.0;180.0;25.0\n1;500.0;181.0;26.0";
        let dir = create_test_case(Some(content), None);

        // Each read hashes with a fresh seed: a hash-ordered check names plant 2 on half of them.
        for _ in 0..16 {
            let message = error_message(read_vha_geometry(dir.path()))?;

            assert_eq!(
                message,
                "plant 1 has duplicate volume 500 at lines 4 and 5 in CAD_GEOMETRIA_UHE.csv"
            );
        }
        Ok(())
    }

    #[test]
    fn read_vha_geometry_same_volume_for_two_plants_is_valid()
    -> Result<(), Box<dyn std::error::Error>> {
        let content =
            "codigo_usina;volume.hm3;cota.m;area.km2\n1;1000.0;250.0;50.0\n2;1000.0;180.0;25.0";
        let dir = create_test_case(Some(content), None);

        let result = read_vha_geometry(dir.path())?;

        assert_eq!(result.len(), 2);
        assert_eq!(result[&1].points.len(), 1);
        assert_eq!(result[&2].points.len(), 1);
        Ok(())
    }

    #[test]
    fn read_tailrace_non_finite_value_error() -> Result<(), Box<dyn std::error::Error>> {
        for (index, column) in TAILRACE_COLUMNS.into_iter().enumerate() {
            for spelling in NON_FINITE_SPELLINGS {
                let mut cells = ["120.5", "0.001", "0.0", "0.0", "0.0"];
                cells[index] = spelling;
                let content = format!(
                    "codigo_usina;a0.m;a1.m/m3s;a2.m/m3s2;a3.m/m3s3;a4.m/m3s4\n2;95.2;0.002;0.0;0.0;0.0\n1;{}",
                    cells.join(";")
                );
                let dir = create_test_case(None, Some(&content));

                let message = error_message(read_tailrace(dir.path()))?;

                assert_eq!(
                    message,
                    format!(
                        "plant 1 has non-finite {column} {spelling} at line 3 in CAD_TAILRACE.csv"
                    )
                );
            }
        }
        Ok(())
    }

    #[test]
    fn read_tailrace_duplicate_plant_error() -> Result<(), Box<dyn std::error::Error>> {
        let content = "codigo_usina;a0.m;a1.m/m3s;a2.m/m3s2;a3.m/m3s3;a4.m/m3s4\n1;120.5;0.001;0.0;0.0;0.0\n2;95.2;0.002;0.0;0.0;0.0\n1;130.0;0.003;0.0;0.0;0.0";
        let dir = create_test_case(None, Some(content));

        let message = error_message(read_tailrace(dir.path()))?;

        assert_eq!(
            message,
            "plant 1 has duplicate rows at lines 2 and 4 in CAD_TAILRACE.csv"
        );
        Ok(())
    }

    #[test]
    fn read_tailrace_absent_optional_columns_default_to_zero()
    -> Result<(), Box<dyn std::error::Error>> {
        let content = "codigo_usina;a0.m\n1;120.5";
        let dir = create_test_case(None, Some(content));

        let result = read_tailrace(dir.path())?;

        assert_eq!(result[&1].coefficients, [120.5, 0.0, 0.0, 0.0, 0.0]);
        Ok(())
    }

    #[test]
    fn read_fpha_config_non_finite_value_error() -> Result<(), Box<dyn std::error::Error>> {
        for (index, column) in ["tolerance", "min_volume_pct", "max_volume_pct"]
            .into_iter()
            .enumerate()
        {
            for spelling in NON_FINITE_SPELLINGS {
                let mut cells = ["0.01", "0.05", "1.0"];
                cells[index] = spelling;
                let content = format!(
                    "codigo_usina;num_planes;tolerance;min_volume_pct;max_volume_pct\n2;5;0.01;0.05;1.0\n1;5;{}",
                    cells.join(";")
                );
                let dir = create_config_test_case(Some(&content));

                let message = error_message(read_fpha_config(dir.path()))?;

                assert_eq!(
                    message,
                    format!(
                        "plant 1 has non-finite {column} {spelling} at line 3 in CAD_FPHA_CONFIG.csv"
                    )
                );
            }
        }
        Ok(())
    }

    #[test]
    fn read_fpha_config_zero_num_planes_error() -> Result<(), Box<dyn std::error::Error>> {
        let content = "codigo_usina;num_planes\n2;5\n1;0";
        let dir = create_config_test_case(Some(content));

        let message = error_message(read_fpha_config(dir.path()))?;

        assert_eq!(
            message,
            "plant 1 has num_planes 0 (must be at least 1) at line 3 in CAD_FPHA_CONFIG.csv"
        );
        Ok(())
    }

    #[test]
    fn read_fpha_config_min_volume_pct_not_below_max_error()
    -> Result<(), Box<dyn std::error::Error>> {
        for (min, max) in [("0.6", "0.5"), ("0.5", "0.5")] {
            let content =
                format!("codigo_usina;min_volume_pct;max_volume_pct\n2;0.1;0.9\n1;{min};{max}");
            let dir = create_config_test_case(Some(&content));

            let message = error_message(read_fpha_config(dir.path()))?;

            assert_eq!(
                message,
                format!(
                    "plant 1 has min_volume_pct {min} not below max_volume_pct {max} at line 3 in CAD_FPHA_CONFIG.csv"
                )
            );
        }
        Ok(())
    }

    #[test]
    fn read_fpha_config_duplicate_plant_error() -> Result<(), Box<dyn std::error::Error>> {
        let content = "codigo_usina;num_planes\n1;5\n3;7\n1;9";
        let dir = create_config_test_case(Some(content));

        let message = error_message(read_fpha_config(dir.path()))?;

        assert_eq!(
            message,
            "plant 1 has duplicate rows at lines 2 and 4 in CAD_FPHA_CONFIG.csv"
        );
        Ok(())
    }

    /// The `fpha_test_case` fixture: three plants in `CAD/CAD_GEOMETRIA_UHE.csv` and
    /// `CAD/CAD_FPHA_CONFIG.csv`.
    fn fpha_test_case_dir() -> std::path::PathBuf {
        Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/fpha_test_case")
    }

    #[test]
    fn read_vha_geometry_from_example() {
        let case_dir = fpha_test_case_dir();

        let result = read_vha_geometry(&case_dir);
        assert!(
            result.is_ok(),
            "Failed to read VHA geometry: {:?}",
            result.err()
        );

        let data = result.unwrap();

        // The file has 3 plants
        assert_eq!(data.len(), 3, "Expected 3 plants in geometry data");

        // Check plant codes are present
        assert!(data.contains_key(&1), "FURNAS (code 1) should be present");
        assert!(
            data.contains_key(&2),
            "EMBORCACAO (code 2) should be present"
        );
        assert!(
            data.contains_key(&3),
            "ILHA_SOLTEIRA (code 3) should be present"
        );

        // FURNAS has 6 points
        let furnas = data.get(&1).unwrap();
        assert_eq!(
            furnas.points.len(),
            6,
            "FURNAS should have 6 geometry points"
        );

        // Verify first and last points of FURNAS
        assert_eq!(furnas.points[0].volume_hm3, 0.0);
        assert_eq!(furnas.points[5].volume_hm3, 22000.0);
    }

    #[test]
    fn read_fpha_config_from_example() {
        let case_dir = fpha_test_case_dir();

        let result = read_fpha_config(&case_dir);
        assert!(
            result.is_ok(),
            "Failed to read FPHA config: {:?}",
            result.err()
        );

        let configs = result.unwrap();

        // The file has 3 plants
        assert_eq!(configs.len(), 3, "Expected 3 plants in config");

        // Check FURNAS config (code 1)
        let furnas_config = configs
            .get(&HydroPlantId(1))
            .expect("FURNAS config not found");
        assert_eq!(furnas_config.max_planes, Some(8));
        assert_eq!(furnas_config.tolerance_deg, Some(1.0));
    }

    #[test]
    fn geometry_and_config_plant_codes_match() {
        let case_dir = fpha_test_case_dir();

        let geometry = read_vha_geometry(&case_dir).expect("Failed to read geometry");
        let configs = read_fpha_config(&case_dir).expect("Failed to read config");

        // All plants in config should have geometry data
        for id in configs.keys() {
            let code = id.0 as u32;
            assert!(
                geometry.contains_key(&code),
                "Plant code {code} in config has no geometry data"
            );
        }
    }
}
