//! FPHA fitting example.
//!
//! Usage:
//!   cargo run --example fpha_fit -- examples/fpha_example
//!
//! Reads from the case directory:
//!   - CAD/CAD_GEOMETRIA_UHE.csv: volume, forebay level and area per plant
//!   - CAD/CAD_TAILRACE.csv: tailrace polynomial per plant
//!   - CAD/CAD_FPHA_CONFIG.csv (optional): per-plant `num_planes`, `tolerance`, `min_volume_pct`
//!     and `max_volume_pct`
//!   - CAD/CAD_UHE_PARAMS.csv, columns
//!     `Codigo;Nome;MaxTurbinedM3s;MaxGenerationMW;HydraulicLossM;Efficiency;MeanInflowM3s`
//!
//! and writes results to RESULTADOS/. The fit follows the ADR-028 defaults and every
//! CAD_FPHA_CONFIG.csv field as the CLI's computed mode does, except that the volume window is a
//! fraction of the geometry-table span because the example has no CAD_UHE.csv `[Vmin, Vmax]`.

use std::collections::HashMap;
use std::env;
use std::path::Path;

use labdessem_fpha::config::{
    DEFAULT_ANGLE_TOLERANCE_DEG, DEFAULT_FLOW_POINTS, DEFAULT_MAX_PLANES, DEFAULT_VOLUME_POINTS,
};
use labdessem_fpha::{
    FitPlantParams, FphaPlantConfig, FphaPlantResult, PlaneReductionConfig, fit_plant,
    forebay_from_vha, tailrace_from_data,
};
use labdessem_io::output::{ComputedFphaRecord, write_fpha_computed};
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
    Ok(rows.into_iter().map(|r| (r.codigo, r)).collect())
}

fn example_config(row: Option<&labdessem_io::readers::FphaPlantConfig>) -> FphaPlantConfig {
    let mut config = FphaPlantConfig::computed(
        DEFAULT_MAX_PLANES,
        DEFAULT_VOLUME_POINTS,
        DEFAULT_FLOW_POINTS,
        PlaneReductionConfig::Angle {
            tolerance_deg: DEFAULT_ANGLE_TOLERANCE_DEG,
        },
    );

    if let Some(row) = row {
        if let Some(n) = row.num_planes {
            config.max_planes = n;
        }
        if let Some(tolerance_deg) = row.tolerance {
            config.reduction_config = PlaneReductionConfig::Angle { tolerance_deg };
        }
        if let Some(min_fraction) = row.min_volume_pct {
            config.volume_window.min_fraction = min_fraction;
        }
        if let Some(max_fraction) = row.max_volume_pct {
            config.volume_window.max_fraction = max_fraction;
        }
    }

    config
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

    println!("Fitting FPHA hyperplanes...");
    println!("─────────────────────────────────────────────────────────────────────");

    let mut results: Vec<FphaPlantResult> = Vec::new();
    let mut plant_names: HashMap<i32, String> = HashMap::new();

    for (codigo, vha) in &vha_data {
        let codigo_i32 = *codigo as i32;

        let Some(params) = plant_params.get(&codigo_i32) else {
            println!(
                "  ⚠ Plant {} not found in CAD_UHE_PARAMS.csv, skipping",
                codigo
            );
            continue;
        };

        let forebay = match forebay_from_vha(vha, &params.nome) {
            Ok(fb) => fb,
            Err(e) => {
                println!(
                    "  ⚠ Plant {} geometry conversion failed: {}, skipping",
                    codigo, e
                );
                continue;
            }
        };

        let fpha_config = example_config(configs.get(codigo));

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

    let records: Vec<ComputedFphaRecord> = results
        .iter()
        .flat_map(|r| {
            let name = plant_names
                .get(&r.codigo)
                .cloned()
                .unwrap_or_else(|| "UNKNOWN".to_string());
            r.planes
                .iter()
                .enumerate()
                .map(move |(idx, plane)| ComputedFphaRecord {
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
                })
        })
        .collect();

    write_fpha_computed(case_path, &records)?;
    println!(
        "Results written to: {}/RESULTADOS/resultado_fpha_computed.csv",
        case_path.display()
    );

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
    println!(
        "║  Plants fitted: {:3}                                              ║",
        results.len()
    );
    println!(
        "║  Total planes:  {:3}                                              ║",
        total_planes
    );
    println!(
        "║  Avg MAE:       {:6.2} MW                                        ║",
        avg_mae
    );
    println!("╚══════════════════════════════════════════════════════════════════╝");

    Ok(())
}
