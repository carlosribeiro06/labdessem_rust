//! Fit-input assembly for computed FPHA (RD-14): builds each plant's `FitPlantParams` from the
//! `System`, fits it, and returns the outcomes in plant order. This module owns the FPHA
//! missing-data policy (RD-22): a plant with generating units and no tailrace polynomial,
//! efficiency or hydraulic loss is a typed failure, never a default value.
//!
//! Results, failures and warnings are data; the caller renders and prints them (R17).

use std::collections::HashMap;

use labdessem_core::entities::{HydroFphaSegment, HydroPlant, InflowHistoryRow};
use labdessem_core::ids::HydroPlantId;
use labdessem_core::system::System;

use super::config::{
    DEFAULT_ANGLE_TOLERANCE_DEG, DEFAULT_FLOW_POINTS, DEFAULT_MAX_PLANES, DEFAULT_VOLUME_POINTS,
    FphaConfigOverrides, FphaPlantConfig,
};
use super::error::FphaFittingError;
use super::pipeline::{FitPlantParams, FphaPlantResult, fit_plant};
use super::reduction::PlaneReductionConfig;
use crate::hydro_geometry::plant_geometry;

/// A physical input that computed FPHA requires for every plant with generating units.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FphaInput {
    Tailrace,
    Efficiency,
    HydraulicLoss,
}

/// Why a plant with generating units has no computed FPHA.
#[derive(Debug)]
pub enum FitFailureReason {
    /// The plant has no VHA table.
    MissingGeometry,
    /// The plant lacks these inputs, in `FphaInput` order.
    MissingInputs(Vec<FphaInput>),
    /// The VHA table is malformed (`FphaFittingError::Geometry`) or the fit failed.
    Fitting(FphaFittingError),
}

/// The inflow history has rows but none for this plant with generating units. The batch stops:
/// a missing history row is fatal, not a per-plant failure.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MissingInflowHistory {
    pub id: HydroPlantId,
    pub name: String,
}

/// A configured volume window that the plant's single-point VHA table ignores (`fit_context`
/// fits a single-point table over its one volume).
#[derive(Debug, Clone, PartialEq)]
pub struct IgnoredVolumeWindow {
    pub id: HydroPlantId,
    pub name: String,
    pub min_volume_fraction: Option<f64>,
    pub max_volume_fraction: Option<f64>,
}

/// The outcome of one plant with generating units.
#[derive(Debug)]
pub struct PlantFit {
    pub id: HydroPlantId,
    pub name: String,
    /// Set only for a plant that reached the fit.
    pub ignored_window: Option<IgnoredVolumeWindow>,
    pub outcome: Result<FphaPlantResult, FitFailureReason>,
}

/// The outcomes of the plants with generating units, in `System::hydro_plants` order.
#[derive(Debug)]
pub struct SystemFit {
    pub plants: Vec<PlantFit>,
    pub plants_without_units: usize,
}

/// Fit every plant of `system` that has generating units; plants without units get no FPHA.
///
/// Per plant, in this order: no VHA table is `MissingGeometry`; then `build_fit_params`; then the
/// ignored-window check; then `fit_plant`. The first plant whose inflow history row is missing
/// stops the batch with `MissingInflowHistory`.
pub fn fit_system(
    system: &System,
    overrides: &HashMap<HydroPlantId, FphaConfigOverrides>,
) -> Result<SystemFit, MissingInflowHistory> {
    let period_duration_hours = system.horizon.period_duration_hours;
    let mlt_table = build_long_term_mean_inflow_table(&system.inflow_history);
    let history_present = !system.inflow_history.is_empty();

    let mut plants = Vec::new();
    let mut plants_without_units = 0;
    for plant in &system.hydro_plants {
        if plant.groups.is_empty() {
            plants_without_units += 1;
            continue;
        }
        let plant_overrides = overrides.get(&plant.id);
        let mut ignored_window = None;
        let outcome = if plant.volume_level_table.is_none() {
            Err(FitFailureReason::MissingGeometry)
        } else {
            match build_fit_params(
                plant,
                plant_overrides,
                period_duration_hours,
                &mlt_table,
                history_present,
            )? {
                Ok(params) => {
                    ignored_window = ignored_volume_window(plant, plant_overrides);
                    fit_plant(&params).map_err(FitFailureReason::Fitting)
                }
                Err(reason) => Err(reason),
            }
        };
        plants.push(PlantFit {
            id: plant.id,
            name: plant.name.clone(),
            ignored_window,
            outcome,
        });
    }
    Ok(SystemFit {
        plants,
        plants_without_units,
    })
}

/// Mean inflow per plant; a row with a negative `hydro_id` names no plant and is skipped.
pub fn build_long_term_mean_inflow_table(
    inflow_history: &[InflowHistoryRow],
) -> HashMap<HydroPlantId, f64> {
    let mut acc: HashMap<HydroPlantId, (f64, u64)> = HashMap::new();

    for row in inflow_history {
        let Ok(id) = usize::try_from(row.hydro_id) else {
            continue;
        };
        let entry = acc.entry(HydroPlantId(id)).or_insert((0.0, 0));
        entry.0 += row.value_m3s;
        entry.1 += 1;
    }

    acc.into_iter()
        .map(|(id, (sum, count))| (id, sum / count as f64))
        .collect()
}

/// The fit parameters of `plant`, checked in this order: the missing inputs, the VHA table, the
/// long-term mean inflow.
///
/// The outer `Err` is fatal: `history_present` and no `mlt_table` row for the plant. The inner
/// `Err` is the plant's own failure.
pub fn build_fit_params(
    plant: &HydroPlant,
    overrides: Option<&FphaConfigOverrides>,
    period_duration_hours: f64,
    mlt_table: &HashMap<HydroPlantId, f64>,
    history_present: bool,
) -> Result<Result<FitPlantParams, FitFailureReason>, MissingInflowHistory> {
    let (Some(_), Some(efficiency), Some(hydraulic_loss_m)) = (
        plant.tailrace_coefficients,
        plant.efficiency,
        plant.hydraulic_loss_m,
    ) else {
        let mut missing_inputs = Vec::new();
        if plant.tailrace_coefficients.is_none() {
            missing_inputs.push(FphaInput::Tailrace);
        }
        if plant.efficiency.is_none() {
            missing_inputs.push(FphaInput::Efficiency);
        }
        if plant.hydraulic_loss_m.is_none() {
            missing_inputs.push(FphaInput::HydraulicLoss);
        }
        return Ok(Err(FitFailureReason::MissingInputs(missing_inputs)));
    };

    let (forebay, tailrace) = match plant_geometry(plant) {
        Ok(Some(geometry)) => geometry,
        Ok(None) => return Ok(Err(FitFailureReason::MissingGeometry)),
        Err(e) => return Ok(Err(FitFailureReason::Fitting(e.into()))),
    };

    let long_term_mean_inflow_m3s =
        long_term_mean_inflow(plant, period_duration_hours, mlt_table, history_present)
            .ok_or_else(|| MissingInflowHistory {
                id: plant.id,
                name: plant.name.clone(),
            })?;

    Ok(Ok(FitPlantParams {
        id: plant.id,
        name: plant.name.clone(),
        forebay,
        tailrace,
        hydraulic_loss_m,
        efficiency,
        max_turbined_m3s: max_turbined_m3s(plant, period_duration_hours),
        max_generation_mw: max_generation_mw(plant),
        long_term_mean_inflow_m3s,
        config: build_fpha_config(
            overrides,
            (
                plant.reservoir.min_volume_hm3,
                plant.reservoir.max_volume_hm3,
            ),
        ),
    }))
}

/// The plant's turbined-flow limit (m3/s): the units of each group, then the groups (RD-23).
///
/// Max swallowing folds the same limits flat over every unit (`swallowing::PlantReference`), which
/// can differ in the last bit; the formulation docs record the unification as a known limitation.
fn max_turbined_m3s(plant: &HydroPlant, period_duration_hours: f64) -> f64 {
    plant
        .groups
        .iter()
        .map(|g| g.units.iter().map(|u| u.max_turbining_hm3).sum::<f64>())
        .sum::<f64>()
        * 1e6
        / (period_duration_hours * 3600.0)
}

/// The plant's generation limit (MW): the units of each group, then the groups (RD-23).
fn max_generation_mw(plant: &HydroPlant) -> f64 {
    plant
        .groups
        .iter()
        .map(|g| g.units.iter().map(|u| u.max_generation_mw).sum::<f64>())
        .sum()
}

/// The `mlt_table` mean; without inflow history, the mean deck inflow (0.0 when the deck has
/// none); `None` when the history exists but has no row for the plant.
fn long_term_mean_inflow(
    plant: &HydroPlant,
    period_duration_hours: f64,
    mlt_table: &HashMap<HydroPlantId, f64>,
    history_present: bool,
) -> Option<f64> {
    if let Some(&mlt) = mlt_table.get(&plant.id) {
        Some(mlt)
    } else if history_present {
        None
    } else if plant.natural_inflow_hm3.is_empty() {
        Some(0.0)
    } else {
        let mean_hm3 =
            plant.natural_inflow_hm3.iter().sum::<f64>() / plant.natural_inflow_hm3.len() as f64;
        Some(mean_hm3 * 1e6 / (period_duration_hours * 3600.0))
    }
}

/// The ADR-028 defaults, with the reference volumes of the window and the plant's overrides.
fn build_fpha_config(
    overrides: Option<&FphaConfigOverrides>,
    reference_hm3: (f64, f64),
) -> FphaPlantConfig {
    let mut fpha_config = FphaPlantConfig::computed(
        DEFAULT_MAX_PLANES,
        DEFAULT_VOLUME_POINTS,
        DEFAULT_FLOW_POINTS,
        PlaneReductionConfig {
            tolerance_deg: DEFAULT_ANGLE_TOLERANCE_DEG,
        },
    );
    fpha_config.window_reference_hm3 = Some(reference_hm3);

    if let Some(c) = overrides {
        if let Some(n) = c.max_planes {
            fpha_config.max_planes = n;
        }
        if let Some(tolerance_deg) = c.tolerance_deg {
            fpha_config.reduction_config = PlaneReductionConfig { tolerance_deg };
        }
        if let Some(min_fraction) = c.min_volume_fraction {
            fpha_config.volume_window.min_fraction = min_fraction;
        }
        if let Some(max_fraction) = c.max_volume_fraction {
            fpha_config.volume_window.max_fraction = max_fraction;
        }
    }

    fpha_config
}

/// The configured volume window of `plant` when its VHA table has a single point.
pub fn ignored_volume_window(
    plant: &HydroPlant,
    overrides: Option<&FphaConfigOverrides>,
) -> Option<IgnoredVolumeWindow> {
    let overrides = overrides?;
    let single_point = plant
        .volume_level_table
        .as_ref()
        .is_some_and(|table| table.len() == 1);
    let configured =
        overrides.min_volume_fraction.is_some() || overrides.max_volume_fraction.is_some();
    (single_point && configured).then(|| IgnoredVolumeWindow {
        id: plant.id,
        name: plant.name.clone(),
        min_volume_fraction: overrides.min_volume_fraction,
        max_volume_fraction: overrides.max_volume_fraction,
    })
}

/// Convert a fit result to model segments.
///
/// Segments carry the unscaled planes. The model multiplies all four coefficients
/// (`gamma_0` as the RHS, `gamma_v`, `gamma_q`, `gamma_s`) by `correction_factor` once.
pub fn convert_to_fpha_segments(result: &FphaPlantResult) -> Vec<HydroFphaSegment> {
    result
        .planes
        .iter()
        .enumerate()
        .map(|(idx, plane)| HydroFphaSegment {
            segment: idx + 1, // 1-indexed
            correction_factor: result.alpha,
            rhs: plane.gamma_0,
            volume_coefficient: plane.gamma_v,
            turbining_coefficient: plane.gamma_q,
            lateral_flow_coefficient: plane.gamma_s,
        })
        .collect()
}

#[cfg(test)]
#[allow(clippy::unwrap_used, clippy::expect_used, clippy::float_cmp)]
mod tests {
    use std::collections::HashMap;

    use labdessem_core::entities::{HydroGroup, HydroPlant, HydroUnit, InflowHistoryRow};
    use labdessem_core::ids::HydroPlantId;
    use labdessem_core::test_support::{minimal_hydro, minimal_system};

    use super::super::config::{FphaConfigOverrides, VolumeWindow};
    use super::super::deviation::FphaFitDeviation;
    use super::super::hull_fit::RawPlane;
    use super::super::pipeline::{FitPlantParams, FphaPlantResult};
    use super::{
        FitFailureReason, FphaInput, IgnoredVolumeWindow, MissingInflowHistory, build_fit_params,
        build_fpha_config, build_long_term_mean_inflow_table, convert_to_fpha_segments, fit_system,
        ignored_volume_window,
    };

    fn fittable_plant(id: usize) -> HydroPlant {
        let mut plant = minimal_hydro(id, 1, 1, 1.0, 0.1, 10.0, 2);
        plant.volume_level_table = Some(vec![(0.0, 100.0), (1000.0, 110.0)]);
        plant.tailrace_coefficients = Some([50.0, 0.0, 0.0, 0.0, 0.0]);
        plant.efficiency = Some(0.9);
        plant.hydraulic_loss_m = Some(1.0);
        plant
    }

    fn with_groups(plant: &mut HydroPlant, groups: &[&[f64]], field: fn(f64) -> (f64, f64)) {
        let group = plant.groups[0].clone();
        let unit = group.units[0].clone();
        plant.groups = groups
            .iter()
            .map(|values| HydroGroup {
                units: values
                    .iter()
                    .map(|&value| {
                        let (max_turbining_hm3, max_generation_mw) = field(value);
                        HydroUnit {
                            max_turbining_hm3,
                            max_generation_mw,
                            ..unit.clone()
                        }
                    })
                    .collect(),
                ..group.clone()
            })
            .collect();
    }

    fn checked_params(
        plant: &HydroPlant,
        period_duration_hours: f64,
        mlt_table: &HashMap<HydroPlantId, f64>,
        history_present: bool,
    ) -> FitPlantParams {
        build_fit_params(
            plant,
            None,
            period_duration_hours,
            mlt_table,
            history_present,
        )
        .expect("inflow history row")
        .expect("fittable plant")
    }

    fn inflow(hydro_id: i32, start: &str, end: &str, value_m3s: f64) -> InflowHistoryRow {
        InflowHistoryRow {
            hydro_id,
            start_date: start.parse().expect("ISO date"),
            end_date: end.parse().expect("ISO date"),
            value_m3s,
        }
    }

    fn result_with_planes(planes: Vec<RawPlane>, alpha: f64) -> FphaPlantResult {
        FphaPlantResult {
            id: HydroPlantId(1),
            name: "TEST".to_string(),
            planes,
            alpha,
            deviation: FphaFitDeviation {
                mean_abs_mw: 0.0,
                max_abs_mw: 0.0,
                rms_mw: 0.0,
                mean_signed_mw: 0.0,
                relative: 0.0,
            },
        }
    }

    #[test]
    fn build_fpha_config_uses_adr028_defaults() {
        let config = build_fpha_config(None, (120.0, 792.0));

        assert_eq!(config.max_planes, 10);
        assert_eq!(config.volume_points, 5);
        assert_eq!(config.flow_points, 5);
        assert_eq!(
            config.reduction_config.tolerance_deg.to_bits(),
            1.0_f64.to_bits()
        );
        assert_eq!(config.volume_window, VolumeWindow::FULL);
        assert_eq!(config.window_reference_hm3, Some((120.0, 792.0)));
    }

    #[test]
    fn build_fpha_config_maps_every_field() {
        let io_config = FphaConfigOverrides {
            max_planes: Some(7),
            tolerance_deg: Some(0.005),
            min_volume_fraction: Some(0.25),
            max_volume_fraction: Some(0.75),
        };

        let config = build_fpha_config(Some(&io_config), (100.0, 900.0));

        assert_eq!(config.max_planes, 7);
        assert_eq!(
            config.reduction_config.tolerance_deg.to_bits(),
            0.005_f64.to_bits()
        );
        assert_eq!(
            config.volume_window,
            VolumeWindow {
                min_fraction: 0.25,
                max_fraction: 0.75
            }
        );
        assert_eq!(config.window_reference_hm3, Some((100.0, 900.0)));
        assert_eq!(config.volume_points, 5);
        assert_eq!(config.flow_points, 5);

        let min_only = FphaConfigOverrides {
            min_volume_fraction: Some(0.3),
            ..FphaConfigOverrides::default()
        };
        assert_eq!(
            build_fpha_config(Some(&min_only), (100.0, 900.0)).volume_window,
            VolumeWindow {
                min_fraction: 0.3,
                max_fraction: 1.0
            }
        );
    }

    /// Pmax sums the units of each group, then the groups (RD-23). Unit limits 0.1 and 0.2 + 0.3
    /// give 0.6 (`0x3fe3_3333_3333_3333`); one flat fold over the units would give
    /// 0.6000000000000001 (`0x3fe3_3333_3333_3334`).
    #[test]
    fn build_fit_params_sums_pmax_per_group_then_across_groups() {
        let mut plant = fittable_plant(1);
        with_groups(&mut plant, &[&[0.1], &[0.2, 0.3]], |pmax| (0.1, pmax));

        let params = checked_params(&plant, 1.0, &HashMap::new(), false);

        assert_eq!(params.max_generation_mw.to_bits(), 0x3fe3_3333_3333_3333);
    }

    /// Q_max sums the units of each group, then the groups (RD-23). The FURNAS unit limits give
    /// 1506.0 m3/s (`0x4097_8800_0000_0000`); one flat fold over the units would give one ulp less
    /// (`0x4097_87ff_ffff_ffff`).
    #[test]
    fn build_fit_params_sums_qmax_per_group_then_across_groups() {
        let (q188, q189) = (188.0 * 0.5 * 0.0036, 189.0 * 0.5 * 0.0036);
        let mut plant = fittable_plant(1);
        with_groups(&mut plant, &[&[q188; 6], &[q189; 2]], |q| (q, 1.0));

        let params = checked_params(&plant, 0.5, &HashMap::new(), false);

        assert_eq!(params.max_turbined_m3s.to_bits(), 0x4097_8800_0000_0000);
    }

    #[test]
    fn build_mlt_table_empty_history() {
        let table = build_long_term_mean_inflow_table(&[]);
        assert!(table.is_empty());
    }

    #[test]
    fn build_mlt_table_single_hydro() {
        let rows = vec![
            inflow(1, "2020-01-01", "2020-02-01", 100.0),
            inflow(1, "2020-02-01", "2020-03-01", 200.0),
        ];

        let table = build_long_term_mean_inflow_table(&rows);

        assert_eq!(table.len(), 1);
        assert!((table[&HydroPlantId(1)] - 150.0).abs() < 1e-9);
    }

    #[test]
    fn build_mlt_table_multiple_hydros() {
        let rows = vec![
            inflow(1, "2020-01-01", "2020-02-01", 100.0),
            inflow(2, "2020-01-01", "2020-02-01", 500.0),
            inflow(1, "2020-02-01", "2020-03-01", 300.0),
        ];

        let table = build_long_term_mean_inflow_table(&rows);

        assert_eq!(table.len(), 2);
        assert!((table[&HydroPlantId(1)] - 200.0).abs() < 1e-9);
        assert!((table[&HydroPlantId(2)] - 500.0).abs() < 1e-9);
    }

    #[test]
    fn fallback_uses_mlt_when_present() {
        let params = checked_params(
            &fittable_plant(1),
            1.0,
            &HashMap::from([(HydroPlantId(1), 123.45)]),
            true,
        );

        assert_eq!(params.long_term_mean_inflow_m3s, 123.45);
    }

    #[test]
    fn missing_inflow_history_row_is_fatal() {
        let mut system = minimal_system();
        system.hydro_plants.push(fittable_plant(1));
        system.inflow_history = vec![inflow(99, "2020-01-01", "2020-02-01", 100.0)];

        let error = fit_system(&system, &HashMap::new()).err();

        assert_eq!(
            error,
            Some(MissingInflowHistory {
                id: HydroPlantId(1),
                name: "HYDRO-1".to_string()
            })
        );
    }

    #[test]
    fn empty_deck_inflow_without_history_gives_zero_mlt() {
        let mut plant = fittable_plant(1);
        plant.natural_inflow_hm3.clear();

        let params = checked_params(&plant, 1.0, &HashMap::new(), false);

        assert_eq!(
            params.long_term_mean_inflow_m3s.to_bits(),
            0.0_f64.to_bits()
        );
    }

    #[test]
    fn fit_system_reports_failures_in_plant_order() {
        let mut no_units = fittable_plant(1);
        no_units.groups.clear();
        let mut no_table_no_tailrace = fittable_plant(2);
        no_table_no_tailrace.volume_level_table = None;
        no_table_no_tailrace.tailrace_coefficients = None;
        let mut no_efficiency_no_loss = fittable_plant(3);
        no_efficiency_no_loss.efficiency = None;
        no_efficiency_no_loss.hydraulic_loss_m = None;
        let mut system = minimal_system();
        system.hydro_plants = vec![no_units, no_table_no_tailrace, no_efficiency_no_loss];

        let fit = fit_system(&system, &HashMap::new()).expect("no inflow history");

        assert_eq!(fit.plants_without_units, 1);
        let outcomes: Vec<_> = fit
            .plants
            .iter()
            .map(|plant| (plant.id, &plant.outcome))
            .collect();
        assert!(
            matches!(
                outcomes.as_slice(),
                [
                    (HydroPlantId(2), Err(FitFailureReason::MissingGeometry)),
                    (HydroPlantId(3), Err(FitFailureReason::MissingInputs(inputs))),
                ] if inputs == &[FphaInput::Efficiency, FphaInput::HydraulicLoss]
            ),
            "{outcomes:?}"
        );
        assert!(
            fit.plants
                .iter()
                .all(|plant| plant.ignored_window.is_none())
        );
    }

    #[test]
    fn ignored_volume_window_needs_a_single_point_table_and_a_window() {
        let mut single_point = fittable_plant(1);
        single_point.volume_level_table = Some(vec![(500.0, 105.0)]);
        let window = FphaConfigOverrides {
            max_volume_fraction: Some(0.6),
            ..FphaConfigOverrides::default()
        };
        let no_window = FphaConfigOverrides {
            max_planes: Some(4),
            ..FphaConfigOverrides::default()
        };

        assert_eq!(
            ignored_volume_window(&single_point, Some(&window)),
            Some(IgnoredVolumeWindow {
                id: HydroPlantId(1),
                name: "HYDRO-1".to_string(),
                min_volume_fraction: None,
                max_volume_fraction: Some(0.6)
            })
        );
        assert_eq!(
            ignored_volume_window(&fittable_plant(1), Some(&window)),
            None
        );
        assert_eq!(ignored_volume_window(&single_point, Some(&no_window)), None);
        assert_eq!(ignored_volume_window(&single_point, None), None);
    }

    #[test]
    fn convert_single_plane_lp_product_is_alpha_times_plane() {
        let result = result_with_planes(
            vec![RawPlane {
                gamma_0: 1000.0,
                gamma_v: -0.002,
                gamma_q: 0.05,
                gamma_s: 0.001,
            }],
            0.95,
        );

        let segments = convert_to_fpha_segments(&result);

        let plane = result.planes[0];
        let segment = &segments[0];
        let alpha = result.alpha;

        assert_eq!(segments.len(), 1);
        assert_eq!(segment.segment, 1);
        assert_eq!(segment.correction_factor, alpha);
        assert_eq!(
            segment.correction_factor * segment.rhs,
            alpha * plane.gamma_0
        );
        assert_eq!(
            segment.correction_factor * segment.volume_coefficient,
            alpha * plane.gamma_v
        );
        assert_eq!(
            segment.correction_factor * segment.turbining_coefficient,
            alpha * plane.gamma_q
        );
        assert_eq!(
            segment.correction_factor * segment.lateral_flow_coefficient,
            alpha * plane.gamma_s
        );
    }

    #[test]
    fn convert_multiple_planes() {
        let result = result_with_planes(
            vec![
                RawPlane {
                    gamma_0: 100.0,
                    gamma_v: 0.1,
                    gamma_q: 0.2,
                    gamma_s: 0.3,
                },
                RawPlane {
                    gamma_0: 200.0,
                    gamma_v: 0.4,
                    gamma_q: 0.5,
                    gamma_s: 0.6,
                },
                RawPlane {
                    gamma_0: 300.0,
                    gamma_v: 0.7,
                    gamma_q: 0.8,
                    gamma_s: 0.9,
                },
            ],
            0.98,
        );

        let segments = convert_to_fpha_segments(&result);

        assert_eq!(segments.len(), 3);
        assert_eq!(segments[0].segment, 1);
        assert_eq!(segments[1].segment, 2);
        assert_eq!(segments[2].segment, 3);
        for seg in &segments {
            assert_eq!(seg.correction_factor, 0.98);
        }
    }

    #[test]
    fn convert_empty_planes() {
        let result = result_with_planes(vec![], 1.0);

        let segments = convert_to_fpha_segments(&result);

        assert_eq!(segments, Vec::new());
    }
}
