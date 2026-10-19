use labdessem_core::{ids::SubmarketId, system::System};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ThermalUnitIndex {
    pub plant_idx: usize,
    pub unit_idx: usize,
    pub submarket_idx: usize,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct HydroUnitIndex {
    pub plant_idx: usize,
    pub group_idx: usize,
    pub unit_idx: usize,
    pub submarket_idx: usize,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RenewablePlantIndex {
    pub plant_idx: usize,
    pub submarket_idx: usize,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct HydroPlantIndex {
    pub plant_idx: usize,
    pub submarket_idx: usize,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PumpingPlantIndex {
    pub plant_idx: usize,
    pub submarket_idx: usize,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct InterchangeIndex {
    pub from_submarket_idx: usize,
    pub to_submarket_idx: usize,
    pub period: usize,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Indexing {
    pub hydro_plant_entries: Vec<HydroPlantIndex>,
    pub thermal_unit_entries: Vec<ThermalUnitIndex>,
    pub hydro_unit_entries: Vec<HydroUnitIndex>,
    pub renewable_plant_entries: Vec<RenewablePlantIndex>,
    pub pumping_plant_entries: Vec<PumpingPlantIndex>,
    pub interchange_entries: Vec<InterchangeIndex>,
    pub submarket_ids: Vec<SubmarketId>,
    pub thermal_units: usize,
    pub hydro_units: usize,
    pub renewable_plants: usize,
    pub pumping_plants: usize,
    pub buses: usize,
    pub submarkets: usize,
    pub future_cost_enabled: bool,
    pub thermal_unit_commitment_enabled: bool,
    pub hydro_unit_commitment_enabled: bool,
}

impl Indexing {
    pub fn from_system(system: &System) -> Self {
        let submarket_ids: Vec<_> = system
            .submarkets
            .iter()
            .map(|submarket| submarket.id)
            .collect();

        let thermal_unit_entries = system
            .thermal_plants
            .iter()
            .enumerate()
            .flat_map(|(plant_idx, plant)| {
                let submarket_idx = submarket_position(&submarket_ids, plant.submarket_id);
                plant
                    .units
                    .iter()
                    .enumerate()
                    .map(move |(unit_idx, _)| ThermalUnitIndex {
                        plant_idx,
                        unit_idx,
                        submarket_idx,
                    })
            })
            .collect::<Vec<_>>();

        let hydro_unit_entries =
            system
                .hydro_plants
                .iter()
                .enumerate()
                .flat_map(|(plant_idx, plant)| {
                    let submarket_idx = submarket_position(&submarket_ids, plant.submarket_id);
                    plant
                        .groups
                        .iter()
                        .enumerate()
                        .flat_map(move |(group_idx, group)| {
                            group.units.iter().enumerate().map(move |(unit_idx, _)| {
                                HydroUnitIndex {
                                    plant_idx,
                                    group_idx,
                                    unit_idx,
                                    submarket_idx,
                                }
                            })
                        })
                })
                .collect::<Vec<_>>();

        let hydro_plant_entries = system
            .hydro_plants
            .iter()
            .enumerate()
            .map(|(plant_idx, plant)| HydroPlantIndex {
                plant_idx,
                submarket_idx: submarket_position(&submarket_ids, plant.submarket_id),
            })
            .collect::<Vec<_>>();

        let renewable_plant_entries = system
            .renewable_plants
            .iter()
            .enumerate()
            .map(|(plant_idx, plant)| RenewablePlantIndex {
                plant_idx,
                submarket_idx: submarket_position(&submarket_ids, plant.submarket_id),
            })
            .collect::<Vec<_>>();

        let pumping_plant_entries = system
            .pumping_plants
            .iter()
            .enumerate()
            .map(|(plant_idx, plant)| PumpingPlantIndex {
                plant_idx,
                submarket_idx: submarket_position(&submarket_ids, plant.submarket_id),
            })
            .collect::<Vec<_>>();

        let interchange_entries = (0..system.horizon.periods)
            .flat_map(|period| {
                (0..system.submarkets.len()).flat_map(move |from_submarket_idx| {
                    (0..system.submarkets.len())
                        .filter(move |to_submarket_idx| *to_submarket_idx != from_submarket_idx)
                        .map(move |to_submarket_idx| InterchangeIndex {
                            from_submarket_idx,
                            to_submarket_idx,
                            period,
                        })
                })
            })
            .collect::<Vec<_>>();

        Self {
            thermal_units: thermal_unit_entries.len(),
            hydro_units: hydro_unit_entries.len(),
            renewable_plants: renewable_plant_entries.len(),
            pumping_plants: pumping_plant_entries.len(),
            buses: system.buses.len(),
            submarkets: system.submarkets.len(),
            future_cost_enabled: system.future_cost_enabled,
            thermal_unit_commitment_enabled: system.thermal_unit_commitment_enabled,
            hydro_unit_commitment_enabled: system.hydro_unit_commitment_enabled,
            hydro_plant_entries,
            thermal_unit_entries,
            hydro_unit_entries,
            renewable_plant_entries,
            pumping_plant_entries,
            interchange_entries,
            submarket_ids,
        }
    }

    /// Compute column offsets for variable types.
    ///
    /// Variable ordering follows the layout in `Variables::for_system`:
    /// 1. thermal_generation
    /// 2. hydro_generation  
    /// 3. hydro_turbining
    /// 4. hydro_spillage
    /// 5. hydro_diversion
    /// 6. pumping
    /// 7. hydro_volume
    /// 8. future_cost
    /// 9. renewable_generation
    /// 10. interchange
    /// 11. thermal_commitment
    /// 12. thermal_startup
    /// 13. thermal_shutdown
    /// 14. hydro_commitment
    /// 15. network_flow_slack
    /// 16. operational_limit_slack
    pub fn compute_column_offsets(&self, horizon: usize) -> ColumnOffsets {
        let mut offset = 0;

        let thermal_generation = offset;
        offset += self.thermal_units * horizon;

        let hydro_generation = offset;
        offset += self.hydro_units * horizon;

        let hydro_turbining = offset;
        offset += self.hydro_units * horizon;

        let hydro_spillage = offset;
        offset += self.hydro_plant_entries.len() * horizon;

        let hydro_diversion = offset;
        offset += self.hydro_plant_entries.len() * horizon;

        let pumping = offset;
        offset += self.pumping_plants * horizon;

        let hydro_volume = offset;
        offset += self.hydro_plant_entries.len() * (horizon + 1);

        let future_cost = offset;
        if self.future_cost_enabled {
            offset += 1;
        }

        let renewable_generation = offset;
        offset += self.renewable_plants * horizon;

        let interchange = offset;
        offset += self.interchange_entries.len();

        let thermal_commitment = offset;
        if self.thermal_unit_commitment_enabled {
            offset += self.thermal_units * horizon;
        }

        let thermal_startup = offset;
        if self.thermal_unit_commitment_enabled {
            offset += self.thermal_units * horizon;
        }

        let thermal_shutdown = offset;
        if self.thermal_unit_commitment_enabled {
            offset += self.thermal_units * horizon;
        }

        let hydro_commitment = offset;
        if self.hydro_unit_commitment_enabled {
            offset += self.hydro_units * horizon;
        }

        // network_flow_slack and operational_limit_slack are dynamic
        let network_flow_slack = offset;
        let operational_limit_slack = offset;

        ColumnOffsets {
            thermal_generation,
            hydro_generation,
            hydro_turbining,
            hydro_spillage,
            hydro_diversion,
            pumping,
            hydro_volume,
            future_cost,
            renewable_generation,
            interchange,
            thermal_commitment,
            thermal_startup,
            thermal_shutdown,
            hydro_commitment,
            network_flow_slack,
            operational_limit_slack,
            total: offset,
        }
    }

    /// Compute row offsets for constraint types.
    ///
    /// Constraint ordering follows the layout in `ConstraintSet::for_system`:
    /// 1. demand_balance (submarkets × horizon)
    /// 2. hydro_balance (hydro_plants × horizon)
    /// 3. interchange_limit (interchange_entries with limits)
    /// 4. hydro_turbining_upper (hydro_units × horizon)
    /// 5. hydro_turbining_lower (hydro_units × horizon)
    /// 6. hydro_spillage_nonnegativity (hydro_plants × horizon)
    /// 7. hydro_fpha OR hydro_productivity (mutually exclusive)
    /// 8. hydro_generation_turbining_coupling (only if FPHA)
    /// 9. future_cost_cut (if FCF enabled)
    /// 10. operational_limit_lower and operational_limit_upper
    /// 11. thermal_min_up_down (if UCT and commitment mode)
    /// 12. thermal_ramps (if UCT and commitment mode)
    /// 13. hydro_commitment_channeling (if UCH and commitment mode)
    pub fn compute_constraint_offsets(&self, params: &ConstraintOffsetParams) -> ConstraintOffsets {
        let horizon = params.horizon;
        let mut offset = 0;

        // 1. demand_balance: submarkets × horizon
        let demand_balance = offset;
        offset += self.submarkets * horizon;

        // 2. hydro_balance: hydro_plants × horizon
        let hydro_balance = offset;
        offset += self.hydro_plant_entries.len() * horizon;

        // 3. interchange_limit: count varies by actual limits defined
        let interchange_limit = offset;
        offset += self.interchange_entries.len();

        // 4-5. hydro_turbining: upper and lower for each unit × horizon
        let hydro_turbining_min = offset;
        offset += self.hydro_units * horizon;
        let hydro_turbining_max = offset;
        offset += self.hydro_units * horizon;

        // 6. hydro_spillage_nonnegativity: hydro_plants × horizon
        let hydro_spillage_nonnegativity = offset;
        offset += self.hydro_plant_entries.len() * horizon;

        // 7-8. FPHA or productivity (mutually exclusive)
        let hydro_fpha;
        let hydro_generation_turbining_coupling;
        let hydro_productivity;

        if params.fpha_enabled {
            hydro_fpha = offset;
            offset += params.num_fpha_segments * horizon;
            hydro_generation_turbining_coupling = offset;
            offset += self.hydro_units * horizon;
            hydro_productivity = offset; // not used
        } else {
            hydro_fpha = offset; // not used
            hydro_generation_turbining_coupling = offset; // not used
            hydro_productivity = offset;
            offset += self.hydro_units * horizon;
        }

        // 9. future_cost_cut (if enabled)
        let future_cost_cut = offset;
        if params.future_cost_enabled {
            offset += params.num_future_cost_cuts;
        }

        // 10. operational_limit: lower and upper for each limit
        let operational_limit_lower = offset;
        offset += params.num_operational_limits;
        let operational_limit_upper = offset;
        offset += params.num_operational_limits;

        // 11-13. Commitment constraints (only in MILP/LP-FIXED modes)
        let thermal_min_up;
        let thermal_min_down;
        let thermal_ramps;
        let hydro_commitment_channeling;

        if params.is_commitment_mode && params.thermal_uct_enabled {
            thermal_min_up = offset;
            offset += self.thermal_units * horizon;
            thermal_min_down = offset;
            offset += self.thermal_units * horizon;
            thermal_ramps = offset;
            offset += self.thermal_units * horizon * 4; // startup_upper, startup_lower, shutdown_upper, shutdown_lower approx
        } else {
            thermal_min_up = offset;
            thermal_min_down = offset;
            thermal_ramps = offset;
        }

        if params.is_commitment_mode && params.hydro_uch_enabled {
            hydro_commitment_channeling = offset;
            offset += self.hydro_units * horizon * 3; // gen_lower, gen_upper, turb_lower
        } else {
            hydro_commitment_channeling = offset;
        }

        ConstraintOffsets {
            demand_balance,
            hydro_balance,
            interchange_limit,
            hydro_turbining_min,
            hydro_turbining_max,
            hydro_spillage_nonnegativity,
            hydro_fpha,
            hydro_generation_turbining_coupling,
            hydro_productivity,
            future_cost_cut,
            operational_limit_lower,
            operational_limit_upper,
            thermal_min_up,
            thermal_min_down,
            thermal_ramps,
            hydro_commitment_channeling,
            total: offset,
        }
    }
}

/// Column offsets for each variable type in the LP matrix.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ColumnOffsets {
    pub thermal_generation: usize,
    pub hydro_generation: usize,
    pub hydro_turbining: usize,
    pub hydro_spillage: usize,
    pub hydro_diversion: usize,
    pub pumping: usize,
    pub hydro_volume: usize,
    pub future_cost: usize,
    pub renewable_generation: usize,
    pub interchange: usize,
    pub thermal_commitment: usize,
    pub thermal_startup: usize,
    pub thermal_shutdown: usize,
    pub hydro_commitment: usize,
    pub network_flow_slack: usize,
    pub operational_limit_slack: usize,
    pub total: usize,
}

fn submarket_position(submarket_ids: &[SubmarketId], submarket_id: SubmarketId) -> usize {
    // Rationale: expect() is safe because System validation guarantees valid submarket IDs
    #[allow(clippy::expect_used)]
    submarket_ids
        .iter()
        .position(|candidate| *candidate == submarket_id)
        .expect("system validation should guarantee known submarket ids")
}

/// Row offsets for each constraint type in the LP matrix.
///
/// Constraint ordering follows `ConstraintSet::for_system`:
/// 1. demand_balance
/// 2. hydro_balance
/// 3. interchange_limit
/// 4. hydro_turbining_limit (min and max)
/// 5. hydro_spillage_nonnegativity
/// 6. hydro_fpha OR hydro_productivity (mutually exclusive)
/// 7. hydro_generation_turbining_coupling (only if FPHA enabled)
/// 8. future_cost_cut (only if FCF enabled)
/// 9. operational_limit (lower and upper)
/// 10. thermal_min_up_down (only in MILP/LP-FIXED modes with UCT)
/// 11. thermal_ramps (only in MILP/LP-FIXED modes with UCT)
/// 12. hydro_commitment_channeling (only in MILP/LP-FIXED modes with UCH)
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ConstraintOffsets {
    pub demand_balance: usize,
    pub hydro_balance: usize,
    pub interchange_limit: usize,
    pub hydro_turbining_min: usize,
    pub hydro_turbining_max: usize,
    pub hydro_spillage_nonnegativity: usize,
    pub hydro_fpha: usize,
    pub hydro_generation_turbining_coupling: usize,
    pub hydro_productivity: usize,
    pub future_cost_cut: usize,
    pub operational_limit_lower: usize,
    pub operational_limit_upper: usize,
    pub thermal_min_up: usize,
    pub thermal_min_down: usize,
    pub thermal_ramps: usize,
    pub hydro_commitment_channeling: usize,
    pub total: usize,
}

/// Parameters for computing constraint offsets.
#[derive(Debug, Clone, Default)]
pub struct ConstraintOffsetParams {
    pub horizon: usize,
    pub fpha_enabled: bool,
    pub num_fpha_segments: usize,
    pub future_cost_enabled: bool,
    pub num_future_cost_cuts: usize,
    pub num_operational_limits: usize,
    pub thermal_uct_enabled: bool,
    pub hydro_uch_enabled: bool,
    pub is_commitment_mode: bool,
}


#[cfg(test)]
mod tests {
    use super::*;

    fn minimal_indexing() -> Indexing {
        Indexing {
            hydro_plant_entries: vec![
                HydroPlantIndex { plant_idx: 0, submarket_idx: 0 },
                HydroPlantIndex { plant_idx: 1, submarket_idx: 0 },
            ],
            thermal_unit_entries: vec![
                ThermalUnitIndex { plant_idx: 0, unit_idx: 0, submarket_idx: 0 },
            ],
            hydro_unit_entries: vec![
                HydroUnitIndex { plant_idx: 0, group_idx: 0, unit_idx: 0, submarket_idx: 0 },
                HydroUnitIndex { plant_idx: 1, group_idx: 0, unit_idx: 0, submarket_idx: 0 },
            ],
            renewable_plant_entries: vec![
                RenewablePlantIndex { plant_idx: 0, submarket_idx: 0 },
            ],
            pumping_plant_entries: vec![],
            interchange_entries: vec![],
            submarket_ids: vec![labdessem_core::ids::SubmarketId(1)],
            thermal_units: 1,
            hydro_units: 2,
            renewable_plants: 1,
            pumping_plants: 0,
            buses: 1,
            submarkets: 1,
            future_cost_enabled: true,
            thermal_unit_commitment_enabled: true,
            hydro_unit_commitment_enabled: true,
        }
    }

    #[test]
    fn constraint_offsets_basic_layout() {
        let indexing = minimal_indexing();
        let params = ConstraintOffsetParams {
            horizon: 3,
            fpha_enabled: false,
            num_fpha_segments: 0,
            future_cost_enabled: false,
            num_future_cost_cuts: 0,
            num_operational_limits: 0,
            thermal_uct_enabled: false,
            hydro_uch_enabled: false,
            is_commitment_mode: false,
        };

        let offsets = indexing.compute_constraint_offsets(&params);

        // demand_balance: 1 submarket × 3 periods = 3
        assert_eq!(offsets.demand_balance, 0);
        // hydro_balance: 2 plants × 3 periods = 6
        assert_eq!(offsets.hydro_balance, 3);
        // interchange_limit: 0 entries
        assert_eq!(offsets.interchange_limit, 9);
        // hydro_turbining_min: 2 units × 3 periods = 6
        assert_eq!(offsets.hydro_turbining_min, 9);
        // hydro_turbining_max: 2 units × 3 periods = 6
        assert_eq!(offsets.hydro_turbining_max, 15);
        // hydro_spillage_nonnegativity: 2 plants × 3 periods = 6
        assert_eq!(offsets.hydro_spillage_nonnegativity, 21);
        // hydro_productivity: 2 units × 3 periods = 6 (FPHA disabled)
        assert_eq!(offsets.hydro_productivity, 27);
    }

    #[test]
    fn constraint_offsets_with_fpha() {
        let indexing = minimal_indexing();
        let params = ConstraintOffsetParams {
            horizon: 3,
            fpha_enabled: true,
            num_fpha_segments: 4, // 2 plants × 2 segments each
            future_cost_enabled: false,
            num_future_cost_cuts: 0,
            num_operational_limits: 0,
            thermal_uct_enabled: false,
            hydro_uch_enabled: false,
            is_commitment_mode: false,
        };

        let offsets = indexing.compute_constraint_offsets(&params);

        // After spillage_nonnegativity (offset 27)
        assert_eq!(offsets.hydro_fpha, 27);
        // hydro_fpha: 4 segments × 3 periods = 12
        assert_eq!(offsets.hydro_generation_turbining_coupling, 39);
        // coupling: 2 units × 3 periods = 6
        assert_eq!(offsets.future_cost_cut, 45);
    }

    #[test]
    fn column_offsets_basic_layout() {
        let indexing = minimal_indexing();
        let offsets = indexing.compute_column_offsets(3);

        // thermal_generation: 1 unit × 3 periods = 3
        assert_eq!(offsets.thermal_generation, 0);
        // hydro_generation: 2 units × 3 periods = 6
        assert_eq!(offsets.hydro_generation, 3);
        // hydro_turbining: 2 units × 3 periods = 6
        assert_eq!(offsets.hydro_turbining, 9);
    }

    #[test]
    fn column_offsets_match_variable_counts() {
        use crate::{variables::Variables, SolveMode};
        use labdessem_io::read_study_from_path;
        use std::path::PathBuf;

        let case_path = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../examples/24Barras");
        let system = read_study_from_path(case_path).expect("24Barras should be readable");
        let indexing = Indexing::from_system(&system);
        let variables =
            Variables::for_system(&system, &indexing, SolveMode::MixedIntegerLinearProgramming);
        let col_offsets = indexing.compute_column_offsets(system.horizon.periods);

        let mut expected_offset = 0;

        assert_eq!(
            col_offsets.thermal_generation, expected_offset,
            "thermal_generation offset mismatch"
        );
        expected_offset += variables.thermal_generation.len();

        assert_eq!(
            col_offsets.hydro_generation, expected_offset,
            "hydro_generation offset mismatch"
        );
        expected_offset += variables.hydro_generation.len();

        assert_eq!(
            col_offsets.hydro_turbining, expected_offset,
            "hydro_turbining offset mismatch"
        );
        expected_offset += variables.hydro_turbining.len();

        assert_eq!(
            col_offsets.hydro_spillage, expected_offset,
            "hydro_spillage offset mismatch"
        );
        expected_offset += variables.hydro_spillage.len();

        assert_eq!(
            col_offsets.hydro_diversion, expected_offset,
            "hydro_diversion offset mismatch"
        );
        expected_offset += variables.hydro_diversion.len();

        assert_eq!(col_offsets.pumping, expected_offset, "pumping offset mismatch");
        expected_offset += variables.pumping.len();

        assert_eq!(
            col_offsets.hydro_volume, expected_offset,
            "hydro_volume offset mismatch"
        );
        expected_offset += variables.hydro_volume.len();

        assert_eq!(
            col_offsets.future_cost, expected_offset,
            "future_cost offset mismatch"
        );
        expected_offset += variables.future_cost.len();

        assert_eq!(
            col_offsets.renewable_generation, expected_offset,
            "renewable_generation offset mismatch"
        );
        expected_offset += variables.renewable_generation.len();

        assert_eq!(
            col_offsets.interchange, expected_offset,
            "interchange offset mismatch"
        );
        expected_offset += variables.interchange.len();

        assert_eq!(
            col_offsets.thermal_commitment, expected_offset,
            "thermal_commitment offset mismatch"
        );
        expected_offset += variables.thermal_commitment.len();

        assert_eq!(
            col_offsets.thermal_startup, expected_offset,
            "thermal_startup offset mismatch"
        );
        expected_offset += variables.thermal_startup.len();

        assert_eq!(
            col_offsets.thermal_shutdown, expected_offset,
            "thermal_shutdown offset mismatch"
        );
        expected_offset += variables.thermal_shutdown.len();

        assert_eq!(
            col_offsets.hydro_commitment, expected_offset,
            "hydro_commitment offset mismatch"
        );
        expected_offset += variables.hydro_commitment.len();

        // network_flow_slack and operational_limit_slack are dynamic (empty in base model)
        assert_eq!(
            col_offsets.network_flow_slack, expected_offset,
            "network_flow_slack offset mismatch"
        );
        expected_offset += variables.network_flow_slack.len();

        assert_eq!(
            col_offsets.operational_limit_slack, expected_offset,
            "operational_limit_slack offset mismatch"
        );
        expected_offset += variables.operational_limit_slack.len();

        assert_eq!(
            col_offsets.total, expected_offset,
            "total offset mismatch: expected {} but got {}",
            expected_offset, col_offsets.total
        );
    }
}
