//! Integration tests for the FPHA fitting pipeline.

#![allow(clippy::unwrap_used, clippy::expect_used)]

use std::error::Error;
use std::path::Path;

use labdessem_core::ids::HydroPlantId;
use labdessem_io::read_study_from_path;
use labdessem_io::readers::read_fpha_config;
use labdessem_model::hydro_geometry::{ForebayTable, TailraceModel, VhaRow};
use labdessem_model::production::{
    FitPlantParams, FphaPlantConfig, PlaneReductionConfig, fit_plant, fit_system,
};

fn storage_plant_params() -> FitPlantParams {
    let rows = vec![
        VhaRow {
            volume_hm3: 0.0,
            height_m: 380.0,
        },
        VhaRow {
            volume_hm3: 10_000.0,
            height_m: 396.0,
        },
        VhaRow {
            volume_hm3: 20_000.0,
            height_m: 404.0,
        },
        VhaRow {
            volume_hm3: 30_000.0,
            height_m: 408.0,
        },
    ];
    FitPlantParams {
        id: HydroPlantId(1),
        name: "STORAGE".to_string(),
        forebay: ForebayTable::new(&rows, "STORAGE").unwrap(),
        tailrace: TailraceModel {
            coefficients: vec![0.0, 0.0008, -2e-8],
        },
        hydraulic_loss_m: 2.0,
        efficiency: 0.92,
        max_turbined_m3s: 3_000.0,
        max_generation_mw: 5_000.0,
        long_term_mean_inflow_m3s: 1_500.0,
        config: FphaPlantConfig::computed(10, 6, 6, PlaneReductionConfig::default()),
    }
}

fn run_of_river_params() -> FitPlantParams {
    let rows = vec![
        VhaRow {
            volume_hm3: 14_999.0,
            height_m: 400.0,
        },
        VhaRow {
            volume_hm3: 15_001.0,
            height_m: 400.0,
        },
    ];
    FitPlantParams {
        id: HydroPlantId(2),
        name: "RUN_OF_RIVER".to_string(),
        forebay: ForebayTable::new(&rows, "RUN_OF_RIVER").unwrap(),
        tailrace: TailraceModel {
            coefficients: vec![380.0],
        },
        hydraulic_loss_m: 0.0,
        efficiency: 0.90,
        max_turbined_m3s: 500.0,
        max_generation_mw: 100.0,
        long_term_mean_inflow_m3s: 400.0,
        config: FphaPlantConfig::computed(10, 2, 6, PlaneReductionConfig::default()),
    }
}

fn high_head_params() -> FitPlantParams {
    let rows = vec![
        VhaRow {
            volume_hm3: 0.0,
            height_m: 600.0,
        },
        VhaRow {
            volume_hm3: 5_000.0,
            height_m: 700.0,
        },
        VhaRow {
            volume_hm3: 10_000.0,
            height_m: 750.0,
        },
    ];
    FitPlantParams {
        id: HydroPlantId(3),
        name: "HIGH_HEAD".to_string(),
        forebay: ForebayTable::new(&rows, "HIGH_HEAD").unwrap(),
        tailrace: TailraceModel {
            coefficients: vec![200.0, 0.001],
        },
        hydraulic_loss_m: 5.0,
        efficiency: 0.88,
        max_turbined_m3s: 1_000.0,
        max_generation_mw: 4_000.0,
        long_term_mean_inflow_m3s: 800.0,
        config: FphaPlantConfig::computed(8, 4, 6, PlaneReductionConfig::default()),
    }
}

#[test]
fn storage_plant_produces_multi_plane_envelope() {
    let result = fit_plant(&storage_plant_params()).unwrap();

    assert!(
        result.planes.len() >= 2,
        "storage plant should produce multiple planes"
    );
    assert!(result.alpha > 0.0, "α must be positive");
    assert!(result.alpha < 2.0, "α should be close to 1.0");
}

#[test]
fn run_of_river_produces_gamma_v_near_zero() {
    let result = fit_plant(&run_of_river_params()).unwrap();

    for plane in &result.planes {
        assert!(
            plane.gamma_v.abs() < 1e-4,
            "run-of-river should have small γᵥ, got {}",
            plane.gamma_v
        );
    }
}

#[test]
fn high_head_plant_fits_successfully() {
    let result = fit_plant(&high_head_params()).unwrap();

    assert!(!result.planes.is_empty());
    assert!(
        result.deviation.relative < 0.15,
        "fit should be reasonably accurate"
    );
}

#[test]
fn determinism_across_multiple_runs() {
    let params = storage_plant_params();

    let r1 = fit_plant(&params).unwrap();
    let r2 = fit_plant(&params).unwrap();
    let r3 = fit_plant(&params).unwrap();

    assert_eq!(r1.planes.len(), r2.planes.len());
    assert_eq!(r2.planes.len(), r3.planes.len());

    for ((p1, p2), p3) in r1.planes.iter().zip(&r2.planes).zip(&r3.planes) {
        assert_eq!(p1.gamma_0.to_bits(), p2.gamma_0.to_bits());
        assert_eq!(p2.gamma_0.to_bits(), p3.gamma_0.to_bits());
    }
}

fn single_point_vha_params() -> FitPlantParams {
    let rows = vec![VhaRow {
        volume_hm3: 1500.0,
        height_m: 386.5,
    }];
    FitPlantParams {
        id: HydroPlantId(2),
        name: "ITUTINGA".to_string(),
        forebay: ForebayTable::new(&rows, "ITUTINGA").unwrap(),
        tailrace: TailraceModel {
            coefficients: vec![0.0, 0.0005, -1e-8],
        },
        hydraulic_loss_m: 1.5,
        efficiency: 0.91,
        max_turbined_m3s: 150.0,
        max_generation_mw: 40.0,
        long_term_mean_inflow_m3s: 120.0,
        config: FphaPlantConfig::computed(10, 6, 6, PlaneReductionConfig::default()),
    }
}

#[test]
fn run_of_river_single_point_vha() {
    let params = single_point_vha_params();
    let result = fit_plant(&params).expect("single-point VHA must fit");

    assert!(!result.planes.is_empty(), "must produce at least one plane");
    for plane in &result.planes {
        assert_eq!(
            plane.gamma_v.to_bits(),
            0.0_f64.to_bits(),
            "run-of-river: γ_V must be exactly 0.0"
        );
        assert!(plane.gamma_q >= 0.0, "γ_Q must be non-negative");
    }
}

#[test]
fn regression_multi_point_vha_unchanged() {
    let params = storage_plant_params();
    let result = fit_plant(&params).expect("multi-point VHA must fit");

    // Multi-point plants should have non-zero γ_V (volume matters)
    let has_nonzero_gamma_v = result.planes.iter().any(|p| p.gamma_v > 1e-9);
    assert!(has_nonzero_gamma_v, "multi-point plant should have γ_V > 0");

    // All planes valid
    for plane in &result.planes {
        assert!(plane.gamma_v >= -1e-10);
        assert!(plane.gamma_q >= -1e-10);
    }
}

#[test]
fn run_of_river_deterministic() {
    let params = single_point_vha_params();

    let r1 = fit_plant(&params).unwrap();
    let r2 = fit_plant(&params).unwrap();

    assert_eq!(r1.planes.len(), r2.planes.len());
    for (p1, p2) in r1.planes.iter().zip(&r2.planes) {
        assert_eq!(p1.gamma_0.to_bits(), p2.gamma_0.to_bits());
        assert_eq!(p1.gamma_v.to_bits(), p2.gamma_v.to_bits());
        assert_eq!(p1.gamma_q.to_bits(), p2.gamma_q.to_bits());
        assert_eq!(p1.gamma_s.to_bits(), p2.gamma_s.to_bits());
    }
}

/// `examples/24Barras` read through io fits every plant with generating units, in plant order
/// (the System-fed replacement of the deleted `full_io_roundtrip`).
#[test]
fn fit_system_fits_every_24barras_plant() -> Result<(), Box<dyn Error>> {
    let case = Path::new(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../../examples/24Barras"
    ));
    let system = read_study_from_path(case)?;
    let overrides = read_fpha_config(case)?;

    let fit = fit_system(&system, &overrides)
        .expect("every 24Barras plant with units has an inflow history row");

    assert_eq!(fit.plants_without_units, 0);
    let ids: Vec<usize> = fit.plants.iter().map(|plant| plant.id.0).collect();
    assert_eq!(ids, [0, 1, 2, 3]);
    for plant in &fit.plants {
        let code = plant.id.0;
        let result = plant
            .outcome
            .as_ref()
            .map_err(|reason| format!("plant {code}: {reason:?}"))?;
        assert!(!result.planes.is_empty(), "plant {code}: no planes");
        assert!(result.alpha > 0.0, "plant {code}: alpha {}", result.alpha);
        assert!(
            result.deviation.mean_abs_mw.is_finite(),
            "plant {code}: mean deviation {}",
            result.deviation.mean_abs_mw
        );
        assert_eq!(plant.ignored_window, None);
    }
    Ok(())
}
