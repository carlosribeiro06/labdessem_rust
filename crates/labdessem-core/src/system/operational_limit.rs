//! Operational limit constraints for plant variables.

use crate::error::CoreError;
use crate::ids::{HydroPlantId, PumpingPlantId, ThermalPlantId};

/// Variable type for operational limits.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum OperationalLimitVariable {
    Generation,
    Spillage,
    Volume,
    Defluence,
    Turbining,
    Diversion,
    Pumping,
}

/// Target type for operational limits.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum OperationalLimitTarget {
    ThermalPlant(ThermalPlantId),
    HydroPlant(HydroPlantId),
    PumpingPlant(PumpingPlantId),
}

/// An operational limit constraint on a plant variable.
#[derive(Debug, Clone, PartialEq)]
pub struct OperationalLimit {
    pub target: OperationalLimitTarget,
    pub plant_name: String,
    pub variable: OperationalLimitVariable,
    pub start_period: usize,
    pub end_period: usize,
    pub lower_bound: Option<f64>,
    pub upper_bound: Option<f64>,
}

impl OperationalLimit {
    /// Validate the operational limit configuration.
    pub fn validate(&self, horizon: usize) -> Result<(), CoreError> {
        if self.plant_name.trim().is_empty() {
            return Err(CoreError::validation(
                "operational limit plant name cannot be empty",
            ));
        }
        if self.start_period == 0 || self.end_period == 0 {
            return Err(CoreError::validation(
                "operational limit periods must start at 1",
            ));
        }
        if self.start_period > self.end_period {
            return Err(CoreError::validation(format!(
                "operational limit for {} has start period greater than end period",
                self.plant_name
            )));
        }
        if self.end_period > horizon {
            return Err(CoreError::validation(format!(
                "operational limit for {} exceeds study horizon",
                self.plant_name
            )));
        }
        if self.lower_bound.is_none() && self.upper_bound.is_none() {
            return Err(CoreError::validation(format!(
                "operational limit for {} must define at least one bound",
                self.plant_name
            )));
        }
        if let (Some(lower), Some(upper)) = (self.lower_bound, self.upper_bound)
            && lower > upper
        {
            return Err(CoreError::validation(format!(
                "operational limit for {} has lower bound greater than upper bound",
                self.plant_name
            )));
        }

        match (self.target, self.variable) {
            (
                OperationalLimitTarget::ThermalPlant(_),
                OperationalLimitVariable::Spillage
                | OperationalLimitVariable::Volume
                | OperationalLimitVariable::Defluence
                | OperationalLimitVariable::Turbining
                | OperationalLimitVariable::Diversion
                | OperationalLimitVariable::Pumping,
            ) => Err(CoreError::validation(format!(
                "thermal plant {} cannot define limit for {:?}",
                self.plant_name, self.variable
            ))),
            (OperationalLimitTarget::HydroPlant(_), OperationalLimitVariable::Pumping) => {
                Err(CoreError::validation(format!(
                    "hydro plant {} cannot define limit for {:?}",
                    self.plant_name, self.variable
                )))
            }
            (OperationalLimitTarget::PumpingPlant(_), OperationalLimitVariable::Pumping) => Ok(()),
            (OperationalLimitTarget::PumpingPlant(_), _) => Err(CoreError::validation(format!(
                "pumping plant {} cannot define limit for {:?}",
                self.plant_name, self.variable
            ))),
            _ => Ok(()),
        }
    }
}
