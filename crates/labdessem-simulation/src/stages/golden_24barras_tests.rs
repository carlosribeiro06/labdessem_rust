//! Golden capture of the official 24Barras case: the max swallowing recalculation through a network MILP.

use std::path::Path;

use labdessem_core::system::System;
use labdessem_core::test_support::golden::{GoldenFailure, GoldenRecord, check_or_bless};
use labdessem_io::{StudyConfig, read_study_config, read_study_from_path};

use crate::engmax::EngMaxState;
use crate::results::IterativeSimulationResult;
use crate::workflow::ExecutionOption;

use super::orchestration::run_simulation_with_state;

fn record_bounds(
    record: &mut GoldenRecord,
    prefix: &str,
    system: &System,
    bound: impl Fn(usize, usize) -> Option<f64>,
) {
    for (plant, hydro) in system.hydro_plants.iter().enumerate() {
        for period in 0..system.horizon.periods {
            let key = format!(
                "{prefix}/uhe={}:{}/t={}",
                hydro.id.0,
                hydro.name,
                period + 1
            );
            match bound(plant, period) {
                Some(value) => record.f64(key, value),
                None => record.text(key, "none"),
            }
        }
    }
}

fn record_run(
    record: &mut GoldenRecord,
    prefix: &str,
    system: &System,
    result: &IterativeSimulationResult,
    state: &EngMaxState,
) {
    for (index, entry) in state.trace().iter().enumerate() {
        let trace = format!("{prefix}/trace={}", index + 1);
        record.text(
            format!("{trace}/stage"),
            format!("{}#{:02}", entry.stage.label(), entry.iteration),
        );
        record.f64(format!("{trace}/objective"), entry.objective);
        record_bounds(record, &format!("{trace}/used"), system, |plant, period| {
            entry.used.get(plant, period)
        });
        record_bounds(
            record,
            &format!("{trace}/produced"),
            system,
            |plant, period| entry.produced.get(plant, period),
        );
    }
    for (index, step) in result.steps.iter().enumerate() {
        let key = format!("{prefix}/step={}", index + 1);
        record.text(
            format!("{key}/stage"),
            format!("{}#{:02}", step.stage.label(), step.iteration),
        );
        record.f64(format!("{key}/objective"), step.objective_value);
    }
    record.f64(
        format!("{prefix}/cmo/objective"),
        result.cmo_solution.objective,
    );

    let outcome = result
        .engmax
        .as_ref()
        .expect("a run with a swallowing state carries the outcome");
    record_bounds(
        record,
        &format!("{prefix}/outcome/used"),
        system,
        |plant, period| outcome.used.get(plant, period),
    );
    record_bounds(
        record,
        &format!("{prefix}/outcome/recomputed"),
        system,
        |plant, period| outcome.recomputed.get(plant, period),
    );
    let diagnostic = &outcome.diagnostic;
    record.text(
        format!("{prefix}/outcome/exceed_count"),
        diagnostic.exceed_count.to_string(),
    );
    record.text(
        format!("{prefix}/outcome/zero_bound_exceed_count"),
        diagnostic.zero_bound_exceed_count.to_string(),
    );
    record.f64(
        format!("{prefix}/outcome/max_excess_m3s"),
        diagnostic.max_excess_m3s,
    );
    record.f64(
        format!("{prefix}/outcome/max_relative_excess"),
        diagnostic.max_relative_excess,
    );
    record.text(
        format!("{prefix}/outcome/worst"),
        diagnostic.worst.as_ref().map_or_else(
            || "none".to_owned(),
            |(name, period)| format!("{name}:{period}"),
        ),
    );
}

fn capture(
    case: &Path,
    config: &StudyConfig,
    option: ExecutionOption,
    system: &System,
) -> Result<GoldenRecord, String> {
    let failure = |error: &dyn std::fmt::Display| format!("{}: {error}", case.display());
    let mut record = GoldenRecord::new();
    record.text("case/opcao", config.opcao_execucao.to_string());
    record.text("case/rede", config.rede.to_string());

    let mut state = EngMaxState::new(system).map_err(|error| failure(&error))?;
    record_bounds(
        &mut record,
        "swallowing/initial/24barras",
        system,
        |plant, period| state.bounds().get(plant, period),
    );
    let result = run_simulation_with_state(system, config.rede != 0, option, Some(&mut state))
        .map_err(|error| failure(&error))?;
    record_run(&mut record, "solve/24barras", system, &result, &state);
    Ok(record)
}

#[test]
fn official_24barras_matches_golden() -> Result<(), GoldenFailure> {
    let case = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../examples/24Barras");
    let failure =
        |error: &dyn std::fmt::Display| GoldenFailure(format!("{}: {error}", case.display()));
    let config =
        read_study_config(case.join("study_config.json")).map_err(|error| failure(&error))?;
    let option = ExecutionOption::from_config_value(config.opcao_execucao)
        .map_err(|error| failure(&error))?;
    let system = read_study_from_path(&case).map_err(|error| failure(&error))?;
    check_or_bless("24barras_official.golden", || {
        capture(&case, &config, option, &system).expect("the official 24Barras case solves")
    })
}
