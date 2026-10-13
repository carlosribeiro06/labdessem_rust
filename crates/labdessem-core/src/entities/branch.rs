//! Transmission line (branch) entity.

use crate::error::CoreError;
use crate::ids::{BranchId, BusId};

/// A transmission line connecting two buses.
#[derive(Debug, Clone, PartialEq)]
pub struct Branch {
    pub id: BranchId,
    pub name: String,
    pub from_bus_id: BusId,
    pub to_bus_id: BusId,
    pub reactance_pu: f64,
    pub thermal_limit_mw: f64,
}

impl Branch {
    pub fn validate(&self) -> Result<(), CoreError> {
        if self.name.trim().is_empty() {
            return Err(CoreError::validation("branch name cannot be empty"));
        }
        if self.from_bus_id == self.to_bus_id {
            return Err(CoreError::validation(format!(
                "branch {:?} connects the same bus on both ends",
                self.id
            )));
        }
        if self.reactance_pu <= 0.0 {
            return Err(CoreError::validation(format!(
                "branch {:?} must have positive reactance",
                self.id
            )));
        }
        if self.thermal_limit_mw <= 0.0 {
            return Err(CoreError::validation(format!(
                "branch {:?} must have positive thermal limit",
                self.id
            )));
        }

        Ok(())
    }
}
