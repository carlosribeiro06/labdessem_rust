//! Core system container.

use std::collections::HashSet;

use crate::{
    entities::{Branch, Bus, HydroPlant, PumpingPlant, RenewablePlant, Submarket, ThermalPlant},
    error::CoreError,
};

use super::{
    helpers, FutureCostCut, InterchangeLimit, OperationalLimit, OperationalLimitTarget,
    ResidualCost, StudyHorizon,
};

#[derive(Debug, Clone, PartialEq)]
pub struct System {
    pub horizon: StudyHorizon,
    pub thermal_unit_commitment_enabled: bool,
    pub hydro_unit_commitment_enabled: bool,
    pub ton_residual_enabled: bool,
    pub fpha_enabled: bool,
    pub future_cost_enabled: bool,
    pub residual_costs: Vec<ResidualCost>,
    pub future_cost_cuts: Vec<FutureCostCut>,
    pub submarkets: Vec<Submarket>,
    pub interchange_limits: Vec<InterchangeLimit>,
    pub operational_limits: Vec<OperationalLimit>,
    pub buses: Vec<Bus>,
    pub branches: Vec<Branch>,
    pub thermal_plants: Vec<ThermalPlant>,
    pub hydro_plants: Vec<HydroPlant>,
    pub pumping_plants: Vec<PumpingPlant>,
    pub renewable_plants: Vec<RenewablePlant>,
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

        helpers::validate_unique_ids(
            self.submarkets.iter().map(|entity| entity.id.0),
            "submarket ids must be unique",
        )?;
        helpers::validate_unique_ids(
            self.buses.iter().map(|entity| entity.id.0),
            "bus ids must be unique",
        )?;
        helpers::validate_unique_ids(
            self.branches.iter().map(|entity| entity.id.0),
            "branch ids must be unique",
        )?;
        helpers::validate_unique_ids(
            self.thermal_plants.iter().map(|entity| entity.id.0),
            "thermal plant ids must be unique",
        )?;
        helpers::validate_unique_ids(
            self.hydro_plants.iter().map(|entity| entity.id.0),
            "hydro plant ids must be unique",
        )?;
        helpers::validate_unique_ids(
            self.pumping_plants.iter().map(|entity| entity.id.0),
            "pumping plant ids must be unique",
        )?;
        helpers::validate_unique_ids(
            self.renewable_plants.iter().map(|entity| entity.id.0),
            "renewable plant ids must be unique",
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

        if self.future_cost_enabled && self.future_cost_cuts.is_empty() {
            return Err(CoreError::validation(
                "future cost function is enabled but no future cost cuts were loaded",
            ));
        }
        for cut in &self.future_cost_cuts {
            cut.validate(self.hydro_plants.len())?;
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
            helpers::validate_bus_and_submarket_membership(
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
            plant.validate(self.horizon.periods, self.fpha_enabled)?;
            helpers::validate_bus_and_submarket_membership(
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

        // Rationale: expect() is safe here because ID existence was validated in the previous loop
        #[allow(clippy::expect_used)]
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

        for plant in &self.pumping_plants {
            plant.validate()?;
            helpers::validate_bus_and_submarket_membership(
                plant.id.0,
                "pumping plant",
                plant.bus_id,
                plant.submarket_id,
                &self.buses,
                &bus_ids,
                &submarket_ids,
            )?;
            if !hydro_ids.contains(&plant.downstream_hydro_id) {
                return Err(CoreError::validation(format!(
                    "pumping plant {:?} references unknown downstream hydro plant {:?}",
                    plant.id, plant.downstream_hydro_id
                )));
            }
            if !hydro_ids.contains(&plant.upstream_hydro_id) {
                return Err(CoreError::validation(format!(
                    "pumping plant {:?} references unknown upstream hydro plant {:?}",
                    plant.id, plant.upstream_hydro_id
                )));
            }
        }

        for plant in &self.renewable_plants {
            plant.validate(self.horizon.periods)?;
            helpers::validate_bus_and_submarket_membership(
                plant.id.0,
                "renewable plant",
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
                                "operational limit references unknown thermal plant {plant_id:?}"
                            ))
                        })?;
                    if plant.name != limit.plant_name {
                        return Err(CoreError::validation(format!(
                            "operational limit plant name mismatch for thermal plant {plant_id:?}"
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
                                "operational limit references unknown hydro plant {plant_id:?}"
                            ))
                        })?;
                    if plant.name != limit.plant_name {
                        return Err(CoreError::validation(format!(
                            "operational limit plant name mismatch for hydro plant {plant_id:?}"
                        )));
                    }
                }
                OperationalLimitTarget::PumpingPlant(plant_id) => {
                    let plant = self
                        .pumping_plants
                        .iter()
                        .find(|candidate| candidate.id == plant_id)
                        .ok_or_else(|| {
                            CoreError::validation(format!(
                                "operational limit references unknown pumping plant {plant_id:?}"
                            ))
                        })?;
                    if plant.name != limit.plant_name {
                        return Err(CoreError::validation(format!(
                            "operational limit plant name mismatch for pumping plant {plant_id:?}"
                        )));
                    }
                }
            }
        }

        Ok(())
    }
}
