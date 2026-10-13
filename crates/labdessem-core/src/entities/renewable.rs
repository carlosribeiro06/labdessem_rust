use crate::{
    error::CoreError,
    ids::{BusId, RenewablePlantId, SubmarketId},
};

#[derive(Debug, Clone, PartialEq)]
pub struct RenewablePlant {
    pub id: RenewablePlantId,
    pub name: String,
    pub submarket_id: SubmarketId,
    pub bus_id: BusId,
    pub available_generation_mw: Vec<f64>,
}

impl RenewablePlant {
    pub fn validate(&self, horizon: usize) -> Result<(), CoreError> {
        if self.name.trim().is_empty() {
            return Err(CoreError::validation(
                "renewable plant name cannot be empty",
            ));
        }
        if self.available_generation_mw.len() != horizon {
            return Err(CoreError::validation(format!(
                "renewable plant {} horizon mismatch: expected {horizon}, found {}",
                self.id.0,
                self.available_generation_mw.len()
            )));
        }
        if self
            .available_generation_mw
            .iter()
            .any(|value| *value < 0.0)
        {
            return Err(CoreError::validation(format!(
                "renewable plant {} has negative available generation",
                self.id.0
            )));
        }

        Ok(())
    }
}
