use crate::{
    error::CoreError,
    ids::{BusId, SolarPlantId, SubmarketId, WindPlantId},
};

#[derive(Debug, Clone, PartialEq)]
pub struct WindPlant {
    pub id: WindPlantId,
    pub name: String,
    pub submarket_id: SubmarketId,
    pub bus_id: BusId,
    pub available_generation_mw: Vec<f64>,
}

impl WindPlant {
    pub fn validate(&self, horizon: usize) -> Result<(), CoreError> {
        validate_renewable_series(
            "wind plant",
            self.id.0,
            &self.name,
            &self.available_generation_mw,
            horizon,
        )
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct SolarPlant {
    pub id: SolarPlantId,
    pub name: String,
    pub submarket_id: SubmarketId,
    pub bus_id: BusId,
    pub available_generation_mw: Vec<f64>,
}

impl SolarPlant {
    pub fn validate(&self, horizon: usize) -> Result<(), CoreError> {
        validate_renewable_series(
            "solar plant",
            self.id.0,
            &self.name,
            &self.available_generation_mw,
            horizon,
        )
    }
}

fn validate_renewable_series(
    entity_name: &str,
    entity_id: usize,
    name: &str,
    available_generation_mw: &[f64],
    horizon: usize,
) -> Result<(), CoreError> {
    if name.trim().is_empty() {
        return Err(CoreError::validation(format!(
            "{entity_name} name cannot be empty"
        )));
    }
    if available_generation_mw.len() != horizon {
        return Err(CoreError::validation(format!(
            "{entity_name} {entity_id} horizon mismatch: expected {horizon}, found {}",
            available_generation_mw.len()
        )));
    }
    if available_generation_mw.iter().any(|value| *value < 0.0) {
        return Err(CoreError::validation(format!(
            "{entity_name} {entity_id} has negative available generation"
        )));
    }

    Ok(())
}
