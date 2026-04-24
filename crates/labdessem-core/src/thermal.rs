use crate::{
    error::CoreError,
    ids::{BusId, SubmarketId, ThermalPlantId, ThermalUnitId},
};

#[derive(Debug, Clone, PartialEq)]
pub struct ThermalInitialCondition {
    pub is_on: bool,
    pub generation_mw: f64,
    pub time_in_state: usize,
    pub time_in_ramp: usize,
    pub is_ramping_up: bool,
    pub is_ramping_down: bool,
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
        if self.initial_condition.is_ramping_up && self.initial_condition.is_ramping_down {
            return Err(CoreError::validation(format!(
                "thermal unit {:?} cannot be ramping up and down at the same time",
                self.id
            )));
        }
        if (self.initial_condition.is_ramping_up || self.initial_condition.is_ramping_down)
            && !self.initial_condition.is_on
        {
            return Err(CoreError::validation(format!(
                "thermal unit {:?} in ramp state must be initially on",
                self.id
            )));
        }
        if self.initial_condition.is_ramping_up {
            self.initial_startup_remaining_trajectory()?;
        }
        if self.initial_condition.is_ramping_down {
            self.initial_shutdown_remaining_trajectory()?;
        }

        if self.initial_condition.is_on
            && !self.initial_condition.is_ramping_up
            && !self.initial_condition.is_ramping_down
        {
            if self.initial_condition.generation_mw < self.min_generation_mw
                || self.initial_condition.generation_mw > self.max_generation_mw
            {
                return Err(CoreError::validation(format!(
                    "thermal unit {:?} initial generation is outside operating limits",
                    self.id
                )));
            }
        } else if !self.initial_condition.is_ramping_up
            && !self.initial_condition.is_ramping_down
            && self.initial_condition.generation_mw > 0.0
        {
            return Err(CoreError::validation(format!(
                "thermal unit {:?} is off but has positive initial generation",
                self.id
            )));
        }

        Ok(())
    }

    pub fn initial_startup_remaining_trajectory(&self) -> Result<Vec<f64>, CoreError> {
        remaining_trajectory_from_time_in_state(
            self.id,
            "startup",
            &self.startup_trajectory_mw,
            self.initial_condition.time_in_ramp,
            self.initial_condition.generation_mw,
        )
    }

    pub fn initial_shutdown_remaining_trajectory(&self) -> Result<Vec<f64>, CoreError> {
        remaining_trajectory_from_time_in_state(
            self.id,
            "shutdown",
            &self.shutdown_trajectory_mw,
            self.initial_condition.time_in_ramp,
            self.initial_condition.generation_mw,
        )
    }
}

fn remaining_trajectory_from_time_in_state(
    unit_id: ThermalUnitId,
    trajectory_name: &str,
    trajectory: &[f64],
    time_in_ramp: usize,
    generation_mw: f64,
) -> Result<Vec<f64>, CoreError> {
    if trajectory.is_empty() {
        return Err(CoreError::validation(format!(
            "thermal unit {:?} is marked in {} trajectory but the trajectory is empty",
            unit_id, trajectory_name
        )));
    }

    if time_in_ramp == 0 {
        return Err(CoreError::validation(format!(
            "thermal unit {:?} is marked in {} trajectory but has zero initial ramp time",
            unit_id, trajectory_name
        )));
    }

    if time_in_ramp > trajectory.len() {
        return Err(CoreError::validation(format!(
            "thermal unit {:?} initial ramp time {} exceeds {} trajectory length {}",
            unit_id,
            time_in_ramp,
            trajectory_name,
            trajectory.len()
        )));
    }

    let current_step_idx = time_in_ramp - 1;
    let expected_generation = trajectory[current_step_idx];
    if (expected_generation - generation_mw).abs() > 1e-6 {
        return Err(CoreError::validation(format!(
            "thermal unit {:?} initial generation {} is inconsistent with {} trajectory step {} ({})",
            unit_id, generation_mw, trajectory_name, time_in_ramp, expected_generation
        )));
    }

    Ok(trajectory[(current_step_idx + 1)..].to_vec())
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
