//! Plant-level maximum swallowing helpers (MM eq. 9.34): the reference values, the net head model
//! with its missing-data policy, the net head and the raw bound of one plant.

use labdessem_core::entities::HydroPlant;

use super::formula::calculate_max_swallowing;
use crate::hydro_geometry::{GeometryError, NetHeadModel, plant_geometry};

/// Plant-level max swallowing reference values (DESSEM eq. 9.34) over all registered units.
///
/// `q_ef_hm3` is one flat left fold over every unit of every group, in input order. The FPHA fit
/// sums the units of each group and then the group totals, so the two plant totals can differ in the
/// last bit (24Barras FURNAS: 2.7107999999999994 hm3 here, 2.7108 hm3 grouped). Both orders are pinned
/// (RD-23, R31); unifying them is the follow-up recorded in the Known limitations of the formulation
/// docs.
pub struct PlantReference {
    /// Maximum turbining at the effective head (`Q_ef`), hm3 per period.
    pub q_ef_hm3: f64,
    /// Effective head (`h_ef`), m: the mean unit `effective_head_m`, 0 without units.
    pub h_ef_m: f64,
    /// Turbine curve exponent of the plant's turbine type.
    pub alpha: f64,
}

impl PlantReference {
    /// Reference values of `plant`.
    pub fn from_plant(plant: &HydroPlant) -> Self {
        let (q_ef_hm3, head_sum_m, unit_count) = plant
            .groups
            .iter()
            .flat_map(|group| &group.units)
            .fold((0.0, 0.0, 0.0), |(q, h, n), unit| {
                (
                    q + unit.max_turbining_hm3,
                    h + unit.effective_head_m,
                    n + 1.0,
                )
            });
        let h_ef_m = if unit_count > 0.0 {
            head_sum_m / unit_count
        } else {
            0.0
        };

        Self {
            q_ef_hm3,
            h_ef_m,
            alpha: plant.turbine_type.alpha(),
        }
    }
}

/// Net head model of a max swallowing plant (one with at least one unit), built once per run
/// (ADR-018).
///
/// Applies the max swallowing missing-data policy (RD-22): `Ok(None)` when the plant has no unit, no
/// VHA table or no tailrace polynomial, so its bound falls back to `h_ef`; a missing hydraulic loss
/// counts as 0 m.
///
/// # Errors
///
/// [`GeometryError`] when a plant with units has a malformed VHA table.
pub fn plant_head_model(plant: &HydroPlant) -> Result<Option<NetHeadModel>, GeometryError> {
    if plant.groups.iter().all(|group| group.units.is_empty()) {
        return Ok(None);
    }
    Ok(plant_geometry(plant)?.map(|(forebay, tailrace)| {
        NetHeadModel::new(forebay, tailrace, plant.hydraulic_loss_m.unwrap_or(0.0))
    }))
}

/// Net head (m) from the plant's net head model, or `fallback_head_m` when the plant has none.
pub fn net_head_m(
    head_model: Option<&NetHeadModel>,
    fallback_head_m: f64,
    volume_hm3: f64,
    outflow_m3s: f64,
) -> f64 {
    head_model.map_or(fallback_head_m, |model| {
        model.net_head(volume_hm3, outflow_m3s)
    })
}

/// Raw plant max swallowing in hm3 per period for a net head.
pub fn plant_bound_hm3(reference: &PlantReference, net_head_m: f64) -> f64 {
    calculate_max_swallowing(
        reference.q_ef_hm3,
        reference.h_ef_m,
        net_head_m,
        reference.alpha,
    )
}

/// Names of the groups holding a unit with `effective_head_m <= 0.0`, in input order,
/// when the plant has such units and also units with `effective_head_m > 0.0`.
pub fn partial_effective_head_groups(plant: &HydroPlant) -> Vec<String> {
    let units = || plant.groups.iter().flat_map(|group| &group.units);
    if !(units().any(|unit| unit.effective_head_m > 0.0)
        && units().any(|unit| unit.effective_head_m <= 0.0))
    {
        return Vec::new();
    }

    plant
        .groups
        .iter()
        .filter(|group| group.units.iter().any(|unit| unit.effective_head_m <= 0.0))
        .map(|group| group.name.clone())
        .collect()
}

#[cfg(test)]
// tests: a panic is the failure report
#[allow(clippy::unwrap_used, clippy::expect_used)]
mod tests {
    use super::{
        PlantReference, net_head_m, partial_effective_head_groups, plant_bound_hm3,
        plant_head_model,
    };
    use labdessem_core::entities::{
        HydroGroup, HydroInitialCondition, HydroPlant, HydroUnit, Reservoir, TurbineType,
    };
    use labdessem_core::ids::{BusId, HydroGroupId, HydroPlantId, HydroUnitId, SubmarketId};

    /// Unit `UG-{n}`: `max_turbining_hm3`, `Hef.m` `effective_head_m`.
    fn unit(n: usize, max_turbining_hm3: f64, effective_head_m: f64) -> HydroUnit {
        HydroUnit {
            id: HydroUnitId(n),
            name: format!("UG-{n}"),
            min_generation_mw: 0.0,
            max_generation_mw: 50.0,
            max_turbining_hm3,
            effective_head_m,
            initial_condition: HydroInitialCondition {
                is_on: true,
                generation_mw: 0.0,
                time_in_state: 1,
            },
        }
    }

    /// Group `CJ-{n}` holding `units`.
    fn group(n: usize, units: Vec<HydroUnit>) -> HydroGroup {
        HydroGroup {
            id: HydroGroupId(n),
            name: format!("CJ-{n}"),
            units,
        }
    }

    /// Francis plant `name` with `groups` and no VHA table, tailrace polynomial or hydraulic loss.
    fn plant(name: &str, groups: Vec<HydroGroup>) -> HydroPlant {
        HydroPlant {
            id: HydroPlantId(1),
            name: name.into(),
            submarket_id: SubmarketId(1),
            bus_id: BusId(1),
            upstream_plant_ids: vec![],
            downstream_plant_id: None,
            diversion_upstream_plant_ids: vec![],
            diversion_plant_id: None,
            fpha_segments: vec![],
            reservoir: Reservoir {
                min_volume_hm3: 100.0,
                max_volume_hm3: 800.0,
                initial_volume_hm3: 250.0,
            },
            natural_inflow_hm3: vec![1.0, 1.0],
            water_withdrawal_hm3: vec![0.0, 0.0],
            spillage_cost_per_hm3: 0.0,
            turbining_cost_per_hm3: 0.0,
            specific_productivity_mw_per_m3s: 1.0,
            hydraulic_loss_m: None,
            efficiency: None,
            groups,
            turbine_type: TurbineType::Francis,
            volume_level_table: None,
            tailrace_coefficients: None,
        }
    }

    /// Plant `UHE-GEO` with VHA and tailrace geometry: `Q_ef` = 0.72 hm3 (two units of 0.36 hm3 =
    /// 100 m3/s each over 1 h), plant max outflow 200 m3/s.
    fn geometry_plant(h_ef_m: f64, loss_m: Option<f64>) -> HydroPlant {
        HydroPlant {
            volume_level_table: Some(vec![(100.0, 905.0), (400.0, 910.0), (800.0, 913.0)]),
            tailrace_coefficients: Some([885.3, 0.0, 1.97e-5, -2.55e-8, 0.0]),
            hydraulic_loss_m: loss_m,
            ..plant(
                "UHE-GEO",
                vec![group(1, vec![unit(1, 0.36, h_ef_m), unit(2, 0.36, h_ef_m)])],
            )
        }
    }

    fn assert_rel(actual: f64, expected: f64) {
        assert!(
            (actual - expected).abs() <= 1e-9 * expected.abs().max(1.0),
            "expected {expected}, got {actual}"
        );
    }

    #[test]
    fn plant_reference_aggregates_all_registered_units() {
        let plant = plant(
            "UHE-MULTI",
            vec![
                group(1, vec![unit(1, 10.0, 40.0), unit(2, 10.0, 40.0)]),
                group(2, vec![unit(3, 20.0, 70.0)]),
            ],
        );

        let reference = PlantReference::from_plant(&plant);

        // Sum of every unit: 10 + 10 + 20
        assert!((reference.q_ef_hm3 - 40.0).abs() < 1e-12);
        // Unit-count weighted: (40 + 40 + 70) / 3, not the group mean (40 + 70) / 2 = 55
        assert!((reference.h_ef_m - 50.0).abs() < 1e-12);
        assert!((reference.alpha - 0.5).abs() < 1e-12);
    }

    #[test]
    fn plant_reference_keeps_the_flat_unit_fold() {
        // 24Barras FURNAS over 0.5 h periods: group 1 has six units of 188 m3/s, group 2 two units
        // of 189 m3/s, converted to hm3 as io's flow_m3s_to_hm3 does.
        let units_188: Vec<HydroUnit> = (1..=6)
            .map(|n| unit(n, 188.0 * 0.5 * 0.0036, 90.0))
            .collect();
        let units_189: Vec<HydroUnit> = (7..=8)
            .map(|n| unit(n, 189.0 * 0.5 * 0.0036, 89.3))
            .collect();
        let furnas = plant("FURNAS", vec![group(1, units_188), group(2, units_189)]);
        let grouped_hm3: f64 = furnas
            .groups
            .iter()
            .map(|group| {
                group
                    .units
                    .iter()
                    .map(|unit| unit.max_turbining_hm3)
                    .sum::<f64>()
            })
            .sum();

        let reference = PlantReference::from_plant(&furnas);

        // Flat fold 2.7107999999999994 hm3; the per-group order of the FPHA fit gives 2.7108 hm3.
        assert_eq!(reference.q_ef_hm3.to_bits(), 0x4005_afb7_e90f_f971);
        assert_eq!(grouped_hm3.to_bits(), 0x4005_afb7_e90f_f972);
    }

    #[test]
    fn net_head_follows_plant_geometry() {
        let head_model = plant_head_model(&geometry_plant(30.0, None)).expect("valid geometry");

        // Tailrace at 200 m3/s: 885.3 + 1.97e-5 * 200^2 - 2.55e-8 * 200^3
        //   = 885.3 + 0.788 - 0.204 = 885.884 m.
        // Forebay table: (100 hm3, 905 m), (400, 910), (800, 913); net head = forebay - 885.884.
        let cases = [
            // segment 1, t = (250 - 100) / 300 = 0.5: 905 + 0.5 * 5 = 907.5
            (250.0, 21.616),
            // segment 2, t = (600 - 400) / 400 = 0.5: 910 + 0.5 * 3 = 911.5
            (600.0, 25.616),
            // breakpoint: 910
            (400.0, 24.116),
            // clamped to 100 hm3: 905
            (50.0, 19.116),
            // clamped to 800 hm3: 913
            (900.0, 27.116),
        ];
        for (volume_hm3, expected_m) in cases {
            assert_rel(
                net_head_m(head_model.as_ref(), 30.0, volume_hm3, 200.0),
                expected_m,
            );
        }

        // The loss is subtracted in metres: 21.616 - 0.095 = 21.521
        let lossy_head_model =
            plant_head_model(&geometry_plant(30.0, Some(0.095))).expect("valid geometry");
        assert_rel(
            net_head_m(lossy_head_model.as_ref(), 30.0, 250.0, 200.0),
            21.521,
        );
    }

    #[test]
    fn net_head_floors_non_positive_gross_head() {
        // Tailrace at 200 m3/s: 920.0 + 0.788 - 0.204 = 920.584 m above the 907.5 m forebay at
        // 250 hm3, so the gross head is 907.5 - 920.584 = -13.084 m.
        let plant = HydroPlant {
            tailrace_coefficients: Some([920.0, 0.0, 1.97e-5, -2.55e-8, 0.0]),
            ..geometry_plant(30.0, None)
        };
        let reference = PlantReference::from_plant(&plant);
        let head_model = plant_head_model(&plant).expect("valid geometry");

        let head_m = net_head_m(head_model.as_ref(), 30.0, 250.0, 200.0);
        assert_rel(head_m, 0.0);
        assert_rel(plant_bound_hm3(&reference, head_m), 0.0);
    }

    #[test]
    fn net_head_falls_back_without_full_geometry() {
        let full = geometry_plant(30.0, None);
        let plants = [
            HydroPlant {
                volume_level_table: None,
                ..full.clone()
            },
            HydroPlant {
                tailrace_coefficients: None,
                ..full.clone()
            },
            HydroPlant {
                volume_level_table: None,
                tailrace_coefficients: None,
                ..full
            },
        ];

        for plant in &plants {
            let head_model = plant_head_model(plant).expect("valid geometry");
            assert!(head_model.is_none());
            assert_rel(net_head_m(head_model.as_ref(), 37.5, 250.0, 200.0), 37.5);

            // Net head falls back to h_ef = 30 m: 0.72 * (30 / 30)^0.5 = Q_ef
            let reference = PlantReference::from_plant(plant);
            let head_m = net_head_m(head_model.as_ref(), reference.h_ef_m, 250.0, 200.0);
            assert_rel(plant_bound_hm3(&reference, head_m), 0.72);
        }
    }

    #[test]
    fn plant_bound_is_below_q_ef_when_head_is_below_h_ef() {
        let plant = geometry_plant(30.0, None);
        let reference = PlantReference::from_plant(&plant);
        let head_model = plant_head_model(&plant).expect("valid geometry");

        // Net head at V = 250 hm3, Q = 200 m3/s is 21.616 m < h_ef = 30 m:
        // 0.72 * (21.616 / 30)^0.5 = 0.72 * 0.848842... = 0.611166491228
        let head_m = net_head_m(head_model.as_ref(), reference.h_ef_m, 250.0, 200.0);
        let bound = plant_bound_hm3(&reference, head_m);

        assert_rel(bound, 0.611_166_491_228);
        assert!(bound < 0.72);
    }

    #[test]
    fn partial_effective_head_groups_names_only_mixed_plants() {
        let mixed = plant(
            "UHE-MULTI",
            vec![
                group(1, vec![unit(1, 10.0, 50.0), unit(2, 10.0, 50.0)]),
                group(2, vec![unit(3, 10.0, 0.0), unit(4, 10.0, 50.0)]),
                group(3, vec![unit(5, 10.0, -1.0)]),
            ],
        );
        let all_zero = plant("UHE-ZERO", vec![group(1, vec![unit(1, 10.0, 0.0)])]);
        let all_positive = plant("UHE-POS", vec![group(1, vec![unit(1, 10.0, 50.0)])]);

        let none: [&str; 0] = [];
        assert_eq!(partial_effective_head_groups(&mixed), ["CJ-2", "CJ-3"]);
        assert_eq!(partial_effective_head_groups(&all_zero), none);
        assert_eq!(partial_effective_head_groups(&all_positive), none);
    }
}
