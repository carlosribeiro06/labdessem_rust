use std::{env, fs, path::PathBuf, process::ExitCode};

use labdessem_io::{output::write_all_results, read_study_config, read_study_from_path};
use labdessem_simulation::{EngMaxDiagnostic, ExecutionOption, run_simulation_with_engmax};

mod banner;
mod conversions;
mod fpha_integration;
mod logging;
mod output_prep;

fn main() -> ExitCode {
    if let Err(error) = logging::init() {
        eprintln!("Warning: log output disabled: {error}");
    }

    let args: Vec<String> = env::args().collect();

    match args.get(1).map(std::string::String::as_str) {
        Some("run") => {
            if matches!(
                args.get(2).map(std::string::String::as_str),
                Some("--help" | "-h")
            ) {
                print_run_help();
                return ExitCode::SUCCESS;
            }

            let case_path = args.get(2).map_or_else(
                || env::current_dir().unwrap_or_else(|_| PathBuf::from(".")),
                PathBuf::from,
            );

            match run_and_print(&case_path) {
                Ok(()) => ExitCode::SUCCESS,
                Err(error) => {
                    eprintln!("Error: {error}");
                    ExitCode::FAILURE
                }
            }
        }
        Some("--help" | "-h") | None => {
            print_help();
            ExitCode::SUCCESS
        }
        Some("--version" | "-V") => {
            println!("labdessem {}", env!("CARGO_PKG_VERSION"));
            ExitCode::SUCCESS
        }
        Some(unknown) => {
            eprintln!("Unknown command: {unknown}");
            eprintln!();
            eprintln!("Run 'labdessem --help' for usage.");
            ExitCode::FAILURE
        }
    }
}

fn print_help() {
    println!("labdessem - Hydrothermal dispatch optimization");
    println!();
    println!("USAGE:");
    println!("    labdessem <COMMAND>");
    println!();
    println!("COMMANDS:");
    println!("    run [CASE_DIR]    Run simulation for a study case");
    println!("                      If CASE_DIR is omitted, uses current directory");
    println!();
    println!("OPTIONS:");
    println!("    -h, --help        Print help information");
    println!("    -V, --version     Print version information");
    println!();
    println!("EXAMPLES:");
    println!("    labdessem run /path/to/case");
    println!("    labdessem run                   # uses current directory");
}

fn print_run_help() {
    println!("Run simulation for a study case");
    println!();
    println!("USAGE:");
    println!("    labdessem run [CASE_DIR]");
    println!();
    println!("ARGS:");
    println!("    [CASE_DIR]    Path to study case directory");
    println!("                  Defaults to current directory if not specified");
    println!();
    println!("The case directory must contain a study_config.json file.");
}

fn run_and_print(case_path: &PathBuf) -> Result<(), Box<dyn std::error::Error>> {
    let config_path = case_path.join("study_config.json");

    if !config_path.exists() {
        return Err(format!(
            "Configuration file not found.\n\
             Expected: {}\n\n\
             Each study case must contain a study_config.json file in its root directory.",
            config_path.display()
        )
        .into());
    }

    let config = read_study_config(&config_path)?;
    let display_path = fs::canonicalize(case_path).unwrap_or_else(|_| case_path.clone());
    let execution_option = ExecutionOption::from_config_value(config.opcao_execucao)?;

    banner::print_banner();
    banner::print_execution_info(&banner::ExecutionInfo {
        case_path: &display_path.display().to_string(),
        execution_mode: execution_option.description(),
        network_enabled: config.rede != 0,
        uct_enabled: config.uct != 0,
        uch_enabled: config.uch != 0,
        fpha_mode: config.fpha,
        fcf_enabled: config.fcf != 0,
        ton_residual_enabled: config.ton_residual != 0,
        engmax_enabled: config.engmax != 0,
    });

    eprintln!("Reading input data...");
    let mut system = read_study_from_path(case_path)?;

    eprintln!(
        "Loaded: {} submarkets, {} thermal plants, {} hydro plants",
        system.submarkets.len(),
        system.thermal_plants.len(),
        system.hydro_plants.len(),
    );

    if config.fpha.is_computed() {
        eprintln!("Computing FPHA hyperplanes from VHA geometry...");
        let fpha_result = fpha_integration::compute_fpha(case_path, &system)?;
        fpha_integration::inject_fpha_segments(&mut system, &fpha_result.plant_results);
        system.fpha_enabled = true;
        fpha_integration::write_computed_results(
            case_path,
            &fpha_result.plant_results,
            &system.hydro_plants,
        )?;
        eprintln!(
            "  Processed {} plants, {} skipped",
            fpha_result.plants_processed, fpha_result.plants_skipped
        );
    }

    eprintln!("Solving...");
    let network_enabled = config.rede != 0;
    let engmax_enabled = config.engmax != 0;
    let result =
        run_simulation_with_engmax(&system, network_enabled, execution_option, engmax_enabled)?;

    let output_data = conversions::convert_to_output_data(&system, &result);

    let output_dir = case_path.join("RESULTADOS");
    fs::create_dir_all(&output_dir)?;

    eprintln!("Writing output files...");

    let variable_values = output_prep::build_variable_values(&system, &result.cmo_solution);
    let constraint_duals = output_prep::build_constraint_duals(&system, &result.cmo_solution);

    write_all_results(
        &system,
        &output_data,
        &variable_values,
        &constraint_duals,
        &output_dir,
        network_enabled,
        result.engmax.as_ref().map(|outcome| &outcome.used),
    )?;

    eprintln!();
    eprintln!("Process");
    for step in &result.steps {
        eprintln!(
            "  {} #{:02} | obj = {:.4} | violations = {} | time = {:.3}s",
            step.stage.label(),
            step.iteration,
            step.objective_value,
            step.violation_count,
            step.solve_time.as_secs_f64()
        );
    }

    eprintln!();
    eprintln!("Summary");
    eprintln!("  Objective:   {:.4}", result.cmo_solution.objective);
    eprintln!("  Flow cuts:   {}", result.accumulated_flow_cuts);
    eprintln!("  Violations:  {}", result.final_violations.len());
    if let Some(outcome) = &result.engmax {
        for line in render_engmax_summary(&outcome.diagnostic) {
            eprintln!("{line}");
        }
    }
    eprintln!(
        "  Output:      {}",
        output_dir.display().to_string().trim_start_matches(r"\\?\")
    );
    Ok(())
}

/// The EngMax lines of the Summary (ADR-011): how far the final turbining exceeds EngMax
/// recomputed from the final solution. The worst item and the relative excess are printed only
/// when some plant-period is counted.
fn render_engmax_summary(diagnostic: &EngMaxDiagnostic) -> Vec<String> {
    let mut lines = vec![format!(
        "  EngMax:      {} plant-periods above the recomputed bound ({} above a zero bound)",
        diagnostic.exceed_count, diagnostic.zero_bound_exceed_count
    )];
    if let Some((plant, period)) = &diagnostic.worst {
        lines.push(format!(
            "  Max excess:  {:.4} m3/s at {plant} period {period}",
            diagnostic.max_excess_m3s
        ));
        lines.push(format!(
            "  Max rel.:    {:.2}%",
            100.0 * diagnostic.max_relative_excess
        ));
    }
    lines
}

#[cfg(test)]
mod tests {
    use labdessem_simulation::EngMaxDiagnostic;

    use super::render_engmax_summary;

    #[test]
    fn render_engmax_summary_lists_worst_item() {
        let diagnostic = EngMaxDiagnostic {
            exceed_count: 2,
            zero_bound_exceed_count: 1,
            max_excess_m3s: 2.0,
            max_relative_excess: 0.0204,
            worst: Some(("UHE-1".to_owned(), 2)),
        };

        assert_eq!(
            render_engmax_summary(&diagnostic),
            [
                "  EngMax:      2 plant-periods above the recomputed bound (1 above a zero bound)",
                "  Max excess:  2.0000 m3/s at UHE-1 period 2",
                "  Max rel.:    2.04%",
            ]
        );
    }

    #[test]
    fn render_engmax_summary_without_exceedance_is_one_line() {
        assert_eq!(
            render_engmax_summary(&EngMaxDiagnostic::default()),
            ["  EngMax:      0 plant-periods above the recomputed bound (0 above a zero bound)"]
        );
    }
}
