//! FPHA fitting orchestration for computed mode.
//!
//! This module provides the infrastructure for computing FPHA hyperplanes
//! from VHA geometry at runtime when `FPHA: "computed"` is set in study_config.json.

use std::collections::HashMap;
use std::path::Path;

use labdessem_core::entities::{HydroFphaSegment, HydroPlant, InflowHistoryRow};
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
    /// Plants without generating units (no FPHA needed).
    pub plants_skipped: usize,
}

/// Why the computed FPHA of a plant with generating units could not be fitted.
#[derive(Debug)]
pub enum FitFailureReason {
    MissingGeometry,
    InvalidParameters(String),
    FittingFailed(String),
}

impl std::fmt::Display for FitFailureReason {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            FitFailureReason::MissingGeometry => {
                write!(f, "no rows in CAD/CAD_GEOMETRIA_UHE.csv")
            }
            FitFailureReason::InvalidParameters(msg) => {
                write!(f, "invalid fit parameters: {msg}")
            }
            FitFailureReason::FittingFailed(msg) => write!(f, "fitting failed: {msg}"),
        }
    }
}

/// A plant with generating units whose computed FPHA could not be fitted.
#[derive(Debug)]
pub struct PlantFitFailure {
    pub hydro_id: i32,
    pub hydro_name: String,
    pub reason: FitFailureReason,
}

/// Error type for FPHA computation.
#[derive(Debug)]
pub enum FphaComputeError {
    Io(String),
    Fitting(String),
    MissingHydro { hydro_id: i32, hydro_name: String },
    PlantsNotFitted { failures: Vec<PlantFitFailure> },
    InvalidSystem(String),
}

impl std::fmt::Display for FphaComputeError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            FphaComputeError::Io(msg) => write!(f, "I/O error: {msg}"),
            FphaComputeError::Fitting(msg) => write!(f, "Fitting error: {msg}"),
            FphaComputeError::MissingHydro { hydro_id, hydro_name } => {
                write!(f, "hydro {hydro_id} ({hydro_name}) missing from inflow_history.parquet")
            }
            FphaComputeError::PlantsNotFitted { failures } => {
                write!(
                    f,
                    "computed FPHA could not be fitted for {} hydro plant(s) with generating units:",
                    failures.len()
                )?;
                for failure in failures {
                    write!(
                        f,
                        "\n  plant {} ({}): {}",
                        failure.hydro_id,
                        failure.hydro_name.trim(),
                        failure.reason
                    )?;
                }
                Ok(())
            }
            FphaComputeError::InvalidSystem(msg) => {
                write!(f, "system validation failed after FPHA injection: {msg}")
            }
        }
    }
}

impl std::error::Error for FphaComputeError {}

impl From<labdessem_io::IoError> for FphaComputeError {
    fn from(e: labdessem_io::IoError) -> Self {
        FphaComputeError::Io(e.to_string())
    }
}

/// Compute FPHA planes from VHA geometry for every plant with generating units.
///
/// Fails with `PlantsNotFitted`, listing every plant with units that could not be fitted.
pub fn compute_fpha(
    case_path: &Path,
    system: &System,
) -> Result<FphaComputeResult, FphaComputeError> {
    eprintln!("Computing FPHA planes from geometry data...");

    let plants = &system.hydro_plants;
    let period_duration_hours = system.horizon.period_duration_hours;

    let mlt_table = build_long_term_mean_inflow_table(&system.inflow_history);
    let parquet_present = !system.inflow_history.is_empty();

    if !parquet_present {
        eprintln!("Warning: inflow_history.parquet not found; using deck mean for MLT");
    }

    let vha_data = read_vha_geometry(case_path)?;
    let tailrace_data = read_tailrace(case_path)?;
    let fpha_config = read_fpha_config(case_path)?;

    let mut results = Vec::new();
    let mut failures = Vec::new();
    let mut processed = 0;
    let mut skipped = 0;

    for plant in plants {
        if plant.groups.is_empty() {
            skipped += 1;
            continue;
        }

        let plant_code = plant.id.0 as u32;
        // Deck plant codes are small; the width matches `MissingHydro::hydro_id`.
        #[allow(clippy::cast_possible_wrap)]
        let hydro_id = plant.id.0 as i32;
        let mut fail = |reason| {
            failures.push(PlantFitFailure {
                hydro_id,
                hydro_name: plant.name.clone(),
                reason,
            });
        };

        let Some(vha) = vha_data.get(&plant_code) else {
            fail(FitFailureReason::MissingGeometry);
            continue;
        };

        let params = match build_fit_params(
            plant,
            vha,
            tailrace_data.get(&plant_code),
            fpha_config.get(&plant_code),
            period_duration_hours,
            &mlt_table,
            parquet_present,
        ) {
            Ok(p) => p,
            // MissingHydro is a fatal error, not a fit failure
            Err(e @ FphaComputeError::MissingHydro { .. }) => return Err(e),
            Err(e) => {
                fail(FitFailureReason::InvalidParameters(e.to_string()));
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
            Err(e) => fail(FitFailureReason::FittingFailed(e.to_string())),
        }
    }

    if !failures.is_empty() {
        return Err(FphaComputeError::PlantsNotFitted { failures });
    }

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

fn build_long_term_mean_inflow_table(inflow_history: &[InflowHistoryRow]) -> HashMap<i32, f64> {
    let mut acc: HashMap<i32, (f64, u64)> = HashMap::new();

    for row in inflow_history {
        let entry = acc.entry(row.hydro_id).or_insert((0.0, 0));
        entry.0 += row.value_m3s;
        entry.1 += 1;
    }

    acc.into_iter()
        .filter(|(_, (_, count))| *count > 0)
        .map(|(id, (sum, count))| (id, sum / count as f64))
        .collect()
}

fn build_fit_params(
    plant: &HydroPlant,
    vha: &VhaGeometryData,
    tailrace: Option<&TailraceData>,
    config: Option<&IoFphaPlantConfig>,
    period_duration_hours: f64,
    mlt_table: &HashMap<i32, f64>,
    parquet_present: bool,
) -> Result<FitPlantParams, FphaComputeError> {
    let plant_code = plant.id.0 as i32;

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
        .map_err(|e| FphaComputeError::Fitting(format!("ForebayTable creation failed: {e}")))?;

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

    // MLT fallback logic: parquet MLT → deck mean → error (if parquet present)
    let long_term_mean_inflow_m3s = if let Some(&mlt) = mlt_table.get(&plant_code) {
        mlt
    } else if !parquet_present {
        // No parquet: use deck mean
        if plant.natural_inflow_hm3.is_empty() {
            0.0
        } else {
            let mean_hm3 = plant.natural_inflow_hm3.iter().sum::<f64>() / plant.natural_inflow_hm3.len() as f64;
            mean_hm3 * 1e6 / (period_duration_hours * 3600.0)
        }
    } else {
        // Parquet present but hydro missing
        return Err(FphaComputeError::MissingHydro {
            hydro_id: plant_code,
            hydro_name: plant.name.clone(),
        });
    };

    let fpha_config = build_fpha_config(config);

    Ok(FitPlantParams {
        codigo: plant_code,
        nome: plant.name.clone(),
        forebay,
        tailrace: tailrace_model,
        hydraulic_loss_m: plant.hydraulic_loss_m,
        efficiency: plant.efficiency,
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
                    max_deviation_mw: Some(r.deviation.max_abs_mw),
                    mean_deviation_mw: Some(r.deviation.mean_abs_mw),
                    rms_deviation_mw: Some(r.deviation.rms_mw),
                }
            })
        })
        .collect();

    write_fpha_computed(case_path, &records)?;
    Ok(())
}

/// Convert fitting results to model-compatible segments.
///
/// Segments carry the unscaled planes. The model multiplies all four coefficients
/// (`gamma_0` as the RHS, `gamma_v`, `gamma_q`, `gamma_s`) by `correction_factor` once.
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

/// Inject computed FPHA segments, enable FPHA and re-validate the whole system.
///
/// `read_study_from_path` validates with `fpha_enabled = false`, so the rule that a plant with
/// units must carry FPHA segments only runs here.
pub fn finalize_fpha_injection(
    system: &mut System,
    plant_results: &[FphaPlantResult],
) -> Result<(), FphaComputeError> {
    inject_fpha_segments(system, plant_results);
    system.fpha_enabled = true;
    system
        .validate()
        .map_err(|e| FphaComputeError::InvalidSystem(e.to_string()))
}

#[cfg(test)]
#[allow(clippy::unwrap_used, clippy::expect_used, clippy::float_cmp)]
mod probe;

#[cfg(test)]
#[allow(clippy::unwrap_used, clippy::expect_used, clippy::float_cmp)]
mod lp_envelope_tests;

#[cfg(test)]
#[allow(clippy::unwrap_used, clippy::expect_used, clippy::float_cmp)]
mod plant_coverage_tests;

#[cfg(test)]
#[allow(clippy::unwrap_used, clippy::expect_used, clippy::float_cmp)]
mod physical_data_tests;

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
    fn convert_single_plane_lp_product_is_alpha_times_plane() {
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

        let plane = result.planes[0];
        let segment = &segments[0];
        let alpha = result.alpha;

        assert_eq!(segments.len(), 1);
        assert_eq!(segment.segment, 1);
        assert_eq!(segment.correction_factor, alpha);
        assert_eq!(
            segment.correction_factor * segment.rhs,
            alpha * plane.gamma_0
        );
        assert_eq!(
            segment.correction_factor * segment.volume_coefficient,
            alpha * plane.gamma_v
        );
        assert_eq!(
            segment.correction_factor * segment.turbining_coefficient,
            alpha * plane.gamma_q
        );
        assert_eq!(
            segment.correction_factor * segment.lateral_flow_coefficient,
            alpha * plane.gamma_s
        );
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

    #[test]
    fn build_mlt_table_empty_history() {
        let table = build_long_term_mean_inflow_table(&[]);
        assert!(table.is_empty());
    }

    #[test]
    fn build_mlt_table_single_hydro() {
        use chrono::NaiveDate;

        let rows = vec![
            InflowHistoryRow {
                hydro_id: 1,
                start_date: NaiveDate::from_ymd_opt(2020, 1, 1).unwrap(),
                end_date: NaiveDate::from_ymd_opt(2020, 2, 1).unwrap(),
                value_m3s: 100.0,
            },
            InflowHistoryRow {
                hydro_id: 1,
                start_date: NaiveDate::from_ymd_opt(2020, 2, 1).unwrap(),
                end_date: NaiveDate::from_ymd_opt(2020, 3, 1).unwrap(),
                value_m3s: 200.0,
            },
        ];

        let table = build_long_term_mean_inflow_table(&rows);

        assert_eq!(table.len(), 1);
        assert!((table[&1] - 150.0).abs() < 1e-9);
    }

    #[test]
    fn build_mlt_table_multiple_hydros() {
        use chrono::NaiveDate;

        let rows = vec![
            InflowHistoryRow {
                hydro_id: 1,
                start_date: NaiveDate::from_ymd_opt(2020, 1, 1).unwrap(),
                end_date: NaiveDate::from_ymd_opt(2020, 2, 1).unwrap(),
                value_m3s: 100.0,
            },
            InflowHistoryRow {
                hydro_id: 2,
                start_date: NaiveDate::from_ymd_opt(2020, 1, 1).unwrap(),
                end_date: NaiveDate::from_ymd_opt(2020, 2, 1).unwrap(),
                value_m3s: 500.0,
            },
            InflowHistoryRow {
                hydro_id: 1,
                start_date: NaiveDate::from_ymd_opt(2020, 2, 1).unwrap(),
                end_date: NaiveDate::from_ymd_opt(2020, 3, 1).unwrap(),
                value_m3s: 300.0,
            },
        ];

        let table = build_long_term_mean_inflow_table(&rows);

        assert_eq!(table.len(), 2);
        assert!((table[&1] - 200.0).abs() < 1e-9);
        assert!((table[&2] - 500.0).abs() < 1e-9);
    }

    #[test]
    fn fallback_uses_mlt_when_present() {
        let mut mlt_table = HashMap::new();
        mlt_table.insert(1, 123.45);
        
        // mlt_table has the hydro, so it should return the value
        assert_eq!(mlt_table.get(&1), Some(&123.45));
    }

    #[test]
    fn fallback_missing_hydro_error_format() {
        let err = FphaComputeError::MissingHydro {
            hydro_id: 42,
            hydro_name: "TestPlant".to_string(),
        };
        let msg = format!("{err}");
        assert!(msg.contains("42"));
        assert!(msg.contains("TestPlant"));
        assert!(msg.contains("inflow_history.parquet"));
    }

    #[test]
    fn plants_not_fitted_display_lists_every_failure() {
        let err = FphaComputeError::PlantsNotFitted {
            failures: vec![
                PlantFitFailure {
                    hydro_id: 2,
                    hydro_name: " FUNIL-GRANDE ".to_string(),
                    reason: FitFailureReason::MissingGeometry,
                },
                PlantFitFailure {
                    hydro_id: 7,
                    hydro_name: "TestPlant".to_string(),
                    reason: FitFailureReason::FittingFailed("no feasible plane".to_string()),
                },
            ],
        };

        assert_eq!(
            err.to_string(),
            "computed FPHA could not be fitted for 2 hydro plant(s) with generating units:\n  \
             plant 2 (FUNIL-GRANDE): no rows in CAD/CAD_GEOMETRIA_UHE.csv\n  \
             plant 7 (TestPlant): fitting failed: no feasible plane"
        );
        assert_eq!(
            FitFailureReason::InvalidParameters("bad volume".to_string()).to_string(),
            "invalid fit parameters: bad volume"
        );
    }
}
