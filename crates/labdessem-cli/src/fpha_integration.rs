//! FPHA fitting orchestration for computed mode.
//!
//! This module provides the infrastructure for computing FPHA hyperplanes
//! from VHA geometry at runtime when `FPHA: "computed"` is set in study_config.json.

use std::collections::HashMap;
use std::path::Path;

use labdessem_core::entities::{HydroFphaSegment, HydroPlant};
use labdessem_core::system::System;
use labdessem_fpha::{
    fit_plant, FitPlantParams, FphaPlantConfig, FphaPlantResult,
    fitting::{ForebayTable, PlaneReductionConfig, VhaRow},
    types::TailraceModel,
};
use labdessem_io::{
    output::{write_fpha_computed, ComputedFphaRecord},
    readers::{
        read_fpha_config, read_tailrace, read_vha_geometry, FphaPlantConfig as IoFphaPlantConfig,
        TailraceData, VhaGeometryData,
    },
};

/// Result of the FPHA computation process.
#[derive(Debug)]
pub struct FphaComputeResult {
    pub plant_results: Vec<FphaPlantResult>,
    pub plants_processed: usize,
    pub plants_skipped: usize,
}

/// Error type for FPHA computation.
#[derive(Debug)]
pub enum FphaComputeError {
    Io(String),
    #[allow(dead_code)]
    Fitting(String),
}

impl std::fmt::Display for FphaComputeError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            FphaComputeError::Io(msg) => write!(f, "I/O error: {msg}"),
            FphaComputeError::Fitting(msg) => write!(f, "Fitting error: {msg}"),
        }
    }
}

impl std::error::Error for FphaComputeError {}

impl From<labdessem_io::IoError> for FphaComputeError {
    fn from(e: labdessem_io::IoError) -> Self {
        FphaComputeError::Io(e.to_string())
    }
}

/// Compute FPHA planes from VHA geometry for all plants with available data.
pub fn compute_fpha(
    case_path: &Path,
    plants: &[HydroPlant],
    period_duration_hours: f64,
) -> Result<FphaComputeResult, FphaComputeError> {
    eprintln!("Computing FPHA planes from geometry data...");

    let vha_data = read_vha_geometry(case_path)?;
    let tailrace_data = read_tailrace(case_path)?;
    let fpha_config = read_fpha_config(case_path)?;

    if vha_data.is_empty() {
        eprintln!("Warning: No VHA geometry data found. No planes will be computed.");
        return Ok(FphaComputeResult {
            plant_results: vec![],
            plants_processed: 0,
            plants_skipped: plants.len(),
        });
    }

    let mut results = Vec::new();
    let mut processed = 0;
    let mut skipped = 0;

    for plant in plants {
        let plant_code = plant.id.0 as u32;

        let Some(vha) = vha_data.get(&plant_code) else {
            eprintln!(
                "Warning: Plant {} ({}) has no VHA geometry data, skipping",
                plant_code, plant.name
            );
            skipped += 1;
            continue;
        };

        let params = match build_fit_params(plant, vha, tailrace_data.get(&plant_code), fpha_config.get(&plant_code), period_duration_hours) {
            Ok(p) => p,
            Err(e) => {
                eprintln!(
                    "Warning: Plant {} ({}): failed to build fit params: {}",
                    plant_code, plant.name, e
                );
                skipped += 1;
                continue;
            }
        };

        match fit_plant(&params) {
            Ok(result) => {
                eprintln!(
                    "Plant {} ({}): fitted {} planes (α = {:.4})",
                    plant_code,
                    plant.name,
                    result.planes.len(),
                    result.alpha
                );
                results.push(result);
                processed += 1;
            }
            Err(e) => {
                eprintln!(
                    "Warning: Plant {} ({}): fitting failed: {}",
                    plant_code, plant.name, e
                );
                skipped += 1;
            }
        }
    }

    write_computed_results(case_path, &results, plants)?;

    eprintln!(
        "FPHA computation complete: {} plants processed, {} skipped",
        processed, skipped
    );

    Ok(FphaComputeResult {
        plant_results: results,
        plants_processed: processed,
        plants_skipped: skipped,
    })
}

fn build_fit_params(
    plant: &HydroPlant,
    vha: &VhaGeometryData,
    tailrace: Option<&TailraceData>,
    config: Option<&IoFphaPlantConfig>,
    period_duration_hours: f64,
) -> Result<FitPlantParams, String> {
    let vha_rows: Vec<VhaRow> = vha
        .points
        .iter()
        .map(|p| VhaRow {
            volume_hm3: p.volume_hm3,
            height_m: p.height_m,
            area_km2: p.area_km2,
        })
        .collect();

    let forebay = ForebayTable::new(&vha_rows, &plant.name)
        .map_err(|e| format!("ForebayTable creation failed: {e}"))?;

    let tailrace_model = tailrace.map(|t| TailraceModel::Polynomial {
        coefficients: t.coefficients.to_vec(),
    });

    let max_turbined_m3s = plant
        .groups
        .iter()
        .map(|g| g.units.iter().map(|u| u.max_turbining_hm3).sum::<f64>())
        .sum::<f64>() * 1e6 / (period_duration_hours * 3600.0);

    let max_generation_mw = plant
        .groups
        .iter()
        .map(|g| g.units.iter().map(|u| u.max_generation_mw).sum::<f64>())
        .sum();

    let long_term_mean_inflow_m3s = if plant.natural_inflow_hm3.is_empty() {
        0.0
    } else {
        let mean_hm3 = plant.natural_inflow_hm3.iter().sum::<f64>() / plant.natural_inflow_hm3.len() as f64;
        mean_hm3 * 1e6 / (period_duration_hours * 3600.0)
    };

    let fpha_config = build_fpha_config(config);

    Ok(FitPlantParams {
        codigo: plant.id.0 as i32,
        nome: plant.name.clone(),
        forebay,
        tailrace: tailrace_model,
        hydraulic_loss_m: None,
        efficiency: None,
        max_turbined_m3s,
        max_generation_mw,
        long_term_mean_inflow_m3s,
        config: fpha_config,
    })
}

fn build_fpha_config(config: Option<&IoFphaPlantConfig>) -> FphaPlantConfig {
    let mut fpha_config = FphaPlantConfig::computed(
        10, 6, 6,
        PlaneReductionConfig::Angle { tolerance_deg: 1.0 },
    );

    if let Some(c) = config {
        if let Some(n) = c.num_planes {
            fpha_config.max_planes = n;
        }
        // tolerance applies to the reduction config, not a direct field
    }

    fpha_config
}

pub fn write_computed_results(
    case_path: &Path,
    results: &[FphaPlantResult],
    plants: &[HydroPlant],
) -> Result<(), FphaComputeError> {
    let plant_names: HashMap<i32, &str> = plants
        .iter()
        .map(|p| (p.id.0 as i32, p.name.as_str()))
        .collect();

    let records: Vec<ComputedFphaRecord> = results
        .iter()
        .flat_map(|r| {
            let name = plant_names.get(&r.codigo).unwrap_or(&"UNKNOWN");
            r.planes.iter().enumerate().map(move |(idx, plane)| {
                ComputedFphaRecord {
                    codigo_usina: r.codigo as u32,
                    nome_usina: (*name).to_string(),
                    plane_index: idx,
                    gamma_0: plane.gamma_0,
                    gamma_v: plane.gamma_v,
                    gamma_q: plane.gamma_q,
                    gamma_s: plane.gamma_s,
                    alpha: r.alpha,
                }
            })
        })
        .collect();

    write_fpha_computed(case_path, &records)?;
    Ok(())
}

/// Convert fitting results to model-compatible segments.
pub fn convert_to_fpha_segments(result: &FphaPlantResult) -> Vec<HydroFphaSegment> {
    result
        .planes
        .iter()
        .enumerate()
        .map(|(idx, plane)| HydroFphaSegment {
            segment: idx + 1, // 1-indexed
            correction_factor: result.alpha,
            rhs: plane.gamma_0,
            volume_coefficient: plane.gamma_v,
            turbining_coefficient: plane.gamma_q,
            lateral_flow_coefficient: plane.gamma_s,
        })
        .collect()
}

/// Inject computed FPHA segments into the system.
pub fn inject_fpha_segments(
    system: &mut System,
    plant_results: &[FphaPlantResult],
) {
    let segments_by_plant: HashMap<i32, Vec<HydroFphaSegment>> = plant_results
        .iter()
        .map(|r| (r.codigo, convert_to_fpha_segments(r)))
        .collect();

    let mut injected = 0;

    for plant in &mut system.hydro_plants {
        let plant_code = plant.id.0 as i32;
        if let Some(segments) = segments_by_plant.get(&plant_code) {
            plant.fpha_segments = segments.clone();
            eprintln!(
                "Injected {} FPHA segments for plant {} ({})",
                segments.len(),
                plant_code,
                plant.name
            );
            injected += 1;
        }
    }

    for result in plant_results {
        if !system
            .hydro_plants
            .iter()
            .any(|p| p.id.0 as i32 == result.codigo)
        {
            eprintln!(
                "Warning: Computed FPHA for plant {} but it doesn't exist in system",
                result.codigo
            );
        }
    }

    eprintln!(
        "FPHA injection complete: {} plants updated",
        injected
    );
}

#[cfg(test)]
#[allow(clippy::unwrap_used, clippy::expect_used)]
mod tests {
    use super::*;

    #[test]
    fn build_fpha_config_defaults() {
        let config = build_fpha_config(None);
        assert_eq!(config.max_planes, 10);
    }

    #[test]
    fn build_fpha_config_with_overrides() {
        let io_config = IoFphaPlantConfig {
            num_planes: Some(7),
            tolerance: Some(0.005),
            min_volume_pct: None,
            max_volume_pct: None,
        };

        let config = build_fpha_config(Some(&io_config));
        assert_eq!(config.max_planes, 7);
    }

    #[test]
    fn convert_single_plane() {
        use labdessem_fpha::fitting::{FphaFitDeviation, RawPlane};

        let result = FphaPlantResult {
            codigo: 1,
            nome: "TEST".to_string(),
            planes: vec![RawPlane {
                gamma_0: 1000.0,
                gamma_v: -0.002,
                gamma_q: 0.05,
                gamma_s: 0.001,
            }],
            alpha: 0.95,
            deviation: FphaFitDeviation {
                mean_abs_mw: 0.0,
                max_abs_mw: 0.0,
                rms_mw: 0.0,
                mean_signed_mw: 0.0,
                relative: 0.0,
            },
        };

        let segments = convert_to_fpha_segments(&result);

        assert_eq!(segments.len(), 1);
        assert_eq!(segments[0].segment, 1);
        assert_eq!(segments[0].rhs, 1000.0);
        assert_eq!(segments[0].volume_coefficient, -0.002);
        assert_eq!(segments[0].turbining_coefficient, 0.05);
        assert_eq!(segments[0].lateral_flow_coefficient, 0.001);
        assert_eq!(segments[0].correction_factor, 0.95);
    }

    #[test]
    fn convert_multiple_planes() {
        use labdessem_fpha::fitting::{FphaFitDeviation, RawPlane};

        let result = FphaPlantResult {
            codigo: 1,
            nome: "TEST".to_string(),
            planes: vec![
                RawPlane { gamma_0: 100.0, gamma_v: 0.1, gamma_q: 0.2, gamma_s: 0.3 },
                RawPlane { gamma_0: 200.0, gamma_v: 0.4, gamma_q: 0.5, gamma_s: 0.6 },
                RawPlane { gamma_0: 300.0, gamma_v: 0.7, gamma_q: 0.8, gamma_s: 0.9 },
            ],
            alpha: 0.98,
            deviation: FphaFitDeviation {
                mean_abs_mw: 0.0,
                max_abs_mw: 0.0,
                rms_mw: 0.0,
                mean_signed_mw: 0.0,
                relative: 0.0,
            },
        };

        let segments = convert_to_fpha_segments(&result);

        assert_eq!(segments.len(), 3);
        assert_eq!(segments[0].segment, 1);
        assert_eq!(segments[1].segment, 2);
        assert_eq!(segments[2].segment, 3);
        for seg in &segments {
            assert_eq!(seg.correction_factor, 0.98);
        }
    }

    #[test]
    fn convert_empty_planes() {
        use labdessem_fpha::fitting::FphaFitDeviation;

        let result = FphaPlantResult {
            codigo: 1,
            nome: "TEST".to_string(),
            planes: vec![],
            alpha: 1.0,
            deviation: FphaFitDeviation {
                mean_abs_mw: 0.0,
                max_abs_mw: 0.0,
                rms_mw: 0.0,
                mean_signed_mw: 0.0,
                relative: 0.0,
            },
        };

        let segments = convert_to_fpha_segments(&result);
        assert!(segments.is_empty());
    }
}
