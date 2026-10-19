//! Solution accessor for typed, index-based access to LP solution data.

use labdessem_model::indexing::{ColumnOffsets, ConstraintOffsetParams, ConstraintOffsets, Indexing};

use crate::types::LpSolution;

/// Ergonomic wrapper for index-based solution access.
///
/// Combines `LpSolution` arrays with `ColumnOffsets` and `ConstraintOffsets`
/// to provide typed accessor methods for each variable and constraint type.
pub struct SolutionAccessor<'a> {
    solution: &'a LpSolution,
    col_offsets: ColumnOffsets,
    row_offsets: ConstraintOffsets,
    horizon: usize,
    num_hydro_plants: usize,
    num_hydro_units: usize,
    num_thermal_units: usize,
    num_renewable_plants: usize,
    num_pumping_plants: usize,
    num_submarkets: usize,
}

impl<'a> SolutionAccessor<'a> {
    /// Create a new accessor from solution and indexing data.
    #[must_use]
    pub fn new(
        solution: &'a LpSolution,
        indexing: &Indexing,
        params: &ConstraintOffsetParams,
    ) -> Self {
        let col_offsets = indexing.compute_column_offsets(params.horizon);
        let row_offsets = indexing.compute_constraint_offsets(params);

        Self {
            solution,
            col_offsets,
            row_offsets,
            horizon: params.horizon,
            num_hydro_plants: indexing.hydro_plant_entries.len(),
            num_hydro_units: indexing.hydro_units,
            num_thermal_units: indexing.thermal_units,
            num_renewable_plants: indexing.renewable_plants,
            num_pumping_plants: indexing.pumping_plants,
            num_submarkets: indexing.submarkets,
        }
    }

    // -------------------------------------------------------------------------
    // Convenience accessors
    // -------------------------------------------------------------------------

    /// Returns the optimal objective value.
    #[must_use]
    pub fn objective(&self) -> f64 {
        self.solution.objective
    }

    /// Returns the raw primal values slice.
    #[must_use]
    pub fn primal_slice(&self) -> &[f64] {
        &self.solution.primal
    }

    /// Returns the raw dual values slice.
    #[must_use]
    pub fn dual_slice(&self) -> &[f64] {
        &self.solution.dual
    }

    /// Returns the column offsets.
    #[must_use]
    pub fn col_offsets(&self) -> &ColumnOffsets {
        &self.col_offsets
    }

    /// Returns the row offsets.
    #[must_use]
    pub fn row_offsets(&self) -> &ConstraintOffsets {
        &self.row_offsets
    }

    // -------------------------------------------------------------------------
    // Primal variable accessors
    // -------------------------------------------------------------------------

    /// Thermal generation for unit at given period.
    #[must_use]
    pub fn thermal_generation(&self, unit_idx: usize, period: usize) -> f64 {
        debug_assert!(unit_idx < self.num_thermal_units);
        debug_assert!(period < self.horizon);
        let idx = self.col_offsets.thermal_generation + unit_idx * self.horizon + period;
        self.solution.primal[idx]
    }

    /// Hydro generation for unit at given period.
    #[must_use]
    pub fn hydro_generation(&self, unit_idx: usize, period: usize) -> f64 {
        debug_assert!(unit_idx < self.num_hydro_units);
        debug_assert!(period < self.horizon);
        let idx = self.col_offsets.hydro_generation + unit_idx * self.horizon + period;
        self.solution.primal[idx]
    }

    /// Hydro turbining for unit at given period.
    #[must_use]
    pub fn hydro_turbining(&self, unit_idx: usize, period: usize) -> f64 {
        debug_assert!(unit_idx < self.num_hydro_units);
        debug_assert!(period < self.horizon);
        let idx = self.col_offsets.hydro_turbining + unit_idx * self.horizon + period;
        self.solution.primal[idx]
    }

    /// Hydro spillage for plant at given period.
    #[must_use]
    pub fn hydro_spillage(&self, plant_idx: usize, period: usize) -> f64 {
        debug_assert!(plant_idx < self.num_hydro_plants);
        debug_assert!(period < self.horizon);
        let idx = self.col_offsets.hydro_spillage + plant_idx * self.horizon + period;
        self.solution.primal[idx]
    }

    /// Hydro diversion for plant at given period.
    #[must_use]
    pub fn hydro_diversion(&self, plant_idx: usize, period: usize) -> f64 {
        debug_assert!(plant_idx < self.num_hydro_plants);
        debug_assert!(period < self.horizon);
        let idx = self.col_offsets.hydro_diversion + plant_idx * self.horizon + period;
        self.solution.primal[idx]
    }

    /// Pumping for plant at given period.
    #[must_use]
    pub fn pumping(&self, plant_idx: usize, period: usize) -> f64 {
        debug_assert!(plant_idx < self.num_pumping_plants);
        debug_assert!(period < self.horizon);
        let idx = self.col_offsets.pumping + plant_idx * self.horizon + period;
        self.solution.primal[idx]
    }

    /// Hydro volume for plant at end of given period.
    ///
    /// Period is 0-indexed but volumes are stored at period boundaries:
    /// - `hydro_volume(plant, 0)` is volume at end of period 0 (initial volume is not stored)
    /// - For initial volume, use the system's initial reservoir state
    #[must_use]
    pub fn hydro_volume(&self, plant_idx: usize, period: usize) -> f64 {
        debug_assert!(plant_idx < self.num_hydro_plants);
        debug_assert!(period <= self.horizon);
        let idx = self.col_offsets.hydro_volume + plant_idx * (self.horizon + 1) + period;
        self.solution.primal[idx]
    }

    /// Future cost function value.
    #[must_use]
    pub fn future_cost(&self) -> f64 {
        self.solution.primal[self.col_offsets.future_cost]
    }

    /// Deficit for submarket at given period.
    #[must_use]
    pub fn deficit(&self, submarket_idx: usize, period: usize) -> f64 {
        debug_assert!(submarket_idx < self.num_submarkets);
        debug_assert!(period < self.horizon);
        let idx = self.col_offsets.deficit + submarket_idx * self.horizon + period;
        self.solution.primal[idx]
    }

    /// Renewable generation for plant at given period.
    #[must_use]
    pub fn renewable_generation(&self, plant_idx: usize, period: usize) -> f64 {
        debug_assert!(plant_idx < self.num_renewable_plants);
        debug_assert!(period < self.horizon);
        let idx = self.col_offsets.renewable_generation + plant_idx * self.horizon + period;
        self.solution.primal[idx]
    }

    /// Interchange flow for given entry index.
    #[must_use]
    pub fn interchange(&self, entry_idx: usize) -> f64 {
        let idx = self.col_offsets.interchange + entry_idx;
        self.solution.primal[idx]
    }

    /// Thermal commitment for unit at given period.
    #[must_use]
    pub fn thermal_commitment(&self, unit_idx: usize, period: usize) -> f64 {
        debug_assert!(unit_idx < self.num_thermal_units);
        debug_assert!(period < self.horizon);
        let idx = self.col_offsets.thermal_commitment + unit_idx * self.horizon + period;
        self.solution.primal[idx]
    }

    /// Thermal startup for unit at given period.
    #[must_use]
    pub fn thermal_startup(&self, unit_idx: usize, period: usize) -> f64 {
        debug_assert!(unit_idx < self.num_thermal_units);
        debug_assert!(period < self.horizon);
        let idx = self.col_offsets.thermal_startup + unit_idx * self.horizon + period;
        self.solution.primal[idx]
    }

    /// Thermal shutdown for unit at given period.
    #[must_use]
    pub fn thermal_shutdown(&self, unit_idx: usize, period: usize) -> f64 {
        debug_assert!(unit_idx < self.num_thermal_units);
        debug_assert!(period < self.horizon);
        let idx = self.col_offsets.thermal_shutdown + unit_idx * self.horizon + period;
        self.solution.primal[idx]
    }

    /// Hydro commitment for unit at given period.
    #[must_use]
    pub fn hydro_commitment(&self, unit_idx: usize, period: usize) -> f64 {
        debug_assert!(unit_idx < self.num_hydro_units);
        debug_assert!(period < self.horizon);
        let idx = self.col_offsets.hydro_commitment + unit_idx * self.horizon + period;
        self.solution.primal.get(idx).copied().unwrap_or(0.0)
    }

    /// Network flow slack at given index.
    #[must_use]
    pub fn network_flow_slack(&self, idx: usize) -> f64 {
        let offset = self.col_offsets.network_flow_slack + idx;
        self.solution.primal.get(offset).copied().unwrap_or(0.0)
    }

    /// Operational limit slack at given index.
    #[must_use]
    pub fn operational_limit_slack(&self, idx: usize) -> f64 {
        let offset = self.col_offsets.operational_limit_slack + idx;
        self.solution.primal.get(offset).copied().unwrap_or(0.0)
    }

    // -------------------------------------------------------------------------
    // Dual variable accessors
    // -------------------------------------------------------------------------

    /// Demand balance dual (CMO) for submarket at given period.
    #[must_use]
    pub fn demand_balance_dual(&self, submarket_idx: usize, period: usize) -> f64 {
        debug_assert!(submarket_idx < self.num_submarkets);
        debug_assert!(period < self.horizon);
        let idx = self.row_offsets.demand_balance + submarket_idx * self.horizon + period;
        self.solution.dual.get(idx).copied().unwrap_or(0.0)
    }

    /// Hydro balance dual for plant at given period.
    #[must_use]
    pub fn hydro_balance_dual(&self, plant_idx: usize, period: usize) -> f64 {
        debug_assert!(plant_idx < self.num_hydro_plants);
        debug_assert!(period < self.horizon);
        let idx = self.row_offsets.hydro_balance + plant_idx * self.horizon + period;
        self.solution.dual.get(idx).copied().unwrap_or(0.0)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use labdessem_model::indexing::{
        HydroPlantIndex, HydroUnitIndex, RenewablePlantIndex, ThermalUnitIndex,
    };

    fn test_indexing() -> Indexing {
        // SubmarketId is re-exported from labdessem_core via labdessem_model
        // We access it through a full path since solver doesn't depend on core
        Indexing {
            hydro_plant_entries: vec![
                HydroPlantIndex { plant_idx: 0, submarket_idx: 0 },
            ],
            thermal_unit_entries: vec![
                ThermalUnitIndex { plant_idx: 0, unit_idx: 0, submarket_idx: 0 },
                ThermalUnitIndex { plant_idx: 0, unit_idx: 1, submarket_idx: 0 },
            ],
            hydro_unit_entries: vec![
                HydroUnitIndex { plant_idx: 0, group_idx: 0, unit_idx: 0, submarket_idx: 0 },
            ],
            renewable_plant_entries: vec![
                RenewablePlantIndex { plant_idx: 0, submarket_idx: 0 },
            ],
            pumping_plant_entries: vec![],
            interchange_entries: vec![],
            submarket_ids: vec![],  // Empty - not used in accessor tests
            thermal_units: 2,
            hydro_units: 1,
            renewable_plants: 1,
            pumping_plants: 0,
            buses: 1,
            submarkets: 1,
        }
    }

    fn test_solution(num_cols: usize, num_rows: usize) -> LpSolution {
        LpSolution {
            objective: 42.0,
            primal: (0..num_cols).map(|i| i as f64).collect(),
            dual: (0..num_rows).map(|i| i as f64 * 10.0).collect(),
            reduced_costs: vec![0.0; num_cols],
            iterations: 0,
            solve_time_seconds: 0.0,
        }
    }

    #[test]
    fn accessor_thermal_generation() {
        let indexing = test_indexing();
        let params = ConstraintOffsetParams {
            horizon: 3,
            ..Default::default()
        };
        let col_offsets = indexing.compute_column_offsets(3);
        let row_offsets = indexing.compute_constraint_offsets(&params);
        let solution = test_solution(col_offsets.total, row_offsets.total);
        let accessor = SolutionAccessor::new(&solution, &indexing, &params);

        // thermal_generation starts at offset 0
        // unit 0, period 0 -> index 0
        assert!((accessor.thermal_generation(0, 0) - 0.0).abs() < 1e-9);
        // unit 0, period 1 -> index 1
        assert!((accessor.thermal_generation(0, 1) - 1.0).abs() < 1e-9);
        // unit 1, period 0 -> index 3
        assert!((accessor.thermal_generation(1, 0) - 3.0).abs() < 1e-9);
    }

    #[test]
    fn accessor_hydro_generation() {
        let indexing = test_indexing();
        let params = ConstraintOffsetParams {
            horizon: 3,
            ..Default::default()
        };
        let col_offsets = indexing.compute_column_offsets(3);
        let row_offsets = indexing.compute_constraint_offsets(&params);
        let solution = test_solution(col_offsets.total, row_offsets.total);
        let accessor = SolutionAccessor::new(&solution, &indexing, &params);

        // hydro_generation starts after thermal_generation (2 units × 3 periods = 6)
        // unit 0, period 0 -> index 6
        assert!((accessor.hydro_generation(0, 0) - 6.0).abs() < 1e-9);
    }

    #[test]
    fn accessor_demand_balance_dual() {
        let indexing = test_indexing();
        let params = ConstraintOffsetParams {
            horizon: 3,
            ..Default::default()
        };
        let col_offsets = indexing.compute_column_offsets(3);
        let row_offsets = indexing.compute_constraint_offsets(&params);
        let solution = test_solution(col_offsets.total, row_offsets.total);
        let accessor = SolutionAccessor::new(&solution, &indexing, &params);

        // demand_balance starts at row 0
        // submarket 0, period 0 -> row 0 -> dual = 0.0
        assert!((accessor.demand_balance_dual(0, 0) - 0.0).abs() < 1e-9);
        // submarket 0, period 1 -> row 1 -> dual = 10.0
        assert!((accessor.demand_balance_dual(0, 1) - 10.0).abs() < 1e-9);
    }

    #[test]
    fn accessor_objective() {
        let indexing = test_indexing();
        let params = ConstraintOffsetParams {
            horizon: 3,
            ..Default::default()
        };
        let col_offsets = indexing.compute_column_offsets(3);
        let row_offsets = indexing.compute_constraint_offsets(&params);
        let solution = test_solution(col_offsets.total, row_offsets.total);
        let accessor = SolutionAccessor::new(&solution, &indexing, &params);

        assert!((accessor.objective() - 42.0).abs() < 1e-9);
    }
}
