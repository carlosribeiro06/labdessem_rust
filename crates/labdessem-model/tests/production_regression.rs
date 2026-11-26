//! Regression tests for FPHA fitting determinism and numerical stability.

#![allow(clippy::unwrap_used, clippy::expect_used, clippy::float_cmp)]

use labdessem_core::ids::HydroPlantId;
use labdessem_model::hydro_geometry::{ForebayTable, TailraceModel, VhaRow};
use labdessem_model::production::{
    FitPlantParams, FittingBounds, FphaPlantConfig, PlaneReductionConfig, ProductionFunction,
    compute_alpha_fpha, fit_gamma_s_for_planes, fit_hull_planes, fit_plant, reduce_planes,
};

fn reference_forebay() -> ForebayTable {
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
    ForebayTable::new(&rows, "REFERENCE").unwrap()
}

fn reference_pf() -> ProductionFunction {
    let tailrace = TailraceModel {
        coefficients: vec![0.0, 0.0008, -2e-8],
    };
    ProductionFunction::new(reference_forebay(), tailrace, 2.0, 0.92, 3_000.0, 5_000.0)
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

fn reference_params(name: &str) -> FitPlantParams {
    FitPlantParams {
        id: HydroPlantId(1),
        name: name.to_string(),
        forebay: reference_forebay(),
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
    let mut planes1 = fit_hull_planes(&pf, &bounds).unwrap();
    let mut planes2 = planes1.clone();

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
    let mut planes = fit_hull_planes(&pf, &bounds).unwrap();
    fit_gamma_s_for_planes(&mut planes, &pf, &bounds, 1_500.0);

    let config = PlaneReductionConfig { tolerance_deg: 1.0 };

    let reduced1 = reduce_planes(&planes, &config);
    let reduced2 = reduce_planes(&planes, &config);

    assert_eq!(reduced1.len(), reduced2.len());
    for (p1, p2) in reduced1.iter().zip(&reduced2) {
        assert_eq!(p1.gamma_0.to_bits(), p2.gamma_0.to_bits());
    }
}

#[test]
fn full_pipeline_is_bit_deterministic() {
    let params = reference_params("REFERENCE");

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

    assert_eq!(
        r1.deviation.mean_abs_mw.to_bits(),
        r2.deviation.mean_abs_mw.to_bits()
    );
}

#[test]
fn planes_satisfy_sign_constraints() {
    let params = reference_params("SIGN_TEST");

    let result = fit_plant(&params).unwrap();

    for (idx, plane) in result.planes.iter().enumerate() {
        assert!(
            plane.gamma_v >= -1e-10,
            "plane {}: γᵥ = {} must be >= 0",
            idx,
            plane.gamma_v
        );
        assert!(
            plane.gamma_q >= -1e-10,
            "plane {}: γᵩ = {} must be >= 0",
            idx,
            plane.gamma_q
        );
        assert!(
            plane.gamma_s <= 1e-10,
            "plane {}: γₛ = {} must be <= 0",
            idx,
            plane.gamma_s
        );
    }
}

#[test]
fn alpha_is_positive_and_reasonable() {
    let params = reference_params("ALPHA_TEST");

    let result = fit_plant(&params).unwrap();

    assert!(result.alpha > 0.0, "α must be positive");
    assert!(result.alpha > 0.5, "α should not be too small");
    assert!(result.alpha < 2.0, "α should not be too large");
}

#[test]
fn plane_limit_is_bit_deterministic() {
    let unlimited = fit_plant(&reference_params("PLANE_LIMIT")).unwrap();
    assert!(
        unlimited.planes.len() > 1,
        "reference fit must yield more than one plane for the limit to bind"
    );

    let mut params = reference_params("PLANE_LIMIT");
    params.config.max_planes = unlimited.planes.len() - 1;

    let r1 = fit_plant(&params).unwrap();
    let r2 = fit_plant(&params).unwrap();

    assert!(r1.planes.len() <= params.config.max_planes);
    assert_eq!(r1.alpha.to_bits(), r2.alpha.to_bits());
    assert_eq!(r1.planes.len(), r2.planes.len());
    for (p1, p2) in r1.planes.iter().zip(&r2.planes) {
        assert_eq!(p1.gamma_0.to_bits(), p2.gamma_0.to_bits());
        assert_eq!(p1.gamma_v.to_bits(), p2.gamma_v.to_bits());
        assert_eq!(p1.gamma_q.to_bits(), p2.gamma_q.to_bits());
        assert_eq!(p1.gamma_s.to_bits(), p2.gamma_s.to_bits());
    }
}
