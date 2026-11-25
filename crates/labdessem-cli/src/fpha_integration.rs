//! FPHA fitting orchestration for computed mode.
//!
//! This module provides the infrastructure for computing FPHA hyperplanes
//! from VHA geometry at runtime when `FPHA: "computed"` is set in study_config.json.

use std::collections::HashMap;
use std::path::Path;

use labdessem_core::entities::{HydroFphaSegment, HydroPlant, InflowHistoryRow};
use labdessem_core::system::System;
use labdessem_fpha::{
    FitPlantParams, FphaPlantConfig, FphaPlantResult,
    config::{
        DEFAULT_ANGLE_TOLERANCE_DEG, DEFAULT_FLOW_POINTS, DEFAULT_MAX_PLANES, DEFAULT_VOLUME_POINTS,
    },
    fit_plant,
    fitting::PlaneReductionConfig,
};
use labdessem_io::{
    output::{ComputedFphaRecord, write_fpha_computed},
    readers::{
        FphaPlantConfig as IoFphaPlantConfig, TailraceData, VhaGeometryData, read_fpha_config,
        read_tailrace, read_vha_geometry,
    },
};
use labdessem_model::hydro_geometry::{ForebayTable, TailraceModel, VhaRow};

/// Result of the FPHA computation process.
#[derive(Debug)]
pub struct FphaComputeResult {
    pub plant_results: Vec<FphaPlantResult>,
    pub plants_processed: usize,
    /// Plants without generating units (no FPHA needed).
    pub plants_skipped: usize,
}

/// A physical input that computed FPHA requires for every plant with generating units.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FphaInput {
    Tailrace,
    Efficiency,
    HydraulicLoss,
}

impl std::fmt::Display for FphaInput {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            FphaInput::Tailrace => write!(f, "row in CAD/CAD_TAILRACE.csv"),
            FphaInput::Efficiency => {
                write!(f, "Eficiencia in CAD/CAD_UHE.csv (column absent or empty)")
            }
            FphaInput::HydraulicLoss => {
                write!(f, "Perda.m in CAD/CAD_UHE.csv (column absent or empty)")
            }
        }
    }
}

fn join_inputs(inputs: &[FphaInput]) -> String {
    inputs
        .iter()
        .map(ToString::to_string)
        .collect::<Vec<_>>()
        .join("; ")
}

/// Why the computed FPHA of a plant with generating units could not be fitted.
#[derive(Debug)]
pub enum FitFailureReason {
    MissingGeometry,
    MissingInputs(Vec<FphaInput>),
    InvalidParameters(String),
    FittingFailed(String),
}

impl std::fmt::Display for FitFailureReason {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            FitFailureReason::MissingGeometry => {
                write!(f, "no rows in CAD/CAD_GEOMETRIA_UHE.csv")
            }
            FitFailureReason::MissingInputs(inputs) => {
                write!(f, "missing inputs: {}", join_inputs(inputs))
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
    MissingHydro {
        hydro_id: i32,
        hydro_name: String,
    },
    MissingFphaInputs {
        hydro_id: i32,
        hydro_name: String,
        inputs: Vec<FphaInput>,
    },
    PlantsNotFitted {
        failures: Vec<PlantFitFailure>,
    },
    InvalidSystem(String),
}

impl std::fmt::Display for FphaComputeError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            FphaComputeError::Io(msg) => write!(f, "I/O error: {msg}"),
            FphaComputeError::Fitting(msg) => write!(f, "Fitting error: {msg}"),
            FphaComputeError::MissingHydro {
                hydro_id,
                hydro_name,
            } => {
                write!(
                    f,
                    "hydro {hydro_id} ({hydro_name}) missing from inflow_history.parquet"
                )
            }
            FphaComputeError::MissingFphaInputs {
                hydro_id,
                hydro_name,
                inputs,
            } => {
                write!(
                    f,
                    "hydro {hydro_id} ({}): missing inputs: {}",
                    hydro_name.trim(),
                    join_inputs(inputs)
                )
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
            Err(FphaComputeError::MissingFphaInputs { inputs, .. }) => {
                fail(FitFailureReason::MissingInputs(inputs));
                continue;
            }
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

    let (Some(tailrace), Some(efficiency), Some(hydraulic_loss_m)) =
        (tailrace, plant.efficiency, plant.hydraulic_loss_m)
    else {
        let mut missing_inputs = Vec::new();
        if tailrace.is_none() {
            missing_inputs.push(FphaInput::Tailrace);
        }
        if plant.efficiency.is_none() {
            missing_inputs.push(FphaInput::Efficiency);
        }
        if plant.hydraulic_loss_m.is_none() {
            missing_inputs.push(FphaInput::HydraulicLoss);
        }
        return Err(FphaComputeError::MissingFphaInputs {
            hydro_id: plant_code,
            hydro_name: plant.name.clone(),
            inputs: missing_inputs,
        });
    };

    let vha_rows: Vec<VhaRow> = vha
        .points
        .iter()
        .map(|p| VhaRow {
            volume_hm3: p.volume_hm3,
            height_m: p.height_m,
        })
        .collect();

    let forebay = ForebayTable::new(&vha_rows, &plant.name)
        .map_err(|e| FphaComputeError::Fitting(format!("ForebayTable creation failed: {e}")))?;

    let tailrace_model = TailraceModel {
        coefficients: tailrace.coefficients.to_vec(),
    };

    let max_turbined_m3s = plant
        .groups
        .iter()
        .map(|g| g.units.iter().map(|u| u.max_turbining_hm3).sum::<f64>())
        .sum::<f64>()
        * 1e6
        / (period_duration_hours * 3600.0);

    let max_generation_mw = plant
        .groups
        .iter()
        .map(|g| g.units.iter().map(|u| u.max_generation_mw).sum::<f64>())
        .sum();

    // MLT fallback logic: parquet MLT → deck mean → error (if parquet present)
    let long_term_mean_inflow_m3s = if let Some(&mlt) = mlt_table.get(&plant_code) {
        mlt
    } else if !parquet_present {
        if plant.natural_inflow_hm3.is_empty() {
            0.0
        } else {
            let mean_hm3 = plant.natural_inflow_hm3.iter().sum::<f64>()
                / plant.natural_inflow_hm3.len() as f64;
            mean_hm3 * 1e6 / (period_duration_hours * 3600.0)
        }
    } else {
        return Err(FphaComputeError::MissingHydro {
            hydro_id: plant_code,
            hydro_name: plant.name.clone(),
        });
    };

    if let Some(msg) = ignored_window_warning(plant, vha, config) {
        eprintln!("Warning: {msg}");
    }

    let fpha_config = build_fpha_config(
        config,
        (
            plant.reservoir.min_volume_hm3,
            plant.reservoir.max_volume_hm3,
        ),
    );

    Ok(FitPlantParams {
        codigo: plant_code,
        nome: plant.name.clone(),
        forebay,
        tailrace: tailrace_model,
        hydraulic_loss_m,
        efficiency,
        max_turbined_m3s,
        max_generation_mw,
        long_term_mean_inflow_m3s,
        config: fpha_config,
    })
}

fn build_fpha_config(
    config: Option<&IoFphaPlantConfig>,
    reference_hm3: (f64, f64),
) -> FphaPlantConfig {
    let mut fpha_config = FphaPlantConfig::computed(
        DEFAULT_MAX_PLANES,
        DEFAULT_VOLUME_POINTS,
        DEFAULT_FLOW_POINTS,
        PlaneReductionConfig::Angle {
            tolerance_deg: DEFAULT_ANGLE_TOLERANCE_DEG,
        },
    );
    fpha_config.window_reference_hm3 = Some(reference_hm3);

    if let Some(c) = config {
        if let Some(n) = c.num_planes {
            fpha_config.max_planes = n;
        }
        if let Some(tolerance_deg) = c.tolerance {
            fpha_config.reduction_config = PlaneReductionConfig::Angle { tolerance_deg };
        }
        if let Some(min_fraction) = c.min_volume_pct {
            fpha_config.volume_window.min_fraction = min_fraction;
        }
        if let Some(max_fraction) = c.max_volume_pct {
            fpha_config.volume_window.max_fraction = max_fraction;
        }
    }

    fpha_config
}

/// `fit_context` ignores the volume window of a single-point geometry table.
fn ignored_window_warning(
    plant: &HydroPlant,
    vha: &VhaGeometryData,
    config: Option<&IoFphaPlantConfig>,
) -> Option<String> {
    let config = config?;
    if vha.points.len() != 1 || (config.min_volume_pct.is_none() && config.max_volume_pct.is_none())
    {
        return None;
    }
    let show = |bound: Option<f64>| bound.map_or_else(|| "unset".to_string(), |v| v.to_string());
    Some(format!(
        "plant {} ({}) has a single-point table in CAD/CAD_GEOMETRIA_UHE.csv, so min_volume_pct {} and max_volume_pct {} in CAD/CAD_FPHA_CONFIG.csv are ignored",
        plant.id.0,
        plant.name.trim(),
        show(config.min_volume_pct),
        show(config.max_volume_pct)
    ))
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
            let name = plant_names.get(&r.codigo).copied().unwrap_or("UNKNOWN");
            r.planes
                .iter()
                .enumerate()
                .map(move |(idx, plane)| ComputedFphaRecord {
                    codigo_usina: r.codigo as u32,
                    nome_usina: name.to_string(),
                    plane_index: idx,
                    gamma_0: plane.gamma_0,
                    gamma_v: plane.gamma_v,
                    gamma_q: plane.gamma_q,
                    gamma_s: plane.gamma_s,
                    alpha: r.alpha,
                    max_deviation_mw: Some(r.deviation.max_abs_mw),
                    mean_deviation_mw: Some(r.deviation.mean_abs_mw),
                    rms_deviation_mw: Some(r.deviation.rms_mw),
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
pub fn inject_fpha_segments(system: &mut System, plant_results: &[FphaPlantResult]) {
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

    eprintln!("FPHA injection complete: {} plants updated", injected);
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
#[allow(clippy::unwrap_used, clippy::expect_used, clippy::float_cmp)]
mod golden_tests;

#[cfg(test)]
#[allow(clippy::unwrap_used, clippy::expect_used)]
mod tests {
    use std::fs;

    use labdessem_fpha::{FphaMode, config::VolumeWindow};
    use labdessem_io::read_study_from_path;

    use super::probe::{DerivedCase, computed_system};
    use super::*;

    #[test]
    fn build_fpha_config_uses_adr028_defaults() {
        let config = build_fpha_config(None, (120.0, 792.0));

        assert_eq!(config.mode, FphaMode::Computed);
        assert_eq!(config.max_planes, 10);
        assert_eq!(config.volume_points, 5);
        assert_eq!(config.flow_points, 5);
        assert!(matches!(
            config.reduction_config,
            PlaneReductionConfig::Angle { tolerance_deg } if tolerance_deg == 1.0
        ));
        assert_eq!(config.volume_window, VolumeWindow::FULL);
        assert_eq!(config.window_reference_hm3, Some((120.0, 792.0)));
    }

    #[test]
    fn build_fpha_config_maps_every_field() {
        let io_config = IoFphaPlantConfig {
            num_planes: Some(7),
            tolerance: Some(0.005),
            min_volume_pct: Some(0.25),
            max_volume_pct: Some(0.75),
        };

        let config = build_fpha_config(Some(&io_config), (100.0, 900.0));

        assert_eq!(config.max_planes, 7);
        assert!(matches!(
            config.reduction_config,
            PlaneReductionConfig::Angle { tolerance_deg } if tolerance_deg == 0.005
        ));
        assert_eq!(
            config.volume_window,
            VolumeWindow {
                min_fraction: 0.25,
                max_fraction: 0.75
            }
        );
        assert_eq!(config.window_reference_hm3, Some((100.0, 900.0)));
        assert_eq!(config.volume_points, 5);
        assert_eq!(config.flow_points, 5);

        let min_only = IoFphaPlantConfig {
            min_volume_pct: Some(0.3),
            ..IoFphaPlantConfig::default()
        };
        assert_eq!(
            build_fpha_config(Some(&min_only), (100.0, 900.0)).volume_window,
            VolumeWindow {
                min_fraction: 0.3,
                max_fraction: 1.0
            }
        );
    }

    fn window(min: Option<f64>, max: Option<f64>) -> IoFphaPlantConfig {
        IoFphaPlantConfig {
            min_volume_pct: min,
            max_volume_pct: max,
            ..IoFphaPlantConfig::default()
        }
    }

    /// Plant `code` of `examples/24Barras` with its VHA table: ITUTINGA (1) has one point,
    /// FURNAS (3) has a hundred.
    fn plant_with_geometry(
        code: usize,
    ) -> Result<(HydroPlant, VhaGeometryData), Box<dyn std::error::Error>> {
        let case = DerivedCase::new("computed")?;
        let plant = read_study_from_path(case.path())?
            .hydro_plants
            .into_iter()
            .find(|p| p.id.0 == code)
            .ok_or("plant not in 24Barras")?;
        let vha = read_vha_geometry(case.path())?
            .remove(&u32::try_from(code)?)
            .ok_or("plant without VHA rows")?;
        Ok((plant, vha))
    }

    #[test]
    fn ignored_window_warning_names_single_point_plant() -> Result<(), Box<dyn std::error::Error>> {
        let (plant, vha) = plant_with_geometry(1)?;
        assert_eq!(vha.points.len(), 1);

        let both = ignored_window_warning(&plant, &vha, Some(&window(Some(0.5), Some(0.6))));
        let max_only = ignored_window_warning(&plant, &vha, Some(&window(None, Some(0.6))));

        assert_eq!(
            both.as_deref(),
            Some(
                "plant 1 (ITUTINGA) has a single-point table in CAD/CAD_GEOMETRIA_UHE.csv, so min_volume_pct 0.5 and max_volume_pct 0.6 in CAD/CAD_FPHA_CONFIG.csv are ignored"
            )
        );
        assert_eq!(
            max_only.as_deref(),
            Some(
                "plant 1 (ITUTINGA) has a single-point table in CAD/CAD_GEOMETRIA_UHE.csv, so min_volume_pct unset and max_volume_pct 0.6 in CAD/CAD_FPHA_CONFIG.csv are ignored"
            )
        );
        Ok(())
    }

    #[test]
    fn ignored_window_warning_is_silent_otherwise() -> Result<(), Box<dyn std::error::Error>> {
        let (single, single_vha) = plant_with_geometry(1)?;
        let (several, several_vha) = plant_with_geometry(3)?;
        assert!(several_vha.points.len() > 1);
        let no_window = IoFphaPlantConfig {
            num_planes: Some(4),
            tolerance: Some(1.5),
            ..IoFphaPlantConfig::default()
        };

        assert_eq!(
            ignored_window_warning(&several, &several_vha, Some(&window(Some(0.5), Some(0.6)))),
            None
        );
        assert_eq!(
            ignored_window_warning(&single, &single_vha, Some(&no_window)),
            None
        );
        assert_eq!(ignored_window_warning(&single, &single_vha, None), None);
        Ok(())
    }

    const CAMARGOS: i32 = 0;
    const FURNAS: i32 = 3;
    const CONFIG_HEADER: &str = "codigo_usina;num_planes;tolerance;min_volume_pct;max_volume_pct";

    fn fit_with_config_rows(
        rows: &[&str],
    ) -> Result<HashMap<i32, FphaPlantResult>, Box<dyn std::error::Error>> {
        let case = DerivedCase::new("computed")?;
        if !rows.is_empty() {
            let csv = format!("{CONFIG_HEADER}\n{}\n", rows.join("\n"));
            fs::write(case.path().join("CAD/CAD_FPHA_CONFIG.csv"), csv)?;
        }
        let (_, result) = computed_system(case.path())?;
        Ok(result
            .plant_results
            .into_iter()
            .map(|r| (r.codigo, r))
            .collect())
    }

    fn bit_pattern(result: &FphaPlantResult) -> Vec<u64> {
        let d = &result.deviation;
        result
            .planes
            .iter()
            .flat_map(|p| [p.gamma_0, p.gamma_v, p.gamma_q, p.gamma_s])
            .chain([
                result.alpha,
                d.mean_abs_mw,
                d.max_abs_mw,
                d.rms_mw,
                d.mean_signed_mw,
                d.relative,
            ])
            .map(f64::to_bits)
            .collect()
    }

    fn assert_only_plant_changed(
        control: &HashMap<i32, FphaPlantResult>,
        configured: &HashMap<i32, FphaPlantResult>,
        changed: i32,
    ) {
        assert_eq!(control.len(), 4, "24Barras has 4 plants with units");
        assert_eq!(configured.len(), control.len());
        for (code, before) in control {
            let same = bit_pattern(before) == bit_pattern(&configured[code]);
            if *code == changed {
                assert!(
                    !same,
                    "plant {code} is unchanged: the config row had no effect"
                );
            } else {
                assert!(
                    same,
                    "plant {code} changed although only plant {changed} is configured"
                );
            }
        }
    }

    #[test]
    fn config_tolerance_changes_only_its_plant() -> Result<(), Box<dyn std::error::Error>> {
        let control = fit_with_config_rows(&[])?;

        let configured = fit_with_config_rows(&["3;;5.0;;"])?;

        assert_only_plant_changed(&control, &configured, FURNAS);
        Ok(())
    }

    #[test]
    fn config_window_changes_only_its_plant() -> Result<(), Box<dyn std::error::Error>> {
        let control = fit_with_config_rows(&[])?;

        let configured = fit_with_config_rows(&["3;;;0.5;0.6"])?;

        assert_only_plant_changed(&control, &configured, FURNAS);
        Ok(())
    }

    #[test]
    fn config_plane_limit_caps_its_plant() -> Result<(), Box<dyn std::error::Error>> {
        let control = fit_with_config_rows(&[])?;
        let limit = 2;
        assert!(
            control[&CAMARGOS].planes.len() > limit,
            "the control fit must exceed the limit for the cap to be observable"
        );

        let configured = fit_with_config_rows(&["0;2;;;"])?;

        assert!(configured[&CAMARGOS].planes.len() <= limit);
        assert_only_plant_changed(&control, &configured, CAMARGOS);
        Ok(())
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
                RawPlane {
                    gamma_0: 100.0,
                    gamma_v: 0.1,
                    gamma_q: 0.2,
                    gamma_s: 0.3,
                },
                RawPlane {
                    gamma_0: 200.0,
                    gamma_v: 0.4,
                    gamma_q: 0.5,
                    gamma_s: 0.6,
                },
                RawPlane {
                    gamma_0: 300.0,
                    gamma_v: 0.7,
                    gamma_q: 0.8,
                    gamma_s: 0.9,
                },
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

    #[test]
    fn missing_inputs_display_lists_every_input() {
        let reason =
            FitFailureReason::MissingInputs(vec![FphaInput::Efficiency, FphaInput::HydraulicLoss]);

        assert_eq!(
            reason.to_string(),
            "missing inputs: Eficiencia in CAD/CAD_UHE.csv (column absent or empty); \
             Perda.m in CAD/CAD_UHE.csv (column absent or empty)"
        );
    }
}
