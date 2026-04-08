use std::{env, fs, path::PathBuf, process::ExitCode};

use labdessem_io::{read_study_config, read_study_from_path};
use labdessem_simulation::{run_iterative_simulation, write_results_csvs};

fn main() -> ExitCode {
    let config_path = env::args().nth(1).map(PathBuf::from).unwrap_or_else(|| {
        PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../labdessem-io/study_config.json")
    });

    match run_and_print(&config_path) {
        Ok(()) => ExitCode::SUCCESS,
        Err(error) => {
            eprintln!("Failed to run iterative simulation: {error}");
            ExitCode::FAILURE
        }
    }
}

fn run_and_print(config_path: &PathBuf) -> Result<(), Box<dyn std::error::Error>> {
    println!("Lendo configuracao do estudo...");
    let config = read_study_config(config_path)?;
    println!("Caso selecionado: {}", config.case_path.display());

    println!("Lendo dados de entrada...");
    let system = read_study_from_path(&config.case_path)?;

    println!("Dados carregados com sucesso.");
    println!(
        "Resumo do caso: {} submercados, {} barras, {} linhas, {} UTEs, {} UHEs, {} eolicas, {} solares",
        system.submarkets.len(),
        system.buses.len(),
        system.branches.len(),
        system.thermal_plants.len(),
        system.hydro_plants.len(),
        system.wind_plants.len(),
        system.solar_plants.len()
    );

    println!("Iniciando processo iterativo de resolucao...");
    let result = run_iterative_simulation(&system)?;
    let output_dir = config.case_path.join("RESULTADOS");

    println!("Gravando arquivos CSV de saida...");
    write_results_csvs(&system, &result, &output_dir)?;
    let display_output_dir = fs::canonicalize(&output_dir).unwrap_or(output_dir.clone());

    println!("Iterative simulation finished.");
    println!("Config: {}", config_path.display());
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
