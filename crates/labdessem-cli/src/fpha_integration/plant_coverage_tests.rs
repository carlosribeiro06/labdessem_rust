//! Characterization of audit finding HI-03 (FP-01, NU-04): in computed-FPHA mode a plant whose
//! planes cannot be computed is skipped with a warning and keeps no link between generation and
//! water, on the pre-fix pipeline.
//!
//! Each test runs the computed-FPHA sequence on a derived copy of `examples/24Barras`, builds the LP
//! model and counts the `hydro_fpha[...]` and `hydro_generation_turbining_coupling[...]` rows by
//! name prefix. The control test shows the same counts are positive when the geometry is complete,
//! so a zero count in a baseline test is a property of the skipped plant, not of the row-name
//! format. The tests print the counts the baseline evidence report quotes.

use labdessem_core::entities::HydroPlant;
use labdessem_core::system::System;
use labdessem_model::constraints::LinearConstraint;
use labdessem_model::{Model, SolveMode};

use super::probe::{DerivedCase, computed_system};

/// Geometry table the computed FPHA reads, relative to the case directory.
const GEOMETRY_FILE: &str = "CAD/CAD_GEOMETRIA_UHE.csv";

/// Plant whose geometry rows are removed in the partial-geometry cases: FUNIL-GRANDE in
/// `examples/24Barras`, three units.
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

/// Number of unit coupling rows of the plant named `plant_name`, over every unit and period.
fn plant_coupling_row_count(model: &Model, plant_name: &str) -> usize {
    let prefix = format!("hydro_generation_turbining_coupling[p={plant_name},");
    count_with_prefix(
        &model.constraints.hydro_generation_turbining_coupling(),
        &prefix,
    )
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

/// Characterizes audit HI-03 (FP-01, NU-04) on the pre-fix code. Replaced by ticket-014.
///
/// A plant without geometry is skipped, not rejected: `computed_system` returns `Ok`, the plant
/// keeps no FPHA segment, and the model holds neither a `hydro_fpha` row nor a unit coupling row
/// for it. `System::validate` would reject the plant once FPHA is enabled, but production code
/// never calls it after the injection. If the skipped plant raised a typed error, the `Ok`
/// expectation would fail; if it were linked to water, the zero-row assertions would fail.
#[test]
fn baseline_a2_plant_without_geometry_has_no_fpha_rows() {
    // Arrange
    let case = DerivedCase::new("computed").expect("derived case from examples/24Barras");
    let removed = case
        .remove_plant_rows(GEOMETRY_FILE, SKIPPED_PLANT_CODE)
        .expect("remove the plant rows from CAD_GEOMETRIA_UHE.csv");
    eprintln!("baseline_a2[without_geometry] removed_geometry_rows={removed}");
    assert!(
        removed >= 1,
        "plant {SKIPPED_PLANT_CODE} has no rows in {GEOMETRY_FILE}: the case would not be derived"
    );

    // Act
    let (system, result) = computed_system(case.path())
        .expect("computed_system must succeed although plant 2 has no geometry");
    let model = Model::from_system(&system, SolveMode::LinearProgramming);
    let skipped = plant_by_code(&system, SKIPPED_PLANT_CODE);
    let skipped_fpha_rows = fpha_row_count(&model, &skipped.name);
    let skipped_coupling_rows = plant_coupling_row_count(&model, &skipped.name);
    let validation = system.validate();
    eprintln!(
        "baseline_a2[without_geometry] plants_processed={} plants_skipped={} plant={} \
         segments={} hydro_fpha_rows={skipped_fpha_rows} coupling_rows={skipped_coupling_rows}",
        result.plants_processed,
        result.plants_skipped,
        skipped.name,
        skipped.fpha_segments.len(),
    );

    // Assert
    assert_eq!(
        result.plants_skipped, 1,
        "only the plant without geometry must be skipped"
    );
    assert_eq!(
        result.plants_processed,
        HYDRO_PLANT_COUNT - 1,
        "the other plants must still be processed"
    );
    assert_eq!(result.plant_results.len(), HYDRO_PLANT_COUNT - 1);
    assert_eq!(
        skipped.fpha_segments.len(),
        0,
        "plant {} must keep no FPHA segment",
        skipped.name
    );
    assert!(
        !skipped.groups.is_empty(),
        "plant {} must have units: a plant without units has no coupling to lose",
        skipped.name
    );
    assert_eq!(
        skipped_fpha_rows, 0,
        "plant {} must have no hydro_fpha row",
        skipped.name
    );
    assert_eq!(
        skipped_coupling_rows, 0,
        "plant {} must have no hydro_generation_turbining_coupling row",
        skipped.name
    );
    for plant in system
        .hydro_plants
        .iter()
        .filter(|plant| plant.id != skipped.id && !plant.groups.is_empty())
    {
        assert!(
            fpha_row_count(&model, &plant.name) >= 1,
            "plant {} kept its geometry and must keep its hydro_fpha rows: the row-name prefix \
             would match nothing otherwise",
            plant.name
        );
    }
    let error = validation
        .expect_err("a plant with units and no FPHA segment must fail validation with FPHA on");
    let message = error.to_string();
    assert!(
        message.contains("at least one FPHA segment"),
        "unexpected validation error: {message}"
    );
}

/// Characterizes audit HI-03 (FP-01, NU-04) on the pre-fix code. Replaced by ticket-014.
///
/// Without the geometry table `compute_fpha` returns early with every plant skipped (a different
/// warning from the per-plant one), and the model keeps no generation-water link for any plant.
/// If a missing table raised a typed error, the `Ok` expectation would fail.
#[test]
fn baseline_a2_missing_geometry_file_skips_every_plant() {
    // Arrange
    let case = DerivedCase::new("computed").expect("derived case from examples/24Barras");
    case.remove_file(GEOMETRY_FILE)
        .expect("remove CAD_GEOMETRIA_UHE.csv from the copy");

    // Act
    let (system, result) = computed_system(case.path())
        .expect("computed_system must succeed although the geometry file is missing");
    let model = Model::from_system(&system, SolveMode::LinearProgramming);
    let fpha_rows = model.constraints.hydro_fpha().len();
    let coupling_rows = model
        .constraints
        .hydro_generation_turbining_coupling()
        .len();
    let injected_segments: usize = system
        .hydro_plants
        .iter()
        .map(|plant| plant.fpha_segments.len())
        .sum();
    eprintln!(
        "baseline_a2[missing_file] plants_processed={} plants_skipped={} hydro_plants={} \
         plant_results={} injected_segments={injected_segments} hydro_fpha_rows={fpha_rows} \
         coupling_rows={coupling_rows}",
        result.plants_processed,
        result.plants_skipped,
        system.hydro_plants.len(),
        result.plant_results.len(),
    );

    // Assert
    assert!(
        result.plant_results.is_empty(),
        "no plant can be fitted without geometry"
    );
    assert_eq!(result.plants_processed, 0);
    assert_eq!(
        result.plants_skipped,
        system.hydro_plants.len(),
        "every hydro plant must be counted as skipped"
    );
    assert_eq!(result.plants_skipped, HYDRO_PLANT_COUNT);
    assert_eq!(injected_segments, 0, "no FPHA segment can be injected");
    assert_eq!(fpha_rows, 0, "the model must hold no hydro_fpha row");
    assert_eq!(
        coupling_rows, 0,
        "the model must hold no hydro_generation_turbining_coupling row"
    );
}

/// Control for the A2 baselines: with the complete geometry no plant is skipped and every plant
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
