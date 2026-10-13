use std::{env, fs, path::PathBuf, process::ExitCode};

use labdessem_io::{read_study_config, read_study_from_path};
use labdessem_simulation::{run_simulation, write_results_csvs, ExecutionOption};

fn main() -> ExitCode {
    let args: Vec<String> = env::args().collect();

    match args.get(1).map(|s| s.as_str()) {
        Some("run") => {
            if args.get(2).map(|s| s.as_str()) == Some("--help")
                || args.get(2).map(|s| s.as_str()) == Some("-h")
            {
                print_run_help();
                return ExitCode::SUCCESS;
            }

            let case_path = args
                .get(2)
                .map(PathBuf::from)
                .unwrap_or_else(|| env::current_dir().unwrap_or_else(|_| PathBuf::from(".")));

            match run_and_print(&case_path) {
                Ok(()) => ExitCode::SUCCESS,
                Err(error) => {
                    eprintln!("Error: {error}");
                    ExitCode::FAILURE
                }
            }
        }
        Some("--help") | Some("-h") | None => {
            print_help();
            ExitCode::SUCCESS
        }
        Some("--version") | Some("-V") => {
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
    println!("Reading study configuration...");
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
    println!("Case: {}", display_path.display());

    let execution_option = ExecutionOption::from_config_value(config.opcao_execucao)?;
    println!("Execution option: {}", execution_option.description());
    println!(
        "Network constraints: {}",
        if config.rede == 0 {
            "disabled"
        } else {
            "enabled"
        }
    );
    println!(
        "Residual TON: {}",
        if config.ton_residual == 0 {
            "disabled"
        } else {
            "enabled"
        }
    );
    println!(
        "Thermal unit commitment: {}",
        if config.uct == 0 {
            "disabled"
        } else {
            "enabled"
        }
    );
    println!(
        "Hydro unit commitment: {}",
        if config.uch == 0 {
            "disabled"
        } else {
            "enabled"
        }
    );

    println!("Reading input data...");
    let system = read_study_from_path(case_path)?;

    println!("Data loaded successfully.");
    println!(
        "Case summary: {} submarkets, {} buses, {} lines, {} thermal plants, {} hydro plants, {} pumping plants, {} renewables",
        system.submarkets.len(),
        system.buses.len(),
        system.branches.len(),
        system.thermal_plants.len(),
        system.hydro_plants.len(),
        system.pumping_plants.len(),
        system.renewable_plants.len()
    );
    println!("Starting iterative solve process...");
    let network_enabled = config.rede != 0;
    let result = run_simulation(&system, network_enabled, execution_option)?;
    let output_dir = case_path.join("RESULTADOS");

    println!("Writing CSV output files...");
    write_results_csvs(&system, &result, &output_dir, network_enabled)?;
    let display_output_dir = fs::canonicalize(&output_dir).unwrap_or(output_dir.clone());

    println!("Iterative simulation finished.");
    println!("Case: {}", display_path.display());
    println!();
    println!("PROCESS");
    for step in &result.steps {
        println!(
            "{} #{:02} | objective = {:.4} | flow violations = {} | solve time = {:.3} s",
            step.stage.label(),
            step.iteration,
            step.objective_value,
            step.violation_count,
            step.solve_time.as_secs_f64()
        );
    }
    println!();
    println!("SUMMARY");
    println!(
        "Final objective value: {:.4}",
        result.final_summary.objective_value
    );
    println!("Accumulated flow cuts: {}", result.accumulated_flow_cuts);
    println!("Final flow violations: {}", result.final_violations.len());
    println!(
        "CSV output directory: {}",
        display_output_dir
            .display()
            .to_string()
            .trim_start_matches(r"\\?\")
    );
    Ok(())
}
