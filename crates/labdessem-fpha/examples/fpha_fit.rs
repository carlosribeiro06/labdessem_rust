//! FPHA fitting example.
//!
//! Usage:
//!   cargo run --example fpha_fit -- examples/fpha_example
//!
//! This reads CSV files from CAD/ and writes results to RESULTADOS/.

use std::collections::HashMap;
use std::env;
use std::path::Path;

use labdessem_fpha::{
    fit_plant, forebay_from_vha, tailrace_from_data,
    FitPlantParams, FphaPlantConfig, FphaPlantResult, PlaneReductionConfig,
};
use labdessem_io::output::{write_fpha_computed, ComputedFphaRecord};
use labdessem_io::readers::{read_csv, read_fpha_config, read_tailrace, read_vha_geometry};
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
    let rows: Vec<PlantParams> = read_csv(path)?;
    let params = rows.into_iter().map(|r| (r.codigo, r)).collect();
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

    println!("╔══════════════════════════════════════════════════════════════════╗");
    println!("║              LABDESSEM-FPHA — FPHA Fitting Example               ║");
    println!("╚══════════════════════════════════════════════════════════════════╝");
    println!();
    println!("Case: {}", case_path.display());
    println!();

    // Read input files using labdessem-io readers
    println!("Reading input files...");
    let vha_data = read_vha_geometry(case_path)?;
    println!("  ✓ CAD_GEOMETRIA_UHE.csv: {} plants", vha_data.len());

    let tailrace_data = read_tailrace(case_path)?;
    println!("  ✓ CAD_TAILRACE.csv: {} plants", tailrace_data.len());

    let configs = read_fpha_config(case_path)?;
    println!("  ✓ CAD_FPHA_CONFIG.csv: {} plants", configs.len());

    let plant_params = read_plant_params(&cad_path.join("CAD_UHE_PARAMS.csv"))?;
    println!("  ✓ CAD_UHE_PARAMS.csv: {} plants", plant_params.len());
    println!();

    // Fit each plant
    println!("Fitting FPHA hyperplanes...");
    println!("─────────────────────────────────────────────────────────────────────");

    let mut results: Vec<FphaPlantResult> = Vec::new();
    let mut plant_names: HashMap<i32, String> = HashMap::new();

    for (codigo, vha) in &vha_data {
        let codigo_i32 = *codigo as i32;

        let params = match plant_params.get(&codigo_i32) {
            Some(p) => p,
            None => {
                println!("  ⚠ Plant {} not found in CAD_UHE_PARAMS.csv, skipping", codigo);
                continue;
            }
        };

        // Convert VHA geometry using from_io conversion
        let forebay = match forebay_from_vha(vha, &params.nome) {
            Ok(fb) => fb,
            Err(e) => {
                println!("  ⚠ Plant {} geometry conversion failed: {}, skipping", codigo, e);
                continue;
            }
        };

        let fpha_config = configs.get(codigo).map(|c| {
            let mut config = FphaPlantConfig::computed(8, 6, 6, PlaneReductionConfig::default());
            if let Some(n) = c.num_planes {
                config.max_planes = n;
            }
            config
        }).unwrap_or_else(|| {
            FphaPlantConfig::computed(8, 6, 6, PlaneReductionConfig::default())
        });

        // Convert tailrace using from_io conversion
        let tailrace_model = tailrace_data.get(codigo).map(tailrace_from_data);

        let fit_params = FitPlantParams {
            codigo: codigo_i32,
            nome: params.nome.clone(),
            forebay: forebay.clone(),
            tailrace: tailrace_model,
            hydraulic_loss_m: Some(params.hydraulic_loss_m),
            efficiency: Some(params.efficiency),
            max_turbined_m3s: params.max_turbined_m3s,
            max_generation_mw: params.max_generation_mw,
            long_term_mean_inflow_m3s: params.mean_inflow_m3s,
            config: fpha_config,
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
                plant_names.insert(codigo_i32, params.nome.clone());
                results.push(result);
            }
            Err(e) => {
                println!("  {:3} {:<16} │ ERROR: {}", codigo, params.nome, e);
            }
        }
    }

    println!("─────────────────────────────────────────────────────────────────────");
    println!();

    // Write results using labdessem-io output
    let records: Vec<ComputedFphaRecord> = results
        .iter()
        .flat_map(|r| {
            let name = plant_names.get(&r.codigo).cloned().unwrap_or_else(|| "UNKNOWN".to_string());
            r.planes.iter().enumerate().map(move |(idx, plane)| {
                ComputedFphaRecord {
                    codigo_usina: r.codigo as u32,
                    nome_usina: name.clone(),
                    plane_index: idx,
                    gamma_0: plane.gamma_0,
                    gamma_v: plane.gamma_v,
                    gamma_q: plane.gamma_q,
                    gamma_s: plane.gamma_s,
                    alpha: r.alpha,
                    max_deviation_mw: Some(r.deviation.max_abs_mw),
                    mean_deviation_mw: Some(r.deviation.mean_abs_mw),
                    rms_deviation_mw: Some(r.deviation.rms_mw),
                }
            })
        })
        .collect();

    write_fpha_computed(case_path, &records)?;
    println!("Results written to: {}/RESULTADOS/resultado_fpha_computed.csv", case_path.display());

    // Print summary
    let total_planes: usize = results.iter().map(|r| r.planes.len()).sum();
    let avg_mae: f64 = if results.is_empty() {
        0.0
    } else {
        results.iter().map(|r| r.deviation.mean_abs_mw).sum::<f64>() / results.len() as f64
    };

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
