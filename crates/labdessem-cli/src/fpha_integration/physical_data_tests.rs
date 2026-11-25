//! Computed FPHA rejects missing physical data (audit finding FP-R02 and the missing tailrace).
//!
//! Computed FPHA builds each plant's production function from `CAD/CAD_UHE.csv` (`Eficiencia`,
//! `Perda.m`) and `CAD/CAD_TAILRACE.csv`. A plant with generating units that lacks any of the three
//! inputs fails `compute_fpha` with `PlantsNotFitted`, listing the plant and every missing input,
//! while an explicit `Perda.m = 0` stays valid. Each test edits a derived copy of
//! `examples/24Barras`. The control test shows the official inputs carry the data.
//!
//! The geometry and tailrace readers run on every study read, so a non-finite value or a duplicate
//! tailrace row fails `read_study_from_path` before any fitting.

use std::collections::BTreeMap;
use std::error::Error;

use labdessem_core::entities::HydroPlant;
use labdessem_core::system::System;
use labdessem_io::read_study_from_path;

use super::probe::{DerivedCase, FitContext, computed_system, fit_contexts};
use super::{FitFailureReason, FphaComputeError, FphaInput, PlantFitFailure, compute_fpha};

/// Tailrace table the computed FPHA reads, relative to the case directory.
const TAILRACE_FILE: &str = "CAD/CAD_TAILRACE.csv";

/// Plant table carrying the `Eficiencia` and `Perda.m` columns, relative to the case directory.
const PLANTS_FILE: &str = "CAD/CAD_UHE.csv";

/// Volume-height-area table the computed FPHA reads, relative to the case directory.
const GEOMETRY_FILE: &str = "CAD/CAD_GEOMETRIA_UHE.csv";

/// Plant whose tailrace row is removed: FURNAS in `examples/24Barras`.
const TAILRACE_PLANT_CODE: u32 = 3;

/// Plant whose `Perda.m` is set to an explicit zero: CAMARGOS in `examples/24Barras`.
const ZERO_LOSS_PLANT_CODE: u32 = 0;

/// Plant whose tailrace `a0.m` is set to `NaN`: ITUTINGA in `examples/24Barras`.
const NON_FINITE_TAILRACE_PLANT_CODE: u32 = 1;

/// Plant whose geometry `cota.m` is set to `NaN`: CAMARGOS in `examples/24Barras`.
const NON_FINITE_GEOMETRY_PLANT_CODE: u32 = 0;

/// Hydro plants of `examples/24Barras`.
const HYDRO_PLANT_COUNT: usize = 4;

/// One computed-FPHA run on a derived case: the system with the injected segments and the fit
/// context of every fitted plant, as the `labdessem` binary would build them.
struct CaseRun {
    system: System,
    contexts: BTreeMap<i32, FitContext>,
    plants_processed: usize,
    plants_skipped: usize,
}

impl CaseRun {
    fn new(case: &DerivedCase) -> Result<Self, Box<dyn Error>> {
        let (system, result) = computed_system(case.path())?;
        let contexts = fit_contexts(case.path(), &system)?;
        Ok(Self {
            plants_processed: result.plants_processed,
            plants_skipped: result.plants_skipped,
            system,
            contexts,
        })
    }

    fn context(&self, code: u32) -> &FitContext {
        let key = i32::try_from(code).expect("plant code fits i32");
        assert!(
            self.contexts.contains_key(&key),
            "no fit context for plant {code}"
        );
        &self.contexts[&key]
    }
}

fn code_of(plant: &HydroPlant) -> u32 {
    u32::try_from(plant.id.0).expect("plant code fits u32")
}

/// Asserts that every hydro plant of `system` carries both `efficiency` and `hydraulic_loss_m`.
fn assert_physical_data_present(system: &System) {
    assert_eq!(
        system.hydro_plants.len(),
        HYDRO_PLANT_COUNT,
        "24Barras has {HYDRO_PLANT_COUNT} hydro plants"
    );
    for plant in &system.hydro_plants {
        assert!(
            plant.efficiency.is_some(),
            "plant {} ({}): efficiency is {:?}, expected it present",
            plant.id.0,
            plant.name,
            plant.efficiency
        );
        assert!(
            plant.hydraulic_loss_m.is_some(),
            "plant {} ({}): hydraulic_loss_m is {:?}, expected it present",
            plant.id.0,
            plant.name,
            plant.hydraulic_loss_m
        );
    }
}

/// Failures of `compute_fpha` on `case`, which must end in `PlantsNotFitted`.
fn fit_failures(case: &DerivedCase) -> Result<Vec<PlantFitFailure>, Box<dyn Error>> {
    let system = read_study_from_path(case.path())?;
    match compute_fpha(case.path(), &system) {
        Err(FphaComputeError::PlantsNotFitted { failures }) => Ok(failures),
        Err(other) => Err(other.into()),
        Ok(result) => Err(format!(
            "compute_fpha fitted {} plants, expected PlantsNotFitted",
            result.plants_processed
        )
        .into()),
    }
}

/// The inputs of a `MissingInputs` failure.
fn missing_inputs(failure: &PlantFitFailure) -> Result<&[FphaInput], Box<dyn Error>> {
    match &failure.reason {
        FitFailureReason::MissingInputs(inputs) => Ok(inputs),
        other => Err(format!(
            "plant {}: expected MissingInputs, got {other:?}",
            failure.hydro_id
        )
        .into()),
    }
}

/// A plant without a `CAD_TAILRACE.csv` row is rejected, not fitted with a 0 m tailrace level.
#[test]
fn missing_tailrace_row_fails_fitting() -> Result<(), Box<dyn Error>> {
    // Arrange
    let case = DerivedCase::new("computed")?;
    let removed = case.remove_plant_rows(TAILRACE_FILE, TAILRACE_PLANT_CODE)?;
    assert_eq!(
        removed, 1,
        "CAD_TAILRACE.csv has exactly one row for plant 3: the case would not be derived"
    );

    // Act
    let failures = fit_failures(&case)?;

    // Assert
    let [failure] = failures.as_slice() else {
        return Err(format!("expected one failure, got {failures:?}").into());
    };
    assert_eq!(failure.hydro_id, 3);
    assert_eq!(missing_inputs(failure)?, [FphaInput::Tailrace]);
    assert!(
        failure.reason.to_string().contains(TAILRACE_FILE),
        "the failure must name {TAILRACE_FILE}: {}",
        failure.reason
    );
    Ok(())
}

/// Without the `Perda.m` and `Eficiencia` columns every plant is rejected, not fitted with
/// efficiency 1.0 and loss 0 m.
#[test]
fn missing_efficiency_and_loss_fail_fitting() -> Result<(), Box<dyn Error>> {
    // Arrange
    let case = DerivedCase::new("computed")?;
    case.drop_columns(PLANTS_FILE, &["Perda.m", "Eficiencia"])?;

    // Act
    let failures = fit_failures(&case)?;

    // Assert
    assert_eq!(failures.len(), HYDRO_PLANT_COUNT, "{failures:?}");
    for failure in &failures {
        assert_eq!(
            missing_inputs(failure)?,
            [FphaInput::Efficiency, FphaInput::HydraulicLoss],
            "plant {}",
            failure.hydro_id
        );
        let message = failure.reason.to_string();
        assert!(
            message.contains("Eficiencia") && message.contains("Perda.m"),
            "plant {}: the failure must name Eficiencia and Perda.m: {message}",
            failure.hydro_id
        );
    }
    Ok(())
}

/// The message of the error `read_study_from_path` returns for `case`.
fn read_error_message(case: &DerivedCase) -> Result<String, Box<dyn Error>> {
    let error = read_study_from_path(case.path())
        .err()
        .ok_or("read_study_from_path accepted the case, expected an error")?;
    Ok(error.to_string())
}

/// A `NaN` tailrace level is rejected at read time, not carried into the net-head model.
#[test]
fn non_finite_tailrace_level_is_rejected() -> Result<(), Box<dyn Error>> {
    // Arrange
    let case = DerivedCase::new("computed")?;
    let changed = case.set_cell(TAILRACE_FILE, NON_FINITE_TAILRACE_PLANT_CODE, "a0.m", "NaN")?;
    assert_eq!(
        changed, 1,
        "CAD_TAILRACE.csv has exactly one row for plant 1"
    );

    // Act
    let message = read_error_message(&case)?;

    // Assert
    for expected in ["CAD_TAILRACE.csv", "plant 1", "a0.m", "NaN"] {
        assert!(
            message.contains(expected),
            "the message must contain {expected}: {message}"
        );
    }
    Ok(())
}

/// A second tailrace row for a plant is rejected at read time, not silently kept as the last one.
#[test]
fn duplicate_tailrace_row_is_rejected() -> Result<(), Box<dyn Error>> {
    // Arrange
    let case = DerivedCase::new("computed")?;
    let copy_line = case.duplicate_plant_row(TAILRACE_FILE, TAILRACE_PLANT_CODE)?;

    // Act
    let message = read_error_message(&case)?;

    // Assert
    let lines = format!("lines {} and {copy_line}", copy_line - 1);
    for expected in ["CAD_TAILRACE.csv", "plant 3", lines.as_str()] {
        assert!(
            message.contains(expected),
            "the message must contain {expected}: {message}"
        );
    }
    Ok(())
}

/// A `NaN` water level is rejected at read time, not carried into the fit.
#[test]
fn non_finite_geometry_level_is_rejected() -> Result<(), Box<dyn Error>> {
    // Arrange
    let case = DerivedCase::new("computed")?;
    let changed = case.set_cell(
        GEOMETRY_FILE,
        NON_FINITE_GEOMETRY_PLANT_CODE,
        "cota.m",
        "NaN",
    )?;
    assert!(changed > 0, "CAD_GEOMETRIA_UHE.csv has rows for plant 0");

    // Act
    let message = read_error_message(&case)?;

    // Assert
    for expected in ["CAD_GEOMETRIA_UHE.csv", "plant 0", "cota.m", "NaN"] {
        assert!(
            message.contains(expected),
            "the message must contain {expected}: {message}"
        );
    }
    Ok(())
}

/// An explicit `Perda.m = 0` is data, not a missing input.
#[test]
fn explicit_zero_loss_is_accepted() -> Result<(), Box<dyn Error>> {
    // Arrange
    let case = DerivedCase::new("computed")?;
    let changed = case.set_cell(PLANTS_FILE, ZERO_LOSS_PLANT_CODE, "Perda.m", "0")?;
    assert_eq!(changed, 1, "CAD_UHE.csv has exactly one row for plant 0");
    let system = read_study_from_path(case.path())?;

    // Act
    let result = compute_fpha(case.path(), &system)?;

    // Assert
    assert_eq!(result.plant_results.len(), HYDRO_PLANT_COUNT);
    Ok(())
}

/// Control for the missing-input tests: the official inputs carry efficiency, hydraulic loss and a
/// tailrace model for every plant with units, so each failure above comes only from the input
/// that test removes.
#[test]
fn control_a3_official_inputs_carry_physical_data() {
    // Arrange
    let case = DerivedCase::new("computed").expect("control case from examples/24Barras");

    // Act
    let run = CaseRun::new(&case).expect("the unmodified case must compute");

    // Assert
    assert_eq!(run.plants_skipped, 0, "the control must skip no plant");
    assert_eq!(run.plants_processed, HYDRO_PLANT_COUNT);
    assert_physical_data_present(&run.system);
    let mut plants_with_groups = 0;
    for plant in run
        .system
        .hydro_plants
        .iter()
        .filter(|plant| !plant.groups.is_empty())
    {
        let code = code_of(plant);
        assert!(
            plant
                .efficiency
                .is_some_and(|efficiency| efficiency > 0.0 && efficiency < 1.0),
            "plant {code} ({}): efficiency {:?} must be in (0, 1), so the 1.0 default differs",
            plant.name,
            plant.efficiency
        );
        assert!(
            plant.hydraulic_loss_m.is_some_and(|loss| loss > 0.0),
            "plant {code} ({}): hydraulic_loss_m {:?} must be positive, so the 0 m default differs",
            plant.name,
            plant.hydraulic_loss_m
        );
        let tailrace = &run.context(code).params.tailrace;
        assert!(
            plant
                .tailrace_coefficients
                .is_some_and(|coefficients| tailrace.coefficients == coefficients),
            "plant {code} ({}): the fit params must carry the plant's tailrace polynomial",
            plant.name
        );
        plants_with_groups += 1;
    }
    assert_eq!(
        plants_with_groups, HYDRO_PLANT_COUNT,
        "every 24Barras hydro plant has units"
    );
}
