//! Regression tests for FPHA fitting determinism and numerical stability.

#![allow(clippy::unwrap_used, clippy::expect_used, clippy::float_cmp)]

use labdessem_fpha::{
    compute_alpha_fpha, compute_fit_deviation, fit_gamma_s_for_planes, fit_hull_planes, fit_plant,
    reduce_planes, scale_planes, FitPlantParams, FittingBounds, ForebayTable, FphaPlantConfig,
    PlaneReductionConfig, ProductionFunction, TailraceModel, TailraceSource, VhaRow,
};

fn reference_forebay() -> ForebayTable {
    let rows = vec![
        VhaRow { volume_hm3: 0.0, height_m: 380.0, area_km2: 1000.0 },
        VhaRow { volume_hm3: 10_000.0, height_m: 396.0, area_km2: 1200.0 },
        VhaRow { volume_hm3: 20_000.0, height_m: 404.0, area_km2: 1300.0 },
        VhaRow { volume_hm3: 30_000.0, height_m: 408.0, area_km2: 1400.0 },
    ];
    ForebayTable::new(&rows, "REFERENCE").unwrap()
}

fn reference_pf() -> ProductionFunction {
    let tailrace = TailraceModel::Polynomial {
        coefficients: vec![0.0, 0.0008, -2e-8],
    };
    ProductionFunction::new(
        reference_forebay(),
        TailraceSource::Entity(Some(tailrace)),
        Some(2.0),
        Some(0.92),
        3_000.0,
        5_000.0,
        "REFERENCE".to_owned(),
    )
}

fn reference_bounds() -> FittingBounds {
    FittingBounds {
        v_min: 0.0,
        v_max: 30_000.0,
        n_volume_points: 6,
        n_flow_points: 6,
        n_spillage_points: 2,
        max_planes_per_hydro: 10,
        single_volume: false,
    }
}

#[test]
fn hull_fitting_is_bit_deterministic() {
    let pf = reference_pf();
    let bounds = reference_bounds();

    let planes1 = fit_hull_planes(&pf, &bounds).unwrap();
    let planes2 = fit_hull_planes(&pf, &bounds).unwrap();

    assert_eq!(planes1.len(), planes2.len());
    for (p1, p2) in planes1.iter().zip(&planes2) {
        assert_eq!(p1.gamma_0.to_bits(), p2.gamma_0.to_bits());
        assert_eq!(p1.gamma_v.to_bits(), p2.gamma_v.to_bits());
        assert_eq!(p1.gamma_q.to_bits(), p2.gamma_q.to_bits());
        assert_eq!(p1.gamma_s.to_bits(), p2.gamma_s.to_bits());
    }
}

#[test]
fn alpha_correction_is_bit_deterministic() {
    let pf = reference_pf();
    let bounds = reference_bounds();
    let planes = fit_hull_planes(&pf, &bounds).unwrap();

    let alpha1 = compute_alpha_fpha(&planes, &pf, &bounds);
    let alpha2 = compute_alpha_fpha(&planes, &pf, &bounds);

    assert_eq!(alpha1.to_bits(), alpha2.to_bits());
}

#[test]
fn gamma_s_fitting_is_bit_deterministic() {
    let pf = reference_pf();
    let bounds = reference_bounds();
    let raw = fit_hull_planes(&pf, &bounds).unwrap();
    let alpha = compute_alpha_fpha(&raw, &pf, &bounds);

    let mut planes1 = scale_planes(&raw, alpha);
    let mut planes2 = scale_planes(&raw, alpha);

    fit_gamma_s_for_planes(&mut planes1, &pf, &bounds, 1_500.0);
    fit_gamma_s_for_planes(&mut planes2, &pf, &bounds, 1_500.0);

    for (p1, p2) in planes1.iter().zip(&planes2) {
        assert_eq!(p1.gamma_s.to_bits(), p2.gamma_s.to_bits());
    }
}

#[test]
fn reduction_is_bit_deterministic() {
    let pf = reference_pf();
    let bounds = reference_bounds();
    let raw = fit_hull_planes(&pf, &bounds).unwrap();
    let alpha = compute_alpha_fpha(&raw, &pf, &bounds);
    let mut planes = scale_planes(&raw, alpha);
    fit_gamma_s_for_planes(&mut planes, &pf, &bounds, 1_500.0);

    let config = PlaneReductionConfig::Angle { tolerance_deg: 1.0 };

    let reduced1 = reduce_planes(&planes, &config, &pf, &bounds, 1, 0);
    let reduced2 = reduce_planes(&planes, &config, &pf, &bounds, 1, 0);

    assert_eq!(reduced1.len(), reduced2.len());
    for (p1, p2) in reduced1.iter().zip(&reduced2) {
        assert_eq!(p1.gamma_0.to_bits(), p2.gamma_0.to_bits());
    }
}

#[test]
fn full_pipeline_is_bit_deterministic() {
    let params = FitPlantParams {
        codigo: 1,
        nome: "REFERENCE".to_string(),
        forebay: reference_forebay(),
        tailrace: Some(TailraceModel::Polynomial {
            coefficients: vec![0.0, 0.0008, -2e-8],
        }),
        hydraulic_loss_m: Some(2.0),
        efficiency: Some(0.92),
        max_turbined_m3s: 3_000.0,
        max_generation_mw: 5_000.0,
        long_term_mean_inflow_m3s: 1_500.0,
        config: FphaPlantConfig::computed(10, 6, 6, PlaneReductionConfig::default()),
    };

    let r1 = fit_plant(&params).unwrap();
    let r2 = fit_plant(&params).unwrap();

    assert_eq!(r1.alpha.to_bits(), r2.alpha.to_bits());
    assert_eq!(r1.planes.len(), r2.planes.len());

    for (p1, p2) in r1.planes.iter().zip(&r2.planes) {
        assert_eq!(p1.gamma_0.to_bits(), p2.gamma_0.to_bits());
        assert_eq!(p1.gamma_v.to_bits(), p2.gamma_v.to_bits());
        assert_eq!(p1.gamma_q.to_bits(), p2.gamma_q.to_bits());
        assert_eq!(p1.gamma_s.to_bits(), p2.gamma_s.to_bits());
    }

    assert_eq!(r1.deviation.mean_abs_mw.to_bits(), r2.deviation.mean_abs_mw.to_bits());
}

#[test]
fn planes_satisfy_sign_constraints() {
    let params = FitPlantParams {
        codigo: 1,
        nome: "SIGN_TEST".to_string(),
        forebay: reference_forebay(),
        tailrace: Some(TailraceModel::Polynomial {
            coefficients: vec![0.0, 0.0008, -2e-8],
        }),
        hydraulic_loss_m: Some(2.0),
        efficiency: Some(0.92),
        max_turbined_m3s: 3_000.0,
        max_generation_mw: 5_000.0,
        long_term_mean_inflow_m3s: 1_500.0,
        config: FphaPlantConfig::computed(10, 6, 6, PlaneReductionConfig::default()),
    };

    let result = fit_plant(&params).unwrap();

    for (idx, plane) in result.planes.iter().enumerate() {
        assert!(
            plane.gamma_v >= -1e-10,
            "plane {}: γᵥ = {} must be >= 0",
            idx, plane.gamma_v
        );
        assert!(
            plane.gamma_q >= -1e-10,
            "plane {}: γᵩ = {} must be >= 0",
            idx, plane.gamma_q
        );
        assert!(
            plane.gamma_s <= 1e-10,
            "plane {}: γₛ = {} must be <= 0",
            idx, plane.gamma_s
        );
    }
}

#[test]
fn alpha_is_positive_and_reasonable() {
    let params = FitPlantParams {
        codigo: 1,
        nome: "ALPHA_TEST".to_string(),
        forebay: reference_forebay(),
        tailrace: Some(TailraceModel::Polynomial {
            coefficients: vec![0.0, 0.0008, -2e-8],
        }),
        hydraulic_loss_m: Some(2.0),
        efficiency: Some(0.92),
        max_turbined_m3s: 3_000.0,
        max_generation_mw: 5_000.0,
        long_term_mean_inflow_m3s: 1_500.0,
        config: FphaPlantConfig::computed(10, 6, 6, PlaneReductionConfig::default()),
    };

    let result = fit_plant(&params).unwrap();

    assert!(result.alpha > 0.0, "α must be positive");
    assert!(result.alpha > 0.5, "α should not be too small");
    assert!(result.alpha < 2.0, "α should not be too large");
}
