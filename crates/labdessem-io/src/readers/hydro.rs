//! Hydro plant CSV reader.

use std::collections::{BTreeMap, HashMap};
use std::path::Path;

use csv::{ReaderBuilder, StringRecord};
use labdessem_core::entities::{
    HydroFphaSegment, HydroGroup, HydroInitialCondition, HydroPlant, HydroUnit, Reservoir,
    Submarket, TurbineType,
};
use labdessem_core::ids::{BusId, HydroGroupId, HydroPlantId, HydroUnitId};
use labdessem_core::system::StudyHorizon;
use serde::Deserialize;

use crate::error::IoError;
use crate::readers::common::{
    bus_id_or_dummy, flow_m3s_to_hm3, hours_to_periods, log_read_file, normalize_max_bound,
    read_csv, submarket_id_by_name,
};
use crate::readers::fpha_geometry::{read_tailrace, read_vha_geometry, TailraceData, VhaGeometryData};

/// Read hydro plants from CAD_UHE.csv, CAD_CONJ_UHE.csv, OPER_VAZAO.csv, and optionally OPER_FPHA.csv.
pub fn read_hydro_plants(
    cad_path: &Path,
    oper_path: &Path,
    submarkets: &[Submarket],
    horizon: &StudyHorizon,
    network_enabled: bool,
    fpha_enabled: bool,
) -> Result<Vec<HydroPlant>, IoError> {
    let plant_rows: Vec<HydroPlantRow> = read_csv(&cad_path.join("CAD_UHE.csv"))?;
    let unit_rows: Vec<HydroUnitRow> = read_csv(&cad_path.join("CAD_CONJ_UHE.csv"))?;
    let inflow_rows: Vec<HydroInflowRow> = read_csv(&oper_path.join("OPER_VAZAO.csv"))?;
    let fpha_rows = if fpha_enabled {
        read_fpha_table(oper_path.join("OPER_FPHA.csv"))?
    } else {
        Vec::new()
    };

    // Read geometry data for net head calculation (optional files)
    let case_path = cad_path.parent().unwrap_or(cad_path);
    let vha_data = read_vha_geometry(case_path)?;
    let tailrace_data = read_tailrace(case_path)?;

    build_hydro_plants(
        &plant_rows,
        &unit_rows,
        &inflow_rows,
        &fpha_rows,
        submarkets,
        horizon,
        network_enabled,
        fpha_enabled,
        &vha_data,
        &tailrace_data,
    )
}

fn read_fpha_table(path: impl AsRef<Path>) -> Result<Vec<HydroFphaRow>, IoError> {
    let path = path.as_ref();
    log_read_file(path);
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
    let rhs_idx = column("RHS.MW")?;
    let volume_idx = column("VARM.MW/HM3")?;
    let turbining_idx = column("QTUR.MW/M3S")?;
    let lateral_idx = column("QLAT.MW/M3S")?;

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

fn build_hydro_plants(
    plant_rows: &[HydroPlantRow],
    unit_rows: &[HydroUnitRow],
    inflow_rows: &[HydroInflowRow],
    fpha_rows: &[HydroFphaRow],
    submarkets: &[Submarket],
    horizon: &StudyHorizon,
    network_enabled: bool,
    fpha_enabled: bool,
    vha_data: &HashMap<u32, VhaGeometryData>,
    tailrace_data: &HashMap<u32, TailraceData>,
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
                            "hydro plant {plant_name} group {group_id} must have at least one unit"
                        )));
                    }

                    for _ in 0..unit_quantity {
                        let unit_id = next_unit_id;
                        next_unit_id += 1;
                        let unit_name = format!("{plant_name}-{group_id}-{unit_id}");
                        let max_generation_mw =
                            normalize_max_bound(unit_row.pmin, unit_row.pmax, "Pmax", &unit_name)?;
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
                            effective_head_m: unit_row.effective_head_m,
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
            let fpha_segments = if fpha_enabled {
                if plant_unit_rows.is_empty() {
                    fpha_by_plant.get(&row.codigo).cloned().unwrap_or_default()
                } else {
                    fpha_by_plant.get(&row.codigo).cloned().ok_or_else(|| {
                        IoError::invalid_data(format!(
                            "missing FPHA data for hydro plant {}",
                            row.codigo
                        ))
                    })?
                }
            } else {
                Vec::new()
            };

            let submarket_id = submarket_id_by_name(submarkets, &row.submercado)?;
            let turbine_type = row
                .turbina
                .trim()
                .parse::<i32>()
                .map(TurbineType::from)
                .unwrap_or_default();
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
                turbining_cost_per_hm3: row.penal_turb,
                specific_productivity_mw_per_m3s: row.prod,
                hydraulic_loss_m: row.perda,
                efficiency: row.eficiencia,
                fpha_segments,
                groups,
                turbine_type,
                volume_level_table: vha_data.get(&(row.codigo as u32)).map(|vha| {
                    vha.points
                        .iter()
                        .map(|p| (p.volume_hm3, p.height_m))
                        .collect()
                }),
                tailrace_coefficients: tailrace_data
                    .get(&(row.codigo as u32))
                    .map(|t| t.coefficients),
            })
        })
        .collect()
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

#[derive(Debug, Deserialize)]
struct HydroPlantRow {
    #[serde(rename = "Codigo")]
    codigo: usize,
    #[serde(rename = "Nome")]
    nome: String,
    #[serde(rename = "VolInic.hm3")]
    vol_inic: f64,
    #[serde(rename = "Vmin.hm3")]
    vmin: f64,
    #[serde(rename = "Vmax.hm3")]
    vmax: f64,
    #[serde(rename = "Tipo")]
    _tipo: String,
    #[serde(rename = "Jusante")]
    jusante: String,
    #[serde(rename = "Desvio")]
    desvio: String,
    #[serde(rename = "Submercado")]
    submercado: String,
    #[serde(rename = "PenalVert.R$/hm3")]
    penal_vert: f64,
    #[serde(rename = "PenalTurb.R$/hm3")]
    penal_turb: f64,
    #[serde(rename = "Prod.mw/m3s")]
    prod: f64,
    #[serde(rename = "Turbina")]
    turbina: String,
    #[serde(rename = "Perda.m", default)]
    perda: Option<f64>,
    #[serde(rename = "Eficiencia", default)]
    eficiencia: Option<f64>,
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
    #[serde(rename = "Pmin.mw")]
    pmin: f64,
    #[serde(rename = "Pmax.mw")]
    pmax: f64,
    #[serde(rename = "StatusInic")]
    status_inic: usize,
    #[serde(rename = "Tinic.h")]
    tinic: f64,
    #[serde(rename = "Barra")]
    barra: Option<usize>,
    #[serde(rename = "MaxTurb.m3/s")]
    max_turb: f64,
    #[serde(rename = "Hef.m", default)]
    effective_head_m: f64,
}

#[derive(Debug, Deserialize)]
struct HydroInflowRow {
    #[serde(rename = "Codigo")]
    codigo: usize,
    #[serde(rename = "Periodo")]
    periodo: usize,
    #[serde(rename = "Nome")]
    _nome: String,
    #[serde(rename = "Afluencia.m3/s")]
    afluencia: f64,
    #[serde(rename = "Retirada.m3/s")]
    retirada: f64,
}
