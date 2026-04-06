use crate::{
    error::CoreError,
    ids::{BusId, SubmarketId, ThermalPlantId, ThermalUnitId},
};

#[derive(Debug, Clone, PartialEq)]
pub struct ThermalInitialCondition {
    pub is_on: bool,
    pub generation_mw: f64,
    pub time_in_state: usize,
}

#[derive(Debug, Clone, PartialEq)]
pub struct ThermalUnit {
    pub id: ThermalUnitId,
    pub name: String,
    pub min_generation_mw: f64,
    pub max_generation_mw: f64,
    pub startup_trajectory_mw: Vec<f64>,
    pub shutdown_trajectory_mw: Vec<f64>,
    pub min_up_time: usize,
    pub min_down_time: usize,
    pub startup_cost: f64,
    pub shutdown_cost: f64,
    pub variable_cost_per_mwh: f64,
    pub initial_condition: ThermalInitialCondition,
}

impl ThermalUnit {
    pub fn validate(&self) -> Result<(), CoreError> {
        if self.name.trim().is_empty() {
            return Err(CoreError::validation("thermal unit name cannot be empty"));
        }
        if self.min_generation_mw < 0.0 {
            return Err(CoreError::validation(format!(
                "thermal unit {:?} has negative minimum generation",
                self.id
            )));
        }
        if self.max_generation_mw <= 0.0 {
            return Err(CoreError::validation(format!(
                "thermal unit {:?} must have positive maximum generation",
                self.id
            )));
        }
        if self.min_generation_mw > self.max_generation_mw {
            return Err(CoreError::validation(format!(
                "thermal unit {:?} has min generation greater than max generation",
                self.id
            )));
        }
        if self.startup_trajectory_mw.iter().any(|value| *value < 0.0)
            || self.shutdown_trajectory_mw.iter().any(|value| *value < 0.0)
        {
            return Err(CoreError::validation(format!(
                "thermal unit {:?} has negative value in startup or shutdown trajectory",
                self.id
            )));
        }
        if self
            .startup_trajectory_mw
            .iter()
            .chain(self.shutdown_trajectory_mw.iter())
            .any(|value| *value > self.max_generation_mw)
        {
            return Err(CoreError::validation(format!(
                "thermal unit {:?} has startup or shutdown trajectory above max generation",
                self.id
            )));
        }
        if self.startup_cost < 0.0 || self.shutdown_cost < 0.0 || self.variable_cost_per_mwh < 0.0 {
            return Err(CoreError::validation(format!(
                "thermal unit {:?} has negative cost parameter",
                self.id
            )));
        }
        if self.initial_condition.generation_mw < 0.0 {
            return Err(CoreError::validation(format!(
                "thermal unit {:?} has negative initial generation",
                self.id
            )));
        }
        if self.initial_condition.is_on {
            if self.initial_condition.generation_mw < self.min_generation_mw
                || self.initial_condition.generation_mw > self.max_generation_mw
            {
                return Err(CoreError::validation(format!(
                    "thermal unit {:?} initial generation is outside operating limits",
                    self.id
                )));
            }
        } else if self.initial_condition.generation_mw > 0.0 {
            return Err(CoreError::validation(format!(
                "thermal unit {:?} is off but has positive initial generation",
                self.id
            )));
        }

        Ok(())
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct ThermalPlant {
    pub id: ThermalPlantId,
    pub name: String,
    pub submarket_id: SubmarketId,
    pub bus_id: BusId,
    pub units: Vec<ThermalUnit>,
}

impl ThermalPlant {
    pub fn validate(&self) -> Result<(), CoreError> {
        if self.name.trim().is_empty() {
            return Err(CoreError::validation("thermal plant name cannot be empty"));
        }
        if self.units.is_empty() {
            return Err(CoreError::validation(format!(
                "thermal plant {:?} must contain at least one unit",
                self.id
            )));
        }

        for unit in &self.units {
            unit.validate()?;
        }

        Ok(())
    }
}
