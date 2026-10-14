//! Pumping station (elevatória) entity.

use crate::error::CoreError;
use crate::ids::{BusId, HydroPlantId, PumpingPlantId, SubmarketId};

/// A pumping station that moves water between two reservoirs.
#[derive(Debug, Clone, PartialEq)]
pub struct PumpingPlant {
    pub id: PumpingPlantId,
    pub name: String,
    pub submarket_id: SubmarketId,
    pub bus_id: BusId,
    pub downstream_hydro_id: HydroPlantId,
    pub upstream_hydro_id: HydroPlantId,
    pub min_pumping_hm3: f64,
    pub max_pumping_hm3: f64,
    pub specific_consumption_mw_per_m3s: f64,
}

impl PumpingPlant {
    pub fn validate(&self) -> Result<(), CoreError> {
        if self.name.trim().is_empty() {
            return Err(CoreError::validation("pumping plant name cannot be empty"));
        }
        if self.downstream_hydro_id == self.upstream_hydro_id {
            return Err(CoreError::validation(format!(
                "pumping plant {:?} cannot connect the same hydro plant as downstream and upstream",
                self.id
            )));
        }
        if self.min_pumping_hm3 < 0.0 || self.max_pumping_hm3 < 0.0 {
            return Err(CoreError::validation(format!(
                "pumping plant {:?} has negative pumping limit",
                self.id
            )));
        }
        if self.min_pumping_hm3 > self.max_pumping_hm3 {
            return Err(CoreError::validation(format!(
                "pumping plant {:?} has minimum pumping above maximum pumping",
                self.id
            )));
        }
        if self.specific_consumption_mw_per_m3s < 0.0 {
            return Err(CoreError::validation(format!(
                "pumping plant {:?} has negative specific consumption",
                self.id
            )));
        }

        Ok(())
    }
}
