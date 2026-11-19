//! FPHA fitting pipeline orchestration.

use crate::config::FphaPlantConfig;
use crate::error::FphaFittingError;
#[cfg(test)]
use crate::fitting::PlaneReductionConfig;
use crate::fitting::{
    ForebayTable, FphaFitDeviation, ProductionFunction, RawPlane, TailraceSource,
    compute_alpha_fpha, compute_fit_deviation, fit_gamma_s_for_planes, fit_hull_planes,
    reduce_planes, resolve_fitting_bounds, scale_planes, validate_fitted_planes,
};
use crate::types::TailraceModel;

/// A fitted plant's FPHA result.
#[derive(Debug, Clone)]
pub struct FphaPlantResult {
    pub codigo: i32,
    pub nome: String,
    pub planes: Vec<RawPlane>,
    pub alpha: f64,
    pub deviation: FphaFitDeviation,
}

/// Parameters for fitting a single plant.
#[derive(Debug, Clone)]
pub struct FitPlantParams {
    pub codigo: i32,
    pub nome: String,
    pub forebay: ForebayTable,
    pub tailrace: Option<TailraceModel>,
    pub hydraulic_loss_m: Option<f64>,
    pub efficiency: Option<f64>,
    pub max_turbined_m3s: f64,
    pub max_generation_mw: f64,
    pub long_term_mean_inflow_m3s: f64,
    pub config: FphaPlantConfig,
}

/// Fit FPHA planes for a single plant.
///
/// # Contract — `alpha` is applied once, by the consumer
///
/// `FphaPlantResult::planes` are **unscaled**: they are not multiplied by `alpha`. The consumer
/// (the model's `correction_factor`) multiplies all four coefficients `γ₀, γᵥ, γᵩ, γₛ` by `alpha`
/// exactly once, so the LP envelope is `alpha × min_k P_k`. `FphaPlantResult::deviation` describes
/// that envelope, `alpha × planes`.
pub fn fit_plant(params: &FitPlantParams) -> Result<FphaPlantResult, FphaFittingError> {
    let pf = ProductionFunction::new(
        params.forebay.clone(),
        TailraceSource::Entity(params.tailrace.clone()),
        params.hydraulic_loss_m,
        params.efficiency,
        params.max_turbined_m3s,
        params.max_generation_mw,
        params.nome.clone(),
    );

    // Get config-based bounds first (n_volume_points, n_flow_points, etc.)
    let config_bounds = params
        .config
        .to_fitting_bounds(params.forebay.v_min(), params.forebay.v_max());

    // For run-of-river detection, useful storage is the forebay table span.
    // Single-point tables yield 0.0, triggering minimum half-width.
    let useful_storage_hm3 = params.forebay.v_max() - params.forebay.v_min();

    // Resolve bounds with single-volume synthesis for run-of-river plants
    let bounds = resolve_fitting_bounds(
        params.forebay.v_min(),
        params.forebay.v_max(),
        useful_storage_hm3,
        &config_bounds,
    );

    // Stage 1: Hull fitting
    let raw_planes = fit_hull_planes(&pf, &bounds).map_err(|e| FphaFittingError::HullFailed {
        hydro_name: params.nome.clone(),
        reason: e.to_string(),
    })?;

    if raw_planes.is_empty() {
        return Err(FphaFittingError::NoHyperplanesProduced {
            hydro_name: params.nome.clone(),
        });
    }

    // Stage 2: α correction (computed here, applied once by the consumer; planes stay unscaled)
    let alpha = compute_alpha_fpha(&raw_planes, &pf, &bounds);
    let mut planes = raw_planes;

    // Stage 3: γₛ secant fitting on the unscaled planes
    fit_gamma_s_for_planes(&mut planes, &pf, &bounds, params.long_term_mean_inflow_m3s);

    // Stage 4: Plane reduction on the unscaled planes
    let planes = reduce_planes(
        &planes,
        &params.config.reduction_config,
        &pf,
        &bounds,
        params.codigo,
        0,
    );

    // Stage 5: Deviation diagnostics of the envelope the LP uses, α × planes
    let deviation = compute_fit_deviation(&scale_planes(&planes, alpha), &pf, &bounds);

    // Stage 6: Validation (sign checks are scale-invariant for α > 0)
    validate_fitted_planes(&planes, alpha, &params.nome)?;

    Ok(FphaPlantResult {
        codigo: params.codigo,
        nome: params.nome.clone(),
        planes,
        alpha,
        deviation,
    })
}

/// Batch fit multiple plants.
pub fn fit_plants(
    params_list: &[FitPlantParams],
) -> Vec<Result<FphaPlantResult, FphaFittingError>> {
    params_list.iter().map(fit_plant).collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::fitting::{FittingBounds, VhaRow};

    fn test_forebay() -> ForebayTable {
        let rows = vec![
            VhaRow {
                volume_hm3: 0.0,
                height_m: 380.0,
                area_km2: 0.0,
            },
            VhaRow {
                volume_hm3: 10_000.0,
                height_m: 396.0,
                area_km2: 0.0,
            },
            VhaRow {
                volume_hm3: 20_000.0,
                height_m: 404.0,
                area_km2: 0.0,
            },
            VhaRow {
                volume_hm3: 30_000.0,
                height_m: 408.0,
                area_km2: 0.0,
            },
        ];
        ForebayTable::new(&rows, "Test").unwrap()
    }

    fn test_params() -> FitPlantParams {
        FitPlantParams {
            codigo: 1,
            nome: "FURNAS".to_string(),
            forebay: test_forebay(),
            tailrace: Some(TailraceModel::Polynomial {
                coefficients: vec![0.0, 0.0008, -2e-8],
            }),
            hydraulic_loss_m: Some(2.0),
            efficiency: Some(0.92),
            max_turbined_m3s: 3_000.0,
            max_generation_mw: 5_000.0,
            long_term_mean_inflow_m3s: 1_500.0,
            config: FphaPlantConfig::computed(
                10,
                6,
                6,
                PlaneReductionConfig::Angle { tolerance_deg: 1.0 },
            ),
        }
    }

    #[test]
    fn fit_plant_produces_valid_result() {
        let params = test_params();
        let result = fit_plant(&params).unwrap();

        assert_eq!(result.codigo, 1);
        assert_eq!(result.nome, "FURNAS");
        assert!(!result.planes.is_empty());
        assert!(result.alpha > 0.0);
    }

    #[test]
    fn planes_pass_validation() {
        let params = test_params();
        let result = fit_plant(&params).unwrap();

        for plane in &result.planes {
            assert!(plane.gamma_v >= -1e-10);
            assert!(plane.gamma_q >= -1e-10);
            assert!(plane.gamma_s <= 1e-10);
        }
    }

    #[test]
    fn batch_fit_handles_multiple_plants() {
        let mut params2 = test_params();
        params2.codigo = 2;
        params2.nome = "ITAIPU".to_string();

        let results = fit_plants(&[test_params(), params2]);
        assert_eq!(results.len(), 2);
        assert!(results[0].is_ok());
        assert!(results[1].is_ok());
    }

    #[test]
    fn fit_plant_is_deterministic() {
        let params = test_params();
        let result1 = fit_plant(&params).unwrap();
        let result2 = fit_plant(&params).unwrap();

        assert_eq!(result1.planes.len(), result2.planes.len());
        for (p1, p2) in result1.planes.iter().zip(&result2.planes) {
            assert_eq!(p1.gamma_0.to_bits(), p2.gamma_0.to_bits());
            assert_eq!(p1.gamma_v.to_bits(), p2.gamma_v.to_bits());
            assert_eq!(p1.gamma_q.to_bits(), p2.gamma_q.to_bits());
            assert_eq!(p1.gamma_s.to_bits(), p2.gamma_s.to_bits());
        }
    }

    fn fit_context(params: &FitPlantParams) -> (ProductionFunction, FittingBounds) {
        let pf = ProductionFunction::new(
            params.forebay.clone(),
            TailraceSource::Entity(params.tailrace.clone()),
            params.hydraulic_loss_m,
            params.efficiency,
            params.max_turbined_m3s,
            params.max_generation_mw,
            params.nome.clone(),
        );
        let config_bounds = params
            .config
            .to_fitting_bounds(params.forebay.v_min(), params.forebay.v_max());
        let bounds = resolve_fitting_bounds(
            params.forebay.v_min(),
            params.forebay.v_max(),
            params.forebay.v_max() - params.forebay.v_min(),
            &config_bounds,
        );
        (pf, bounds)
    }

    #[test]
    fn fit_plant_returns_unscaled_planes() -> Result<(), Box<dyn std::error::Error>> {
        let params = test_params();
        let result = fit_plant(&params)?;
        assert!(
            (result.alpha - 1.0).abs() > 1e-6,
            "fixture alpha {} must differ from 1 for the test to discriminate",
            result.alpha
        );

        let (pf, bounds) = fit_context(&params);
        let mut chain = fit_hull_planes(&pf, &bounds)?;
        fit_gamma_s_for_planes(&mut chain, &pf, &bounds, params.long_term_mean_inflow_m3s);
        let expected = reduce_planes(
            &chain,
            &params.config.reduction_config,
            &pf,
            &bounds,
            params.codigo,
            0,
        );

        assert_eq!(result.planes.len(), expected.len());
        for (got, want) in result.planes.iter().zip(&expected) {
            assert_eq!(got.gamma_0.to_bits(), want.gamma_0.to_bits());
            assert_eq!(got.gamma_v.to_bits(), want.gamma_v.to_bits());
            assert_eq!(got.gamma_q.to_bits(), want.gamma_q.to_bits());
            assert_eq!(got.gamma_s.to_bits(), want.gamma_s.to_bits());
        }
        Ok(())
    }

    #[test]
    fn fit_plant_deviation_matches_alpha_scaled_envelope() -> Result<(), Box<dyn std::error::Error>>
    {
        let params = test_params();
        let result = fit_plant(&params)?;
        let (pf, bounds) = fit_context(&params);

        let expected =
            compute_fit_deviation(&scale_planes(&result.planes, result.alpha), &pf, &bounds);

        assert_eq!(
            result.deviation.mean_abs_mw.to_bits(),
            expected.mean_abs_mw.to_bits()
        );
        assert_eq!(
            result.deviation.max_abs_mw.to_bits(),
            expected.max_abs_mw.to_bits()
        );
        assert_eq!(result.deviation.rms_mw.to_bits(), expected.rms_mw.to_bits());
        assert_eq!(
            result.deviation.mean_signed_mw.to_bits(),
            expected.mean_signed_mw.to_bits()
        );
        assert_eq!(
            result.deviation.relative.to_bits(),
            expected.relative.to_bits()
        );
        Ok(())
    }

    #[test]
    fn fit_plant_run_of_river() {
        // Single-point forebay table
        let rows = vec![VhaRow {
            volume_hm3: 500.0,
            height_m: 400.0,
            area_km2: 5.0,
        }];
        let forebay = ForebayTable::new(&rows, "FioAgua").unwrap();

        let params = FitPlantParams {
            codigo: 99,
            nome: "FIO_AGUA".to_string(),
            forebay,
            tailrace: Some(TailraceModel::Constant { level_m: 350.0 }),
            hydraulic_loss_m: Some(1.0),
            efficiency: Some(0.9),
            max_turbined_m3s: 100.0,
            max_generation_mw: 50.0,
            long_term_mean_inflow_m3s: 80.0,
            config: FphaPlantConfig::computed(10, 6, 6, PlaneReductionConfig::default()),
        };

        let result = fit_plant(&params);
        assert!(result.is_ok(), "run-of-river plant should fit successfully");

        let result = result.unwrap();
        for plane in &result.planes {
            assert_eq!(
                plane.gamma_v.to_bits(),
                0.0_f64.to_bits(),
                "run-of-river plant must have γ_V = 0"
            );
        }
    }
}
