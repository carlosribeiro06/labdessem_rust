//! Integration tests for the FPHA fitting pipeline.

#![allow(clippy::unwrap_used, clippy::expect_used)]

use labdessem_fpha::{
    fit_plant, fit_plants, forebay_from_vha, tailrace_from_data,
    FitPlantParams, ForebayTable, FphaPlantConfig,
    PlaneReductionConfig, TailraceModel, VhaRow,
};
use labdessem_io::readers::{TailraceData, VhaGeometryData, VhaPoint};

fn storage_plant_params() -> FitPlantParams {
    let rows = vec![
        VhaRow { volume_hm3: 0.0, height_m: 380.0, area_km2: 1000.0 },
        VhaRow { volume_hm3: 10_000.0, height_m: 396.0, area_km2: 1200.0 },
        VhaRow { volume_hm3: 20_000.0, height_m: 404.0, area_km2: 1300.0 },
        VhaRow { volume_hm3: 30_000.0, height_m: 408.0, area_km2: 1400.0 },
    ];
    FitPlantParams {
        codigo: 1,
        nome: "STORAGE".to_string(),
        forebay: ForebayTable::new(&rows, "STORAGE").unwrap(),
        tailrace: Some(TailraceModel::Polynomial {
            coefficients: vec![0.0, 0.0008, -2e-8],
        }),
        hydraulic_loss_m: Some(2.0),
        efficiency: Some(0.92),
        max_turbined_m3s: 3_000.0,
        max_generation_mw: 5_000.0,
        long_term_mean_inflow_m3s: 1_500.0,
        config: FphaPlantConfig::computed(10, 6, 6, PlaneReductionConfig::default()),
    }
}

fn run_of_river_params() -> FitPlantParams {
    let rows = vec![
        VhaRow { volume_hm3: 14_999.0, height_m: 400.0, area_km2: 50.0 },
        VhaRow { volume_hm3: 15_001.0, height_m: 400.0, area_km2: 50.0 },
    ];
    FitPlantParams {
        codigo: 2,
        nome: "RUN_OF_RIVER".to_string(),
        forebay: ForebayTable::new(&rows, "RUN_OF_RIVER").unwrap(),
        tailrace: Some(TailraceModel::Polynomial {
            coefficients: vec![380.0],
        }),
        hydraulic_loss_m: None,
        efficiency: Some(0.90),
        max_turbined_m3s: 500.0,
        max_generation_mw: 100.0,
        long_term_mean_inflow_m3s: 400.0,
        config: FphaPlantConfig::computed(10, 2, 6, PlaneReductionConfig::default()),
    }
}

fn high_head_params() -> FitPlantParams {
    let rows = vec![
        VhaRow { volume_hm3: 0.0, height_m: 600.0, area_km2: 100.0 },
        VhaRow { volume_hm3: 5_000.0, height_m: 700.0, area_km2: 120.0 },
        VhaRow { volume_hm3: 10_000.0, height_m: 750.0, area_km2: 140.0 },
    ];
    FitPlantParams {
        codigo: 3,
        nome: "HIGH_HEAD".to_string(),
        forebay: ForebayTable::new(&rows, "HIGH_HEAD").unwrap(),
        tailrace: Some(TailraceModel::Polynomial {
            coefficients: vec![200.0, 0.001],
        }),
        hydraulic_loss_m: Some(5.0),
        efficiency: Some(0.88),
        max_turbined_m3s: 1_000.0,
        max_generation_mw: 4_000.0,
        long_term_mean_inflow_m3s: 800.0,
        config: FphaPlantConfig::computed(8, 4, 6, PlaneReductionConfig::default()),
    }
}

#[test]
fn storage_plant_produces_multi_plane_envelope() {
    let result = fit_plant(&storage_plant_params()).unwrap();

    assert!(result.planes.len() >= 2, "storage plant should produce multiple planes");
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
    assert!(result.deviation.relative < 0.15, "fit should be reasonably accurate");
}

#[test]
fn batch_fit_multiple_plant_types() {
    let results = fit_plants(&[
        storage_plant_params(),
        run_of_river_params(),
        high_head_params(),
    ]);

    assert_eq!(results.len(), 3);
    for result in &results {
        assert!(result.is_ok(), "all plants should fit: {:?}", result);
    }
}

#[test]
fn full_io_roundtrip() {
    // Create VHA geometry data using labdessem_io types
    let vha_data = VhaGeometryData {
        plant_code: 1,
        points: vec![
            VhaPoint { volume_hm3: 0.0, height_m: 380.0, area_km2: 1000.0 },
            VhaPoint { volume_hm3: 30000.0, height_m: 408.0, area_km2: 1400.0 },
        ],
    };

    // Create tailrace data using labdessem_io types
    let tailrace_data = TailraceData {
        plant_code: 1,
        coefficients: [0.0, 0.0008, -2e-8, 0.0, 0.0],
    };

    // Convert to domain types using from_io utilities
    let forebay = forebay_from_vha(&vha_data, "FURNAS").unwrap();
    let tailrace = tailrace_from_data(&tailrace_data);

    let params = FitPlantParams {
        codigo: 1,
        nome: "FURNAS".to_string(),
        forebay,
        tailrace: Some(tailrace),
        hydraulic_loss_m: Some(2.0),
        efficiency: Some(0.92),
        max_turbined_m3s: 3_000.0,
        max_generation_mw: 5_000.0,
        long_term_mean_inflow_m3s: 1_500.0,
        config: FphaPlantConfig::computed(10, 6, 6, PlaneReductionConfig::default()),
    };

    let result = fit_plant(&params).unwrap();

    // Verify the fitting produces valid results
    assert!(!result.planes.is_empty(), "should produce at least one plane");
    assert!(result.alpha > 0.0, "α must be positive");
    assert!(result.deviation.mean_abs_mw.is_finite(), "deviation must be finite");
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

// --- Run-of-River Single-Point VHA Tests (epic-01 ticket-004) ---

fn single_point_vha_params() -> FitPlantParams {
    let rows = vec![
        VhaRow { volume_hm3: 1500.0, height_m: 386.5, area_km2: 10.0 },
    ];
    FitPlantParams {
        codigo: 2,
        nome: "ITUTINGA".to_string(),
        forebay: ForebayTable::new(&rows, "ITUTINGA").unwrap(),
        tailrace: Some(TailraceModel::Polynomial {
            coefficients: vec![0.0, 0.0005, -1e-8],
        }),
        hydraulic_loss_m: Some(1.5),
        efficiency: Some(0.91),
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
