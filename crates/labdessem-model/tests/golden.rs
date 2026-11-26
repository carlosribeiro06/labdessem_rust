//! Golden capture of the FPHA engine fixtures and the maximum-swallowing formula.

#![allow(clippy::unwrap_used, clippy::expect_used)]

use labdessem_core::ids::HydroPlantId;
use labdessem_core::test_support::golden::{GoldenFailure, GoldenRecord, check_or_bless};
use labdessem_model::hydro_geometry::{ForebayTable, TailraceModel, VhaRow};
use labdessem_model::production::{
    FitPlantParams, FphaPlantConfig, PlaneReductionConfig, VolumeWindow, fit_plant,
};
use labdessem_model::swallowing::calculate_max_swallowing;

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

fn fixtures() -> Vec<(&'static str, FitPlantParams)> {
    let unlimited_planes = fit_plant(&reference_params("PLANE_LIMIT"))
        .unwrap()
        .planes
        .len();
    let mut plane_limit = reference_params("PLANE_LIMIT");
    plane_limit.config.max_planes = unlimited_planes - 1;

    let mut window = reference_params("WINDOW");
    window.config.volume_window = VolumeWindow {
        min_fraction: 0.2,
        max_fraction: 0.8,
    };

    let mut angle_tolerance = reference_params("ANGLE_TOLERANCE");
    angle_tolerance.config.reduction_config = PlaneReductionConfig { tolerance_deg: 5.0 };

    let mut zero_mlt = reference_params("ZERO_MLT");
    zero_mlt.long_term_mean_inflow_m3s = 0.0;

    vec![
        ("reference", reference_params("REFERENCE")),
        ("plane_limit", plane_limit),
        ("window", window),
        ("angle_tolerance", angle_tolerance),
        ("storage", reference_params("STORAGE")),
        ("run_of_river", run_of_river_params()),
        ("high_head", high_head_params()),
        ("single_point_vha", single_point_vha_params()),
        ("zero_mlt", zero_mlt),
    ]
}

fn record_fit(record: &mut GoldenRecord, fixture: &str, params: &FitPlantParams) {
    let prefix = format!("fpha/{fixture}");
    match fit_plant(params) {
        Ok(result) => {
            record.text(format!("{prefix}/planes"), result.planes.len().to_string());
            for (index, plane) in result.planes.iter().enumerate() {
                let k = index + 1;
                record.f64(format!("{prefix}/plane={k}/gamma_0"), plane.gamma_0);
                record.f64(format!("{prefix}/plane={k}/gamma_v"), plane.gamma_v);
                record.f64(format!("{prefix}/plane={k}/gamma_q"), plane.gamma_q);
                record.f64(format!("{prefix}/plane={k}/gamma_s"), plane.gamma_s);
            }
            record.f64(format!("{prefix}/alpha"), result.alpha);
            let deviation = &result.deviation;
            record.f64(
                format!("{prefix}/deviation/mean_abs_mw"),
                deviation.mean_abs_mw,
            );
            record.f64(
                format!("{prefix}/deviation/max_abs_mw"),
                deviation.max_abs_mw,
            );
            record.f64(format!("{prefix}/deviation/rms_mw"), deviation.rms_mw);
            record.f64(
                format!("{prefix}/deviation/mean_signed_mw"),
                deviation.mean_signed_mw,
            );
            record.f64(format!("{prefix}/deviation/relative"), deviation.relative);
        }
        Err(error) => record.text(format!("{prefix}/error"), error.to_string()),
    }
}

#[test]
fn production_fixtures_match_golden() -> Result<(), GoldenFailure> {
    check_or_bless("production_fixtures.golden", || {
        let mut record = GoldenRecord::new();
        for (fixture, params) in fixtures() {
            record_fit(&mut record, fixture, &params);
        }
        record
    })
}

#[test]
fn swallowing_formula_matches_golden() -> Result<(), GoldenFailure> {
    check_or_bless("swallowing_formula.golden", || {
        let mut record = GoldenRecord::new();
        for q_ef in [500.0, 1234.5] {
            for h_ef in [80.0, 0.0, -1.0] {
                for h_l in [0.0, -2.0, 40.0, 80.0, 120.0] {
                    for alpha in [0.5, 0.2] {
                        record.f64(
                            format!(
                                "swallowing/formula/q_ef={q_ef:?}/h_ef={h_ef:?}/h_l={h_l:?}/alpha={alpha:?}"
                            ),
                            calculate_max_swallowing(q_ef, h_ef, h_l, alpha),
                        );
                    }
                }
            }
        }
        record
    })
}
