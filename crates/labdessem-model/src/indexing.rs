use labdessem_core::{ids::SubmarketId, system::System};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ThermalUnitIndex {
    pub plant_idx: usize,
    pub unit_idx: usize,
    pub submarket_idx: usize,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct HydroUnitIndex {
    pub plant_idx: usize,
    pub group_idx: usize,
    pub unit_idx: usize,
    pub submarket_idx: usize,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RenewablePlantIndex {
    pub plant_idx: usize,
    pub submarket_idx: usize,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct HydroPlantIndex {
    pub plant_idx: usize,
    pub submarket_idx: usize,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PumpingPlantIndex {
    pub plant_idx: usize,
    pub submarket_idx: usize,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct InterchangeIndex {
    pub from_submarket_idx: usize,
    pub to_submarket_idx: usize,
    pub period: usize,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Indexing {
    pub hydro_plant_entries: Vec<HydroPlantIndex>,
    pub thermal_unit_entries: Vec<ThermalUnitIndex>,
    pub hydro_unit_entries: Vec<HydroUnitIndex>,
    pub wind_plant_entries: Vec<RenewablePlantIndex>,
    pub solar_plant_entries: Vec<RenewablePlantIndex>,
    pub pumping_plant_entries: Vec<PumpingPlantIndex>,
    pub interchange_entries: Vec<InterchangeIndex>,
    pub submarket_ids: Vec<SubmarketId>,
    pub thermal_units: usize,
    pub hydro_units: usize,
    pub wind_plants: usize,
    pub solar_plants: usize,
    pub pumping_plants: usize,
    pub buses: usize,
    pub submarkets: usize,
}

impl Indexing {
    pub fn from_system(system: &System) -> Self {
        let submarket_ids: Vec<_> = system
            .submarkets
            .iter()
            .map(|submarket| submarket.id)
            .collect();

        let thermal_unit_entries = system
            .thermal_plants
            .iter()
            .enumerate()
            .flat_map(|(plant_idx, plant)| {
                let submarket_idx = submarket_position(&submarket_ids, plant.submarket_id);
                plant
                    .units
                    .iter()
                    .enumerate()
                    .map(move |(unit_idx, _)| ThermalUnitIndex {
                        plant_idx,
                        unit_idx,
                        submarket_idx,
                    })
            })
            .collect::<Vec<_>>();

        let hydro_unit_entries =
            system
                .hydro_plants
                .iter()
                .enumerate()
                .flat_map(|(plant_idx, plant)| {
                    let submarket_idx = submarket_position(&submarket_ids, plant.submarket_id);
                    plant
                        .groups
                        .iter()
                        .enumerate()
                        .flat_map(move |(group_idx, group)| {
                            group.units.iter().enumerate().map(move |(unit_idx, _)| {
                                HydroUnitIndex {
                                    plant_idx,
                                    group_idx,
                                    unit_idx,
                                    submarket_idx,
                                }
                            })
                        })
                })
                .collect::<Vec<_>>();

        let hydro_plant_entries = system
            .hydro_plants
            .iter()
            .enumerate()
            .map(|(plant_idx, plant)| HydroPlantIndex {
                plant_idx,
                submarket_idx: submarket_position(&submarket_ids, plant.submarket_id),
            })
            .collect::<Vec<_>>();

        let wind_plant_entries = system
            .wind_plants
            .iter()
            .enumerate()
            .map(|(plant_idx, plant)| RenewablePlantIndex {
                plant_idx,
                submarket_idx: submarket_position(&submarket_ids, plant.submarket_id),
            })
            .collect::<Vec<_>>();

        let solar_plant_entries = system
            .solar_plants
            .iter()
            .enumerate()
            .map(|(plant_idx, plant)| RenewablePlantIndex {
                plant_idx,
                submarket_idx: submarket_position(&submarket_ids, plant.submarket_id),
            })
            .collect::<Vec<_>>();

        let pumping_plant_entries = system
            .pumping_plants
            .iter()
            .enumerate()
            .map(|(plant_idx, plant)| PumpingPlantIndex {
                plant_idx,
                submarket_idx: submarket_position(&submarket_ids, plant.submarket_id),
            })
            .collect::<Vec<_>>();

        let interchange_entries = (0..system.horizon.periods)
            .flat_map(|period| {
                (0..system.submarkets.len()).flat_map(move |from_submarket_idx| {
                    (0..system.submarkets.len())
                        .filter(move |to_submarket_idx| *to_submarket_idx != from_submarket_idx)
                        .map(move |to_submarket_idx| InterchangeIndex {
                            from_submarket_idx,
                            to_submarket_idx,
                            period,
                        })
                })
            })
            .collect::<Vec<_>>();

        Self {
            thermal_units: thermal_unit_entries.len(),
            hydro_units: hydro_unit_entries.len(),
            wind_plants: wind_plant_entries.len(),
            solar_plants: solar_plant_entries.len(),
            pumping_plants: pumping_plant_entries.len(),
            buses: system.buses.len(),
            submarkets: system.submarkets.len(),
            hydro_plant_entries,
            thermal_unit_entries,
            hydro_unit_entries,
            wind_plant_entries,
            solar_plant_entries,
            pumping_plant_entries,
            interchange_entries,
            submarket_ids,
        }
    }
}

fn submarket_position(submarket_ids: &[SubmarketId], submarket_id: SubmarketId) -> usize {
    submarket_ids
        .iter()
        .position(|candidate| *candidate == submarket_id)
        .expect("system validation should guarantee known submarket ids")
}
