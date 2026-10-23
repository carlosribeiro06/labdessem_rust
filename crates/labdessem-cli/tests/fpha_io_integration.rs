#![allow(clippy::unwrap_used, clippy::expect_used)]

use labdessem_io::readers::{read_fpha_config, read_vha_geometry};
use std::path::PathBuf;

mod common {
    use std::path::PathBuf;

    pub fn fpha_test_case_dir() -> PathBuf {
        PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/fpha_test_case")
    }
}

// ====================
// VHA Geometry Reader Tests
// ====================

#[test]
fn read_vha_geometry_from_example() {
    let case_dir = common::fpha_test_case_dir();

    let result = read_vha_geometry(&case_dir);
    assert!(result.is_ok(), "Failed to read VHA geometry: {:?}", result.err());

    let data = result.unwrap();

    // The file has 3 plants
    assert_eq!(data.len(), 3, "Expected 3 plants in geometry data");

    // Check plant codes are present
    assert!(data.contains_key(&1), "FURNAS (code 1) should be present");
    assert!(data.contains_key(&2), "EMBORCACAO (code 2) should be present");
    assert!(data.contains_key(&3), "ILHA_SOLTEIRA (code 3) should be present");

    // FURNAS has 6 points
    let furnas = data.get(&1).unwrap();
    assert_eq!(furnas.points.len(), 6, "FURNAS should have 6 geometry points");

    // Verify first and last points of FURNAS
    assert_eq!(furnas.points[0].volume_hm3, 0.0);
    assert_eq!(furnas.points[5].volume_hm3, 22000.0);
}

#[test]
fn read_vha_geometry_missing_file_returns_empty() {
    let missing_dir = PathBuf::from("/nonexistent/path");
    let result = read_vha_geometry(&missing_dir);
    // Geometry file is optional, should return empty HashMap
    assert!(result.is_ok(), "Should succeed even if file is missing");
    assert!(result.unwrap().is_empty(), "Missing file should return empty HashMap");
}

// ====================
// FPHA Config Reader Tests
// ====================

#[test]
fn read_fpha_config_from_example() {
    let case_dir = common::fpha_test_case_dir();

    let result = read_fpha_config(&case_dir);
    assert!(result.is_ok(), "Failed to read FPHA config: {:?}", result.err());

    let configs = result.unwrap();

    // The file has 3 plants
    assert_eq!(configs.len(), 3, "Expected 3 plants in config");

    // Check FURNAS config (code 1)
    let furnas_config = configs.get(&1).expect("FURNAS config not found");
    assert_eq!(furnas_config.num_planes, Some(8));
    assert_eq!(furnas_config.tolerance, Some(1.0));
}

#[test]
fn read_fpha_config_missing_file_returns_empty() {
    let missing_dir = PathBuf::from("/nonexistent/path");
    let result = read_fpha_config(&missing_dir);
    // Config file is optional, should return empty HashMap
    assert!(result.is_ok(), "Config file should be optional");
    assert!(result.unwrap().is_empty(), "Missing config should return empty HashMap");
}

// ====================
// Integration: Geometry + Config combined
// ====================

#[test]
fn geometry_and_config_plant_codes_match() {
    let case_dir = common::fpha_test_case_dir();

    let geometry = read_vha_geometry(&case_dir).expect("Failed to read geometry");
    let configs = read_fpha_config(&case_dir).expect("Failed to read config");

    // All plants in config should have geometry data
    for (code, _config) in &configs {
        assert!(
            geometry.contains_key(code),
            "Plant code {} in config has no geometry data",
            code
        );
    }
}
