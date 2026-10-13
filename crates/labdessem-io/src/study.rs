use std::{
    collections::BTreeMap,
    fs,
    path::{Path, PathBuf},
};

use labdessem_core::system::{FutureCostCut, StudyHorizon, System};
use serde::Deserialize;

use crate::error::IoError;
use crate::readers::{
    build_dummy_buses, read_branches, read_buses, read_hydro_plants, read_interchange_limits,
    read_operational_limits, read_pumping_plants, read_renewable_plants, read_residual_costs,
    read_submarkets, read_thermal_plants,
};

#[derive(Debug, Clone, Deserialize, PartialEq, Eq)]
pub struct StudyConfig {
    pub opcao_execucao: u8,
    pub rede: u8,
    #[serde(rename = "UCT")]
    pub uct: u8,
    #[serde(rename = "UCH")]
    pub uch: u8,
    #[serde(rename = "FPHA")]
    pub fpha: u8,
    #[serde(rename = "FCF", default)]
    pub fcf: u8,
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

    let config: StudyConfig = serde_json::from_str(&contents)?;
    Ok(config)
}

pub fn read_study_from_path(case_path: impl AsRef<Path>) -> Result<System, IoError> {
    let case_path = case_path.as_ref();
    let config_path = case_path.join("study_config.json");

    if !config_path.exists() {
        return Err(IoError::invalid_data(format!(
            "Configuration file not found.\n\
             Expected: {}\n\n\
             Each study case must contain a study_config.json file in its root directory.",
            config_path.display()
        )));
    }

    let config = read_study_config(&config_path)?;
    read_study_from_path_with_options(
        case_path,
        config.ton_residual != 0,
        config.rede != 0,
        config.uct != 0,
        config.uch != 0,
        config.fpha != 0,
        config.fcf != 0,
    )
}

fn read_study_from_path_with_options(
    base_path: impl AsRef<Path>,
    ton_residual_enabled: bool,
    network_enabled: bool,
    thermal_unit_commitment_enabled: bool,
    hydro_unit_commitment_enabled: bool,
    fpha_enabled: bool,
    future_cost_enabled: bool,
) -> Result<System, IoError> {
    let base_path = base_path.as_ref();
    let cad_path = base_path.join("CAD");
    let oper_path = base_path.join("OPER");

    let duration_rows: Vec<DurationRow> = crate::readers::read_csv(oper_path.join("OPER_DURACAO.csv"))?;
    let horizon = build_horizon(&duration_rows)?;

    let submarkets = read_submarkets(&cad_path, &oper_path, &horizon)?;
    let buses = if network_enabled {
        read_buses(&oper_path, &horizon)?
    } else {
        build_dummy_buses(&submarkets, horizon.periods)
    };
    let branches = if network_enabled {
        read_branches(&cad_path)?
    } else {
        Vec::new()
    };
    let interchange_limits = read_interchange_limits(&oper_path)?;
    let residual_costs = read_residual_costs(&oper_path, ton_residual_enabled)?;
    let future_cost_cuts = if future_cost_enabled {
        read_future_cost_cuts(oper_path.join("OPER_FCF.bin"))?
    } else {
        Vec::new()
    };

    let thermal_plants = read_thermal_plants(
        &cad_path,
        &submarkets,
        horizon.period_duration_hours,
        network_enabled,
        thermal_unit_commitment_enabled,
    )?;
    let hydro_plants = read_hydro_plants(
        &cad_path,
        &oper_path,
        &submarkets,
        &horizon,
        network_enabled,
        fpha_enabled,
    )?;
    let pumping_plants = read_pumping_plants(
        &cad_path,
        &hydro_plants,
        &submarkets,
        horizon.period_duration_hours,
        network_enabled,
    )?;
    let renewable_plants = read_renewable_plants(
        &cad_path,
        &oper_path,
        &submarkets,
        &horizon,
        network_enabled,
    )?;
    let operational_limits = read_operational_limits(
        &oper_path,
        &thermal_plants,
        &hydro_plants,
        &pumping_plants,
        &horizon,
    )?;

    let system = System {
        horizon,
        thermal_unit_commitment_enabled,
        hydro_unit_commitment_enabled,
        ton_residual_enabled,
        fpha_enabled,
        future_cost_enabled,
        residual_costs,
        future_cost_cuts,
        submarkets,
        interchange_limits,
        operational_limits,
        buses,
        branches,
        thermal_plants,
        hydro_plants,
        pumping_plants,
        renewable_plants,
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

fn read_future_cost_cuts(path: PathBuf) -> Result<Vec<FutureCostCut>, IoError> {
    let bytes = fs::read(&path).map_err(|error| {
        IoError::invalid_data(format!(
            "failed to read future cost binary {}: {error}",
            path.display()
        ))
    })?;
    if bytes.len() < 8 {
        return Err(IoError::invalid_data(format!(
            "future cost binary {} is too small to be a valid FlatBuffers buffer",
            path.display()
        )));
    }

    let root_pos = read_fb_u32(&bytes, 0)? as usize;
    let stage_table = fb_table(&bytes, root_pos, "StageCuts")?;
    let stage_id = fb_get_u32(&bytes, &stage_table, 0)?.unwrap_or(0) as usize;
    let cuts_vector_pos = fb_get_vector_pos(&bytes, &stage_table, 4)?.ok_or_else(|| {
        IoError::invalid_data(format!(
            "future cost binary {} does not contain a cuts vector",
            path.display()
        ))
    })?;
    let cut_count = fb_vector_len(&bytes, cuts_vector_pos)?;
    let mut cuts = Vec::with_capacity(cut_count);

    for cut_idx in 0..cut_count {
        let cut_table_pos = fb_get_table_from_vector(&bytes, cuts_vector_pos, cut_idx)?;
        let cut_table = fb_table(&bytes, cut_table_pos, "Cut")?;
        let coefficients = if let Some(coeff_vector_pos) = fb_get_vector_pos(&bytes, &cut_table, 6)?
        {
            let coefficient_count = fb_vector_len(&bytes, coeff_vector_pos)?;
            (0..coefficient_count)
                .map(|coefficient_idx| {
                    fb_get_f64_from_vector(&bytes, coeff_vector_pos, coefficient_idx)
                })
                .collect::<Result<Vec<_>, IoError>>()?
        } else {
            Vec::new()
        };

        cuts.push(FutureCostCut {
            stage_id,
            cut_id: fb_get_u64(&bytes, &cut_table, 0)?.unwrap_or(0),
            slot_index: fb_get_u32(&bytes, &cut_table, 1)?.unwrap_or(cut_idx as u32) as usize,
            iteration: fb_get_u32(&bytes, &cut_table, 2)?.unwrap_or(0) as usize,
            forward_pass_index: fb_get_u32(&bytes, &cut_table, 3)?.unwrap_or(0) as usize,
            intercept: fb_get_f64(&bytes, &cut_table, 5)?.unwrap_or(0.0),
            coefficients,
            is_active: fb_get_bool(&bytes, &cut_table, 8)?.unwrap_or(false),
        });
    }

    Ok(cuts)
}

#[derive(Debug, Clone, Copy)]
struct FbTable {
    table_pos: usize,
    vtable_pos: usize,
}

fn fb_table(bytes: &[u8], table_pos: usize, label: &str) -> Result<FbTable, IoError> {
    let vtable_distance = read_fb_i32(bytes, table_pos)?;
    let vtable_pos = if vtable_distance >= 0 {
        table_pos
            .checked_sub(vtable_distance as usize)
            .ok_or_else(|| {
                IoError::invalid_data(format!(
                    "{label} table at position {table_pos} points to an invalid vtable"
                ))
            })?
    } else {
        table_pos
            .checked_add(vtable_distance.unsigned_abs() as usize)
            .ok_or_else(|| {
                IoError::invalid_data(format!(
                    "{label} table at position {table_pos} points to an invalid vtable"
                ))
            })?
    };
    let _ = read_fb_u16(bytes, vtable_pos)?;
    let _ = read_fb_u16(bytes, vtable_pos + 2)?;

    Ok(FbTable {
        table_pos,
        vtable_pos,
    })
}

fn fb_field_offset(
    bytes: &[u8],
    table: &FbTable,
    field_id: usize,
) -> Result<Option<usize>, IoError> {
    let field_pos = table.vtable_pos + 4 + field_id * 2;
    if field_pos + 2 > bytes.len() {
        return Ok(None);
    }
    let offset = read_fb_u16(bytes, field_pos)? as usize;
    if offset == 0 {
        Ok(None)
    } else {
        Ok(Some(table.table_pos + offset))
    }
}

fn fb_get_u32(bytes: &[u8], table: &FbTable, field_id: usize) -> Result<Option<u32>, IoError> {
    fb_field_offset(bytes, table, field_id)?
        .map(|pos| read_fb_u32(bytes, pos))
        .transpose()
}

fn fb_get_u64(bytes: &[u8], table: &FbTable, field_id: usize) -> Result<Option<u64>, IoError> {
    fb_field_offset(bytes, table, field_id)?
        .map(|pos| read_fb_u64(bytes, pos))
        .transpose()
}

fn fb_get_f64(bytes: &[u8], table: &FbTable, field_id: usize) -> Result<Option<f64>, IoError> {
    fb_field_offset(bytes, table, field_id)?
        .map(|pos| read_fb_f64(bytes, pos))
        .transpose()
}

fn fb_get_bool(bytes: &[u8], table: &FbTable, field_id: usize) -> Result<Option<bool>, IoError> {
    fb_field_offset(bytes, table, field_id)?
        .map(|pos| read_fb_bool(bytes, pos))
        .transpose()
}

fn fb_get_vector_pos(
    bytes: &[u8],
    table: &FbTable,
    field_id: usize,
) -> Result<Option<usize>, IoError> {
    let Some(field_pos) = fb_field_offset(bytes, table, field_id)? else {
        return Ok(None);
    };
    let vector_offset = read_fb_u32(bytes, field_pos)? as usize;
    Ok(Some(field_pos + vector_offset))
}

fn fb_vector_len(bytes: &[u8], vector_pos: usize) -> Result<usize, IoError> {
    Ok(read_fb_u32(bytes, vector_pos)? as usize)
}

fn fb_get_table_from_vector(
    bytes: &[u8],
    vector_pos: usize,
    index: usize,
) -> Result<usize, IoError> {
    let element_pos = vector_pos + 4 + index * 4;
    let relative = read_fb_u32(bytes, element_pos)? as usize;
    Ok(element_pos + relative)
}

fn fb_get_f64_from_vector(bytes: &[u8], vector_pos: usize, index: usize) -> Result<f64, IoError> {
    let element_pos = vector_pos + 4 + index * 8;
    read_fb_f64(bytes, element_pos)
}

fn read_fb_u16(bytes: &[u8], pos: usize) -> Result<u16, IoError> {
    let slice = bytes.get(pos..pos + 2).ok_or_else(|| {
        IoError::invalid_data(format!(
            "FlatBuffers read out of bounds at byte range [{pos}, {})",
            pos + 2
        ))
    })?;
    Ok(u16::from_le_bytes([slice[0], slice[1]]))
}

fn read_fb_u32(bytes: &[u8], pos: usize) -> Result<u32, IoError> {
    let slice = bytes.get(pos..pos + 4).ok_or_else(|| {
        IoError::invalid_data(format!(
            "FlatBuffers read out of bounds at byte range [{pos}, {})",
            pos + 4
        ))
    })?;
    Ok(u32::from_le_bytes([slice[0], slice[1], slice[2], slice[3]]))
}

fn read_fb_i32(bytes: &[u8], pos: usize) -> Result<i32, IoError> {
    let slice = bytes.get(pos..pos + 4).ok_or_else(|| {
        IoError::invalid_data(format!(
            "FlatBuffers read out of bounds at byte range [{pos}, {})",
            pos + 4
        ))
    })?;
    Ok(i32::from_le_bytes([slice[0], slice[1], slice[2], slice[3]]))
}

fn read_fb_u64(bytes: &[u8], pos: usize) -> Result<u64, IoError> {
    let slice = bytes.get(pos..pos + 8).ok_or_else(|| {
        IoError::invalid_data(format!(
            "FlatBuffers read out of bounds at byte range [{pos}, {})",
            pos + 8
        ))
    })?;
    Ok(u64::from_le_bytes([
        slice[0], slice[1], slice[2], slice[3], slice[4], slice[5], slice[6], slice[7],
    ]))
}

fn read_fb_f64(bytes: &[u8], pos: usize) -> Result<f64, IoError> {
    let slice = bytes.get(pos..pos + 8).ok_or_else(|| {
        IoError::invalid_data(format!(
            "FlatBuffers read out of bounds at byte range [{pos}, {})",
            pos + 8
        ))
    })?;
    Ok(f64::from_le_bytes([
        slice[0], slice[1], slice[2], slice[3], slice[4], slice[5], slice[6], slice[7],
    ]))
}

fn read_fb_bool(bytes: &[u8], pos: usize) -> Result<bool, IoError> {
    let value = *bytes.get(pos).ok_or_else(|| {
        IoError::invalid_data(format!(
            "FlatBuffers read out of bounds at byte index {pos}"
        ))
    })?;
    Ok(value != 0)
}

#[derive(Debug, Deserialize)]
struct DurationRow {
    #[serde(rename = "Periodo")]
    periodo: usize,
    #[serde(rename = "Duracao")]
    duracao: f64,
}

#[cfg(test)]
mod tests {
    use super::{read_study_config, read_study_from_path};
    use std::path::PathBuf;

    fn examples_24barras_path() -> PathBuf {
        PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../examples/24Barras")
    }

    #[test]
    fn reads_current_case() {
        let case_path = examples_24barras_path();

        let system = read_study_from_path(&case_path).expect("24Barras should be readable");
        assert_eq!(system.submarkets.len(), 2);
        assert_eq!(system.buses.len(), 2);
        assert!(system.branches.is_empty());
        assert_eq!(system.thermal_plants.len(), 3);
        assert_eq!(system.hydro_plants.len(), 4);
        assert!(!system.hydro_unit_commitment_enabled);
    }

    #[test]
    fn reads_study_config_without_case_path() {
        let config_path = examples_24barras_path().join("study_config.json");

        let config = read_study_config(&config_path).expect("study config should be readable");
        assert_eq!(config.rede, 0);
        assert!(config.uct <= 1);
        assert!(config.uch <= 1);
        assert!(config.fcf <= 1);
        assert!(config.ton_residual <= 1);
    }

    #[test]
    fn reads_study_from_case_directory() {
        let case_path = examples_24barras_path();

        let system =
            read_study_from_path(&case_path).expect("study config should build a valid system");
        assert_eq!(system.submarkets.len(), 2);
        assert_eq!(system.buses.len(), 2);
    }

    #[test]
    fn returns_error_when_config_missing() {
        let case_path = PathBuf::from("/nonexistent/path");

        let result = read_study_from_path(&case_path);
        assert!(result.is_err());
        let error_message = result.unwrap_err().to_string();
        assert!(error_message.contains("Configuration file not found"));
        assert!(error_message.contains("study_config.json"));
    }

    #[test]
    fn reads_thermal_plants_with_trajectories() {
        let case_path = examples_24barras_path();

        let system = read_study_from_path(&case_path).expect("24Barras should be readable");

        let termica1 = system
            .thermal_plants
            .iter()
            .find(|p| p.name == "Termica-1")
            .expect("Termica-1 should exist");

        let unit1 = termica1
            .units
            .iter()
            .find(|u| u.id.0 == 1)
            .expect("Termica-1 unit 1 should exist");

        assert_eq!(
            unit1.startup_trajectory_mw,
            vec![0.0, 0.0, 20.0, 20.0, 60.0, 80.0, 120.0, 160.0, 180.0, 220.0, 280.0, 410.0, 450.0, 523.0]
        );
        assert_eq!(unit1.shutdown_trajectory_mw, vec![255.0, 0.0]);
    }
}
