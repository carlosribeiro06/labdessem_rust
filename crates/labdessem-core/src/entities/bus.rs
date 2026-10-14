//! Electrical bus entity.

use crate::error::CoreError;
use crate::ids::{BusId, SubmarketId};

/// An electrical bus in the network.
#[derive(Debug, Clone, PartialEq)]
pub struct Bus {
    pub id: BusId,
    pub name: String,
    pub submarket_id: SubmarketId,
    pub angle_reference: bool,
    pub demand_mw: Vec<f64>,
}

impl Bus {
    pub fn validate(&self, horizon: usize) -> Result<(), CoreError> {
        if self.name.trim().is_empty() {
            return Err(CoreError::validation("bus name cannot be empty"));
        }
        if self.demand_mw.len() != horizon {
            return Err(CoreError::validation(format!(
                "bus {:?} demand horizon mismatch: expected {}, found {}",
                self.id,
                horizon,
                self.demand_mw.len()
            )));
        }
        if self.demand_mw.iter().any(|value| *value < 0.0) {
            return Err(CoreError::validation(format!(
                "bus {:?} has negative demand",
                self.id
            )));
        }

        Ok(())
    }
}
