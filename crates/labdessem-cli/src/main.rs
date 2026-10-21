use std::{env, fs, path::PathBuf, process::ExitCode};

use labdessem_io::{output::write_all_results, read_study_config, read_study_from_path};
use labdessem_simulation::{ExecutionOption, run_simulation};

mod banner;
mod conversions;
mod output_prep;

fn main() -> ExitCode {
    let args: Vec<String> = env::args().collect();

    match args.get(1).map(std::string::String::as_str) {
        Some("run") => {
            if args.get(2).map(std::string::String::as_str) == Some("--help")
                || args.get(2).map(std::string::String::as_str) == Some("-h")
            {
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
        fpha_enabled: config.fpha != 0,
        fcf_enabled: config.fcf != 0,
        ton_residual_enabled: config.ton_residual != 0,
    });

    eprintln!("Reading input data...");
    let system = read_study_from_path(case_path)?;

    eprintln!(
        "Loaded: {} submarkets, {} thermal plants, {} hydro plants",
        system.submarkets.len(),
        system.thermal_plants.len(),
        system.hydro_plants.len(),
    );

    eprintln!("Solving...");
    let network_enabled = config.rede != 0;
    let result = run_simulation(&system, network_enabled, execution_option)?;

    // Convert to output DTOs
    let output_data = conversions::convert_to_output_data(&system, &result);

    let output_dir = case_path.join("RESULTADOS");
    std::fs::create_dir_all(&output_dir)?;

    eprintln!("Writing output files...");

    // Build maps from solution
    let variable_values = output_prep::build_variable_values(&system, &result.cmo_solution);
    let constraint_duals = output_prep::build_constraint_duals(&system, &result.cmo_solution);

    // Write all results via orchestrator
    write_all_results(
        &system,
        &output_data,
        &variable_values,
        &constraint_duals,
        &output_dir,
        network_enabled,
    )?;

    let display_output_dir = output_dir.clone();

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
    eprintln!(
        "  Output:      {}",
        display_output_dir
            .display()
            .to_string()
            .trim_start_matches(r"\\?\")
    );
    Ok(())
}
