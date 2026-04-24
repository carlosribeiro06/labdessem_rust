use std::{env, fs, path::PathBuf, process::ExitCode};

use labdessem_io::{read_study_config, read_study_from_config};
use labdessem_simulation::{ExecutionOption, run_simulation, write_results_csvs};

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
    let execution_option = ExecutionOption::from_config_value(config.opcao_execucao)?;
    println!("Opcao de execucao: {}", execution_option.description());
    println!(
        "Restricoes de rede: {}",
        if config.rede == 0 {
            "desativadas"
        } else {
            "ativadas"
        }
    );
    println!(
        "TON residual: {}",
        if config.ton_residual == 0 {
            "desativado"
        } else {
            "ativado"
        }
    );
    println!(
        "Unit commitment termico: {}",
        if config.uct == 0 {
            "desativado"
        } else {
            "ativado"
        }
    );
    println!(
        "Unit commitment hidraulico: {}",
        if config.uch == 0 {
            "desativado"
        } else {
            "ativado"
        }
    );

    println!("Lendo dados de entrada...");
    let system = read_study_from_config(config_path)?;

    println!("Dados carregados com sucesso.");
    println!(
        "Resumo do caso: {} submercados, {} barras, {} linhas, {} UTEs, {} UHEs, {} elevatorias, {} renovaveis",
        system.submarkets.len(),
        system.buses.len(),
        system.branches.len(),
        system.thermal_plants.len(),
        system.hydro_plants.len(),
        system.pumping_plants.len(),
        system.renewable_plants.len()
    );
    println!("Iniciando processo iterativo de resolucao...");
    let network_enabled = config.rede != 0;
    let result = run_simulation(&system, network_enabled, execution_option)?;
    let output_dir = config.case_path.join("RESULTADOS");

    println!("Gravando arquivos CSV de saida...");
    write_results_csvs(&system, &result, &output_dir, network_enabled)?;
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
