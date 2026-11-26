//! Golden capture of the computed-FPHA path on the 24Barras deck: fit inputs, fit outputs, the LP
//! rows built from them, the solve, and the two on-disk artefacts.

use std::error::Error;
use std::fmt::Display;
use std::fs;
use std::path::Path;

use labdessem_core::system::System;
use labdessem_core::test_support::golden::{
    GoldenFailure, GoldenRecord, check_or_bless, check_or_bless_bytes, fnv1a64,
};
use labdessem_io::output::{write_all_results, write_fpha_computed};
use labdessem_io::read_study_from_path;
use labdessem_model::constraints::{ConstraintSense, LinearConstraint};
use labdessem_model::{Model, SolveMode};
use labdessem_simulation::{ExecutionOption, run_simulation_with_max_swallowing};

use super::FphaComputeResult;
use super::probe::{DerivedCase, computed_system, fit_contexts};

const CONFIG_HEADER: &str = "codigo_usina;num_planes;tolerance;min_volume_pct;max_volume_pct";

fn uhe(code: impl Display, name: &str) -> String {
    format!("uhe={code}:{name}")
}

fn record_optional(record: &mut GoldenRecord, key: String, value: Option<f64>) {
    match value {
        Some(value) => record.f64(key, value),
        None => record.text(key, "none"),
    }
}

fn record_fit_inputs(record: &mut GoldenRecord, case: &Path) -> Result<(), Box<dyn Error>> {
    let system = read_study_from_path(case)?;
    for context in fit_contexts(case, &system)?.values() {
        let params = &context.params;
        let config = &params.config;
        let bounds = &context.bounds;
        let prefix = format!("fit/{}", uhe(params.id.0, &params.name));

        record.f64(
            format!("{prefix}/max_turbined_m3s"),
            params.max_turbined_m3s,
        );
        record.f64(
            format!("{prefix}/max_generation_mw"),
            params.max_generation_mw,
        );
        record.f64(
            format!("{prefix}/long_term_mean_inflow_m3s"),
            params.long_term_mean_inflow_m3s,
        );
        record.f64(
            format!("{prefix}/hydraulic_loss_m"),
            params.hydraulic_loss_m,
        );
        record.f64(format!("{prefix}/efficiency"), params.efficiency);

        record.text(
            format!("{prefix}/config/max_planes"),
            config.max_planes.to_string(),
        );
        record.text(
            format!("{prefix}/config/volume_points"),
            config.volume_points.to_string(),
        );
        record.text(
            format!("{prefix}/config/flow_points"),
            config.flow_points.to_string(),
        );
        record.f64(
            format!("{prefix}/config/window_min_fraction"),
            config.volume_window.min_fraction,
        );
        record.f64(
            format!("{prefix}/config/window_max_fraction"),
            config.volume_window.max_fraction,
        );
        match config.window_reference_hm3 {
            Some((lo, hi)) => {
                record.f64(format!("{prefix}/config/window_reference/lo"), lo);
                record.f64(format!("{prefix}/config/window_reference/hi"), hi);
            }
            None => record.text(format!("{prefix}/config/window_reference"), "none"),
        }
        record.f64(
            format!("{prefix}/config/reduction/angle_tolerance_deg"),
            config.reduction_config.tolerance_deg,
        );

        record.f64(format!("{prefix}/bounds/v_min"), bounds.v_min);
        record.f64(format!("{prefix}/bounds/v_max"), bounds.v_max);
        record.text(
            format!("{prefix}/bounds/n_volume_points"),
            bounds.n_volume_points.to_string(),
        );
        record.text(
            format!("{prefix}/bounds/n_flow_points"),
            bounds.n_flow_points.to_string(),
        );
        record.text(
            format!("{prefix}/bounds/n_spillage_points"),
            bounds.n_spillage_points.to_string(),
        );
        record.text(
            format!("{prefix}/bounds/max_planes_per_hydro"),
            bounds.max_planes_per_hydro.to_string(),
        );
        record.text(
            format!("{prefix}/bounds/single_volume"),
            bounds.single_volume.to_string(),
        );

        for k in 0..=10_u8 {
            let volume = bounds.v_min + (bounds.v_max - bounds.v_min) * f64::from(k) / 10.0;
            record.f64(
                format!("{prefix}/forebay/height_at={k}"),
                params.forebay.height(volume),
            );
        }
        for k in 0..=4_u8 {
            record.f64(
                format!("{prefix}/tailrace/level_at={k}"),
                params
                    .tailrace
                    .evaluate(params.max_turbined_m3s * f64::from(k) / 4.0),
            );
        }
    }
    Ok(())
}

fn record_fit_outputs(record: &mut GoldenRecord, result: &FphaComputeResult) {
    for plant in &result.plant_results {
        let prefix = format!("fpha/{}", uhe(plant.id.0, &plant.name));
        record.text(format!("{prefix}/planes"), plant.planes.len().to_string());
        for (index, plane) in plant.planes.iter().enumerate() {
            record.f64(format!("{prefix}/plane={index}/gamma_0"), plane.gamma_0);
            record.f64(format!("{prefix}/plane={index}/gamma_v"), plane.gamma_v);
            record.f64(format!("{prefix}/plane={index}/gamma_q"), plane.gamma_q);
            record.f64(format!("{prefix}/plane={index}/gamma_s"), plane.gamma_s);
        }
        record.f64(format!("{prefix}/alpha"), plant.alpha);
        let deviation = &plant.deviation;
        record.f64(
            format!("{prefix}/deviation/mean_abs_mw"),
            deviation.mean_abs_mw,
        );
        record.f64(
            format!("{prefix}/deviation/max_abs_mw"),
            deviation.max_abs_mw,
        );
        record.f64(format!("{prefix}/deviation/rms_mw"), deviation.rms_mw);
        record.f64(
            format!("{prefix}/deviation/mean_signed_mw"),
            deviation.mean_signed_mw,
        );
        record.f64(format!("{prefix}/deviation/relative"), deviation.relative);
    }
    record.text("fpha/processed", result.plants_processed.to_string());
    record.text("fpha/skipped", result.plants_skipped.to_string());
}

fn record_row_shape(record: &mut GoldenRecord, row: &LinearConstraint) {
    let sense = match row.sense {
        ConstraintSense::LessOrEqual => "<=",
        ConstraintSense::GreaterOrEqual => ">=",
        ConstraintSense::Equal => "=",
    };
    record.text(format!("lp/{}/sense", row.name), sense);
    for term in &row.terms {
        record.f64(
            format!("lp/{}/coef/{}", row.name, term.variable),
            term.coefficient,
        );
    }
}

fn record_name_set(record: &mut GoldenRecord, prefix: &str, mut names: Vec<&str>) {
    names.sort_unstable();
    record.text(format!("{prefix}/count"), names.len().to_string());
    record.text(
        format!("{prefix}/fnv1a64"),
        format!("0x{:016x}", fnv1a64(names.join("\n").as_bytes())),
    );
}

fn record_lp_structure(record: &mut GoldenRecord, system: &System) {
    let model = Model::from_system(system, SolveMode::LinearProgramming);
    for row in model.constraints.hydro_fpha() {
        record.f64(format!("lp/{}/rhs", row.name), row.rhs);
        record_row_shape(record, row);
    }
    record_name_set(record, "lp/constraints", model.constraints.names());
    let variables = &model.variables;
    let variable_names = [
        &variables.thermal_generation,
        &variables.hydro_generation,
        &variables.hydro_turbining,
        &variables.hydro_spillage,
        &variables.hydro_diversion,
        &variables.pumping,
        &variables.hydro_volume,
        &variables.future_cost,
        &variables.renewable_generation,
        &variables.interchange,
        &variables.thermal_commitment,
        &variables.thermal_startup,
        &variables.thermal_shutdown,
        &variables.hydro_commitment,
        &variables.network_flow_slack,
        &variables.operational_limit_slack,
    ]
    .into_iter()
    .flatten()
    .map(|variable| variable.name.as_str())
    .collect();
    record_name_set(record, "lp/variables", variable_names);
}

fn record_bounds(
    record: &mut GoldenRecord,
    prefix: &str,
    system: &System,
    bound: impl Fn(usize, usize) -> Option<f64>,
) {
    for (plant, hydro) in system.hydro_plants.iter().enumerate() {
        for period in 0..system.horizon.periods {
            let key = format!("{prefix}/{}/t={}", uhe(hydro.id.0, &hydro.name), period + 1);
            record_optional(record, key, bound(plant, period));
        }
    }
}

fn record_solve(
    record: &mut GoldenRecord,
    system: &System,
    case: &Path,
) -> Result<(), Box<dyn Error>> {
    let result =
        run_simulation_with_max_swallowing(system, false, ExecutionOption::SingleMilp, true)?;
    for (index, step) in result.steps.iter().enumerate() {
        let key = format!("solve/step={}", index + 1);
        record.text(
            format!("{key}/stage"),
            format!("{}#{:02}", step.stage.label(), step.iteration),
        );
        record.f64(format!("{key}/objective"), step.objective_value);
    }
    record.f64("solve/cmo/objective", result.cmo_solution.objective);

    let outcome = result
        .max_swallowing
        .as_ref()
        .ok_or("a run with max swallowing enabled carries the outcome")?;
    record_bounds(record, "solve/outcome/used", system, |plant, period| {
        outcome.used.get(plant, period)
    });
    record_bounds(
        record,
        "solve/outcome/recomputed",
        system,
        |plant, period| outcome.recomputed.get(plant, period),
    );
    let diagnostic = &outcome.diagnostic;
    record.text(
        "solve/outcome/exceed_count",
        diagnostic.exceed_count.to_string(),
    );
    record.text(
        "solve/outcome/zero_bound_exceed_count",
        diagnostic.zero_bound_exceed_count.to_string(),
    );
    record.f64("solve/outcome/max_excess_m3s", diagnostic.max_excess_m3s);
    record.f64(
        "solve/outcome/max_relative_excess",
        diagnostic.max_relative_excess,
    );
    record.text(
        "solve/outcome/worst",
        diagnostic.worst.as_ref().map_or_else(
            || "none".to_owned(),
            |(name, period)| format!("{name}:{period}"),
        ),
    );

    let mut model = Model::from_system(system, SolveMode::LinearProgramming);
    model.constraints.extend_with_max_swallowing(
        system,
        &model.indexing,
        &model.variables,
        &outcome.used,
    )?;
    for row in model.constraints.max_swallowing() {
        record_row_shape(record, row);
        record.f64(format!("solve/{}/rhs", row.name), row.rhs);
    }

    let output_data = crate::conversions::convert_to_output_data(system, &result);
    let variable_values = crate::output_prep::build_variable_values(system, &result.cmo_solution);
    let constraint_duals = crate::output_prep::build_constraint_duals(system, &result.cmo_solution);
    let output_dir = case.join("RESULTADOS");
    write_all_results(
        system,
        &output_data,
        &variable_values,
        &constraint_duals,
        &output_dir,
        false,
        Some(&outcome.used),
    )?;
    let bytes = fs::read(output_dir.join("resultado_hidreletricas.csv"))?;
    for (index, line) in std::str::from_utf8(&bytes)?.lines().enumerate() {
        record.text(format!("csv/hidreletricas/line={}", index + 1), line);
    }
    record.text("csv/hidreletricas/bytes", bytes.len().to_string());
    record.text(
        "csv/hidreletricas/fnv1a64",
        format!("0x{:016x}", fnv1a64(&bytes)),
    );
    Ok(())
}

fn capture_computed() -> Result<GoldenRecord, Box<dyn Error>> {
    let case = DerivedCase::new("computed")?;
    let mut record = GoldenRecord::new();
    record_fit_inputs(&mut record, case.path())?;
    let (system, result) = computed_system(case.path())?;
    record_fit_outputs(&mut record, &result);
    record_lp_structure(&mut record, &system);
    record_solve(&mut record, &system, case.path())?;
    Ok(record)
}

fn capture_fit_only(case: &DerivedCase) -> Result<GoldenRecord, Box<dyn Error>> {
    let mut record = GoldenRecord::new();
    record_fit_inputs(&mut record, case.path())?;
    let (_, result) = computed_system(case.path())?;
    record_fit_outputs(&mut record, &result);
    Ok(record)
}

fn capture_without_inflow_history() -> Result<GoldenRecord, Box<dyn Error>> {
    let case = DerivedCase::new("computed")?;
    case.remove_file("OPER/inflow_history.parquet")?;
    capture_fit_only(&case)
}

fn capture_overrides() -> Result<GoldenRecord, Box<dyn Error>> {
    let case = DerivedCase::new("computed")?;
    fs::write(
        case.path().join("CAD/CAD_FPHA_CONFIG.csv"),
        format!("{CONFIG_HEADER}\n0;2;;;\n3;;5.0;0.5;0.6\n"),
    )?;
    capture_fit_only(&case)
}

fn capture_fpha_csv() -> Result<Vec<u8>, Box<dyn Error>> {
    let case = DerivedCase::new("computed")?;
    let (system, result) = computed_system(case.path())?;
    write_fpha_computed(case.path(), &system, &result.plant_results)?;
    Ok(fs::read(
        case.path().join("RESULTADOS/resultado_fpha_computed.csv"),
    )?)
}

#[test]
fn computed_24barras_matches_golden() -> Result<(), GoldenFailure> {
    check_or_bless("24barras_computed.golden", || {
        capture_computed().expect("the computed-FPHA 24Barras case fits and solves")
    })
}

#[test]
fn computed_fpha_csv_matches_golden() -> Result<(), GoldenFailure> {
    check_or_bless_bytes("24barras_computed/resultado_fpha_computed.csv", || {
        capture_fpha_csv().expect("the computed-FPHA CSV of the 24Barras case is written")
    })
}

#[test]
fn computed_24barras_without_inflow_history_matches_golden() -> Result<(), GoldenFailure> {
    check_or_bless("24barras_computed_without_inflow_history.golden", || {
        capture_without_inflow_history().expect("the 24Barras case fits without inflow history")
    })
}

#[test]
fn computed_24barras_overrides_match_golden() -> Result<(), GoldenFailure> {
    check_or_bless("24barras_computed_overrides.golden", || {
        capture_overrides().expect("the 24Barras case fits with the plant overrides")
    })
}
