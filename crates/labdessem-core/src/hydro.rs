use crate::{
    error::CoreError,
    ids::{BusId, HydroGroupId, HydroPlantId, HydroUnitId, SubmarketId},
};

#[derive(Debug, Clone, PartialEq)]
pub struct HydroInitialCondition {
    pub is_on: bool,
    pub generation_mw: f64,
    pub time_in_state: usize,
}

#[derive(Debug, Clone, PartialEq)]
pub struct HydroUnit {
    pub id: HydroUnitId,
    pub name: String,
    pub min_generation_mw: f64,
    pub max_generation_mw: f64,
    pub max_turbining_hm3: f64,
    pub startup_trajectory_mw: Vec<f64>,
    pub shutdown_trajectory_mw: Vec<f64>,
    pub min_up_time: usize,
    pub min_down_time: usize,
    pub startup_cost: f64,
    pub shutdown_cost: f64,
    pub initial_condition: HydroInitialCondition,
}

impl HydroUnit {
    pub fn validate(&self) -> Result<(), CoreError> {
        if self.name.trim().is_empty() {
            return Err(CoreError::validation("hydro unit name cannot be empty"));
        }
        if self.min_generation_mw < 0.0 {
            return Err(CoreError::validation(format!(
                "hydro unit {:?} has negative minimum generation",
                self.id
            )));
        }
        if self.max_generation_mw <= 0.0 {
            return Err(CoreError::validation(format!(
                "hydro unit {:?} must have positive maximum generation",
                self.id
            )));
        }
        if self.min_generation_mw > self.max_generation_mw {
            return Err(CoreError::validation(format!(
                "hydro unit {:?} has min generation greater than max generation",
                self.id
            )));
        }
        if self.max_turbining_hm3 <= 0.0 {
            return Err(CoreError::validation(format!(
                "hydro unit {:?} must have positive maximum turbining",
                self.id
            )));
        }
        if self.startup_trajectory_mw.iter().any(|value| *value < 0.0)
            || self.shutdown_trajectory_mw.iter().any(|value| *value < 0.0)
        {
            return Err(CoreError::validation(format!(
                "hydro unit {:?} has negative value in startup or shutdown trajectory",
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
                "hydro unit {:?} has startup or shutdown trajectory above max generation",
                self.id
            )));
        }
        if self.startup_cost < 0.0 || self.shutdown_cost < 0.0 {
            return Err(CoreError::validation(format!(
                "hydro unit {:?} has negative startup or shutdown cost",
                self.id
            )));
        }
        if self.initial_condition.generation_mw < 0.0 {
            return Err(CoreError::validation(format!(
                "hydro unit {:?} has negative initial generation",
                self.id
            )));
        }
        if self.initial_condition.is_on {
            if self.initial_condition.generation_mw < self.min_generation_mw
                || self.initial_condition.generation_mw > self.max_generation_mw
            {
                return Err(CoreError::validation(format!(
                    "hydro unit {:?} initial generation is outside operating limits",
                    self.id
                )));
            }
        } else if self.initial_condition.generation_mw > 0.0 {
            return Err(CoreError::validation(format!(
                "hydro unit {:?} is off but has positive initial generation",
                self.id
            )));
        }

        Ok(())
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct HydroGroup {
    pub id: HydroGroupId,
    pub name: String,
    pub units: Vec<HydroUnit>,
}

impl HydroGroup {
    pub fn validate(&self) -> Result<(), CoreError> {
        if self.name.trim().is_empty() {
            return Err(CoreError::validation("hydro group name cannot be empty"));
        }
        if self.units.is_empty() {
            return Err(CoreError::validation(format!(
                "hydro group {:?} must contain at least one unit",
                self.id
            )));
        }

        for unit in &self.units {
            unit.validate()?;
        }

        Ok(())
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct Reservoir {
    pub min_volume_hm3: f64,
    pub max_volume_hm3: f64,
    pub initial_volume_hm3: f64,
}

impl Reservoir {
    pub fn validate(&self, plant_id: HydroPlantId) -> Result<(), CoreError> {
        if self.min_volume_hm3 < 0.0 || self.max_volume_hm3 < 0.0 || self.initial_volume_hm3 < 0.0 {
            return Err(CoreError::validation(format!(
                "hydro plant {:?} has negative reservoir volume",
                plant_id
            )));
        }
        if self.min_volume_hm3 > self.max_volume_hm3 {
            return Err(CoreError::validation(format!(
                "hydro plant {:?} has minimum reservoir volume above maximum",
                plant_id
            )));
        }
        if self.initial_volume_hm3 < self.min_volume_hm3
            || self.initial_volume_hm3 > self.max_volume_hm3
        {
            return Err(CoreError::validation(format!(
                "hydro plant {:?} has initial reservoir volume outside bounds",
                plant_id
            )));
        }

        Ok(())
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct HydroFphaSegment {
    pub segment: usize,
    pub correction_factor: f64,
    pub rhs: f64,
    pub volume_coefficient: f64,
    pub turbining_coefficient: f64,
    pub lateral_flow_coefficient: f64,
}

impl HydroFphaSegment {
    pub fn validate(&self, plant_id: HydroPlantId) -> Result<(), CoreError> {
        if self.segment == 0 {
            return Err(CoreError::validation(format!(
                "hydro plant {:?} has FPHA segment zero",
                plant_id
            )));
        }
        if self.correction_factor < 0.0 {
            return Err(CoreError::validation(format!(
                "hydro plant {:?} has negative FPHA correction factor",
                plant_id
            )));
        }

        Ok(())
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct HydroPlant {
    pub id: HydroPlantId,
    pub name: String,
    pub submarket_id: SubmarketId,
    pub bus_id: BusId,
    pub upstream_plant_ids: Vec<HydroPlantId>,
    pub downstream_plant_id: Option<HydroPlantId>,
    pub diversion_upstream_plant_ids: Vec<HydroPlantId>,
    pub diversion_plant_id: Option<HydroPlantId>,
    pub reservoir: Reservoir,
    pub natural_inflow_hm3: Vec<f64>,
    pub water_withdrawal_hm3: Vec<f64>,
    pub spillage_cost_per_hm3: f64,
    pub turbining_cost_per_hm3: f64,
    pub fpha_segments: Vec<HydroFphaSegment>,
    pub groups: Vec<HydroGroup>,
}

impl HydroPlant {
    pub fn validate(&self, horizon: usize) -> Result<(), CoreError> {
        if self.name.trim().is_empty() {
            return Err(CoreError::validation("hydro plant name cannot be empty"));
        }
        if self.natural_inflow_hm3.len() != horizon {
            return Err(CoreError::validation(format!(
                "hydro plant {:?} inflow horizon mismatch: expected {}, found {}",
                self.id,
                horizon,
                self.natural_inflow_hm3.len()
            )));
        }
        if self.natural_inflow_hm3.iter().any(|value| *value < 0.0) {
            return Err(CoreError::validation(format!(
                "hydro plant {:?} has negative inflow",
                self.id
            )));
        }
        if self.water_withdrawal_hm3.len() != horizon {
            return Err(CoreError::validation(format!(
                "hydro plant {:?} withdrawal horizon mismatch: expected {}, found {}",
                self.id,
                horizon,
                self.water_withdrawal_hm3.len()
            )));
        }
        if self.spillage_cost_per_hm3 < 0.0 {
            return Err(CoreError::validation(format!(
                "hydro plant {:?} has negative spillage cost",
                self.id
            )));
        }
        if self.turbining_cost_per_hm3 < 0.0 {
            return Err(CoreError::validation(format!(
                "hydro plant {:?} has negative turbining cost",
                self.id
            )));
        }
        if !self.groups.is_empty() && self.fpha_segments.is_empty() {
            return Err(CoreError::validation(format!(
                "hydro plant {:?} must contain at least one FPHA segment",
                self.id
            )));
        }
        for segment in &self.fpha_segments {
            segment.validate(self.id)?;
        }

        self.reservoir.validate(self.id)?;

        for group in &self.groups {
            group.validate()?;
        }

        Ok(())
    }
}
