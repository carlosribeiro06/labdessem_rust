//! System construction via builder pattern.

use super::{
    FutureCostCut, InterchangeLimit, OperationalLimit, ResidualCost, StudyHorizon, System,
    validate_system,
};
use crate::entities::{
    Branch, Bus, HydroPlant, InflowHistoryRow, PumpingPlant, RenewablePlant, Submarket, ThermalPlant,
};
use crate::error::CoreError;

/// Builder for constructing a validated [`System`].
///
/// # Example
///
/// ```ignore
/// let system = SystemBuilder::new(horizon)
///     .with_submarkets(submarkets)
///     .with_thermal_plants(thermals)
///     .with_hydro_plants(hydros)
///     .build()?;
/// ```
#[derive(Debug, Default)]
pub struct SystemBuilder {
    horizon: Option<StudyHorizon>,
    submarkets: Vec<Submarket>,
    buses: Vec<Bus>,
    branches: Vec<Branch>,
    thermal_plants: Vec<ThermalPlant>,
    hydro_plants: Vec<HydroPlant>,
    pumping_plants: Vec<PumpingPlant>,
    renewable_plants: Vec<RenewablePlant>,
    operational_limits: Vec<OperationalLimit>,
    interchange_limits: Vec<InterchangeLimit>,
    residual_costs: Vec<ResidualCost>,
    future_cost_cuts: Vec<FutureCostCut>,
    inflow_history: Vec<InflowHistoryRow>,

    thermal_unit_commitment_enabled: bool,
    hydro_unit_commitment_enabled: bool,
    ton_residual_enabled: bool,
    fpha_enabled: bool,
    future_cost_enabled: bool,
}

impl SystemBuilder {
    /// Create a new builder with the given study horizon.
    pub fn new(horizon: StudyHorizon) -> Self {
        Self {
            horizon: Some(horizon),
            ..Default::default()
        }
    }

    /// Set the submarkets.
    pub fn with_submarkets(mut self, submarkets: Vec<Submarket>) -> Self {
        self.submarkets = submarkets;
        self
    }

    /// Set the buses.
    pub fn with_buses(mut self, buses: Vec<Bus>) -> Self {
        self.buses = buses;
        self
    }

    /// Set the branches.
    pub fn with_branches(mut self, branches: Vec<Branch>) -> Self {
        self.branches = branches;
        self
    }

    /// Set the thermal plants.
    pub fn with_thermal_plants(mut self, plants: Vec<ThermalPlant>) -> Self {
        self.thermal_plants = plants;
        self
    }

    /// Set the hydro plants.
    pub fn with_hydro_plants(mut self, plants: Vec<HydroPlant>) -> Self {
        self.hydro_plants = plants;
        self
    }

    /// Set the pumping plants.
    pub fn with_pumping_plants(mut self, plants: Vec<PumpingPlant>) -> Self {
        self.pumping_plants = plants;
        self
    }

    /// Set the renewable plants.
    pub fn with_renewable_plants(mut self, plants: Vec<RenewablePlant>) -> Self {
        self.renewable_plants = plants;
        self
    }

    /// Set the operational limits.
    pub fn with_operational_limits(mut self, limits: Vec<OperationalLimit>) -> Self {
        self.operational_limits = limits;
        self
    }

    /// Set the interchange limits.
    pub fn with_interchange_limits(mut self, limits: Vec<InterchangeLimit>) -> Self {
        self.interchange_limits = limits;
        self
    }

    /// Set the residual costs.
    pub fn with_residual_costs(mut self, costs: Vec<ResidualCost>) -> Self {
        self.residual_costs = costs;
        self
    }

    /// Set the future cost cuts.
    pub fn with_future_cost_cuts(mut self, cuts: Vec<FutureCostCut>) -> Self {
        self.future_cost_cuts = cuts;
        self
    }

    /// Set the inflow history observations.
    pub fn with_inflow_history(mut self, inflow_history: Vec<InflowHistoryRow>) -> Self {
        self.inflow_history = inflow_history;
        self
    }

    /// Set all configuration flags at once.
    pub fn with_flags(
        mut self,
        thermal_unit_commitment: bool,
        hydro_unit_commitment: bool,
        ton_residual: bool,
        fpha: bool,
        future_cost: bool,
    ) -> Self {
        self.thermal_unit_commitment_enabled = thermal_unit_commitment;
        self.hydro_unit_commitment_enabled = hydro_unit_commitment;
        self.ton_residual_enabled = ton_residual;
        self.fpha_enabled = fpha;
        self.future_cost_enabled = future_cost;
        self
    }

    /// Build and validate the system.
    ///
    /// # Errors
    ///
    /// Returns [`CoreError`] if the horizon is missing or validation fails.
    pub fn build(self) -> Result<System, CoreError> {
        let horizon = self
            .horizon
            .ok_or_else(|| CoreError::validation("SystemBuilder: horizon is required"))?;

        let system = System {
            horizon,
            thermal_unit_commitment_enabled: self.thermal_unit_commitment_enabled,
            hydro_unit_commitment_enabled: self.hydro_unit_commitment_enabled,
            ton_residual_enabled: self.ton_residual_enabled,
            fpha_enabled: self.fpha_enabled,
            future_cost_enabled: self.future_cost_enabled,
            residual_costs: self.residual_costs,
            future_cost_cuts: self.future_cost_cuts,
            submarkets: self.submarkets,
            interchange_limits: self.interchange_limits,
            operational_limits: self.operational_limits,
            buses: self.buses,
            branches: self.branches,
            thermal_plants: self.thermal_plants,
            hydro_plants: self.hydro_plants,
            pumping_plants: self.pumping_plants,
            renewable_plants: self.renewable_plants,
            inflow_history: self.inflow_history,
        };

        // Validation is performed via the validate module
        validate_system(&system)?;

        Ok(system)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn minimal_horizon() -> StudyHorizon {
        StudyHorizon {
            periods: 1,
            period_duration_hours: 1.0,
            original_periods: 1,
            original_period_durations_hours: vec![1.0],
            internal_to_original_period: vec![1],
            internal_subperiod_index: vec![1],
        }
    }

    #[test]
    fn builder_requires_horizon() {
        let result = SystemBuilder::default().build();
        assert!(result.is_err());
    }

    #[test]
    fn builder_requires_valid_system() {
        // A system with just a horizon but no submarkets/buses should fail validation
        let result = SystemBuilder::new(minimal_horizon()).build();
        assert!(result.is_err());
    }
}
