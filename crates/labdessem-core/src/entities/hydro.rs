use crate::{
    error::CoreError,
    ids::{BusId, HydroGroupId, HydroPlantId, HydroUnitId, SubmarketId},
};

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub enum TurbineType {
    Francis,
    Kaplan,
    #[default]
    Unknown,
}

impl TurbineType {
    #[inline]
    pub const fn alpha(&self) -> f64 {
        match self {
            Self::Francis => 0.5,
            Self::Kaplan => 0.2,
            Self::Unknown => 0.5,
        }
    }
}

impl From<i32> for TurbineType {
    fn from(value: i32) -> Self {
        match value {
            1 => Self::Francis,
            2 => Self::Kaplan,
            other => {
                tracing::warn!(turbine_code = other, "unknown turbine type, defaulting to Unknown");
                Self::Unknown
            }
        }
    }
}

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
    pub effective_head_m: f64,
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
                "hydro plant {plant_id:?} has negative reservoir volume"
            )));
        }
        if self.min_volume_hm3 > self.max_volume_hm3 {
            return Err(CoreError::validation(format!(
                "hydro plant {plant_id:?} has minimum reservoir volume above maximum"
            )));
        }
        if self.initial_volume_hm3 < self.min_volume_hm3
            || self.initial_volume_hm3 > self.max_volume_hm3
        {
            return Err(CoreError::validation(format!(
                "hydro plant {plant_id:?} has initial reservoir volume outside bounds"
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
                "hydro plant {plant_id:?} has FPHA segment zero"
            )));
        }
        if self.correction_factor < 0.0 {
            return Err(CoreError::validation(format!(
                "hydro plant {plant_id:?} has negative FPHA correction factor"
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
    pub specific_productivity_mw_per_m3s: f64,
    pub hydraulic_loss_m: Option<f64>,
    pub efficiency: Option<f64>,
    pub fpha_segments: Vec<HydroFphaSegment>,
    pub groups: Vec<HydroGroup>,
    pub turbine_type: TurbineType,
}

impl HydroPlant {
    pub fn validate(&self, horizon: usize, fpha_enabled: bool) -> Result<(), CoreError> {
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
        if self.specific_productivity_mw_per_m3s < 0.0 {
            return Err(CoreError::validation(format!(
                "hydro plant {:?} has negative specific productivity",
                self.id
            )));
        }
        if fpha_enabled && !self.groups.is_empty() && self.fpha_segments.is_empty() {
            return Err(CoreError::validation(format!(
                "hydro plant {:?} must contain at least one FPHA segment",
                self.id
            )));
        }
        if !fpha_enabled && !self.groups.is_empty() && self.specific_productivity_mw_per_m3s <= 0.0
        {
            return Err(CoreError::validation(format!(
                "hydro plant {:?} must contain positive specific productivity when FPHA is disabled",
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

        if let Some(eff) = self.efficiency {
            if !(0.0..=1.0).contains(&eff) {
                return Err(CoreError::validation(format!(
                    "hydro plant {:?} efficiency must be between 0 and 1, got {}",
                    self.id, eff
                )));
            }
        }
        if let Some(loss) = self.hydraulic_loss_m {
            if loss < 0.0 {
                return Err(CoreError::validation(format!(
                    "hydro plant {:?} hydraulic loss cannot be negative",
                    self.id
                )));
            }
        }

        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn turbine_type_francis_alpha() {
        assert!((TurbineType::Francis.alpha() - 0.5).abs() < f64::EPSILON);
    }

    #[test]
    fn turbine_type_kaplan_alpha() {
        assert!((TurbineType::Kaplan.alpha() - 0.2).abs() < f64::EPSILON);
    }

    #[test]
    fn turbine_type_unknown_alpha() {
        assert!((TurbineType::Unknown.alpha() - 0.5).abs() < f64::EPSILON);
    }

    #[test]
    fn turbine_type_from_1_is_francis() {
        assert_eq!(TurbineType::from(1_i32), TurbineType::Francis);
    }

    #[test]
    fn turbine_type_from_2_is_kaplan() {
        assert_eq!(TurbineType::from(2_i32), TurbineType::Kaplan);
    }

    #[test]
    fn turbine_type_from_0_is_unknown() {
        assert_eq!(TurbineType::from(0_i32), TurbineType::Unknown);
    }

    #[test]
    fn turbine_type_from_99_is_unknown() {
        assert_eq!(TurbineType::from(99_i32), TurbineType::Unknown);
    }

    #[test]
    fn turbine_type_default_is_unknown() {
        assert_eq!(TurbineType::default(), TurbineType::Unknown);
    }
}