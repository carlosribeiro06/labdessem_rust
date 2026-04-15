use std::{
    collections::{BTreeMap, HashMap},
    fs,
    fs::File,
    io,
    io::Write,
    path::{Path, PathBuf},
};

use csv::{ReaderBuilder, StringRecord};
use labdessem_core::{
    hydro::{
        HydroFphaSegment, HydroGroup, HydroInitialCondition, HydroPlant, HydroUnit, Reservoir,
    },
    ids::{
        BranchId, BusId, HydroGroupId, HydroPlantId, HydroUnitId, PumpingPlantId, SubmarketId,
        ThermalPlantId, ThermalUnitId, WindPlantId,
    },
    renewable::{SolarPlant, WindPlant},
    system::{
        Branch, Bus, InterchangeLimit, OperationalLimit, OperationalLimitTarget,
        OperationalLimitVariable, PumpingPlant, ResidualCost, StudyHorizon, Submarket, System,
    },
    thermal::{ThermalInitialCondition, ThermalPlant, ThermalUnit},
};
use serde::{Deserialize, Deserializer};

use crate::error::IoError;

#[derive(Debug, Clone, Deserialize, PartialEq, Eq)]
pub struct StudyConfig {
    pub case_path: PathBuf,
    pub rede: u8,
    #[serde(rename = "UCT")]
    pub uct: u8,
    #[serde(rename = "UCH")]
    pub uch: u8,
    #[serde(rename = "TON_Residual")]
    pub ton_residual: u8,
}

pub fn read_study_config(config_path: impl AsRef<Path>) -> Result<StudyConfig, IoError> {
    let config_path = config_path.as_ref();
    let contents = fs::read_to_string(config_path).map_err(|error| {
        IoError::invalid_data(format!(
            "failed to read study config {}: {error}",
            config_path.display()
        ))
    })?;

    let mut config: StudyConfig = serde_json::from_str(&contents)?;
    if config.case_path.is_relative() {
        let config_dir = config_path.parent().ok_or_else(|| {
            IoError::invalid_data(format!(
                "study config path {} does not have a parent directory",
                config_path.display()
            ))
        })?;
        config.case_path = config_dir.join(&config.case_path);
    }

    Ok(config)
}

pub fn read_study_from_config(config_path: impl AsRef<Path>) -> Result<System, IoError> {
    let config = read_study_config(config_path)?;
    read_study_from_path_with_options(
        config.case_path,
        config.ton_residual != 0,
        config.rede != 0,
        config.uct != 0,
        config.uch != 0,
    )
}

fn read_study_from_path_with_options(
    base_path: impl AsRef<Path>,
    ton_residual_enabled: bool,
    network_enabled: bool,
    thermal_unit_commitment_enabled: bool,
    hydro_unit_commitment_enabled: bool,
) -> Result<System, IoError> {
    let base_path = base_path.as_ref();
    let cad_path = base_path.join("CAD");
    let oper_path = base_path.join("OPER");

    let submarket_catalog: Vec<SubmarketCatalogRow> = read_csv(cad_path.join("CAD_SBM.csv"))?;
    let duration_rows: Vec<DurationRow> = read_csv(oper_path.join("OPER_DURACAO.csv"))?;
    let submarket_operation_rows: Vec<SubmarketOperationRow> =
        read_csv(oper_path.join("OPER_SBM.csv"))?;
    let bus_rows: Vec<BusOperationRow> = if network_enabled {
        read_csv(oper_path.join("OPER_CARGA_BARRA.csv"))?
    } else {
        Vec::new()
    };
    let branch_rows: Vec<BranchRow> = if network_enabled {
        read_csv(oper_path.join("OPER_LINHA.csv"))?
    } else {
        Vec::new()
    };
    let interchange_rows: Vec<InterchangeRow> =
        read_csv(oper_path.join("OPER_LIMITE_INTERCAMBIO.csv"))?;
    let residual_cost_rows: Vec<ResidualCostRow> = if ton_residual_enabled {
        read_csv(oper_path.join("OPER_CUSTO_RESIDUAL.csv"))?
    } else {
        Vec::new()
    };
    let thermal_rows: Vec<ThermalUnitRow> = read_csv(cad_path.join("CAD_UNID_UTE.csv"))?;
    let hydro_rows: Vec<HydroPlantRow> = read_csv(cad_path.join("CAD_UHE.csv"))?;
    let pumping_rows = read_pumping_table(cad_path.join("CAD_USIE.csv"))?;
    let hydro_unit_rows: Vec<HydroUnitRow> = read_csv(cad_path.join("CAD_CONJ_UHE.csv"))?;
    let hydro_inflow_rows: Vec<HydroInflowRow> = read_csv(oper_path.join("OPER_VAZAO.csv"))?;
    let hydro_fpha_rows = read_fpha_table(oper_path.join("OPER_FPHA.csv"))?;
    let renewable_catalog_rows = read_renewable_catalog_table(cad_path.join("CAD_REN.csv"))?;
    let renewable_operation_rows = read_renewable_operation_table(oper_path.join("OPER_REN.csv"))?;

    let (thermal_startup_trajectories, thermal_shutdown_trajectories) =
        if thermal_unit_commitment_enabled {
            read_thermal_trajectory_table(cad_path.join("CAD_RAMPAS_TERMICAS.csv"))?
        } else {
            (HashMap::new(), HashMap::new())
        };
    let hydro_startup_trajectories = if hydro_unit_commitment_enabled {
        read_trajectory_table(cad_path.join("CAD_RAMPAS_UP_UHE.csv"))?
    } else {
        HashMap::new()
    };
    let hydro_shutdown_trajectories = if hydro_unit_commitment_enabled {
        read_trajectory_table(cad_path.join("CAD_RAMPAS_DOWN_UHE.csv"))?
    } else {
        HashMap::new()
    };

    let horizon = build_horizon(&duration_rows)?;
    let submarkets = build_submarkets(&submarket_catalog, &submarket_operation_rows, &horizon)?;
    let buses = if network_enabled {
        build_buses(&bus_rows, &horizon)?
    } else {
        build_dummy_buses(&submarkets, horizon.periods)
    };
    let branches = if network_enabled {
        build_branches(&branch_rows)
    } else {
        Vec::new()
    };
    let interchange_limits = build_interchange_limits(&interchange_rows);
    let residual_costs = build_residual_costs(&residual_cost_rows);
    let operational_limit_rows: Vec<OperationalLimitRow> =
        read_csv(oper_path.join("OPER_REST_LIM.csv"))?;
    let thermal_plants = build_thermal_plants(
        &thermal_rows,
        &thermal_startup_trajectories,
        &thermal_shutdown_trajectories,
        &submarkets,
        horizon.period_duration_hours,
        network_enabled,
        thermal_unit_commitment_enabled,
    )?;
    let hydro_plants = build_hydro_plants(
        &hydro_rows,
        &hydro_unit_rows,
        &hydro_inflow_rows,
        &hydro_fpha_rows,
        &hydro_startup_trajectories,
        &hydro_shutdown_trajectories,
        &submarkets,
        &horizon,
        network_enabled,
        hydro_unit_commitment_enabled,
    )?;
    let pumping_plants = build_pumping_plants(
        &pumping_rows,
        &hydro_plants,
        &submarkets,
        horizon.period_duration_hours,
        network_enabled,
    )?;
    let (wind_plants, solar_plants) = build_renewables(
        &renewable_catalog_rows,
        &renewable_operation_rows,
        &submarkets,
        &horizon,
        network_enabled,
    )?;
    let operational_limits = build_operational_limits(
        &operational_limit_rows,
        &thermal_plants,
        &hydro_plants,
        &horizon,
    )?;

    let system = System {
        horizon,
        thermal_unit_commitment_enabled,
        hydro_unit_commitment_enabled,
        ton_residual_enabled,
        residual_costs,
        submarkets,
        interchange_limits,
        operational_limits,
        buses,
        branches,
        thermal_plants,
        hydro_plants,
        pumping_plants,
        wind_plants,
        solar_plants,
    };

    system.validate()?;

    Ok(system)
}

const INTERNAL_PERIOD_DURATION_HOURS: f64 = 0.5;

fn build_horizon(duration_rows: &[DurationRow]) -> Result<StudyHorizon, IoError> {
    if duration_rows.is_empty() {
        return Err(IoError::invalid_data("OPER_DURACAO.csv cannot be empty"));
    }

    let mut periods = BTreeMap::new();
    for row in duration_rows {
        periods.insert(row.periodo, row.duracao);
    }

    let expected_periods = periods.len();
    for period in 1..=expected_periods {
        if !periods.contains_key(&period) {
            return Err(IoError::invalid_data(format!(
                "missing period {period} in OPER_DURACAO.csv"
            )));
        }
    }

    let mut original_period_durations_hours = Vec::with_capacity(expected_periods);
    let mut internal_to_original_period = Vec::new();
    let mut internal_subperiod_index = Vec::new();

    for original_period in 1..=expected_periods {
        let duration = *periods
            .get(&original_period)
            .expect("duration should exist after sequence validation");
        if duration <= 0.0 {
            return Err(IoError::invalid_data(format!(
                "duration for period {original_period} must be positive"
            )));
        }
        let subperiods = duration / INTERNAL_PERIOD_DURATION_HOURS;
        let rounded_subperiods = subperiods.round();
        if (subperiods - rounded_subperiods).abs() > 1e-8 {
            return Err(IoError::invalid_data(format!(
                "duration for period {original_period} must be a multiple of {INTERNAL_PERIOD_DURATION_HOURS} hour"
            )));
        }
        let subperiod_count = rounded_subperiods as usize;
        if subperiod_count == 0 {
            return Err(IoError::invalid_data(format!(
                "duration for period {original_period} is below the internal time step"
            )));
        }

        original_period_durations_hours.push(duration);
        for subperiod in 1..=subperiod_count {
            internal_to_original_period.push(original_period);
            internal_subperiod_index.push(subperiod);
        }
    }

    Ok(StudyHorizon {
        periods: internal_to_original_period.len(),
        period_duration_hours: INTERNAL_PERIOD_DURATION_HOURS,
        original_periods: expected_periods,
        original_period_durations_hours,
        internal_to_original_period,
        internal_subperiod_index,
    })
}

fn build_submarkets(
    catalog_rows: &[SubmarketCatalogRow],
    operation_rows: &[SubmarketOperationRow],
    horizon: &StudyHorizon,
) -> Result<Vec<Submarket>, IoError> {
    let mut demand_by_submarket: HashMap<usize, Vec<f64>> = HashMap::new();
    let mut deficit_cost_by_submarket: HashMap<usize, f64> = HashMap::new();

    for row in operation_rows {
        let demand_series = demand_by_submarket
            .entry(row.codigo_submercado)
            .or_insert_with(|| vec![0.0; horizon.periods]);
        for period_idx in internal_periods_for_input_period(row.periodo, horizon, "OPER_SBM.csv")? {
            demand_series[period_idx] = row.demanda;
        }

        match deficit_cost_by_submarket.get(&row.codigo_submercado) {
            Some(cost) if (*cost - row.custo_deficit).abs() > f64::EPSILON => {
                return Err(IoError::invalid_data(format!(
                    "submarket {} has inconsistent deficit cost across periods",
                    row.codigo_submercado
                )));
            }
            Some(_) => {}
            None => {
                deficit_cost_by_submarket.insert(row.codigo_submercado, row.custo_deficit);
            }
        }
    }

    catalog_rows
        .iter()
        .map(|row| {
            let demand_mw = demand_by_submarket
                .get(&row.codigo)
                .cloned()
                .unwrap_or_else(|| vec![0.0; horizon.periods]);

            let deficit_cost_per_mwh =
                *deficit_cost_by_submarket
                    .get(&row.codigo)
                    .unwrap_or_else(|| {
                        deficit_cost_by_submarket
                            .values()
                            .next()
                            .expect("OPER_SBM.csv must contain at least one deficit cost")
                    });

            Ok(Submarket {
                id: SubmarketId(row.codigo),
                name: row.nome.clone(),
                demand_mw,
                deficit_cost_per_mwh,
            })
        })
        .collect()
}

fn build_buses(rows: &[BusOperationRow], horizon: &StudyHorizon) -> Result<Vec<Bus>, IoError> {
    let mut buses = BTreeMap::<usize, BusAccumulator>::new();

    for row in rows {
        let entry = buses
            .entry(row.codigo_barra)
            .or_insert_with(|| BusAccumulator {
                name: row.nome_barra.clone(),
                submarket_id: row.codigo_submercado,
                angle_reference: row.swing,
                demand_mw: vec![0.0; horizon.periods],
            });

        if entry.name != row.nome_barra {
            return Err(IoError::invalid_data(format!(
                "bus {} has inconsistent name across periods",
                row.codigo_barra
            )));
        }
        if entry.submarket_id != row.codigo_submercado {
            return Err(IoError::invalid_data(format!(
                "bus {} has inconsistent submarket across periods",
                row.codigo_barra
            )));
        }
        if entry.angle_reference != row.swing {
            return Err(IoError::invalid_data(format!(
                "bus {} has inconsistent swing flag across periods",
                row.codigo_barra
            )));
        }

        for period_idx in
            internal_periods_for_input_period(row.periodo, horizon, "OPER_CARGA_BARRA.csv")?
        {
            entry.demand_mw[period_idx] = row.carga;
        }
    }

    Ok(buses
        .into_iter()
        .map(|(id, bus)| Bus {
            id: BusId(id),
            name: bus.name,
            submarket_id: SubmarketId(bus.submarket_id),
            angle_reference: bus.angle_reference,
            demand_mw: bus.demand_mw,
        })
        .collect())
}

fn internal_periods_for_input_period(
    period: usize,
    horizon: &StudyHorizon,
    file_name: &str,
) -> Result<Vec<usize>, IoError> {
    if !(1..=horizon.original_periods).contains(&period) {
        return Err(IoError::invalid_data(format!(
            "invalid period {period} in {file_name}; expected values between 1 and {}",
            horizon.original_periods
        )));
    }

    Ok(horizon.internal_periods_for_original(period))
}

fn build_dummy_buses(submarkets: &[Submarket], horizon: usize) -> Vec<Bus> {
    submarkets
        .iter()
        .map(|submarket| Bus {
            id: BusId(submarket.id.0),
            name: format!("DUMMY-BUS-{}", submarket.name),
            submarket_id: submarket.id,
            angle_reference: submarket.id == SubmarketId(1),
            demand_mw: vec![0.0; horizon],
        })
        .collect()
}

fn build_branches(rows: &[BranchRow]) -> Vec<Branch> {
    rows.iter()
        .map(|row| Branch {
            id: BranchId(row.codigo_linha),
            name: format!("LINHA-{}", row.codigo_linha),
            from_bus_id: BusId(row.de),
            to_bus_id: BusId(row.para),
            reactance_pu: row.reatancia,
            thermal_limit_mw: row.capacidade,
        })
        .collect()
}

fn build_interchange_limits(rows: &[InterchangeRow]) -> Vec<InterchangeLimit> {
    rows.iter()
        .map(|row| InterchangeLimit {
            from_submarket_id: SubmarketId(row.sbm_de),
            to_submarket_id: SubmarketId(row.sbm_para),
            max_flow_mw: row.limite,
            penalty_cost_per_mwh: row.penalidade,
        })
        .collect()
}

fn build_thermal_plants(
    rows: &[ThermalUnitRow],
    startup_trajectories: &HashMap<String, Vec<f64>>,
    shutdown_trajectories: &HashMap<String, Vec<f64>>,
    submarkets: &[Submarket],
    period_duration_hours: f64,
    network_enabled: bool,
    thermal_unit_commitment_enabled: bool,
) -> Result<Vec<ThermalPlant>, IoError> {
    let mut grouped = BTreeMap::<usize, Vec<&ThermalUnitRow>>::new();
    for row in rows {
        grouped.entry(row.codigo_ute).or_default().push(row);
    }

    grouped
        .into_iter()
        .map(|(plant_code, plant_rows)| {
            let first = plant_rows[0];
            let mut units = Vec::with_capacity(plant_rows.len());

            for row in plant_rows {
                let plant_name = row.nome_ute.trim();
                let unit_trajectory_key = format!("{}-{}", plant_name, row.unidade);
                let max_generation_mw =
                    normalize_max_bound(row.pmin, row.pmax, "Pmax", &unit_trajectory_key)?;
                let startup = if thermal_unit_commitment_enabled {
                    trajectory_for(startup_trajectories, &unit_trajectory_key)?
                } else {
                    Vec::new()
                };
                let shutdown = if thermal_unit_commitment_enabled {
                    trajectory_for(shutdown_trajectories, &unit_trajectory_key)?
                } else {
                    Vec::new()
                };

                units.push(ThermalUnit {
                    id: ThermalUnitId(row.unidade),
                    name: format!("{}-{}", plant_name, row.unidade),
                    min_generation_mw: row.pmin,
                    max_generation_mw,
                    startup_trajectory_mw: startup,
                    shutdown_trajectory_mw: shutdown,
                    min_up_time: hours_to_periods(
                        row.ton,
                        period_duration_hours,
                        "Ton",
                        plant_name,
                    )?,
                    min_down_time: hours_to_periods(
                        row.toff,
                        period_duration_hours,
                        "Toff",
                        plant_name,
                    )?,
                    startup_cost: row.custo_partida,
                    shutdown_cost: row.custo_desliga,
                    variable_cost_per_mwh: row.cvu,
                    initial_condition: ThermalInitialCondition {
                        is_on: row.status_inic != 0,
                        generation_mw: row.ger_inic,
                        time_in_state: hours_to_periods(
                            row.tinic,
                            period_duration_hours,
                            "Tinic",
                            plant_name,
                        )?,
                        is_ramping_up: row.rup_inic != 0,
                        is_ramping_down: row.rdown_inic != 0,
                    },
                });
            }

            let submarket_id = submarket_id_by_name(submarkets, &first.submercado)?;
            Ok(ThermalPlant {
                id: ThermalPlantId(plant_code),
                name: first.nome_ute.trim().to_string(),
                submarket_id,
                bus_id: BusId(bus_id_or_dummy(
                    first.barra,
                    network_enabled,
                    first.nome_ute.trim(),
                    submarket_id,
                )?),
                units,
            })
        })
        .collect()
}

fn build_hydro_plants(
    plant_rows: &[HydroPlantRow],
    unit_rows: &[HydroUnitRow],
    inflow_rows: &[HydroInflowRow],
    fpha_rows: &[HydroFphaRow],
    startup_trajectories: &HashMap<String, Vec<f64>>,
    shutdown_trajectories: &HashMap<String, Vec<f64>>,
    submarkets: &[Submarket],
    horizon: &StudyHorizon,
    network_enabled: bool,
    hydro_unit_commitment_enabled: bool,
) -> Result<Vec<HydroPlant>, IoError> {
    let period_duration_hours = horizon.period_duration_hours;
    let hydro_code_to_id: HashMap<_, _> = plant_rows
        .iter()
        .map(|row| (row.codigo, HydroPlantId(row.codigo)))
        .collect();

    let downstream_by_code: HashMap<usize, Option<HydroPlantId>> = plant_rows
        .iter()
        .map(|row| {
            optional_hydro_reference_by_code(&row.jusante, &hydro_code_to_id)
                .map(|downstream| (row.codigo, downstream))
        })
        .collect::<Result<_, _>>()?;
    let diversion_by_code: HashMap<usize, Option<HydroPlantId>> = plant_rows
        .iter()
        .map(|row| {
            optional_hydro_reference_by_code(&row.desvio, &hydro_code_to_id)
                .map(|diversion| (row.codigo, diversion))
        })
        .collect::<Result<_, _>>()?;
    let mut upstreams_by_code = HashMap::<usize, Vec<HydroPlantId>>::new();
    let mut diversion_upstreams_by_code = HashMap::<usize, Vec<HydroPlantId>>::new();
    for row in plant_rows {
        if let Some(downstream) = downstream_by_code.get(&row.codigo).copied().flatten() {
            upstreams_by_code
                .entry(downstream.0)
                .or_default()
                .push(HydroPlantId(row.codigo));
        }
        if let Some(diversion) = diversion_by_code.get(&row.codigo).copied().flatten() {
            diversion_upstreams_by_code
                .entry(diversion.0)
                .or_default()
                .push(HydroPlantId(row.codigo));
        }
    }

    let mut inflows_by_plant = HashMap::<usize, Vec<f64>>::new();
    let mut withdrawals_by_plant = HashMap::<usize, Vec<f64>>::new();
    for row in inflow_rows {
        let series = inflows_by_plant
            .entry(row.codigo)
            .or_insert_with(|| vec![0.0; horizon.periods]);
        let withdrawals = withdrawals_by_plant
            .entry(row.codigo)
            .or_insert_with(|| vec![0.0; horizon.periods]);
        for period_idx in internal_periods_for_input_period(row.periodo, horizon, "OPER_VAZAO.csv")?
        {
            series[period_idx] = flow_m3s_to_hm3(row.afluencia, period_duration_hours);
            withdrawals[period_idx] = flow_m3s_to_hm3(row.retirada, period_duration_hours);
        }
    }

    let mut units_by_plant = BTreeMap::<usize, Vec<&HydroUnitRow>>::new();
    for row in unit_rows {
        units_by_plant.entry(row.codigo).or_default().push(row);
    }

    let mut fpha_by_plant = BTreeMap::<usize, Vec<HydroFphaSegment>>::new();
    for row in fpha_rows {
        fpha_by_plant
            .entry(row.codigo)
            .or_default()
            .push(HydroFphaSegment {
                segment: row.segmento,
                correction_factor: row.fator_correcao,
                rhs: row.rhs,
                volume_coefficient: row.volume_coefficient,
                turbining_coefficient: row.turbining_coefficient,
                lateral_flow_coefficient: row.lateral_flow_coefficient,
            });
    }

    plant_rows
        .iter()
        .map(|row| {
            let plant_unit_rows = units_by_plant.get(&row.codigo).cloned().unwrap_or_default();

            let mut groups_by_id = BTreeMap::<usize, Vec<&HydroUnitRow>>::new();
            for unit_row in &plant_unit_rows {
                groups_by_id
                    .entry(unit_row.conjunto)
                    .or_default()
                    .push(unit_row);
            }

            let mut groups = Vec::with_capacity(groups_by_id.len());
            for (group_id, group_rows) in groups_by_id {
                let unit_count = group_rows
                    .iter()
                    .map(|unit_row| unit_row.unidades)
                    .sum::<usize>();
                let mut units = Vec::with_capacity(unit_count);
                let mut next_unit_id = 1usize;
                for unit_row in group_rows {
                    let plant_name = row.nome.trim();
                    let unit_quantity = unit_row.unidades;
                    if unit_quantity == 0 {
                        return Err(IoError::invalid_data(format!(
                            "hydro plant {} group {} must have at least one unit",
                            plant_name, group_id
                        )));
                    }

                    for _ in 0..unit_quantity {
                        let unit_id = next_unit_id;
                        next_unit_id += 1;
                        let unit_name = format!("{}-{}-{}", plant_name, group_id, unit_id);
                        let max_generation_mw =
                            normalize_max_bound(unit_row.pmin, unit_row.pmax, "Pmax", &unit_name)?;
                        let startup = if hydro_unit_commitment_enabled {
                            trajectory_for(startup_trajectories, plant_name)?
                        } else {
                            Vec::new()
                        };
                        let shutdown = if hydro_unit_commitment_enabled {
                            trajectory_for(shutdown_trajectories, plant_name)?
                        } else {
                            Vec::new()
                        };
                        let is_on = unit_row.status_inic != 0;
                        units.push(HydroUnit {
                            id: HydroUnitId(unit_id),
                            name: unit_name,
                            min_generation_mw: unit_row.pmin,
                            max_generation_mw,
                            max_turbining_hm3: flow_m3s_to_hm3(
                                unit_row.max_turb,
                                period_duration_hours,
                            ),
                            startup_trajectory_mw: startup,
                            shutdown_trajectory_mw: shutdown,
                            min_up_time: hours_to_periods(
                                unit_row.ton,
                                period_duration_hours,
                                "Ton",
                                plant_name,
                            )?,
                            min_down_time: hours_to_periods(
                                unit_row.toff,
                                period_duration_hours,
                                "Toff",
                                plant_name,
                            )?,
                            startup_cost: unit_row.custo_partida,
                            shutdown_cost: unit_row.custo_desliga,
                            initial_condition: HydroInitialCondition {
                                is_on,
                                generation_mw: if is_on { unit_row.pmin } else { 0.0 },
                                time_in_state: hours_to_periods(
                                    unit_row.tinic,
                                    period_duration_hours,
                                    "Tinic",
                                    plant_name,
                                )?,
                            },
                        });
                    }
                }

                groups.push(HydroGroup {
                    id: HydroGroupId(group_id),
                    name: format!("{}-{}", row.nome.trim(), group_id),
                    units,
                });
            }

            let downstream_plant_id = downstream_by_code.get(&row.codigo).copied().flatten();
            let diversion_plant_id = diversion_by_code.get(&row.codigo).copied().flatten();
            let upstream_plant_ids = upstreams_by_code
                .get(&row.codigo)
                .cloned()
                .unwrap_or_default();
            let diversion_upstream_plant_ids = diversion_upstreams_by_code
                .get(&row.codigo)
                .cloned()
                .unwrap_or_default();
            let natural_inflow_hm3 = inflows_by_plant
                .get(&row.codigo)
                .cloned()
                .unwrap_or_else(|| vec![0.0; horizon.periods]);
            let water_withdrawal_hm3 = withdrawals_by_plant
                .get(&row.codigo)
                .cloned()
                .unwrap_or_else(|| vec![0.0; horizon.periods]);
            let fpha_segments = if plant_unit_rows.is_empty() {
                fpha_by_plant.get(&row.codigo).cloned().unwrap_or_default()
            } else {
                fpha_by_plant.get(&row.codigo).cloned().ok_or_else(|| {
                    IoError::invalid_data(format!(
                        "missing FPHA data for hydro plant {}",
                        row.codigo
                    ))
                })?
            };

            let submarket_id = submarket_id_by_name(submarkets, &row.submercado)?;
            Ok(HydroPlant {
                id: HydroPlantId(row.codigo),
                name: row.nome.trim().to_string(),
                submarket_id,
                bus_id: BusId(bus_id_or_dummy(
                    plant_unit_rows.first().and_then(|unit| unit.barra),
                    network_enabled,
                    row.nome.trim(),
                    submarket_id,
                )?),
                upstream_plant_ids,
                downstream_plant_id,
                diversion_upstream_plant_ids,
                diversion_plant_id,
                reservoir: Reservoir {
                    min_volume_hm3: row.vmin,
                    max_volume_hm3: row.vmax,
                    initial_volume_hm3: row.vol_inic,
                },
                natural_inflow_hm3,
                water_withdrawal_hm3,
                spillage_cost_per_hm3: row.penal_vert,
                fpha_segments,
                groups,
            })
        })
        .collect()
}

fn build_pumping_plants(
    rows: &[PumpingPlantRow],
    hydro_plants: &[HydroPlant],
    submarkets: &[Submarket],
    period_duration_hours: f64,
    network_enabled: bool,
) -> Result<Vec<PumpingPlant>, IoError> {
    rows.iter()
        .map(|row| {
            let name = row.nome.trim().to_string();
            let submarket_id = submarket_id_by_name(submarkets, &row.submercado)?;
            let downstream_hydro_id = hydro_plants
                .iter()
                .find(|plant| plant.id.0 == row.jusante)
                .map(|plant| plant.id)
                .ok_or_else(|| {
                    IoError::invalid_data(format!(
                        "pumping plant {} references unknown downstream hydro plant {}",
                        name, row.jusante
                    ))
                })?;
            let upstream_hydro_id = hydro_plants
                .iter()
                .find(|plant| plant.id.0 == row.montante)
                .map(|plant| plant.id)
                .ok_or_else(|| {
                    IoError::invalid_data(format!(
                        "pumping plant {} references unknown upstream hydro plant {}",
                        name, row.montante
                    ))
                })?;

            Ok(PumpingPlant {
                id: PumpingPlantId(row.codigo),
                name: name.clone(),
                submarket_id,
                bus_id: BusId(bus_id_or_dummy(
                    row.barra,
                    network_enabled,
                    name.as_str(),
                    submarket_id,
                )?),
                downstream_hydro_id,
                upstream_hydro_id,
                min_pumping_hm3: flow_m3s_to_hm3(row.qbomb_min, period_duration_hours),
                max_pumping_hm3: flow_m3s_to_hm3(row.qbomb_max, period_duration_hours),
                specific_consumption_mw_per_m3s: row.consumo_especifico,
            })
        })
        .collect()
}

fn build_renewables(
    catalog_rows: &[RenewableCatalogRow],
    operation_rows: &[RenewableOperationRow],
    submarkets: &[Submarket],
    horizon: &StudyHorizon,
    network_enabled: bool,
) -> Result<(Vec<WindPlant>, Vec<SolarPlant>), IoError> {
    let mut generation_by_plant = HashMap::<usize, Vec<f64>>::new();

    for row in operation_rows {
        let series = generation_by_plant
            .entry(row.codigo)
            .or_insert_with(|| vec![0.0; horizon.periods]);
        for period_idx in internal_periods_for_input_period(row.periodo, horizon, "OPER_REN.csv")? {
            series[period_idx] = row.ger_prog;
        }
    }

    let wind_plants = catalog_rows
        .iter()
        .map(|row| {
            let name = row.nome.trim().to_string();
            let submarket_id = submarket_id_by_name(submarkets, &row.submercado)?;
            Ok(WindPlant {
                id: WindPlantId(row.codigo),
                name: name.clone(),
                submarket_id,
                bus_id: BusId(bus_id_or_dummy(
                    row.barra,
                    network_enabled,
                    name.as_str(),
                    submarket_id,
                )?),
                available_generation_mw: generation_by_plant
                    .get(&row.codigo)
                    .cloned()
                    .unwrap_or_else(|| vec![0.0; horizon.periods]),
            })
        })
        .collect::<Result<Vec<_>, IoError>>()?;

    Ok((wind_plants, Vec::new()))
}

fn build_residual_costs(rows: &[ResidualCostRow]) -> Vec<ResidualCost> {
    rows.iter()
        .map(|row| ResidualCost {
            submarket_id: SubmarketId(row.submercado),
            cmo_per_mwh: row.cmo,
        })
        .collect()
}

fn build_operational_limits(
    rows: &[OperationalLimitRow],
    thermal_plants: &[ThermalPlant],
    hydro_plants: &[HydroPlant],
    horizon: &StudyHorizon,
) -> Result<Vec<OperationalLimit>, IoError> {
    rows.iter()
        .map(|row| {
            let start_original =
                parse_restriction_period(&row.periodo_inicial, horizon.original_periods, true)?;
            let end_original =
                parse_restriction_period(&row.periodo_final, horizon.original_periods, false)?;
            if start_original > end_original {
                return Err(IoError::invalid_data(format!(
                    "operational limit for {} has initial period after final period",
                    row.nome_usina
                )));
            }
            let start_period = horizon
                .internal_periods_for_original(start_original)
                .first()
                .copied()
                .map(|period| period + 1)
                .ok_or_else(|| {
                    IoError::invalid_data(format!(
                        "period {start_original} has no internal representation"
                    ))
                })?;
            let end_period = horizon
                .internal_periods_for_original(end_original)
                .last()
                .copied()
                .map(|period| period + 1)
                .ok_or_else(|| {
                    IoError::invalid_data(format!(
                        "period {end_original} has no internal representation"
                    ))
                })?;
            let variable = parse_operational_limit_variable(&row.variavel)?;
            let lower_bound = parse_optional_bound(&row.linf, "Linf")?;
            let upper_bound = parse_optional_bound(&row.lsup, "Lsup")?;

            let thermal_match = thermal_plants
                .iter()
                .find(|plant| plant.id.0 == row.codigo_usina && plant.name == row.nome_usina);
            let hydro_match = hydro_plants
                .iter()
                .find(|plant| plant.id.0 == row.codigo_usina && plant.name == row.nome_usina);

            let target = match variable {
                OperationalLimitVariable::Generation => {
                    match (thermal_match, hydro_match) {
                        (Some(plant), None) => {
                            OperationalLimitTarget::ThermalPlant(plant.id)
                        }
                        (None, Some(plant)) => {
                            OperationalLimitTarget::HydroPlant(plant.id)
                        }
                        (Some(_), Some(_)) => {
                            return Err(IoError::invalid_data(format!(
                                "operational limit for {} is ambiguous between thermal and hydro generation",
                                row.nome_usina
                            )));
                        }
                        (None, None) => {
                            return Err(IoError::invalid_data(format!(
                                "unknown plant {} ({}) in OPER_REST_LIM.csv",
                                row.nome_usina, row.codigo_usina
                            )));
                        }
                    }
                }
                _ => {
                    if let Some(thermal) = thermal_match {
                        return Err(IoError::invalid_data(format!(
                            "thermal plant {} cannot define restriction for variable {}",
                            thermal.name, row.variavel
                        )));
                    }

                    let hydro = hydro_match.ok_or_else(|| {
                        IoError::invalid_data(format!(
                            "unknown hydro plant {} ({}) in OPER_REST_LIM.csv",
                            row.nome_usina, row.codigo_usina
                        ))
                    })?;
                    OperationalLimitTarget::HydroPlant(hydro.id)
                }
            };

            Ok(OperationalLimit {
                target,
                plant_name: row.nome_usina.clone(),
                variable,
                start_period,
                end_period,
                lower_bound,
                upper_bound,
            })
        })
        .collect()
}

fn parse_restriction_period(value: &str, horizon: usize, is_start: bool) -> Result<usize, IoError> {
    let trimmed = value.trim();
    if trimmed.eq_ignore_ascii_case("I") {
        return Ok(1);
    }
    if trimmed.eq_ignore_ascii_case("F") {
        return Ok(horizon);
    }

    let period = trimmed.parse::<usize>().map_err(|_| {
        IoError::invalid_data(format!("invalid restriction period value '{trimmed}'"))
    })?;

    if !(1..=horizon).contains(&period) {
        return Err(IoError::invalid_data(format!(
            "restriction {} period {} is outside study horizon 1..={}",
            if is_start { "initial" } else { "final" },
            period,
            horizon
        )));
    }

    Ok(period)
}

fn parse_optional_bound(value: &str, field_name: &str) -> Result<Option<f64>, IoError> {
    let trimmed = value.trim();
    if trimmed.is_empty() {
        return Ok(None);
    }

    let parsed = trimmed.parse::<f64>().map_err(|_| {
        IoError::invalid_data(format!(
            "invalid {field_name} value '{trimmed}' in OPER_REST_LIM.csv"
        ))
    })?;
    Ok(Some(parsed))
}

fn parse_operational_limit_variable(value: &str) -> Result<OperationalLimitVariable, IoError> {
    match value.trim().to_ascii_uppercase().as_str() {
        "GER" => Ok(OperationalLimitVariable::Generation),
        "VERT" => Ok(OperationalLimitVariable::Spillage),
        "VOL" => Ok(OperationalLimitVariable::Volume),
        "DEFLU" => Ok(OperationalLimitVariable::Defluence),
        "TURB" => Ok(OperationalLimitVariable::Turbining),
        other => Err(IoError::invalid_data(format!(
            "unknown operational limit variable '{other}' in OPER_REST_LIM.csv"
        ))),
    }
}

fn hours_to_periods(
    hours: f64,
    period_duration_hours: f64,
    field_name: &str,
    asset_name: &str,
) -> Result<usize, IoError> {
    if hours < 0.0 {
        return Err(IoError::invalid_data(format!(
            "{field_name} for {asset_name} cannot be negative"
        )));
    }

    let periods = (hours / period_duration_hours).ceil();
    if !periods.is_finite() {
        return Err(IoError::invalid_data(format!(
            "failed to convert {field_name} for {asset_name} from hours to periods"
        )));
    }

    Ok(periods as usize)
}

fn flow_m3s_to_hm3(flow_m3s: f64, period_duration_hours: f64) -> f64 {
    flow_m3s * period_duration_hours * 0.0036
}

fn submarket_id_by_name(submarkets: &[Submarket], value: &str) -> Result<SubmarketId, IoError> {
    let submarket_name = value.trim();
    submarkets
        .iter()
        .find(|submarket| submarket.name == submarket_name)
        .map(|submarket| submarket.id)
        .ok_or_else(|| IoError::invalid_data(format!("unknown submarket '{submarket_name}'")))
}

fn normalize_max_bound(
    min_value: f64,
    max_value: f64,
    field_name: &str,
    asset_name: &str,
) -> Result<f64, IoError> {
    if min_value <= max_value {
        return Ok(max_value);
    }

    if (min_value - max_value).abs() <= 1e-2 {
        return Ok(min_value);
    }

    Err(IoError::invalid_data(format!(
        "{field_name} for {asset_name} is below the minimum value"
    )))
}

fn optional_hydro_reference_by_code(
    value: &str,
    hydro_code_to_id: &HashMap<usize, HydroPlantId>,
) -> Result<Option<HydroPlantId>, IoError> {
    let trimmed = value.trim();
    if trimmed.is_empty() || trimmed == "-" {
        return Ok(None);
    }

    let code = trimmed
        .parse::<usize>()
        .map_err(|_| IoError::invalid_data(format!("invalid hydro reference code '{trimmed}'")))?;

    hydro_code_to_id
        .get(&code)
        .copied()
        .map(Some)
        .ok_or_else(|| IoError::invalid_data(format!("unknown hydro reference code '{code}'")))
}

fn trajectory_for(
    trajectories: &HashMap<String, Vec<f64>>,
    asset_name: &str,
) -> Result<Vec<f64>, IoError> {
    trajectories
        .get(asset_name)
        .cloned()
        .ok_or_else(|| IoError::invalid_data(format!("missing trajectory for '{asset_name}'")))
}

fn bus_id_or_dummy(
    bus_id: Option<usize>,
    network_enabled: bool,
    asset_name: &str,
    dummy_submarket_id: SubmarketId,
) -> Result<usize, IoError> {
    if network_enabled {
        bus_id.ok_or_else(|| {
            IoError::invalid_data(format!(
                "missing Barra for {asset_name}; Barra is required when rede = 1"
            ))
        })
    } else {
        Ok(dummy_submarket_id.0)
    }
}

fn read_csv<T: for<'de> Deserialize<'de>>(path: PathBuf) -> Result<Vec<T>, IoError> {
    log_read_file(&path);
    let mut reader = ReaderBuilder::new()
        .delimiter(b';')
        .has_headers(true)
        .from_path(path)?;

    let mut rows = Vec::new();
    for record in reader.deserialize() {
        rows.push(record?);
    }

    Ok(rows)
}

fn read_fpha_table(path: PathBuf) -> Result<Vec<HydroFphaRow>, IoError> {
    log_read_file(&path);
    let mut reader = ReaderBuilder::new()
        .delimiter(b';')
        .has_headers(true)
        .from_path(path)?;

    let headers = reader
        .headers()
        .map_err(IoError::from)?
        .iter()
        .map(|header| header.trim().to_ascii_uppercase())
        .collect::<Vec<_>>();

    let column = |name: &str| -> Result<usize, IoError> {
        headers
            .iter()
            .position(|header| header == name)
            .ok_or_else(|| {
                IoError::invalid_data(format!("missing column '{name}' in OPER_FPHA.csv"))
            })
    };

    let codigo_idx = column("USIH")?;
    let segmento_idx = column("SEGFPHA")?;
    let fator_idx = column("FCORREC")?;
    let rhs_idx = column("RHS")?;
    let volume_idx = column("VARM")?;
    let turbining_idx = column("QTUR")?;
    let lateral_idx = column("QLAT")?;

    let mut rows = Vec::new();
    for record in reader.records() {
        let record = record?;
        let first_field = record.get(codigo_idx).unwrap_or_default().trim();
        if first_field.is_empty() || first_field.starts_with('-') {
            continue;
        }

        rows.push(HydroFphaRow {
            codigo: parse_record_usize(&record, codigo_idx, "USIH", "OPER_FPHA.csv")?,
            segmento: parse_record_usize(&record, segmento_idx, "SegFPHA", "OPER_FPHA.csv")?,
            fator_correcao: parse_record_f64(&record, fator_idx, "Fcorrec", "OPER_FPHA.csv")?,
            rhs: parse_record_f64(&record, rhs_idx, "Rhs", "OPER_FPHA.csv")?,
            volume_coefficient: parse_record_f64(&record, volume_idx, "Varm", "OPER_FPHA.csv")?,
            turbining_coefficient: parse_record_f64(
                &record,
                turbining_idx,
                "Qtur",
                "OPER_FPHA.csv",
            )?,
            lateral_flow_coefficient: parse_record_f64(
                &record,
                lateral_idx,
                "Qlat",
                "OPER_FPHA.csv",
            )?,
        });
    }

    Ok(rows)
}

fn read_renewable_catalog_table(path: PathBuf) -> Result<Vec<RenewableCatalogRow>, IoError> {
    log_read_file(&path);
    let mut reader = ReaderBuilder::new()
        .delimiter(b';')
        .has_headers(true)
        .from_path(path)?;

    let headers = reader
        .headers()
        .map_err(IoError::from)?
        .iter()
        .map(|header| header.trim().to_ascii_uppercase())
        .collect::<Vec<_>>();

    let column = |name: &str| -> Result<usize, IoError> {
        headers
            .iter()
            .position(|header| header == name)
            .ok_or_else(|| IoError::invalid_data(format!("missing column '{name}' in CAD_REN.csv")))
    };

    let codigo_idx = column("CODIGO")?;
    let nome_idx = column("NOME")?;
    let barra_idx = column("BARRA")?;
    let submercado_idx = column("SUBMERCADO")?;

    let mut rows = Vec::new();
    for record in reader.records() {
        let record = record?;
        let first_field = record.get(codigo_idx).unwrap_or_default().trim();
        if first_field.is_empty() || first_field.starts_with('-') {
            continue;
        }

        let barra_text = record.get(barra_idx).unwrap_or_default().trim();
        rows.push(RenewableCatalogRow {
            codigo: parse_record_usize(&record, codigo_idx, "Codigo", "CAD_REN.csv")?,
            nome: record.get(nome_idx).unwrap_or_default().trim().to_string(),
            barra: if barra_text.is_empty() || barra_text == "-" {
                None
            } else {
                Some(parse_record_usize(
                    &record,
                    barra_idx,
                    "Barra",
                    "CAD_REN.csv",
                )?)
            },
            submercado: record
                .get(submercado_idx)
                .unwrap_or_default()
                .trim()
                .to_string(),
        });
    }

    Ok(rows)
}

fn read_renewable_operation_table(path: PathBuf) -> Result<Vec<RenewableOperationRow>, IoError> {
    log_read_file(&path);
    let mut reader = ReaderBuilder::new()
        .delimiter(b';')
        .has_headers(true)
        .from_path(path)?;

    let headers = reader
        .headers()
        .map_err(IoError::from)?
        .iter()
        .map(|header| header.trim().to_ascii_uppercase())
        .collect::<Vec<_>>();

    let column = |name: &str| -> Result<usize, IoError> {
        headers
            .iter()
            .position(|header| header == name)
            .ok_or_else(|| {
                IoError::invalid_data(format!("missing column '{name}' in OPER_REN.csv"))
            })
    };

    let periodo_idx = column("PERIODO")?;
    let codigo_idx = column("CODIGO")?;
    let nome_idx = column("NOME")?;
    let ger_prog_idx = column("GERPROG")?;

    let mut rows = Vec::new();
    for record in reader.records() {
        let record = record?;
        let first_field = record.get(periodo_idx).unwrap_or_default().trim();
        if first_field.is_empty() || first_field.starts_with('-') {
            continue;
        }

        rows.push(RenewableOperationRow {
            periodo: parse_record_usize(&record, periodo_idx, "Periodo", "OPER_REN.csv")?,
            codigo: parse_record_usize(&record, codigo_idx, "Codigo", "OPER_REN.csv")?,
            _nome: record.get(nome_idx).unwrap_or_default().trim().to_string(),
            ger_prog: parse_record_f64(&record, ger_prog_idx, "GerProg", "OPER_REN.csv")?,
        });
    }

    Ok(rows)
}

fn parse_record_usize(
    record: &StringRecord,
    index: usize,
    field_name: &str,
    file_name: &str,
) -> Result<usize, IoError> {
    record
        .get(index)
        .unwrap_or_default()
        .trim()
        .parse::<usize>()
        .map_err(|_| IoError::invalid_data(format!("invalid {field_name} in {file_name}")))
}

fn parse_record_f64(
    record: &StringRecord,
    index: usize,
    field_name: &str,
    file_name: &str,
) -> Result<f64, IoError> {
    record
        .get(index)
        .unwrap_or_default()
        .trim()
        .parse::<f64>()
        .map_err(|_| IoError::invalid_data(format!("invalid {field_name} in {file_name}")))
}

fn parse_optional_record_usize(
    record: &StringRecord,
    index: usize,
    field_name: &str,
    file_name: &str,
) -> Result<Option<usize>, IoError> {
    let value = record.get(index).unwrap_or_default().trim();
    if value.is_empty() || value == "-" {
        return Ok(None);
    }

    value
        .parse::<usize>()
        .map(Some)
        .map_err(|_| IoError::invalid_data(format!("invalid {field_name} in {file_name}")))
}

fn read_pumping_table(path: PathBuf) -> Result<Vec<PumpingPlantRow>, IoError> {
    log_read_file(&path);
    let mut reader = ReaderBuilder::new()
        .delimiter(b';')
        .has_headers(true)
        .from_path(path)?;

    let headers = reader
        .headers()
        .map_err(IoError::from)?
        .iter()
        .map(normalize_column_name)
        .collect::<Vec<_>>();

    let column = |names: &[&str]| -> Result<usize, IoError> {
        headers
            .iter()
            .position(|header| names.iter().any(|name| header == name))
            .ok_or_else(|| {
                IoError::invalid_data(format!(
                    "missing column '{}' in CAD_USIE.csv",
                    names.join("' or '")
                ))
            })
    };

    let codigo_idx = column(&["CODIGO", "USIE"])?;
    let nome_idx = column(&["NOME", "NOMEUSIE"])?;
    let submarket_idx = column(&["SUBMERCADO", "NOMESIST"])?;
    let downstream_idx = column(&["JUSANTE", "USIHJUS"])?;
    let upstream_idx = column(&["MONTANTE", "MONTATE", "USIHMON"])?;
    let min_idx = column(&["QBOMBMIN"])?;
    let max_idx = column(&["QBOMBMAX"])?;
    let consumption_idx = column(&["CONSUMO", "CONSESPEC"])?;
    let bus_idx = headers.iter().position(|header| header == "BARRA");

    let mut rows = Vec::new();
    for record in reader.records() {
        let record = record?;
        let first_field = record.get(codigo_idx).unwrap_or_default().trim();
        if first_field.is_empty() || first_field.starts_with('-') {
            continue;
        }

        rows.push(PumpingPlantRow {
            codigo: parse_record_usize(&record, codigo_idx, "Codigo", "CAD_USIE.csv")?,
            nome: record.get(nome_idx).unwrap_or_default().trim().to_string(),
            submercado: record
                .get(submarket_idx)
                .unwrap_or_default()
                .trim()
                .to_string(),
            jusante: parse_record_usize(&record, downstream_idx, "Jusante", "CAD_USIE.csv")?,
            montante: parse_record_usize(&record, upstream_idx, "Montante", "CAD_USIE.csv")?,
            qbomb_min: parse_record_f64(&record, min_idx, "QbombMin", "CAD_USIE.csv")?,
            qbomb_max: parse_record_f64(&record, max_idx, "QbombMax", "CAD_USIE.csv")?,
            consumo_especifico: parse_record_f64(
                &record,
                consumption_idx,
                "Consumo",
                "CAD_USIE.csv",
            )?,
            barra: match bus_idx {
                Some(idx) => parse_optional_record_usize(&record, idx, "Barra", "CAD_USIE.csv")?,
                None => None,
            },
        });
    }

    Ok(rows)
}

fn normalize_column_name(value: &str) -> String {
    value
        .chars()
        .filter(|character| !character.is_whitespace())
        .collect::<String>()
        .to_ascii_uppercase()
}

fn read_thermal_trajectory_table(
    path: PathBuf,
) -> Result<(HashMap<String, Vec<f64>>, HashMap<String, Vec<f64>>), IoError> {
    let rows: Vec<ThermalRampRow> = read_csv(path)?;
    let mut startup_steps = BTreeMap::<String, Vec<(usize, f64)>>::new();
    let mut shutdown_steps = BTreeMap::<String, Vec<(usize, f64)>>::new();

    for row in rows {
        let key = format!("{}-{}", row.nome_ute.trim(), row.unidade);
        let target = match row.traj.trim().to_ascii_uppercase().as_str() {
            "A" => &mut startup_steps,
            "D" => &mut shutdown_steps,
            other => {
                return Err(IoError::invalid_data(format!(
                    "unknown thermal ramp trajectory '{other}' for {key}; expected A or D"
                )));
            }
        };
        target
            .entry(key)
            .or_default()
            .push((row.indice_passo, row.passo));
    }

    Ok((
        build_ordered_trajectory_map(startup_steps),
        build_ordered_trajectory_map(shutdown_steps),
    ))
}

fn build_ordered_trajectory_map(
    steps_by_unit: BTreeMap<String, Vec<(usize, f64)>>,
) -> HashMap<String, Vec<f64>> {
    steps_by_unit
        .into_iter()
        .map(|(unit, mut steps)| {
            steps.sort_by_key(|(step_idx, _)| *step_idx);
            (unit, steps.into_iter().map(|(_, value)| value).collect())
        })
        .collect()
}

fn read_trajectory_table(path: PathBuf) -> Result<HashMap<String, Vec<f64>>, IoError> {
    log_read_file(&path);
    let file = File::open(&path).map_err(|error| {
        IoError::invalid_data(format!("failed to open {}: {error}", path.display()))
    })?;
    let mut reader = ReaderBuilder::new()
        .delimiter(b';')
        .has_headers(true)
        .flexible(true)
        .from_reader(file);

    let headers = reader
        .headers()
        .map_err(IoError::from)?
        .iter()
        .map(|value| value.trim().to_string())
        .collect::<Vec<_>>();

    let mut trajectories: HashMap<String, Vec<f64>> = headers
        .iter()
        .filter(|header| !header.is_empty())
        .map(|header| (header.clone(), Vec::new()))
        .collect();

    for row in reader.records() {
        let row = row?;
        append_trajectory_row(&headers, &row, &mut trajectories);
    }

    Ok(trajectories)
}

fn append_trajectory_row(
    headers: &[String],
    row: &StringRecord,
    trajectories: &mut HashMap<String, Vec<f64>>,
) {
    for (column_idx, header) in headers.iter().enumerate() {
        if header.is_empty() {
            continue;
        }
        let value = row.get(column_idx).unwrap_or("").trim();
        if value.is_empty() {
            continue;
        }
        if let Ok(parsed) = value.parse::<f64>() {
            trajectories.entry(header.clone()).or_default().push(parsed);
        }
    }
}

#[derive(Debug, Deserialize)]
struct DurationRow {
    #[serde(rename = "Periodo")]
    periodo: usize,
    #[serde(rename = "Duracao")]
    duracao: f64,
}

#[derive(Debug, Deserialize)]
struct SubmarketCatalogRow {
    #[serde(rename = "Codigo")]
    codigo: usize,
    #[serde(rename = "Nome")]
    nome: String,
}

#[derive(Debug, Deserialize)]
struct SubmarketOperationRow {
    #[serde(rename = "CodigoSubmercado")]
    codigo_submercado: usize,
    #[serde(rename = "Periodo")]
    periodo: usize,
    #[serde(rename = "Demanda")]
    demanda: f64,
    #[serde(rename = "CustoDeficit")]
    custo_deficit: f64,
}

#[derive(Debug, Deserialize)]
struct BusOperationRow {
    #[serde(rename = "CodigoBarra")]
    codigo_barra: usize,
    #[serde(rename = "NomeBarra")]
    nome_barra: String,
    #[serde(rename = "Periodo")]
    periodo: usize,
    #[serde(rename = "Carga")]
    carga: f64,
    #[serde(rename = "CodigoSubmercado")]
    codigo_submercado: usize,
    #[serde(rename = "Swing")]
    #[serde(deserialize_with = "deserialize_bool_flag")]
    swing: bool,
}

#[derive(Debug, Deserialize)]
struct BranchRow {
    #[serde(rename = "De")]
    de: usize,
    #[serde(rename = "Para")]
    para: usize,
    #[serde(rename = "Capacidade")]
    capacidade: f64,
    #[serde(rename = "Reatancia")]
    reatancia: f64,
    #[serde(rename = "CodigoLinha")]
    codigo_linha: usize,
}

#[derive(Debug, Deserialize)]
struct InterchangeRow {
    #[serde(rename = "SbmDe")]
    sbm_de: usize,
    #[serde(rename = "SbmPara")]
    sbm_para: usize,
    #[serde(rename = "Limite")]
    limite: f64,
    #[serde(rename = "Penalidade")]
    penalidade: f64,
}

#[derive(Debug, Deserialize)]
struct ResidualCostRow {
    #[serde(rename = "Submercado")]
    submercado: usize,
    #[serde(rename = "CMO")]
    cmo: f64,
}

#[derive(Debug, Deserialize)]
struct ThermalUnitRow {
    #[serde(rename = "CodigoUTE")]
    codigo_ute: usize,
    #[serde(rename = "NomeUTE")]
    nome_ute: String,
    #[serde(rename = "Unidade")]
    unidade: usize,
    #[serde(rename = "Pmin")]
    pmin: f64,
    #[serde(rename = "Pmax")]
    pmax: f64,
    #[serde(rename = "Ton")]
    ton: f64,
    #[serde(rename = "Toff")]
    toff: f64,
    #[serde(rename = "GerInic")]
    ger_inic: f64,
    #[serde(rename = "StatusInic")]
    status_inic: usize,
    #[serde(rename = "Tinic")]
    tinic: f64,
    #[serde(rename = "RupInic")]
    rup_inic: usize,
    #[serde(rename = "RdownInic")]
    rdown_inic: usize,
    #[serde(rename = "CVU")]
    cvu: f64,
    #[serde(rename = "CustoPartida")]
    custo_partida: f64,
    #[serde(rename = "CustoDesliga")]
    custo_desliga: f64,
    #[serde(rename = "Barra")]
    barra: Option<usize>,
    #[serde(rename = "Submercado")]
    submercado: String,
}

#[derive(Debug, Deserialize)]
struct ThermalRampRow {
    #[serde(rename = "CodigoUTE")]
    _codigo_ute: usize,
    #[serde(rename = "NomeUTE")]
    nome_ute: String,
    #[serde(rename = "Unidade")]
    unidade: usize,
    #[serde(rename = "Traj")]
    traj: String,
    #[serde(rename = "Passo")]
    passo: f64,
    #[serde(rename = " Ipasso")]
    indice_passo: usize,
}

#[derive(Debug, Deserialize)]
struct HydroPlantRow {
    #[serde(rename = "Codigo")]
    codigo: usize,
    #[serde(rename = "Nome")]
    nome: String,
    #[serde(rename = "VolInic")]
    vol_inic: f64,
    #[serde(rename = "Vmin")]
    vmin: f64,
    #[serde(rename = "Vmax")]
    vmax: f64,
    #[serde(rename = "Tipo")]
    _tipo: String,
    #[serde(rename = "Jusante")]
    jusante: String,
    #[serde(rename = "Desvio")]
    desvio: String,
    #[serde(rename = "Submercado")]
    submercado: String,
    #[serde(rename = "PenalVert")]
    penal_vert: f64,
}

#[derive(Debug)]
struct HydroFphaRow {
    codigo: usize,
    segmento: usize,
    fator_correcao: f64,
    rhs: f64,
    volume_coefficient: f64,
    turbining_coefficient: f64,
    lateral_flow_coefficient: f64,
}

#[derive(Debug, Deserialize)]
struct HydroUnitRow {
    #[serde(rename = "Codigo")]
    codigo: usize,
    #[serde(rename = "Conjunto")]
    conjunto: usize,
    #[serde(rename = "Unidades")]
    unidades: usize,
    #[serde(rename = "Pmin")]
    pmin: f64,
    #[serde(rename = "Pmax")]
    pmax: f64,
    #[serde(rename = "Ton")]
    ton: f64,
    #[serde(rename = "Toff")]
    toff: f64,
    #[serde(rename = "StatusInic")]
    status_inic: usize,
    #[serde(rename = "Tinic")]
    tinic: f64,
    #[serde(rename = "Barra")]
    barra: Option<usize>,
    #[serde(rename = "MaxTurb")]
    max_turb: f64,
    #[serde(rename = "CustoPartida")]
    custo_partida: f64,
    #[serde(rename = "CustoDesliga")]
    custo_desliga: f64,
}

#[derive(Debug, Deserialize)]
struct HydroInflowRow {
    #[serde(rename = "Codigo")]
    codigo: usize,
    #[serde(rename = "Periodo")]
    periodo: usize,
    #[serde(rename = "Nome")]
    _nome: String,
    #[serde(rename = "Afluencia")]
    afluencia: f64,
    #[serde(rename = "Retirada")]
    retirada: f64,
}

#[derive(Debug)]
struct PumpingPlantRow {
    codigo: usize,
    nome: String,
    submercado: String,
    jusante: usize,
    montante: usize,
    qbomb_min: f64,
    qbomb_max: f64,
    consumo_especifico: f64,
    barra: Option<usize>,
}

#[derive(Debug, Deserialize)]
struct RenewableCatalogRow {
    #[serde(rename = "Codigo")]
    codigo: usize,
    #[serde(rename = "Nome")]
    nome: String,
    #[serde(rename = "Barra")]
    barra: Option<usize>,
    #[serde(rename = "Submercado")]
    submercado: String,
}

#[derive(Debug, Deserialize)]
struct RenewableOperationRow {
    #[serde(rename = "Periodo")]
    periodo: usize,
    #[serde(rename = "Codigo")]
    codigo: usize,
    #[serde(rename = "Nome")]
    _nome: String,
    #[serde(rename = "GerProg")]
    ger_prog: f64,
}

fn log_read_file(path: &Path) {
    println!("Lendo arquivo: {}", file_label(path));
    io::stdout().flush().ok();
}

fn file_label(path: &Path) -> String {
    path.file_name()
        .map(|name| name.to_string_lossy().into_owned())
        .unwrap_or_else(|| path.display().to_string())
}

#[derive(Debug, Deserialize)]
struct OperationalLimitRow {
    #[serde(rename = "PeriodoInicial")]
    periodo_inicial: String,
    #[serde(rename = "PeriodoFinal")]
    periodo_final: String,
    #[serde(rename = "CodigoUsina")]
    codigo_usina: usize,
    #[serde(rename = "NomeUsina")]
    nome_usina: String,
    #[serde(rename = "Variavel")]
    variavel: String,
    #[serde(rename = "Linf")]
    linf: String,
    #[serde(rename = "Lsup")]
    lsup: String,
}

#[derive(Debug)]
struct BusAccumulator {
    name: String,
    submarket_id: usize,
    angle_reference: bool,
    demand_mw: Vec<f64>,
}

fn deserialize_bool_flag<'de, D>(deserializer: D) -> Result<bool, D::Error>
where
    D: Deserializer<'de>,
{
    let value = String::deserialize(deserializer)?;
    match value.trim().to_ascii_lowercase().as_str() {
        "true" | "1" => Ok(true),
        "false" | "0" => Ok(false),
        other => Err(serde::de::Error::custom(format!(
            "invalid boolean flag '{other}'"
        ))),
    }
}

#[cfg(test)]
mod tests {
    use super::{read_study_config, read_study_from_config, read_thermal_trajectory_table};
    use std::path::PathBuf;

    #[test]
    fn reads_current_case() {
        let config_path = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("study_config.json");

        let system =
            read_study_from_config(config_path).expect("caso_sin_sem_rede should be readable");
        assert_eq!(system.submarkets.len(), 4);
        assert_eq!(system.buses.len(), 4);
        assert!(system.branches.is_empty());
        assert_eq!(system.thermal_plants.len(), 96);
        assert_eq!(system.hydro_plants.len(), 168);
        assert!(!system.hydro_unit_commitment_enabled);
    }

    #[test]
    fn reads_study_from_json_config() {
        let config_path = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("study_config.json");

        let config = read_study_config(&config_path).expect("study config should be readable");
        assert!(config.case_path.is_absolute());
        assert!(config.case_path.exists());
        assert_eq!(config.rede, 0);
        assert!(config.uct <= 1);
        assert!(config.uch <= 1);
        assert!(config.ton_residual <= 1);

        let system =
            read_study_from_config(config_path).expect("study config should build a valid system");
        assert_eq!(system.submarkets.len(), 4);
        assert_eq!(system.buses.len(), 4);
    }

    #[test]
    fn reads_single_thermal_ramp_file_by_trajectory_type() {
        let path = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .join("../../examples/caso_sin_sem_rede/CAD/CAD_RAMPAS_TERMICAS.csv");

        let (startup, shutdown) =
            read_thermal_trajectory_table(path).expect("thermal ramp file should be readable");

        assert_eq!(
            startup
                .get("ANGRA 1-1")
                .expect("ANGRA 1 startup should exist"),
            &vec![122.0, 212.0, 302.0, 392.0, 460.0]
        );
        assert_eq!(
            shutdown
                .get("ANGRA 1-1")
                .expect("ANGRA 1 shutdown should exist"),
            &vec![460.0, 392.0, 302.0, 212.0, 122.0, 0.0]
        );
    }
}
