//! FPHA fitting example.
//!
//! Usage:
//!   cargo run --example fpha_fit -- examples/fpha_example
//!
//! This reads CSV files from CAD/ and writes results to RESULTADOS/.

use std::collections::HashMap;
use std::env;
use std::fs;
use std::path::Path;

use csv::ReaderBuilder;
use labdessem_fpha::{
    fit_plant, read_fpha_config, read_geometry_table, read_tailrace_table, write_fpha_results,
    FitPlantParams, FphaPlantConfig, PlaneReductionConfig,
};
use serde::Deserialize;

/// Plant parameters from CAD_UHE_PARAMS.csv
#[derive(Debug, Deserialize)]
struct PlantParams {
    #[serde(alias = "Codigo")]
    codigo: i32,
    #[serde(alias = "Nome")]
    nome: String,
    #[serde(alias = "MaxTurbinedM3s")]
    max_turbined_m3s: f64,
    #[serde(alias = "MaxGenerationMW")]
    max_generation_mw: f64,
    #[serde(alias = "HydraulicLossM")]
    hydraulic_loss_m: f64,
    #[serde(alias = "Efficiency")]
    efficiency: f64,
    #[serde(alias = "MeanInflowM3s")]
    mean_inflow_m3s: f64,
}

fn read_plant_params(path: &Path) -> Result<HashMap<i32, PlantParams>, Box<dyn std::error::Error>> {
    let mut reader = ReaderBuilder::new()
        .delimiter(b';')
        .has_headers(true)
        .from_path(path)?;

    let mut params = HashMap::new();
    for result in reader.deserialize() {
        let row: PlantParams = result?;
        params.insert(row.codigo, row);
    }
    Ok(params)
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let args: Vec<String> = env::args().collect();
    let case_path = if args.len() > 1 {
        Path::new(&args[1])
    } else {
        Path::new("examples/fpha_example")
    };

    let cad_path = case_path.join("CAD");
    let result_path = case_path.join("RESULTADOS");

    println!("╔══════════════════════════════════════════════════════════════════╗");
    println!("║              LABDESSEM-FPHA — FPHA Fitting Example               ║");
    println!("╚══════════════════════════════════════════════════════════════════╝");
    println!();
    println!("Case: {}", case_path.display());
    println!();

    // Read input files
    println!("Reading input files...");
    let geometry = read_geometry_table(cad_path.join("CAD_GEOMETRIA_UHE.csv"))?;
    println!("  ✓ CAD_GEOMETRIA_UHE.csv: {} plants", geometry.len());

    let tailrace = read_tailrace_table(cad_path.join("CAD_TAILRACE.csv"))?;
    println!("  ✓ CAD_TAILRACE.csv: {} plants", tailrace.len());

    let configs = read_fpha_config(cad_path.join("CAD_FPHA_CONFIG.csv"))?;
    println!("  ✓ CAD_FPHA_CONFIG.csv: {} plants", configs.len());

    let plant_params = read_plant_params(&cad_path.join("CAD_UHE_PARAMS.csv"))?;
    println!("  ✓ CAD_UHE_PARAMS.csv: {} plants", plant_params.len());
    println!();

    // Fit each plant
    println!("Fitting FPHA hyperplanes...");
    println!("─────────────────────────────────────────────────────────────────────");

    let mut results = Vec::new();

    for (codigo, forebay) in &geometry {
        let params = match plant_params.get(codigo) {
            Some(p) => p,
            None => {
                println!("  ⚠ Plant {} not found in CAD_UHE_PARAMS.csv, skipping", codigo);
                continue;
            }
        };

        let config = configs.get(codigo).cloned().unwrap_or_else(|| {
            FphaPlantConfig::computed(8, 6, 6, PlaneReductionConfig::default())
        });

        let tailrace_model = tailrace.get(codigo).cloned();

        let fit_params = FitPlantParams {
            codigo: *codigo,
            nome: params.nome.clone(),
            forebay: forebay.clone(),
            tailrace: tailrace_model,
            hydraulic_loss_m: Some(params.hydraulic_loss_m),
            efficiency: Some(params.efficiency),
            max_turbined_m3s: params.max_turbined_m3s,
            max_generation_mw: params.max_generation_mw,
            long_term_mean_inflow_m3s: params.mean_inflow_m3s,
            config,
        };

        match fit_plant(&fit_params) {
            Ok(result) => {
                let head_range = forebay.height(forebay.v_max()) - forebay.height(forebay.v_min());

                println!(
                    "  {:3} {:<16} │ α={:.4} │ planes={:2} │ ΔH={:5.1}m │ MAE={:6.2} MW │ Max={:6.2} MW",
                    codigo,
                    params.nome,
                    result.alpha,
                    result.planes.len(),
                    head_range,
                    result.deviation.mean_abs_mw,
                    result.deviation.max_abs_mw,
                );
                results.push(result);
            }
            Err(e) => {
                println!("  {:3} {:<16} │ ERROR: {}", codigo, params.nome, e);
            }
        }
    }

    println!("─────────────────────────────────────────────────────────────────────");
    println!();

    // Write results
    fs::create_dir_all(&result_path)?;
    let output_file = result_path.join("resultado_fpha_computed.csv");
    write_fpha_results(&results, &output_file)?;
    println!("Results written to: {}", output_file.display());

    // Print summary
    let total_planes: usize = results.iter().map(|r| r.planes.len()).sum();
    let avg_mae: f64 = results.iter().map(|r| r.deviation.mean_abs_mw).sum::<f64>() / results.len() as f64;

    println!();
    println!("╔══════════════════════════════════════════════════════════════════╗");
    println!("║  Summary                                                         ║");
    println!("╠══════════════════════════════════════════════════════════════════╣");
    println!("║  Plants fitted: {:3}                                              ║", results.len());
    println!("║  Total planes:  {:3}                                              ║", total_planes);
    println!("║  Avg MAE:       {:6.2} MW                                        ║", avg_mae);
    println!("╚══════════════════════════════════════════════════════════════════╝");

    Ok(())
}
