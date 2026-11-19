//! Test-only LP envelope probe for computed FPHA.
//!
//! The probe reads the `hydro_fpha[...]` rows that `labdessem_model` builds from a computed system
//! and evaluates the generation cap they imply, independently of the `HydroFphaSegment` structs.
//! It also rebuilds the fit context (`ProductionFunction`, `FittingBounds`) of each plant so tests
//! can compare the LP envelope with the production function on the same grids.
//!
//! Derived cases are copies of `examples/24Barras` inside a temporary directory; the source case
//! is only ever read.

use std::collections::BTreeMap;
use std::error::Error;
use std::fs;
use std::io;
use std::path::{Component, Path, PathBuf};

use labdessem_core::entities::HydroFphaSegment;
use labdessem_core::system::System;
use labdessem_fpha::{
    FitPlantParams, FittingBounds, ProductionFunction, TailraceSource, build_grid,
    compute_alpha_fpha, fit_hull_planes, resolve_fitting_bounds,
};
use labdessem_io::readers::{read_fpha_config, read_tailrace, read_vha_geometry};
use labdessem_io::{read_study_config, read_study_from_path};
use labdessem_model::constraints::{ConstraintSense, LinearConstraint};
use labdessem_model::{Model, SolveMode};
use tempfile::TempDir;

use super::{FphaComputeError, FphaComputeResult};

/// Official example every derived case is copied from. Read-only.
const SOURCE_CASE: &str = concat!(env!("CARGO_MANIFEST_DIR"), "/../../examples/24Barras");

fn fast_study_config(fpha_mode: &str) -> String {
    format!(
        r#"{{
  "opcao_execucao": 2,
  "rede": 0,
  "UCT": 0,
  "UCH": 0,
  "TON_Residual": 0,
  "FPHA": "{fpha_mode}",
  "FCF": 1,
  "EngMax": 0
}}
"#
    )
}

fn invalid_input(message: String) -> io::Error {
    io::Error::new(io::ErrorKind::InvalidInput, message)
}

fn invalid_data(message: String) -> io::Error {
    io::Error::new(io::ErrorKind::InvalidData, message)
}

fn copy_dir_recursive(from: &Path, to: &Path) -> io::Result<()> {
    fs::create_dir_all(to)?;
    for entry in fs::read_dir(from)? {
        let entry = entry?;
        let target = to.join(entry.file_name());
        if entry.file_type()?.is_dir() {
            copy_dir_recursive(&entry.path(), &target)?;
        } else {
            fs::copy(entry.path(), target)?;
        }
    }
    Ok(())
}

/// Splits a line into its content and its trailing `\r\n` / `\n`, so edits preserve line endings.
fn split_terminator(line: &str) -> (&str, &str) {
    let content = line.trim_end_matches(['\r', '\n']);
    (content, &line[content.len()..])
}

/// Temporary copy of `examples/24Barras` (`CAD`, `OPER`) with the fast `study_config.json`.
///
/// Dropping the value deletes the copy. Every edit helper resolves its `relative` path below the
/// copy and rejects absolute paths and `..`, so nothing can reach the source case.
#[derive(Debug)]
pub(super) struct DerivedCase {
    dir: TempDir,
}

impl DerivedCase {
    /// Copies `CAD` and `OPER` and writes the fast configuration with `"FPHA": <fpha_mode>`.
    pub(super) fn new(fpha_mode: &str) -> io::Result<Self> {
        let dir = TempDir::new()?;
        let source = Path::new(SOURCE_CASE);
        for subdir in ["CAD", "OPER"] {
            copy_dir_recursive(&source.join(subdir), &dir.path().join(subdir))?;
        }
        fs::write(
            dir.path().join("study_config.json"),
            fast_study_config(fpha_mode),
        )?;
        Ok(Self { dir })
    }

    pub(super) fn path(&self) -> &Path {
        self.dir.path()
    }

    fn resolve(&self, relative: &str) -> io::Result<PathBuf> {
        let relative_path = Path::new(relative);
        if relative_path
            .components()
            .any(|component| !matches!(component, Component::Normal(_)))
        {
            return Err(invalid_input(format!(
                "{relative}: derived-case paths must be relative and free of `..`"
            )));
        }
        Ok(self.dir.path().join(relative_path))
    }

    /// Drops the data lines whose first `;` field, trimmed, equals `plant_code`; keeps the header.
    /// Returns the number of dropped lines.
    pub(super) fn remove_plant_rows(&self, relative: &str, plant_code: u32) -> io::Result<usize> {
        let path = self.resolve(relative)?;
        let text = fs::read_to_string(&path)?;
        let mut kept = String::with_capacity(text.len());
        let mut removed = 0;
        for (index, line) in text.split_inclusive('\n').enumerate() {
            let (content, _) = split_terminator(line);
            let first_field = content.split(';').next().unwrap_or_default();
            let is_plant_row =
                index > 0 && first_field.trim().parse::<u32>().ok() == Some(plant_code);
            if is_plant_row {
                removed += 1;
            } else {
                kept.push_str(line);
            }
        }
        fs::write(path, kept)?;
        Ok(removed)
    }

    /// Removes the named columns (matched against the trimmed header fields) from every line.
    pub(super) fn drop_columns(&self, relative: &str, columns: &[&str]) -> io::Result<()> {
        let path = self.resolve(relative)?;
        let text = fs::read_to_string(&path)?;
        let header_line = text
            .split_inclusive('\n')
            .next()
            .ok_or_else(|| invalid_data(format!("{relative}: file is empty")))?;
        let header_fields: Vec<&str> = split_terminator(header_line)
            .0
            .split(';')
            .map(str::trim)
            .collect();
        let mut dropped = Vec::with_capacity(columns.len());
        for column in columns {
            let index = header_fields
                .iter()
                .position(|field| field == column)
                .ok_or_else(|| {
                    invalid_input(format!(
                        "{relative}: column {column} not found in the header"
                    ))
                })?;
            dropped.push(index);
        }

        let mut rewritten = String::with_capacity(text.len());
        for line in text.split_inclusive('\n') {
            let (content, terminator) = split_terminator(line);
            if content.trim().is_empty() {
                rewritten.push_str(line);
                continue;
            }
            let fields: Vec<&str> = content.split(';').collect();
            if fields.len() != header_fields.len() {
                return Err(invalid_data(format!(
                    "{relative}: line has {} fields, the header has {}",
                    fields.len(),
                    header_fields.len()
                )));
            }
            let remaining: Vec<&str> = fields
                .iter()
                .enumerate()
                .filter(|(index, _)| !dropped.contains(index))
                .map(|(_, field)| *field)
                .collect();
            rewritten.push_str(&remaining.join(";"));
            rewritten.push_str(terminator);
        }
        fs::write(path, rewritten)
    }

    /// Sets the `column` field (matched against the trimmed header fields) to `value` on every data
    /// line whose first `;` field, trimmed, equals `plant_code`; keeps every other field and the
    /// line terminator. Returns the number of changed lines.
    pub(super) fn set_cell(
        &self,
        relative: &str,
        plant_code: u32,
        column: &str,
        value: &str,
    ) -> io::Result<usize> {
        let path = self.resolve(relative)?;
        let text = fs::read_to_string(&path)?;
        let header_line = text
            .split_inclusive('\n')
            .next()
            .ok_or_else(|| invalid_data(format!("{relative}: file is empty")))?;
        let header_fields: Vec<&str> = split_terminator(header_line)
            .0
            .split(';')
            .map(str::trim)
            .collect();
        let column_index = header_fields
            .iter()
            .position(|field| *field == column)
            .ok_or_else(|| {
                invalid_input(format!(
                    "{relative}: column {column} not found in the header"
                ))
            })?;

        let mut rewritten = String::with_capacity(text.len());
        let mut changed = 0;
        for (index, line) in text.split_inclusive('\n').enumerate() {
            let (content, terminator) = split_terminator(line);
            if index == 0 || content.trim().is_empty() {
                rewritten.push_str(line);
                continue;
            }
            let mut fields: Vec<&str> = content.split(';').collect();
            if fields.len() != header_fields.len() {
                return Err(invalid_data(format!(
                    "{relative}: line has {} fields, the header has {}",
                    fields.len(),
                    header_fields.len()
                )));
            }
            if fields[0].trim().parse::<u32>().ok() == Some(plant_code) {
                fields[column_index] = value;
                changed += 1;
            }
            rewritten.push_str(&fields.join(";"));
            rewritten.push_str(terminator);
        }
        fs::write(path, rewritten)?;
        Ok(changed)
    }

    pub(super) fn remove_file(&self, relative: &str) -> io::Result<()> {
        fs::remove_file(self.resolve(relative)?)
    }
}

/// Runs the computed-FPHA sequence of `run_and_print` in `main.rs` (`compute_fpha`,
/// `finalize_fpha_injection`). `write_computed_results` is not called.
pub(super) fn computed_system(case: &Path) -> Result<(System, FphaComputeResult), Box<dyn Error>> {
    let mut system = read_study_from_path(case)?;
    let result = super::compute_fpha(case, &system)?;
    super::finalize_fpha_injection(&mut system, &result.plant_results)?;
    Ok((system, result))
}

/// One period-1 `hydro_fpha` row rewritten as a generation cap:
/// `g <= rhs_mw + volume_mw_per_hm3 * V + turbining_mw_per_m3s * Q + spillage_mw_per_m3s * S`,
/// with `V` in hm3 and `Q`, `S` in m3/s.
#[derive(Debug, Clone, Copy, PartialEq)]
pub(super) struct EnvelopeRow {
    pub(super) rhs_mw: f64,
    pub(super) volume_mw_per_hm3: f64,
    pub(super) turbining_mw_per_m3s: f64,
    pub(super) spillage_mw_per_m3s: f64,
}

fn coefficients_of(row: &LinearConstraint, variable_prefix: &str) -> Vec<f64> {
    row.terms
        .iter()
        .filter(|term| term.variable.starts_with(variable_prefix))
        .map(|term| term.coefficient)
        .collect()
}

fn single_coefficient(row: &LinearConstraint, variable_prefix: &str) -> f64 {
    let coefficients = coefficients_of(row, variable_prefix);
    assert!(
        coefficients.len() == 1,
        "{}: expected exactly one {variable_prefix} term, found {}",
        row.name,
        coefficients.len()
    );
    coefficients[0]
}

fn envelope_row(row: &LinearConstraint, flow_conversion: f64) -> EnvelopeRow {
    assert!(
        row.sense == ConstraintSense::LessOrEqual,
        "{}: an FPHA row must be an upper bound on generation",
        row.name
    );
    let generation = coefficients_of(row, "hydro_generation[");
    assert!(
        !generation.is_empty() && generation.iter().all(|coefficient| *coefficient == 1.0),
        "{}: every generation term must have coefficient 1",
        row.name
    );
    let turbining = coefficients_of(row, "hydro_turbining[");
    let unit_turbining = *turbining
        .first()
        .expect("an FPHA row must contain at least one turbining term");
    assert!(
        turbining
            .iter()
            .all(|coefficient| *coefficient == unit_turbining),
        "{}: unit turbining coefficients differ: {turbining:?}",
        row.name
    );

    // The row carries turbining and spillage in hm3 per period; `flow_conversion` converts back
    // to m3/s. The volume term references V[t+1] and is evaluated at the caller's `v_hm3`: the
    // probe measures the row's algebra, not a period trajectory.
    EnvelopeRow {
        rhs_mw: row.rhs,
        volume_mw_per_hm3: -single_coefficient(row, "hydro_volume["),
        turbining_mw_per_m3s: -unit_turbining * flow_conversion,
        spillage_mw_per_m3s: -single_coefficient(row, "hydro_spillage[") * flow_conversion,
    }
}

/// Period-1 `hydro_fpha` rows of the plant named `plant_name` (as stored in the `System`).
///
/// `flow_conversion` is `period_duration_hours * 0.0036`, the factor the model uses between
/// m3/s and hm3 per period. A plant without rows yields an empty vector.
pub(super) fn lp_fpha_rows(
    model: &Model,
    plant_name: &str,
    flow_conversion: f64,
) -> Vec<EnvelopeRow> {
    let prefix = format!("hydro_fpha[p={plant_name},");
    model
        .constraints
        .hydro_fpha()
        .into_iter()
        .filter(|row| {
            // Exact `,t=1]` suffix: `t=10` and above must never match.
            row.name
                .strip_prefix(&prefix)
                .is_some_and(|rest| rest.starts_with("seg=") && rest.ends_with(",t=1]"))
        })
        .map(|row| envelope_row(row, flow_conversion))
        .collect()
}

/// Generation cap in MW implied by the rows: the minimum over rows, `f64::INFINITY` when empty.
pub(super) fn envelope_mw(rows: &[EnvelopeRow], v_hm3: f64, q_m3s: f64, s_m3s: f64) -> f64 {
    rows.iter()
        .map(|row| {
            row.rhs_mw
                + row.volume_mw_per_hm3 * v_hm3
                + row.turbining_mw_per_m3s * q_m3s
                + row.spillage_mw_per_m3s * s_m3s
        })
        .fold(f64::INFINITY, f64::min)
}

/// Generation cap in MW of the injected segments: `min(cf * (rhs + vc*v + tc*q + lc*s))`.
pub(super) fn segment_envelope_mw(
    segments: &[HydroFphaSegment],
    v_hm3: f64,
    q_m3s: f64,
    s_m3s: f64,
) -> f64 {
    segments
        .iter()
        .map(|segment| {
            segment.correction_factor
                * (segment.rhs
                    + segment.volume_coefficient * v_hm3
                    + segment.turbining_coefficient * q_m3s
                    + segment.lateral_flow_coefficient * s_m3s)
        })
        .fold(f64::INFINITY, f64::min)
}

/// Inputs `labdessem_fpha::pipeline::fit_plant` derives for one plant.
#[derive(Debug)]
pub(super) struct FitContext {
    pub(super) params: FitPlantParams,
    pub(super) pf: ProductionFunction,
    pub(super) bounds: FittingBounds,
}

/// Fit contexts keyed by plant code. Loads the data as `compute_fpha` does; plants without VHA
/// rows are omitted, every other failure is returned.
pub(super) fn fit_contexts(
    case: &Path,
    system: &System,
) -> Result<BTreeMap<i32, FitContext>, FphaComputeError> {
    let period_duration_hours = system.horizon.period_duration_hours;
    let mlt_table = super::build_long_term_mean_inflow_table(&system.inflow_history);
    let parquet_present = !system.inflow_history.is_empty();
    let vha_data = read_vha_geometry(case)?;
    let tailrace_data = read_tailrace(case)?;
    let fpha_config = read_fpha_config(case)?;

    let mut contexts = BTreeMap::new();
    for plant in &system.hydro_plants {
        let plant_code = plant.id.0 as u32;
        let Some(vha) = vha_data.get(&plant_code) else {
            continue;
        };
        let params = super::build_fit_params(
            plant,
            vha,
            tailrace_data.get(&plant_code),
            fpha_config.get(&plant_code),
            period_duration_hours,
            &mlt_table,
            parquet_present,
        )?;

        // Mirrors labdessem_fpha::pipeline::fit_plant lines 41-67; keep in sync.
        let pf = ProductionFunction::new(
            params.forebay.clone(),
            TailraceSource::Entity(params.tailrace.clone()),
            params.hydraulic_loss_m,
            params.efficiency,
            params.max_turbined_m3s,
            params.max_generation_mw,
            params.nome.clone(),
        );
        let config_bounds = params
            .config
            .to_fitting_bounds(params.forebay.v_min(), params.forebay.v_max());
        let useful_storage_hm3 = params.forebay.v_max() - params.forebay.v_min();
        let bounds = resolve_fitting_bounds(
            params.forebay.v_min(),
            params.forebay.v_max(),
            useful_storage_hm3,
            &config_bounds,
        );

        contexts.insert(params.codigo, FitContext { params, pf, bounds });
    }
    Ok(contexts)
}

/// The `(V hm3, Q m3/s)` nodes of the fit grid: `build_grid` `v_points` x `q_points`.
pub(super) fn fit_grid_nodes(ctx: &FitContext) -> Vec<(f64, f64)> {
    let grid = build_grid(&ctx.pf, &ctx.bounds);
    grid.v_points
        .iter()
        .flat_map(|&v| grid.q_points.iter().map(move |&q| (v, q)))
        .collect()
}

fn linspace(min: f64, max: f64, n: usize) -> Vec<f64> {
    match n {
        0 => Vec::new(),
        1 => vec![min],
        _ => (0..n)
            .map(|i| {
                if i + 1 == n {
                    max
                } else {
                    min + (max - min) * (i as f64) / ((n - 1) as f64)
                }
            })
            .collect(),
    }
}

/// `n` x `n` uniform `(V hm3, Q m3/s)` nodes over `[bounds.v_min, bounds.v_max]` x
/// `[0, pf.max_turbined_m3s()]`, endpoints included.
pub(super) fn evaluation_grid_nodes(ctx: &FitContext, n: usize) -> Vec<(f64, f64)> {
    let volumes = linspace(ctx.bounds.v_min, ctx.bounds.v_max, n);
    let flows = linspace(0.0, ctx.pf.max_turbined_m3s(), n);
    volumes
        .iter()
        .flat_map(|&v| flows.iter().map(move |&q| (v, q)))
        .collect()
}

/// Paths of every file below `root`, relative to `root`, sorted.
fn files_below(root: &Path) -> io::Result<Vec<PathBuf>> {
    let mut files = Vec::new();
    let mut pending = vec![root.to_path_buf()];
    while let Some(dir) = pending.pop() {
        for entry in fs::read_dir(&dir)? {
            let entry = entry?;
            if entry.file_type()?.is_dir() {
                pending.push(entry.path());
            } else {
                let relative = entry
                    .path()
                    .strip_prefix(root)
                    .map_err(io::Error::other)?
                    .to_path_buf();
                files.push(relative);
            }
        }
    }
    files.sort();
    Ok(files)
}

#[test]
fn probe_derived_case_copies_official_example() {
    // Arrange
    let source = Path::new(SOURCE_CASE);

    // Act
    let case = DerivedCase::new("computed").expect("derived case from examples/24Barras");

    // Assert
    for subdir in ["CAD", "OPER"] {
        let source_dir = source.join(subdir);
        let copy_dir = case.path().join(subdir);
        let source_files = files_below(&source_dir).expect("list source files");
        let copied_files = files_below(&copy_dir).expect("list copied files");
        assert!(
            !source_files.is_empty(),
            "examples/24Barras/{subdir} has no files"
        );
        assert_eq!(
            copied_files, source_files,
            "{subdir}: copied file list differs"
        );
        for relative in &source_files {
            let original = fs::read(source_dir.join(relative)).expect("read source file");
            let copy = fs::read(copy_dir.join(relative)).expect("read copied file");
            assert!(
                original == copy,
                "{subdir}/{}: copied bytes differ from the source",
                relative.display()
            );
        }
    }
    let config = read_study_config(case.path().join("study_config.json"))
        .expect("read the copy's study_config.json");
    assert!(
        config.fpha.is_computed(),
        "FPHA must be computed, got {}",
        config.fpha
    );
}

#[test]
fn probe_derived_case_edits_stay_local() {
    // Arrange
    let source = Path::new(SOURCE_CASE);
    let geometry = "CAD/CAD_GEOMETRIA_UHE.csv";
    let plants = "CAD/CAD_UHE.csv";
    let fpha_table = "OPER/OPER_FPHA.csv";
    let snapshots: Vec<(&str, Vec<u8>)> = [geometry, plants, fpha_table]
        .into_iter()
        .map(|relative| {
            let bytes = fs::read(source.join(relative)).expect("snapshot source file");
            (relative, bytes)
        })
        .collect();
    let source_geometry = fs::read_to_string(source.join(geometry)).expect("read source geometry");
    let source_plants = fs::read_to_string(source.join(plants)).expect("read source plants");
    let case = DerivedCase::new("computed").expect("derived case from examples/24Barras");

    // Act
    let removed = case
        .remove_plant_rows(geometry, 2)
        .expect("remove plant 2 rows from CAD_GEOMETRIA_UHE.csv");
    case.drop_columns(plants, &["Perda.m", "Eficiencia"])
        .expect("drop Perda.m and Eficiencia from CAD_UHE.csv");
    case.remove_file(fpha_table)
        .expect("remove OPER_FPHA.csv from the copy");

    // Assert
    assert!(removed > 0, "plant 2 has rows in CAD_GEOMETRIA_UHE.csv");
    let edited_geometry =
        fs::read_to_string(case.path().join(geometry)).expect("read edited geometry");
    assert_eq!(
        edited_geometry.lines().next(),
        source_geometry.lines().next(),
        "the header must be kept"
    );
    assert_eq!(
        edited_geometry.lines().count(),
        source_geometry.lines().count() - removed,
        "only the plant 2 rows may be dropped"
    );
    for line in edited_geometry.lines().skip(1) {
        let first_field = line.split(';').next().unwrap_or_default().trim();
        assert_ne!(first_field, "2", "a plant 2 row survived: {line}");
    }

    let edited_plants = fs::read_to_string(case.path().join(plants)).expect("read edited plants");
    let header: Vec<&str> = edited_plants
        .lines()
        .next()
        .expect("edited CAD_UHE.csv keeps its header")
        .split(';')
        .collect();
    let source_header_width = source_plants
        .lines()
        .next()
        .expect("source CAD_UHE.csv has a header")
        .split(';')
        .count();
    assert!(
        !header.contains(&"Perda.m") && !header.contains(&"Eficiencia"),
        "dropped columns still in the header: {header:?}"
    );
    assert_eq!(header.len(), source_header_width - 2);
    for line in edited_plants.lines() {
        assert_eq!(line.split(';').count(), header.len(), "ragged line: {line}");
    }

    assert!(
        !case.path().join(fpha_table).exists(),
        "OPER_FPHA.csv must be gone from the copy"
    );
    for (relative, bytes) in &snapshots {
        let current =
            fs::read(source.join(relative)).expect("examples/24Barras file must still exist");
        assert!(
            current == *bytes,
            "examples/24Barras/{relative} changed while editing the derived case"
        );
    }
}

#[test]
fn probe_derived_case_sets_a_cell() -> Result<(), Box<dyn Error>> {
    // Arrange
    let plants = "CAD/CAD_UHE.csv";
    let source_path = Path::new(SOURCE_CASE).join(plants);
    let source_bytes = fs::read(&source_path)?;
    let source_text = String::from_utf8(source_bytes.clone())?;
    let source_lines: Vec<&str> = source_text.split_inclusive('\n').collect();
    let header: Vec<&str> = source_lines
        .first()
        .ok_or("source CAD_UHE.csv is empty")?
        .split(';')
        .map(str::trim)
        .collect();
    let column = header
        .iter()
        .position(|field| *field == "Perda.m")
        .ok_or("Perda.m is not in the header of CAD_UHE.csv")?;
    let plant_line = source_lines
        .iter()
        .skip(1)
        .position(|line| line.split(';').next().map(str::trim) == Some("0"))
        .ok_or("CAD_UHE.csv has no row for plant 0")?
        + 1;
    let case = DerivedCase::new("computed")?;
    fs::write(case.path().join("CAD/ragged.csv"), "Codigo;Valor\n0;1;2\n")?;

    // Act
    let changed = case.set_cell(plants, 0, "Perda.m", "0")?;
    let unknown_column = case.set_cell(plants, 0, "NoSuchColumn", "0");
    let ragged_line = case.set_cell("CAD/ragged.csv", 0, "Valor", "9");

    // Assert
    assert_eq!(changed, 1, "plant 0 has exactly one row in CAD_UHE.csv");
    assert!(
        matches!(&unknown_column, Err(error) if error.kind() == io::ErrorKind::InvalidInput),
        "an unknown column must be InvalidInput, got {unknown_column:?}"
    );
    assert!(
        matches!(&ragged_line, Err(error) if error.kind() == io::ErrorKind::InvalidData),
        "a line with the wrong field count must be InvalidData, got {ragged_line:?}"
    );
    let edited_text = fs::read_to_string(case.path().join(plants))?;
    let edited_lines: Vec<&str> = edited_text.split_inclusive('\n').collect();
    assert_eq!(edited_lines.len(), source_lines.len());
    let mut differing_cells = Vec::new();
    for (index, (source_line, edited_line)) in source_lines.iter().zip(&edited_lines).enumerate() {
        let (source_content, source_terminator) = split_terminator(source_line);
        let (edited_content, edited_terminator) = split_terminator(edited_line);
        assert_eq!(
            edited_terminator, source_terminator,
            "line {index}: terminator changed"
        );
        let source_fields: Vec<&str> = source_content.split(';').collect();
        let edited_fields: Vec<&str> = edited_content.split(';').collect();
        assert_eq!(
            edited_fields.len(),
            source_fields.len(),
            "line {index}: field count changed"
        );
        for (cell, (source_field, edited_field)) in
            source_fields.iter().zip(&edited_fields).enumerate()
        {
            if source_field != edited_field {
                differing_cells.push((index, cell, *edited_field));
            }
        }
    }
    assert_eq!(differing_cells, [(plant_line, column, "0")]);
    assert!(
        fs::read(&source_path)? == source_bytes,
        "examples/24Barras/{plants} changed while editing the derived case"
    );
    Ok(())
}

#[test]
fn probe_lp_rows_match_injected_segments() {
    // Arrange
    let case = DerivedCase::new("computed").expect("derived case from examples/24Barras");
    let (system, _result) = computed_system(case.path()).expect("computed system");
    let contexts = fit_contexts(case.path(), &system).expect("fit contexts");
    let model = Model::from_system(&system, SolveMode::LinearProgramming);
    let flow_conversion = system.horizon.period_duration_hours * 0.0036;
    assert_eq!(system.hydro_plants.len(), 4, "24Barras has 4 hydro plants");

    for plant in &system.hydro_plants {
        let code = i32::try_from(plant.id.0).expect("plant code fits i32");
        assert!(
            contexts.contains_key(&code),
            "no fit context for plant {code} ({})",
            plant.name
        );
        let ctx = &contexts[&code];
        // Q_max is the plant total `pf.max_turbined_m3s()`: the axis of `evaluation_grid_nodes`
        // and the spillage axis of `build_grid`.
        let q_max = ctx.pf.max_turbined_m3s();

        // Act
        let rows = lp_fpha_rows(&model, &plant.name, flow_conversion);

        // Assert
        assert!(
            !plant.fpha_segments.is_empty(),
            "plant {} has no injected segments",
            plant.name
        );
        assert_eq!(
            rows.len(),
            plant.fpha_segments.len(),
            "plant {}: period-1 rows vs segments",
            plant.name
        );
        let nodes = fit_grid_nodes(ctx);
        assert!(
            !nodes.is_empty(),
            "plant {} has an empty fit grid",
            plant.name
        );
        for (v, q) in nodes {
            for s in [0.0, 0.5 * q_max, q_max] {
                let from_rows = envelope_mw(&rows, v, q, s);
                let from_segments = segment_envelope_mw(&plant.fpha_segments, v, q, s);
                assert!(
                    (from_rows - from_segments).abs() <= 1e-9 * from_segments.abs().max(1.0),
                    "plant {} at v={v}, q={q}, s={s}: LP rows give {from_rows}, segments give {from_segments}",
                    plant.name
                );
            }
        }
    }
}

#[test]
fn probe_fit_context_reproduces_pipeline_alpha() -> Result<(), Box<dyn Error>> {
    // Arrange
    let case = DerivedCase::new("computed")?;
    let (system, result) = computed_system(case.path())?;
    let evaluation_nodes_per_axis = 7;

    // Act
    let contexts = fit_contexts(case.path(), &system)?;

    // Assert
    assert_eq!(contexts.len(), result.plant_results.len());
    for plant_result in &result.plant_results {
        let ctx = contexts.get(&plant_result.codigo).ok_or_else(|| {
            format!(
                "no fit context for plant {} ({})",
                plant_result.codigo, plant_result.nome
            )
        })?;
        assert_eq!(ctx.params.codigo, plant_result.codigo);
        assert_eq!(ctx.params.nome, plant_result.nome);

        let alpha = compute_alpha_fpha(
            &fit_hull_planes(&ctx.pf, &ctx.bounds)?,
            &ctx.pf,
            &ctx.bounds,
        );
        assert_eq!(
            alpha.to_bits(),
            plant_result.alpha.to_bits(),
            "plant {}: probe alpha {alpha} vs pipeline alpha {}",
            plant_result.nome,
            plant_result.alpha
        );

        let q_max = ctx.pf.max_turbined_m3s();
        let nodes = evaluation_grid_nodes(ctx, evaluation_nodes_per_axis);
        assert_eq!(
            nodes.len(),
            evaluation_nodes_per_axis * evaluation_nodes_per_axis
        );
        assert_eq!(nodes.first(), Some(&(ctx.bounds.v_min, 0.0)));
        assert_eq!(nodes.last(), Some(&(ctx.bounds.v_max, q_max)));
        for (v, q) in nodes {
            assert!(
                (ctx.bounds.v_min..=ctx.bounds.v_max).contains(&v) && (0.0..=q_max).contains(&q),
                "plant {}: evaluation node ({v}, {q}) outside the fit bounds",
                plant_result.nome
            );
        }
    }
    Ok(())
}
