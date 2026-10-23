//! Conversion utilities from labdessem-io DTOs to labdessem-fpha domain types.
//!
//! This module bridges the I/O layer (`labdessem-io`) and the computational
//! layer (`labdessem-fpha`), converting data transfer objects read from CSV
//! files into the domain types used by the FPHA fitting algorithms.

use labdessem_io::readers::{TailraceData, VhaGeometryData};

use crate::error::FphaFittingError;
use crate::fitting::{ForebayTable, VhaRow};
use crate::types::TailraceModel;

/// Convert VHA geometry data from labdessem-io into a ForebayTable.
///
/// # Arguments
/// * `data` - VHA geometry data read from `CAD_GEOMETRIA_UHE.csv`
/// * `name` - Plant name for error messages and ForebayTable identification
///
/// # Errors
/// Returns `FphaFittingError` if the VHA data is empty or contains duplicate volumes.
pub fn forebay_from_vha(data: &VhaGeometryData, name: &str) -> Result<ForebayTable, FphaFittingError> {
    let rows: Vec<VhaRow> = data
        .points
        .iter()
        .map(|p| VhaRow {
            volume_hm3: p.volume_hm3,
            height_m: p.height_m,
            area_km2: p.area_km2,
        })
        .collect();

    ForebayTable::new(&rows, name)
}

/// Convert tailrace data from labdessem-io into a TailraceModel.
///
/// Creates a polynomial tailrace model from the coefficient array.
/// The coefficients represent: level = c0 + c1*q + c2*q² + c3*q³ + c4*q⁴
///
/// # Arguments
/// * `data` - Tailrace data read from `CAD_TAILRACE.csv`
///
/// # Returns
/// A `TailraceModel::Polynomial` with the coefficient vector.
pub fn tailrace_from_data(data: &TailraceData) -> TailraceModel {
    TailraceModel::Polynomial {
        coefficients: data.coefficients.to_vec(),
    }
}

#[cfg(test)]
#[allow(clippy::unwrap_used, clippy::expect_used)]
mod tests {
    use super::*;
    use labdessem_io::readers::VhaPoint;

    #[test]
    fn forebay_from_vha_preserves_all_rows() {
        let data = VhaGeometryData {
            plant_code: 1,
            points: vec![
                VhaPoint {
                    volume_hm3: 1000.0,
                    height_m: 250.0,
                    area_km2: 50.0,
                },
                VhaPoint {
                    volume_hm3: 2000.0,
                    height_m: 260.0,
                    area_km2: 75.0,
                },
                VhaPoint {
                    volume_hm3: 3000.0,
                    height_m: 270.0,
                    area_km2: 100.0,
                },
            ],
        };

        let table = forebay_from_vha(&data, "TEST").unwrap();

        assert_eq!(table.v_min(), 1000.0);
        assert_eq!(table.v_max(), 3000.0);
        assert_eq!(table.hydro_name(), "TEST");
    }

    #[test]
    fn forebay_from_vha_interpolates_correctly() {
        let data = VhaGeometryData {
            plant_code: 1,
            points: vec![
                VhaPoint {
                    volume_hm3: 0.0,
                    height_m: 100.0,
                    area_km2: 10.0,
                },
                VhaPoint {
                    volume_hm3: 100.0,
                    height_m: 200.0,
                    area_km2: 20.0,
                },
            ],
        };

        let table = forebay_from_vha(&data, "INTERP").unwrap();

        // Midpoint should interpolate to 150m
        let h = table.height(50.0);
        assert!((h - 150.0).abs() < 1e-9);
    }

    #[test]
    fn forebay_from_vha_errors_on_empty() {
        let data = VhaGeometryData {
            plant_code: 1,
            points: vec![],
        };

        let result = forebay_from_vha(&data, "EMPTY");
        assert!(result.is_err());
    }

    #[test]
    fn tailrace_from_data_preserves_coefficients() {
        let data = TailraceData {
            plant_code: 1,
            coefficients: [100.0, 0.001, -1e-8, 0.0, 0.0],
        };

        let model = tailrace_from_data(&data);

        if let TailraceModel::Polynomial { coefficients } = model {
            assert_eq!(coefficients.len(), 5);
            assert_eq!(coefficients[0], 100.0);
            assert_eq!(coefficients[1], 0.001);
            assert_eq!(coefficients[2], -1e-8);
        } else {
            panic!("Expected Polynomial model");
        }
    }

    #[test]
    fn tailrace_from_data_evaluates_correctly() {
        let data = TailraceData {
            plant_code: 1,
            coefficients: [50.0, 0.01, 0.0, 0.0, 0.0],
        };

        let model = tailrace_from_data(&data);

        // At q=100 m³/s: level = 50 + 0.01*100 = 51.0
        let level = model.evaluate(100.0);
        assert!((level - 51.0).abs() < 1e-9);
    }
}
