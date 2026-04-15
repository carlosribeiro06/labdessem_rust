use std::collections::HashSet;

use crate::{
    error::CoreError,
    hydro::HydroPlant,
    ids::{BranchId, BusId, HydroPlantId, SubmarketId, ThermalPlantId},
    renewable::{SolarPlant, WindPlant},
    thermal::ThermalPlant,
};

#[derive(Debug, Clone, PartialEq)]
pub struct StudyHorizon {
    pub periods: usize,
    pub period_duration_hours: f64,
}

impl StudyHorizon {
    pub fn validate(&self) -> Result<(), CoreError> {
        if self.periods == 0 {
            return Err(CoreError::validation(
                "study horizon must contain at least one period",
            ));
        }
        if self.period_duration_hours <= 0.0 {
            return Err(CoreError::validation(
                "study horizon period duration must be positive",
            ));
        }

        Ok(())
    }
}

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

#[derive(Debug, Clone, PartialEq)]
pub struct InterchangeLimit {
    pub from_submarket_id: SubmarketId,
    pub to_submarket_id: SubmarketId,
    pub max_flow_mw: f64,
    pub penalty_cost_per_mwh: f64,
}

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

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum OperationalLimitVariable {
    Generation,
    Spillage,
    Volume,
    Defluence,
    Turbining,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum OperationalLimitTarget {
    ThermalPlant(ThermalPlantId),
    HydroPlant(HydroPlantId),
}

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
        if let (Some(lower), Some(upper)) = (self.lower_bound, self.upper_bound) {
            if lower > upper {
                return Err(CoreError::validation(format!(
                    "operational limit for {} has lower bound greater than upper bound",
                    self.plant_name
                )));
            }
        }

        match (self.target, self.variable) {
            (
                OperationalLimitTarget::ThermalPlant(_),
                OperationalLimitVariable::Spillage
                | OperationalLimitVariable::Volume
                | OperationalLimitVariable::Defluence
                | OperationalLimitVariable::Turbining,
            ) => Err(CoreError::validation(format!(
                "thermal plant {} cannot define limit for {:?}",
                self.plant_name, self.variable
            ))),
            _ => Ok(()),
        }
    }
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

#[derive(Debug, Clone, PartialEq)]
pub struct System {
    pub horizon: StudyHorizon,
    pub thermal_unit_commitment_enabled: bool,
    pub hydro_unit_commitment_enabled: bool,
    pub ton_residual_enabled: bool,
    pub residual_costs: Vec<ResidualCost>,
    pub submarkets: Vec<Submarket>,
    pub interchange_limits: Vec<InterchangeLimit>,
    pub operational_limits: Vec<OperationalLimit>,
    pub buses: Vec<Bus>,
    pub branches: Vec<Branch>,
    pub thermal_plants: Vec<ThermalPlant>,
    pub hydro_plants: Vec<HydroPlant>,
    pub wind_plants: Vec<WindPlant>,
    pub solar_plants: Vec<SolarPlant>,
}

impl System {
    pub fn validate(&self) -> Result<(), CoreError> {
        self.horizon.validate()?;

        if self.submarkets.is_empty() {
            return Err(CoreError::validation(
                "system must contain at least one submarket",
            ));
        }
        if self.buses.is_empty() {
            return Err(CoreError::validation(
                "system must contain at least one bus",
            ));
        }

        validate_unique_ids(
            self.submarkets.iter().map(|entity| entity.id.0),
            "submarket ids must be unique",
        )?;
        validate_unique_ids(
            self.buses.iter().map(|entity| entity.id.0),
            "bus ids must be unique",
        )?;
        validate_unique_ids(
            self.branches.iter().map(|entity| entity.id.0),
            "branch ids must be unique",
        )?;
        validate_unique_ids(
            self.thermal_plants.iter().map(|entity| entity.id.0),
            "thermal plant ids must be unique",
        )?;
        validate_unique_ids(
            self.hydro_plants.iter().map(|entity| entity.id.0),
            "hydro plant ids must be unique",
        )?;
        validate_unique_ids(
            self.wind_plants.iter().map(|entity| entity.id.0),
            "wind plant ids must be unique",
        )?;
        validate_unique_ids(
            self.solar_plants.iter().map(|entity| entity.id.0),
            "solar plant ids must be unique",
        )?;

        for submarket in &self.submarkets {
            submarket.validate(self.horizon.periods)?;
        }

        let submarket_ids: HashSet<_> = self.submarkets.iter().map(|entity| entity.id).collect();

        let mut seen_residual_costs = HashSet::new();
        for residual_cost in &self.residual_costs {
            residual_cost.validate()?;
            if !submarket_ids.contains(&residual_cost.submarket_id) {
                return Err(CoreError::validation(format!(
                    "residual cost references unknown submarket {:?}",
                    residual_cost.submarket_id
                )));
            }
            if !seen_residual_costs.insert(residual_cost.submarket_id) {
                return Err(CoreError::validation(
                    "residual costs must be unique per submarket",
                ));
            }
        }

        if self.ton_residual_enabled && self.residual_costs.len() != self.submarkets.len() {
            return Err(CoreError::validation(
                "TON residual objective is enabled but residual costs are missing for some submarkets",
            ));
        }

        let mut seen_interchange_pairs = HashSet::new();
        for limit in &self.interchange_limits {
            limit.validate()?;
            if !submarket_ids.contains(&limit.from_submarket_id)
                || !submarket_ids.contains(&limit.to_submarket_id)
            {
                return Err(CoreError::validation(
                    "interchange limit references unknown submarket",
                ));
            }
            if !seen_interchange_pairs.insert((limit.from_submarket_id, limit.to_submarket_id)) {
                return Err(CoreError::validation(
                    "interchange limits must be unique per directed submarket pair",
                ));
            }
        }

        for bus in &self.buses {
            bus.validate(self.horizon.periods)?;
            if !submarket_ids.contains(&bus.submarket_id) {
                return Err(CoreError::validation(format!(
                    "bus {:?} references unknown submarket {:?}",
                    bus.id, bus.submarket_id
                )));
            }
        }

        let slack_bus_count = self.buses.iter().filter(|bus| bus.angle_reference).count();
        if slack_bus_count != 1 {
            return Err(CoreError::validation(format!(
                "system must contain exactly one angle reference bus, found {slack_bus_count}"
            )));
        }

        let bus_ids: HashSet<_> = self.buses.iter().map(|entity| entity.id).collect();

        for branch in &self.branches {
            branch.validate()?;
            if !bus_ids.contains(&branch.from_bus_id) || !bus_ids.contains(&branch.to_bus_id) {
                return Err(CoreError::validation(format!(
                    "branch {:?} references an unknown bus",
                    branch.id
                )));
            }
        }

        for plant in &self.thermal_plants {
            plant.validate()?;
            validate_bus_and_submarket_membership(
                plant.id.0,
                "thermal plant",
                plant.bus_id,
                plant.submarket_id,
                &self.buses,
                &bus_ids,
                &submarket_ids,
            )?;
        }

        let hydro_ids: HashSet<_> = self.hydro_plants.iter().map(|entity| entity.id).collect();

        for plant in &self.hydro_plants {
            plant.validate(self.horizon.periods)?;
            validate_bus_and_submarket_membership(
                plant.id.0,
                "hydro plant",
                plant.bus_id,
                plant.submarket_id,
                &self.buses,
                &bus_ids,
                &submarket_ids,
            )?;
            let mut seen_upstreams = HashSet::new();
            for upstream in &plant.upstream_plant_ids {
                if !seen_upstreams.insert(*upstream) {
                    return Err(CoreError::validation(format!(
                        "hydro plant {:?} has duplicated upstream reference {:?}",
                        plant.id, upstream
                    )));
                }
                if *upstream == plant.id {
                    return Err(CoreError::validation(format!(
                        "hydro plant {:?} cannot point to itself as upstream",
                        plant.id
                    )));
                }
                if !hydro_ids.contains(upstream) {
                    return Err(CoreError::validation(format!(
                        "hydro plant {:?} references unknown upstream plant {:?}",
                        plant.id, upstream
                    )));
                }
            }
            if let Some(downstream) = plant.downstream_plant_id {
                if downstream == plant.id {
                    return Err(CoreError::validation(format!(
                        "hydro plant {:?} cannot point to itself as downstream",
                        plant.id
                    )));
                }
                if !hydro_ids.contains(&downstream) {
                    return Err(CoreError::validation(format!(
                        "hydro plant {:?} references unknown downstream plant {:?}",
                        plant.id, downstream
                    )));
                }
            }

            let mut seen_diversion_upstreams = HashSet::new();
            for upstream in &plant.diversion_upstream_plant_ids {
                if !seen_diversion_upstreams.insert(*upstream) {
                    return Err(CoreError::validation(format!(
                        "hydro plant {:?} has duplicated diversion upstream reference {:?}",
                        plant.id, upstream
                    )));
                }
                if *upstream == plant.id {
                    return Err(CoreError::validation(format!(
                        "hydro plant {:?} cannot point to itself as diversion upstream",
                        plant.id
                    )));
                }
                if !hydro_ids.contains(upstream) {
                    return Err(CoreError::validation(format!(
                        "hydro plant {:?} references unknown diversion upstream plant {:?}",
                        plant.id, upstream
                    )));
                }
            }
            if let Some(diversion) = plant.diversion_plant_id {
                if diversion == plant.id {
                    return Err(CoreError::validation(format!(
                        "hydro plant {:?} cannot point to itself as diversion destination",
                        plant.id
                    )));
                }
                if !hydro_ids.contains(&diversion) {
                    return Err(CoreError::validation(format!(
                        "hydro plant {:?} references unknown diversion destination plant {:?}",
                        plant.id, diversion
                    )));
                }
            }
        }

        for plant in &self.hydro_plants {
            for upstream in &plant.upstream_plant_ids {
                let upstream_plant = self
                    .hydro_plants
                    .iter()
                    .find(|candidate| candidate.id == *upstream)
                    .expect("upstream hydro plant should exist after id validation");

                if upstream_plant.downstream_plant_id != Some(plant.id) {
                    return Err(CoreError::validation(format!(
                        "hydro plant {:?} has inconsistent upstream/downstream linkage with {:?}",
                        plant.id, upstream
                    )));
                }
            }

            if let Some(downstream) = plant.downstream_plant_id {
                let downstream_plant = self
                    .hydro_plants
                    .iter()
                    .find(|candidate| candidate.id == downstream)
                    .expect("downstream hydro plant should exist after id validation");

                if !downstream_plant.upstream_plant_ids.contains(&plant.id) {
                    return Err(CoreError::validation(format!(
                        "hydro plant {:?} has inconsistent downstream/upstream linkage with {:?}",
                        plant.id, downstream
                    )));
                }
            }

            for upstream in &plant.diversion_upstream_plant_ids {
                let upstream_plant = self
                    .hydro_plants
                    .iter()
                    .find(|candidate| candidate.id == *upstream)
                    .expect("diversion upstream hydro plant should exist after id validation");

                if upstream_plant.diversion_plant_id != Some(plant.id) {
                    return Err(CoreError::validation(format!(
                        "hydro plant {:?} has inconsistent diversion upstream linkage with {:?}",
                        plant.id, upstream
                    )));
                }
            }

            if let Some(diversion) = plant.diversion_plant_id {
                let diversion_plant = self
                    .hydro_plants
                    .iter()
                    .find(|candidate| candidate.id == diversion)
                    .expect("diversion hydro plant should exist after id validation");

                if !diversion_plant
                    .diversion_upstream_plant_ids
                    .contains(&plant.id)
                {
                    return Err(CoreError::validation(format!(
                        "hydro plant {:?} has inconsistent diversion destination linkage with {:?}",
                        plant.id, diversion
                    )));
                }
            }
        }

        for plant in &self.wind_plants {
            plant.validate(self.horizon.periods)?;
            validate_bus_and_submarket_membership(
                plant.id.0,
                "wind plant",
                plant.bus_id,
                plant.submarket_id,
                &self.buses,
                &bus_ids,
                &submarket_ids,
            )?;
        }

        for plant in &self.solar_plants {
            plant.validate(self.horizon.periods)?;
            validate_bus_and_submarket_membership(
                plant.id.0,
                "solar plant",
                plant.bus_id,
                plant.submarket_id,
                &self.buses,
                &bus_ids,
                &submarket_ids,
            )?;
        }

        for limit in &self.operational_limits {
            limit.validate(self.horizon.periods)?;
            match limit.target {
                OperationalLimitTarget::ThermalPlant(plant_id) => {
                    let plant = self
                        .thermal_plants
                        .iter()
                        .find(|candidate| candidate.id == plant_id)
                        .ok_or_else(|| {
                            CoreError::validation(format!(
                                "operational limit references unknown thermal plant {:?}",
                                plant_id
                            ))
                        })?;
                    if plant.name != limit.plant_name {
                        return Err(CoreError::validation(format!(
                            "operational limit plant name mismatch for thermal plant {:?}",
                            plant_id
                        )));
                    }
                }
                OperationalLimitTarget::HydroPlant(plant_id) => {
                    let plant = self
                        .hydro_plants
                        .iter()
                        .find(|candidate| candidate.id == plant_id)
                        .ok_or_else(|| {
                            CoreError::validation(format!(
                                "operational limit references unknown hydro plant {:?}",
                                plant_id
                            ))
                        })?;
                    if plant.name != limit.plant_name {
                        return Err(CoreError::validation(format!(
                            "operational limit plant name mismatch for hydro plant {:?}",
                            plant_id
                        )));
                    }
                }
            }
        }

        Ok(())
    }
}

fn validate_unique_ids(
    ids: impl IntoIterator<Item = usize>,
    message: &str,
) -> Result<(), CoreError> {
    let mut seen = HashSet::new();
    for id in ids {
        if !seen.insert(id) {
            return Err(CoreError::validation(message));
        }
    }

    Ok(())
}

fn validate_bus_and_submarket_membership(
    entity_id: usize,
    entity_name: &str,
    bus_id: BusId,
    submarket_id: SubmarketId,
    buses: &[Bus],
    bus_ids: &HashSet<BusId>,
    submarket_ids: &HashSet<SubmarketId>,
) -> Result<(), CoreError> {
    if !bus_ids.contains(&bus_id) {
        return Err(CoreError::validation(format!(
            "{entity_name} {entity_id} references unknown bus {:?}",
            bus_id
        )));
    }
    if !submarket_ids.contains(&submarket_id) {
        return Err(CoreError::validation(format!(
            "{entity_name} {entity_id} references unknown submarket {:?}",
            submarket_id
        )));
    }

    let bus = buses
        .iter()
        .find(|bus| bus.id == bus_id)
        .expect("bus id should exist after membership check");

    if bus.submarket_id != submarket_id {
        return Err(CoreError::validation(format!(
            "{entity_name} {entity_id} has inconsistent bus/submarket mapping"
        )));
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{
        hydro::{HydroFphaSegment, HydroGroup, HydroInitialCondition, HydroUnit, Reservoir},
        ids::{
            BranchId, BusId, HydroGroupId, HydroPlantId, HydroUnitId, SolarPlantId, SubmarketId,
            ThermalPlantId, ThermalUnitId, WindPlantId,
        },
        renewable::{SolarPlant, WindPlant},
        thermal::{ThermalInitialCondition, ThermalUnit},
    };

    fn fpha_segments() -> Vec<HydroFphaSegment> {
        vec![HydroFphaSegment {
            segment: 1,
            correction_factor: 1.0,
            rhs: 0.0,
            volume_coefficient: 0.0,
            turbining_coefficient: 1.0,
            lateral_flow_coefficient: 0.0,
        }]
    }

    fn valid_system() -> System {
        System {
            horizon: StudyHorizon {
                periods: 2,
                period_duration_hours: 1.0,
            },
            thermal_unit_commitment_enabled: true,
            hydro_unit_commitment_enabled: true,
            ton_residual_enabled: false,
            residual_costs: vec![],
            submarkets: vec![Submarket {
                id: SubmarketId(1),
                name: "SE".into(),
                demand_mw: vec![100.0, 110.0],
                deficit_cost_per_mwh: 1_000.0,
            }],
            interchange_limits: vec![],
            operational_limits: vec![],
            buses: vec![Bus {
                id: BusId(1),
                name: "BUS-1".into(),
                submarket_id: SubmarketId(1),
                angle_reference: true,
                demand_mw: vec![100.0, 110.0],
            }],
            branches: vec![],
            thermal_plants: vec![ThermalPlant {
                id: ThermalPlantId(1),
                name: "UTE-1".into(),
                submarket_id: SubmarketId(1),
                bus_id: BusId(1),
                units: vec![ThermalUnit {
                    id: ThermalUnitId(1),
                    name: "GT-1".into(),
                    min_generation_mw: 20.0,
                    max_generation_mw: 100.0,
                    startup_trajectory_mw: vec![20.0, 40.0, 80.0],
                    shutdown_trajectory_mw: vec![80.0, 40.0, 20.0],
                    min_up_time: 1,
                    min_down_time: 1,
                    startup_cost: 500.0,
                    shutdown_cost: 100.0,
                    variable_cost_per_mwh: 250.0,
                    initial_condition: ThermalInitialCondition {
                        is_on: true,
                        generation_mw: 40.0,
                        time_in_state: 2,
                        is_ramping_up: false,
                        is_ramping_down: false,
                    },
                }],
            }],
            hydro_plants: vec![HydroPlant {
                id: HydroPlantId(1),
                name: "UHE-1".into(),
                submarket_id: SubmarketId(1),
                bus_id: BusId(1),
                upstream_plant_ids: vec![],
                downstream_plant_id: None,
                diversion_upstream_plant_ids: vec![],
                diversion_plant_id: None,
                fpha_segments: fpha_segments(),
                reservoir: Reservoir {
                    min_volume_hm3: 10.0,
                    max_volume_hm3: 100.0,
                    initial_volume_hm3: 50.0,
                },
                natural_inflow_hm3: vec![5.0, 6.0],
                spillage_cost_per_hm3: 1.0,
                groups: vec![HydroGroup {
                    id: HydroGroupId(1),
                    name: "CJ-1".into(),
                    units: vec![HydroUnit {
                        id: HydroUnitId(1),
                        name: "UG-1".into(),
                        min_generation_mw: 10.0,
                        max_generation_mw: 80.0,
                        max_turbining_hm3: 40.0,
                        startup_trajectory_mw: vec![10.0, 30.0, 60.0],
                        shutdown_trajectory_mw: vec![60.0, 30.0, 10.0],
                        min_up_time: 1,
                        min_down_time: 1,
                        startup_cost: 0.0,
                        shutdown_cost: 0.0,
                        initial_condition: HydroInitialCondition {
                            is_on: true,
                            generation_mw: 20.0,
                            time_in_state: 1,
                        },
                    }],
                }],
            }],
            wind_plants: vec![WindPlant {
                id: WindPlantId(1),
                name: "EOL-1".into(),
                submarket_id: SubmarketId(1),
                bus_id: BusId(1),
                available_generation_mw: vec![30.0, 25.0],
            }],
            solar_plants: vec![SolarPlant {
                id: SolarPlantId(1),
                name: "SOL-1".into(),
                submarket_id: SubmarketId(1),
                bus_id: BusId(1),
                available_generation_mw: vec![10.0, 15.0],
            }],
        }
    }

    #[test]
    fn validates_a_consistent_system() {
        assert!(valid_system().validate().is_ok());
    }

    #[test]
    fn rejects_system_without_single_reference_bus() {
        let mut system = valid_system();
        system.buses.push(Bus {
            id: BusId(2),
            name: "BUS-2".into(),
            submarket_id: SubmarketId(1),
            angle_reference: true,
            demand_mw: vec![0.0, 0.0],
        });
        system.branches.push(Branch {
            id: BranchId(1),
            name: "LT-1".into(),
            from_bus_id: BusId(1),
            to_bus_id: BusId(2),
            reactance_pu: 0.1,
            thermal_limit_mw: 100.0,
        });

        assert!(system.validate().is_err());
    }

    #[test]
    fn rejects_inconsistent_hydro_cascade_links() {
        let mut system = valid_system();
        system.hydro_plants.push(HydroPlant {
            id: HydroPlantId(2),
            name: "UHE-2".into(),
            submarket_id: SubmarketId(1),
            bus_id: BusId(1),
            upstream_plant_ids: vec![],
            downstream_plant_id: None,
            diversion_upstream_plant_ids: vec![],
            diversion_plant_id: None,
            fpha_segments: fpha_segments(),
            reservoir: Reservoir {
                min_volume_hm3: 5.0,
                max_volume_hm3: 50.0,
                initial_volume_hm3: 20.0,
            },
            natural_inflow_hm3: vec![2.0, 2.0],
            spillage_cost_per_hm3: 1.0,
            groups: vec![HydroGroup {
                id: HydroGroupId(2),
                name: "CJ-2".into(),
                units: vec![HydroUnit {
                    id: HydroUnitId(2),
                    name: "UG-2".into(),
                    min_generation_mw: 5.0,
                    max_generation_mw: 30.0,
                    max_turbining_hm3: 15.0,
                    startup_trajectory_mw: vec![5.0, 15.0],
                    shutdown_trajectory_mw: vec![15.0, 5.0],
                    min_up_time: 1,
                    min_down_time: 1,
                    startup_cost: 0.0,
                    shutdown_cost: 0.0,
                    initial_condition: HydroInitialCondition {
                        is_on: true,
                        generation_mw: 10.0,
                        time_in_state: 1,
                    },
                }],
            }],
        });

        system.hydro_plants[0].downstream_plant_id = Some(HydroPlantId(2));

        assert!(system.validate().is_err());
    }

    #[test]
    fn rejects_duplicated_upstream_reference() {
        let mut system = valid_system();
        system.hydro_plants.push(HydroPlant {
            id: HydroPlantId(2),
            name: "UHE-2".into(),
            submarket_id: SubmarketId(1),
            bus_id: BusId(1),
            upstream_plant_ids: vec![],
            downstream_plant_id: Some(HydroPlantId(1)),
            diversion_upstream_plant_ids: vec![],
            diversion_plant_id: None,
            fpha_segments: fpha_segments(),
            reservoir: Reservoir {
                min_volume_hm3: 5.0,
                max_volume_hm3: 50.0,
                initial_volume_hm3: 20.0,
            },
            natural_inflow_hm3: vec![2.0, 2.0],
            spillage_cost_per_hm3: 1.0,
            groups: vec![HydroGroup {
                id: HydroGroupId(2),
                name: "CJ-2".into(),
                units: vec![HydroUnit {
                    id: HydroUnitId(2),
                    name: "UG-2".into(),
                    min_generation_mw: 5.0,
                    max_generation_mw: 30.0,
                    max_turbining_hm3: 15.0,
                    startup_trajectory_mw: vec![5.0, 15.0],
                    shutdown_trajectory_mw: vec![15.0, 5.0],
                    min_up_time: 1,
                    min_down_time: 1,
                    startup_cost: 0.0,
                    shutdown_cost: 0.0,
                    initial_condition: HydroInitialCondition {
                        is_on: true,
                        generation_mw: 10.0,
                        time_in_state: 1,
                    },
                }],
            }],
        });
        system.hydro_plants[0].upstream_plant_ids = vec![HydroPlantId(2), HydroPlantId(2)];

        assert!(system.validate().is_err());
    }

    #[test]
    fn rejects_duplicated_directed_interchange_limit() {
        let mut system = valid_system();
        system.submarkets.push(Submarket {
            id: SubmarketId(2),
            name: "S".into(),
            demand_mw: vec![50.0, 50.0],
            deficit_cost_per_mwh: 1_000.0,
        });
        system.interchange_limits = vec![
            InterchangeLimit {
                from_submarket_id: SubmarketId(1),
                to_submarket_id: SubmarketId(2),
                max_flow_mw: 100.0,
                penalty_cost_per_mwh: 1.0,
            },
            InterchangeLimit {
                from_submarket_id: SubmarketId(1),
                to_submarket_id: SubmarketId(2),
                max_flow_mw: 80.0,
                penalty_cost_per_mwh: 1.0,
            },
        ];

        assert!(system.validate().is_err());
    }
}
