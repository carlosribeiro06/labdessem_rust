//! FPHA fitting pipeline orchestration.

use crate::config::FphaPlantConfig;
use crate::error::FphaFittingError;
use crate::fitting::{
    compute_alpha_fpha, compute_fit_deviation, fit_gamma_s_for_planes, fit_hull_planes,
    reduce_planes, scale_planes, validate_fitted_planes, ForebayTable, ProductionFunction,
    TailraceSource,
};
#[cfg(test)]
use crate::fitting::PlaneReductionConfig;
use crate::io::FphaPlantResult;
use crate::types::TailraceModel;

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

    let bounds = params.config.to_fitting_bounds(
        params.forebay.v_min(),
        params.forebay.v_max(),
    );

    // Stage 1: Hull fitting
    let raw_planes = fit_hull_planes(&pf, &bounds).map_err(|e| {
        FphaFittingError::HullFailed {
            hydro_name: params.nome.clone(),
            reason: e.to_string(),
        }
    })?;

    if raw_planes.is_empty() {
        return Err(FphaFittingError::NoHyperplanesProduced {
            hydro_name: params.nome.clone(),
        });
    }

    // Stage 2: α correction
    let alpha = compute_alpha_fpha(&raw_planes, &pf, &bounds);
    let mut planes = scale_planes(&raw_planes, alpha);

    // Stage 3: γₛ secant fitting
    fit_gamma_s_for_planes(&mut planes, &pf, &bounds, params.long_term_mean_inflow_m3s);

    // Stage 4: Plane reduction
    let planes = reduce_planes(
        &planes,
        &params.config.reduction_config,
        &pf,
        &bounds,
        params.codigo,
        0,
    );

    // Stage 5: Deviation diagnostics
    let deviation = compute_fit_deviation(&planes, &pf, &bounds);

    // Stage 6: Validation
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
pub fn fit_plants(params_list: &[FitPlantParams]) -> Vec<Result<FphaPlantResult, FphaFittingError>> {
    params_list.iter().map(fit_plant).collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::fitting::VhaRow;

    fn test_forebay() -> ForebayTable {
        let rows = vec![
            VhaRow { volume_hm3: 0.0, height_m: 380.0, area_km2: 0.0 },
            VhaRow { volume_hm3: 10_000.0, height_m: 396.0, area_km2: 0.0 },
            VhaRow { volume_hm3: 20_000.0, height_m: 404.0, area_km2: 0.0 },
            VhaRow { volume_hm3: 30_000.0, height_m: 408.0, area_km2: 0.0 },
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
                10, 6, 6,
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
}
