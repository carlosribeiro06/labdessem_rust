//! Plant coverage of the computed FPHA (audit finding HI-03, FP-01, NU-04): a plant with generating
//! units whose planes cannot be fitted fails `compute_fpha` with `PlantsNotFitted`, and a plant
//! without units is excluded from fitting.
//!
//! Each test runs `compute_fpha` on a derived copy of `examples/24Barras`. The control test runs the
//! computed-FPHA sequence, builds the LP model and counts the `hydro_fpha[...]` and
//! `hydro_generation_turbining_coupling[...]` rows by name prefix, so the failure tests are a
//! property of the missing input, not of the row-name format.

use std::error::Error;
use std::path::Path;

use labdessem_core::entities::HydroPlant;
use labdessem_core::system::System;
use labdessem_io::read_study_from_path;
use labdessem_model::constraints::LinearConstraint;
use labdessem_model::{Model, SolveMode};

use super::probe::{DerivedCase, computed_system};
use super::{FitFailureReason, FphaComputeError, PlantFitFailure, compute_fpha};

/// Geometry table the computed FPHA reads, relative to the case directory.
const GEOMETRY_FILE: &str = "CAD/CAD_GEOMETRIA_UHE.csv";

/// Unit table (one row per plant and unit group), relative to the case directory.
const UNITS_FILE: &str = "CAD/CAD_CONJ_UHE.csv";

/// Plant whose rows are removed in the derived cases: FUNIL-GRANDE in `examples/24Barras`, three
/// units.
const SKIPPED_PLANT_CODE: u32 = 2;

/// Hydro plants of `examples/24Barras`.
const HYDRO_PLANT_COUNT: usize = 4;

fn plant_by_code(system: &System, code: u32) -> &HydroPlant {
    let id = usize::try_from(code).expect("plant code fits usize");
    system
        .hydro_plants
        .iter()
        .find(|plant| plant.id.0 == id)
        .expect("the 24Barras system has a hydro plant with the requested code")
}

fn count_with_prefix(rows: &[&LinearConstraint], prefix: &str) -> usize {
    rows.iter()
        .filter(|row| row.name.starts_with(prefix))
        .count()
}

/// Number of `hydro_fpha` rows of the plant named `plant_name`, over every segment and period.
fn fpha_row_count(model: &Model, plant_name: &str) -> usize {
    let prefix = format!("hydro_fpha[p={plant_name},");
    count_with_prefix(&model.constraints.hydro_fpha(), &prefix)
}

/// Message and failures of `compute_fpha` on `case`, which must fail with `PlantsNotFitted`.
fn plants_not_fitted(
    case: &Path,
    system: &System,
) -> Result<(String, Vec<PlantFitFailure>), Box<dyn Error>> {
    match compute_fpha(case, system) {
        Err(error) => {
            let message = error.to_string();
            match error {
                FphaComputeError::PlantsNotFitted { failures } => Ok((message, failures)),
                other => Err(format!("expected PlantsNotFitted, got: {other}").into()),
            }
        }
        Ok(result) => Err(format!(
            "compute_fpha must fail, but it fitted {} plants",
            result.plant_results.len()
        )
        .into()),
    }
}

/// Number of coupling rows of one unit over the horizon: the full `p=,g=,u=,` prefix is matched so
/// a unit name that prefixes another one cannot be counted twice.
fn unit_coupling_row_count(
    model: &Model,
    plant_name: &str,
    group_name: &str,
    unit_name: &str,
) -> usize {
    let prefix =
        format!("hydro_generation_turbining_coupling[p={plant_name},g={group_name},u={unit_name},");
    count_with_prefix(
        &model.constraints.hydro_generation_turbining_coupling(),
        &prefix,
    )
}

/// A plant with units and no geometry rows fails the computed FPHA, naming the plant and the
/// geometry table, instead of reaching the model without a link between generation and water.
#[test]
fn plant_without_geometry_fails_with_typed_error() -> Result<(), Box<dyn Error>> {
    // Arrange
    let case = DerivedCase::new("computed")?;
    let removed = case.remove_plant_rows(GEOMETRY_FILE, SKIPPED_PLANT_CODE)?;
    eprintln!("a2[without_geometry] removed_geometry_rows={removed}");
    assert!(
        removed >= 1,
        "plant {SKIPPED_PLANT_CODE} has no rows in {GEOMETRY_FILE}: the case would not be derived"
    );
    let system = read_study_from_path(case.path())?;
    let skipped = plant_by_code(&system, SKIPPED_PLANT_CODE);
    assert!(
        !skipped.groups.is_empty(),
        "plant {} must have units: a plant without units is not fitted",
        skipped.name
    );

    // Act
    let (message, failures) = plants_not_fitted(case.path(), &system)?;
    eprintln!("a2[without_geometry] failures={failures:?}\n{message}");

    // Assert
    assert!(
        matches!(
            failures.as_slice(),
            [PlantFitFailure {
                hydro_id: 2,
                reason: FitFailureReason::MissingGeometry,
                ..
            }]
        ),
        "expected one MissingGeometry failure for plant 2, got {failures:?}"
    );
    assert!(
        message.contains(skipped.name.trim()),
        "the error does not name plant {}: {message}",
        skipped.name
    );
    assert!(
        message.contains(GEOMETRY_FILE),
        "the error does not name {GEOMETRY_FILE}: {message}"
    );
    Ok(())
}

/// Without the geometry table every plant with units is listed in the one error.
#[test]
fn missing_geometry_file_lists_every_plant_with_units() -> Result<(), Box<dyn Error>> {
    // Arrange
    let case = DerivedCase::new("computed")?;
    case.remove_file(GEOMETRY_FILE)?;
    let system = read_study_from_path(case.path())?;

    // Act
    let (message, failures) = plants_not_fitted(case.path(), &system)?;
    eprintln!("a2[missing_file] failures={failures:?}\n{message}");

    // Assert
    let hydro_ids: Vec<i32> = failures.iter().map(|failure| failure.hydro_id).collect();
    assert_eq!(
        hydro_ids,
        [0, 1, 2, 3],
        "one failure per plant, in plant order"
    );
    assert_eq!(failures.len(), HYDRO_PLANT_COUNT);
    assert!(
        failures
            .iter()
            .all(|failure| matches!(failure.reason, FitFailureReason::MissingGeometry)),
        "every failure must be MissingGeometry: {failures:?}"
    );
    assert!(
        message.contains(GEOMETRY_FILE),
        "the error does not name {GEOMETRY_FILE}: {message}"
    );
    Ok(())
}

/// A plant without units needs no FPHA: it is excluded before any geometry lookup, so its missing
/// geometry is not a failure.
#[test]
fn plant_without_units_is_excluded_from_fitting() -> Result<(), Box<dyn Error>> {
    // Arrange
    let case = DerivedCase::new("computed")?;
    let removed_units = case.remove_plant_rows(UNITS_FILE, SKIPPED_PLANT_CODE)?;
    let removed_geometry = case.remove_plant_rows(GEOMETRY_FILE, SKIPPED_PLANT_CODE)?;
    eprintln!(
        "a2[without_units] removed_unit_rows={removed_units} removed_geometry_rows={removed_geometry}"
    );
    assert!(
        removed_units >= 1 && removed_geometry >= 1,
        "plant {SKIPPED_PLANT_CODE} has no rows in {UNITS_FILE} or {GEOMETRY_FILE}: the case would \
         not be derived"
    );
    let system = read_study_from_path(case.path())?;
    let skipped = plant_by_code(&system, SKIPPED_PLANT_CODE);
    assert!(
        skipped.groups.is_empty(),
        "plant {} must have no units after its rows are removed from {UNITS_FILE}",
        skipped.name
    );

    // Act
    let result = compute_fpha(case.path(), &system)?;
    eprintln!(
        "a2[without_units] plants_processed={} plants_skipped={}",
        result.plants_processed, result.plants_skipped
    );

    // Assert
    assert_eq!(result.plant_results.len(), HYDRO_PLANT_COUNT - 1);
    assert_eq!(result.plants_processed, HYDRO_PLANT_COUNT - 1);
    assert!(
        result
            .plant_results
            .iter()
            .all(|plant_result| plant_result.codigo != 2),
        "plant {SKIPPED_PLANT_CODE} has no units and must have no fitted result"
    );
    assert_eq!(result.plants_skipped, 1);
    Ok(())
}

/// Control for the A2 cases: with the complete geometry no plant is skipped and every plant
/// with units keeps both links between generation and water. Stays valid after the fix.
#[test]
fn control_a2_full_geometry_links_every_plant() {
    // Arrange
    let case = DerivedCase::new("computed").expect("derived case from examples/24Barras");

    // Act
    let (system, result) =
        computed_system(case.path()).expect("computed system with the complete geometry");
    let model = Model::from_system(&system, SolveMode::LinearProgramming);
    let horizon = system.horizon.periods;
    eprintln!(
        "control_a2[full_geometry] plants_processed={} plants_skipped={} horizon_periods={horizon}",
        result.plants_processed, result.plants_skipped,
    );

    // Assert
    assert_eq!(result.plants_skipped, 0, "no plant may be skipped");
    assert_eq!(result.plants_processed, HYDRO_PLANT_COUNT);
    let mut plants_with_units = 0;
    for plant in system
        .hydro_plants
        .iter()
        .filter(|plant| !plant.groups.is_empty())
    {
        plants_with_units += 1;
        let fpha_rows = fpha_row_count(&model, &plant.name);
        eprintln!(
            "control_a2[full_geometry] plant={} segments={} hydro_fpha_rows={fpha_rows}",
            plant.name,
            plant.fpha_segments.len(),
        );
        assert!(
            fpha_rows >= 1,
            "plant {} must have at least one hydro_fpha row",
            plant.name
        );
        assert_eq!(
            fpha_rows,
            plant.fpha_segments.len() * horizon,
            "plant {}: one hydro_fpha row per segment and period",
            plant.name
        );
        for group in &plant.groups {
            for unit in &group.units {
                let coupling_rows =
                    unit_coupling_row_count(&model, &plant.name, &group.name, &unit.name);
                eprintln!(
                    "control_a2[full_geometry] plant={} group={} unit={} coupling_rows={coupling_rows}",
                    plant.name, group.name, unit.name,
                );
                assert!(
                    coupling_rows >= 1,
                    "unit {}/{}/{} must have at least one coupling row",
                    plant.name,
                    group.name,
                    unit.name
                );
                assert_eq!(
                    coupling_rows, horizon,
                    "unit {}/{}/{}: one coupling row per period",
                    plant.name, group.name, unit.name
                );
            }
        }
    }
    assert!(
        plants_with_units >= 1,
        "no hydro plant has units: the per-plant checks would pass vacuously"
    );
}
