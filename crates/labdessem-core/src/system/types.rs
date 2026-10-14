//! Auxiliary types for the system module.

use crate::{error::CoreError, ids::SubmarketId};

/// Limit on power flow between two submarkets.
#[derive(Debug, Clone, PartialEq)]
pub struct InterchangeLimit {
    pub from_submarket_id: SubmarketId,
    pub to_submarket_id: SubmarketId,
    pub max_flow_mw: f64,
    pub penalty_cost_per_mwh: f64,
}

impl InterchangeLimit {
    pub fn validate(&self) -> Result<(), CoreError> {
        if self.from_submarket_id == self.to_submarket_id {
            return Err(CoreError::validation(
                "interchange limit cannot connect the same submarket on both ends",
            ));
        }
        if self.max_flow_mw < 0.0 {
            return Err(CoreError::validation(
                "interchange limit must be non-negative",
            ));
        }
        if self.penalty_cost_per_mwh < 0.0 {
            return Err(CoreError::validation(
                "interchange penalty cost must be non-negative",
            ));
        }

        Ok(())
    }
}

/// Residual marginal cost for a submarket.
#[derive(Debug, Clone, PartialEq)]
pub struct ResidualCost {
    pub submarket_id: SubmarketId,
    pub cmo_per_mwh: f64,
}

impl ResidualCost {
    pub fn validate(&self) -> Result<(), CoreError> {
        if self.cmo_per_mwh < 0.0 {
            return Err(CoreError::validation(format!(
                "residual cost for submarket {:?} must be non-negative",
                self.submarket_id
            )));
        }

        Ok(())
    }
}

/// A Benders cut for the future cost function.
#[derive(Debug, Clone, PartialEq)]
pub struct FutureCostCut {
    pub stage_id: usize,
    pub cut_id: u64,
    pub slot_index: usize,
    pub iteration: usize,
    pub forward_pass_index: usize,
    pub intercept: f64,
    pub coefficients: Vec<f64>,
    pub is_active: bool,
}

impl FutureCostCut {
    pub fn validate(&self, hydro_plant_count: usize) -> Result<(), CoreError> {
        if !self.intercept.is_finite() {
            return Err(CoreError::validation(format!(
                "future cost cut {} has non-finite intercept",
                self.cut_id
            )));
        }
        if self.coefficients.len() != hydro_plant_count {
            return Err(CoreError::validation(format!(
                "future cost cut {} coefficient dimension mismatch: expected {}, found {}",
                self.cut_id,
                hydro_plant_count,
                self.coefficients.len()
            )));
        }
        if self.coefficients.iter().any(|value| !value.is_finite()) {
            return Err(CoreError::validation(format!(
                "future cost cut {} has non-finite coefficients",
                self.cut_id
            )));
        }

        Ok(())
    }
}
