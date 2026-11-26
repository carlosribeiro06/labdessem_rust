//! FPHA fitting orchestration for computed mode.
//!
//! This module provides the infrastructure for computing FPHA hyperplanes
//! from VHA geometry at runtime when `FPHA: "computed"` is set in study_config.json.

use std::collections::HashMap;
use std::path::Path;

use labdessem_core::entities::HydroFphaSegment;
use labdessem_core::ids::HydroPlantId;
use labdessem_core::system::System;
use labdessem_io::readers::read_fpha_config;
use labdessem_model::production::{
    FitFailureReason, FphaInput, FphaPlantResult, IgnoredVolumeWindow, MissingInflowHistory,
    convert_to_fpha_segments, fit_system,
};

/// Result of the FPHA computation process.
#[derive(Debug)]
pub struct FphaComputeResult {
    pub plant_results: Vec<FphaPlantResult>,
    pub plants_processed: usize,
    /// Plants without generating units (no FPHA needed).
    pub plants_skipped: usize,
}

/// A plant with generating units whose computed FPHA could not be fitted.
#[derive(Debug)]
pub struct PlantFitFailure {
    pub hydro_id: HydroPlantId,
    pub hydro_name: String,
    pub reason: FitFailureReason,
}

/// Error type for FPHA computation.
#[derive(Debug)]
pub enum FphaComputeError {
    Io(String),
    MissingHydro {
        hydro_id: HydroPlantId,
        hydro_name: String,
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
            FphaComputeError::MissingHydro {
                hydro_id: HydroPlantId(hydro_id),
                hydro_name,
            } => {
                write!(
                    f,
                    "hydro {hydro_id} ({hydro_name}) missing from inflow_history.parquet"
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
                        failure.hydro_id.0,
                        failure.hydro_name.trim(),
                        fit_failure_text(&failure.reason)
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

impl From<MissingInflowHistory> for FphaComputeError {
    fn from(e: MissingInflowHistory) -> Self {
        FphaComputeError::MissingHydro {
            hydro_id: e.id,
            hydro_name: e.name,
        }
    }
}

/// The deck file and column that supply `input`.
fn fpha_input_text(input: FphaInput) -> &'static str {
    match input {
        FphaInput::Tailrace => "row in CAD/CAD_TAILRACE.csv",
        FphaInput::Efficiency => "Eficiencia in CAD/CAD_UHE.csv (column absent or empty)",
        FphaInput::HydraulicLoss => "Perda.m in CAD/CAD_UHE.csv (column absent or empty)",
    }
}

/// Why a plant was not fitted, naming the deck files to fix.
fn fit_failure_text(reason: &FitFailureReason) -> String {
    match reason {
        FitFailureReason::MissingGeometry => "no rows in CAD/CAD_GEOMETRIA_UHE.csv".to_string(),
        FitFailureReason::MissingInputs(inputs) => {
            let inputs: Vec<&str> = inputs.iter().map(|&input| fpha_input_text(input)).collect();
            format!("missing inputs: {}", inputs.join("; "))
        }
        FitFailureReason::Fitting(e) => format!("fitting failed: {e}"),
    }
}

/// The warning for a volume window that a single-point VHA table ignores.
fn ignored_window_warning(window: &IgnoredVolumeWindow) -> String {
    let show = |bound: Option<f64>| bound.map_or_else(|| "unset".to_string(), |v| v.to_string());
    format!(
        "plant {} ({}) has a single-point table in CAD/CAD_GEOMETRIA_UHE.csv, so min_volume_pct {} and max_volume_pct {} in CAD/CAD_FPHA_CONFIG.csv are ignored",
        window.id.0,
        window.name.trim(),
        show(window.min_volume_fraction),
        show(window.max_volume_fraction)
    )
}

/// Compute FPHA planes from VHA geometry for every plant with generating units.
///
/// Fails with `PlantsNotFitted`, listing every plant with units that could not be fitted.
pub fn compute_fpha(
    case_path: &Path,
    system: &System,
) -> Result<FphaComputeResult, FphaComputeError> {
    eprintln!("Computing FPHA planes from geometry data...");

    if system.inflow_history.is_empty() {
        eprintln!("Warning: inflow_history.parquet not found; using deck mean for MLT");
    }

    let overrides = read_fpha_config(case_path)?;
    let fit = fit_system(system, &overrides)?;

    let mut results = Vec::new();
    let mut failures = Vec::new();

    for plant in fit.plants {
        if let Some(window) = &plant.ignored_window {
            eprintln!("Warning: {}", ignored_window_warning(window));
        }
        match plant.outcome {
            Ok(result) => {
                eprintln!(
                    "Plant {} ({}): fitted {} planes (α = {:.4})",
                    plant.id.0,
                    plant.name,
                    result.planes.len(),
                    result.alpha
                );
                results.push(result);
            }
            Err(reason) => failures.push(PlantFitFailure {
                hydro_id: plant.id,
                hydro_name: plant.name,
                reason,
            }),
        }
    }

    if !failures.is_empty() {
        return Err(FphaComputeError::PlantsNotFitted { failures });
    }

    let processed = results.len();
    let skipped = fit.plants_without_units;
    eprintln!("FPHA computation complete: {processed} plants processed, {skipped} skipped");

    Ok(FphaComputeResult {
        plant_results: results,
        plants_processed: processed,
        plants_skipped: skipped,
    })
}

/// Inject computed FPHA segments into the system.
pub fn inject_fpha_segments(system: &mut System, plant_results: &[FphaPlantResult]) {
    let segments_by_plant: HashMap<HydroPlantId, Vec<HydroFphaSegment>> = plant_results
        .iter()
        .map(|r| (r.id, convert_to_fpha_segments(r)))
        .collect();

    let mut injected = 0;

    for plant in &mut system.hydro_plants {
        if let Some(segments) = segments_by_plant.get(&plant.id) {
            plant.fpha_segments = segments.clone();
            eprintln!(
                "Injected {} FPHA segments for plant {} ({})",
                segments.len(),
                plant.id.0,
                plant.name
            );
            injected += 1;
        }
    }

    for result in plant_results {
        if !system.hydro_plants.iter().any(|p| p.id == result.id) {
            eprintln!(
                "Warning: Computed FPHA for plant {} but it doesn't exist in system",
                result.id.0
            );
        }
    }

    eprintln!("FPHA injection complete: {injected} plants updated");
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
    use std::collections::HashMap;
    use std::fs;

    use labdessem_core::entities::HydroPlant;
    use labdessem_core::ids::HydroPlantId;
    use labdessem_io::read_study_from_path;
    use labdessem_model::production::{
        FitFailureReason, FphaConfigOverrides, FphaFittingError, FphaInput, FphaPlantResult,
        ignored_volume_window,
    };

    use super::probe::{DerivedCase, computed_system};
    use super::{FphaComputeError, PlantFitFailure, fit_failure_text, ignored_window_warning};

    fn window(min: Option<f64>, max: Option<f64>) -> FphaConfigOverrides {
        FphaConfigOverrides {
            min_volume_fraction: min,
            max_volume_fraction: max,
            ..FphaConfigOverrides::default()
        }
    }

    /// Plant `code` of `examples/24Barras` with its VHA table: ITUTINGA (1) has one point,
    /// FURNAS (3) has a hundred.
    fn plant_with_geometry(code: usize) -> Result<HydroPlant, Box<dyn std::error::Error>> {
        let case = DerivedCase::new("computed")?;
        let plant = read_study_from_path(case.path())?
            .hydro_plants
            .into_iter()
            .find(|p| p.id.0 == code)
            .ok_or("plant not in 24Barras")?;
        Ok(plant)
    }

    /// The warning `compute_fpha` prints for `plant` with `overrides`, if any.
    fn warning(plant: &HydroPlant, overrides: Option<&FphaConfigOverrides>) -> Option<String> {
        ignored_volume_window(plant, overrides)
            .as_ref()
            .map(ignored_window_warning)
    }

    #[test]
    fn ignored_window_warning_names_single_point_plant() -> Result<(), Box<dyn std::error::Error>> {
        let plant = plant_with_geometry(1)?;
        assert_eq!(plant.volume_level_table.as_ref().map(Vec::len), Some(1));

        let both = warning(&plant, Some(&window(Some(0.5), Some(0.6))));
        let max_only = warning(&plant, Some(&window(None, Some(0.6))));

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
        let single = plant_with_geometry(1)?;
        let several = plant_with_geometry(3)?;
        assert!(several.volume_level_table.as_ref().map_or(0, Vec::len) > 1);
        let no_window = FphaConfigOverrides {
            max_planes: Some(4),
            tolerance_deg: Some(1.5),
            ..FphaConfigOverrides::default()
        };

        assert_eq!(warning(&several, Some(&window(Some(0.5), Some(0.6)))), None);
        assert_eq!(warning(&single, Some(&no_window)), None);
        assert_eq!(warning(&single, None), None);
        Ok(())
    }

    const CAMARGOS: HydroPlantId = HydroPlantId(0);
    const FURNAS: HydroPlantId = HydroPlantId(3);
    const CONFIG_HEADER: &str = "codigo_usina;num_planes;tolerance;min_volume_pct;max_volume_pct";

    fn fit_with_config_rows(
        rows: &[&str],
    ) -> Result<HashMap<HydroPlantId, FphaPlantResult>, Box<dyn std::error::Error>> {
        let case = DerivedCase::new("computed")?;
        if !rows.is_empty() {
            let csv = format!("{CONFIG_HEADER}\n{}\n", rows.join("\n"));
            fs::write(case.path().join("CAD/CAD_FPHA_CONFIG.csv"), csv)?;
        }
        let (_, result) = computed_system(case.path())?;
        Ok(result
            .plant_results
            .into_iter()
            .map(|r| (r.id, r))
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
        control: &HashMap<HydroPlantId, FphaPlantResult>,
        configured: &HashMap<HydroPlantId, FphaPlantResult>,
        changed: HydroPlantId,
    ) {
        assert_eq!(control.len(), 4, "24Barras has 4 plants with units");
        assert_eq!(configured.len(), control.len());
        for (id, before) in control {
            let same = bit_pattern(before) == bit_pattern(&configured[id]);
            let (code, changed_code) = (id.0, changed.0);
            if *id == changed {
                assert!(
                    !same,
                    "plant {code} is unchanged: the config row had no effect"
                );
            } else {
                assert!(
                    same,
                    "plant {code} changed although only plant {changed_code} is configured"
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
    fn fallback_missing_hydro_error_format() {
        let err = FphaComputeError::MissingHydro {
            hydro_id: HydroPlantId(42),
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
                    hydro_id: HydroPlantId(2),
                    hydro_name: " FUNIL-GRANDE ".to_string(),
                    reason: FitFailureReason::MissingGeometry,
                },
                PlantFitFailure {
                    hydro_id: HydroPlantId(7),
                    hydro_name: "TestPlant".to_string(),
                    reason: FitFailureReason::Fitting(FphaFittingError::NoHyperplanesProduced {
                        hydro_name: "TestPlant".to_string(),
                    }),
                },
            ],
        };

        assert_eq!(
            err.to_string(),
            "computed FPHA could not be fitted for 2 hydro plant(s) with generating units:\n  \
             plant 2 (FUNIL-GRANDE): no rows in CAD/CAD_GEOMETRIA_UHE.csv\n  \
             plant 7 (TestPlant): fitting failed: hydro TestPlant: no hyperplanes produced"
        );
    }

    #[test]
    fn missing_inputs_display_lists_every_input() {
        let reason =
            FitFailureReason::MissingInputs(vec![FphaInput::Efficiency, FphaInput::HydraulicLoss]);

        assert_eq!(
            fit_failure_text(&reason),
            "missing inputs: Eficiencia in CAD/CAD_UHE.csv (column absent or empty); \
             Perda.m in CAD/CAD_UHE.csv (column absent or empty)"
        );
    }
}
