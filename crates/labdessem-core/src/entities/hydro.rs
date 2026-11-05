use crate::{
    error::CoreError,
    ids::{BusId, HydroGroupId, HydroPlantId, HydroUnitId, SubmarketId},
};

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub enum TurbineType {
    Francis,
    Kaplan,
    /// Pelton turbine, `Turbina` code 3 (alpha 0.5, MM eq. 9.34).
    ///
    /// The code 3 -> Pelton mapping is inferred from ONS decks (4 plants carry code 3; G.P. SOUZA
    /// and HENRY BORDEN have `h_ef` 711-714 m), not defined by the manual: the available CEPEL
    /// sources do not define the hidr.dat `tipo_turbina` codes. If a CEPEL source defines them and
    /// code 3 is not Pelton, remap it (ADR-019).
    Pelton,
    #[default]
    Unknown,
}

impl TurbineType {
    /// Turbine exponent of MM eq. 9.34. `Unknown` uses the Francis value and is reported where
    /// the type is read ([`TurbineType::from_deck_field`]).
    #[inline]
    pub const fn alpha(&self) -> f64 {
        match self {
            Self::Francis | Self::Pelton | Self::Unknown => 0.5,
            Self::Kaplan => 0.2,
        }
    }

    /// Turbine type of the plant `plant_name` from its raw `Turbina` deck value, warning once
    /// when it falls back to [`TurbineType::Unknown`] (R24, ADR-019).
    ///
    /// No warning for codes 1-3, or for code 0 on a plant without units (such plants get no
    /// EngMax row). Every other fallback warns, including a non-integer value.
    pub fn from_deck_field(raw: &str, plant_name: &str, has_units: bool) -> Self {
        let (turbine_type, warning) = Self::classify_deck_field(raw, plant_name, has_units);
        if let Some(message) = warning {
            tracing::warn!("{message}");
        }
        turbine_type
    }

    fn classify_deck_field(raw: &str, plant_name: &str, has_units: bool) -> (Self, Option<String>) {
        let raw = raw.trim();
        let Ok(code) = raw.parse::<i32>() else {
            let warning = format!(
                "hydro plant {plant_name}: Turbina value {raw:?} is not an integer; using TurbineType::Unknown (alpha 0.5)"
            );
            return (Self::Unknown, Some(warning));
        };
        let turbine_type = Self::from(code);
        let warning = (turbine_type == Self::Unknown && (code != 0 || has_units)).then(|| {
            format!(
                "hydro plant {plant_name}: unknown Turbina code {code}; using TurbineType::Unknown (alpha 0.5)"
            )
        });
        (turbine_type, warning)
    }
}

impl From<i32> for TurbineType {
    fn from(value: i32) -> Self {
        match value {
            1 => Self::Francis,
            2 => Self::Kaplan,
            3 => Self::Pelton, // deck-inferred, see TurbineType::Pelton
            _ => Self::Unknown,
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

    /// Volume-level table for forebay interpolation: Vec<(volume_hm3, level_m)>
    /// Sorted by volume ascending. Used for net head calculation in EngMax.
    /// None if geometry data is not available for this plant.
    pub volume_level_table: Option<Vec<(f64, f64)>>,

    /// Tailrace polynomial coefficients [a0, a1, a2, a3, a4] for:
    /// tailrace_level(q) = a0 + a1*q + a2*q² + a3*q³ + a4*q⁴
    /// where q is outflow in m³/s and result is level in meters.
    /// None if tailrace data is not available for this plant.
    pub tailrace_coefficients: Option<[f64; 5]>,
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

        if let Some(eff) = self.efficiency
            && !(0.0..=1.0).contains(&eff)
        {
            return Err(CoreError::validation(format!(
                "hydro plant {:?} efficiency must be between 0 and 1, got {}",
                self.id, eff
            )));
        }
        if let Some(loss) = self.hydraulic_loss_m
            && loss < 0.0
        {
            return Err(CoreError::validation(format!(
                "hydro plant {:?} hydraulic loss cannot be negative",
                self.id
            )));
        }

        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::TurbineType;

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

    #[test]
    fn turbine_type_from_3_is_pelton() {
        assert_eq!(TurbineType::from(3_i32), TurbineType::Pelton);
    }

    #[test]
    fn turbine_type_pelton_alpha() {
        assert!((TurbineType::Pelton.alpha() - 0.5).abs() < f64::EPSILON);
    }

    #[test]
    fn turbine_type_deck_field_fallbacks_follow_adr_019() {
        let cases = [
            ("1", true, TurbineType::Francis, false),
            ("2", true, TurbineType::Kaplan, false),
            ("3", true, TurbineType::Pelton, false),
            (" 3 ", false, TurbineType::Pelton, false),
            ("0", false, TurbineType::Unknown, false),
            ("0", true, TurbineType::Unknown, true),
            ("99", false, TurbineType::Unknown, true),
            ("-1", true, TurbineType::Unknown, true),
            ("X", true, TurbineType::Unknown, true),
            ("", false, TurbineType::Unknown, true),
        ];
        for (raw, has_units, expected_type, warns) in cases {
            let (turbine_type, warning) = TurbineType::classify_deck_field(raw, "UHE-T", has_units);
            assert_eq!(
                turbine_type, expected_type,
                "raw {raw:?}, has_units {has_units}"
            );
            if warns {
                assert!(
                    warning
                        .as_deref()
                        .is_some_and(|m| m.contains("UHE-T") && m.contains(raw.trim())),
                    "raw {raw:?}, has_units {has_units}: {warning:?}"
                );
            } else {
                assert!(
                    warning.is_none(),
                    "raw {raw:?}, has_units {has_units}: {warning:?}"
                );
            }
        }
        assert_eq!(
            TurbineType::from_deck_field(" 3 ", "UHE-T", true),
            TurbineType::Pelton
        );
    }
}
