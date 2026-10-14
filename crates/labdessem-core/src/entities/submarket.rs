//! Submarket (subsistema) entity.

use crate::error::CoreError;
use crate::ids::SubmarketId;

/// A submarket (subsystem) in the power system.
#[derive(Debug, Clone, PartialEq)]
pub struct Submarket {
    pub id: SubmarketId,
    pub name: String,
    pub demand_mw: Vec<f64>,
    pub deficit_cost_per_mwh: f64,
}

impl Submarket {
    pub fn validate(&self, horizon: usize) -> Result<(), CoreError> {
        if self.name.trim().is_empty() {
            return Err(CoreError::validation("submarket name cannot be empty"));
        }
        if self.demand_mw.len() != horizon {
            return Err(CoreError::validation(format!(
                "submarket {:?} demand horizon mismatch: expected {}, found {}",
                self.id,
                horizon,
                self.demand_mw.len()
            )));
        }
        if self.demand_mw.iter().any(|value| *value < 0.0) {
            return Err(CoreError::validation(format!(
                "submarket {:?} has negative demand",
                self.id
            )));
        }
        if self.deficit_cost_per_mwh < 0.0 {
            return Err(CoreError::validation(format!(
                "submarket {:?} has negative deficit cost",
                self.id
            )));
        }

        Ok(())
    }
}
