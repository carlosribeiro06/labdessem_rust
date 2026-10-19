//! Study loading orchestration.
//!
//! This module provides the entry point for loading a complete study case
//! from the filesystem.

use std::{
    fs,
    path::Path,
};

use labdessem_core::system::System;
use serde::Deserialize;

use crate::error::IoError;
use crate::readers::{
    build_dummy_buses, read_branches, read_buses, read_future_cost_cuts, read_horizon,
    read_hydro_plants, read_interchange_limits, read_operational_limits, read_pumping_plants,
    read_renewable_plants, read_residual_costs, read_submarkets, read_thermal_plants,
};

use schemars::JsonSchema;

/// Study configuration read from study_config.json.
#[derive(Debug, Clone, Deserialize, PartialEq, Eq, JsonSchema)]
pub struct StudyConfig {
    /// Execution mode: 1 for iterative (LP→MILP→LP-FIXED→LP-CMO), 2 for single MILP.
    pub opcao_execucao: u8,
    /// Enable network constraints (0 = disabled, 1 = enabled).
    pub rede: u8,
    /// Enable thermal unit commitment (0 = disabled, 1 = enabled).
    #[serde(rename = "UCT")]
    pub uct: u8,
    /// Enable hydro unit commitment (0 = disabled, 1 = enabled).
    #[serde(rename = "UCH")]
    pub uch: u8,
    /// Enable FPHA hydro generation formulation (0 = disabled, 1 = enabled).
    #[serde(rename = "FPHA")]
    pub fpha: u8,
    /// Enable future cost function (0 = disabled, 1 = enabled).
    #[serde(rename = "FCF", default)]
    pub fcf: u8,
    /// Enable thermal residual TON treatment (0 = disabled, 1 = enabled).
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

    let horizon = read_horizon(&oper_path)?;

    let submarkets = read_submarkets(&cad_path, &oper_path, &horizon)?;
    let buses = if network_enabled {
        read_buses(&oper_path, &horizon)?
    } else {
        build_dummy_buses(&submarkets, horizon.periods)
    };
    let branches = if network_enabled {
        read_branches(&oper_path)?
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

/// Generate JSON Schema for StudyConfig.
pub fn generate_study_config_schema() -> String {
    let schema = schemars::schema_for!(StudyConfig);
    // Rationale: serde_json serialization of valid schemars schema cannot fail
    #[allow(clippy::expect_used)]
    serde_json::to_string_pretty(&schema).expect("schema serialization should not fail")
}

#[cfg(test)]
// Rationale: Tests use expect() for cleaner failure messages
#[allow(clippy::unwrap_used, clippy::expect_used)]
mod tests {
    use super::{generate_study_config_schema, read_study_config, read_study_from_path};
    use std::path::PathBuf;

    fn examples_24barras_path() -> PathBuf {
        PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../examples/24Barras")
    }

    #[test]
    fn reads_current_case() {
        let case_path = examples_24barras_path();

        let system = read_study_from_path(&case_path).expect("24Barras should be readable");
        assert_eq!(system.submarkets.len(), 2);
        assert_eq!(system.buses.len(), 24);
        assert!(!system.branches.is_empty());
        assert_eq!(system.thermal_plants.len(), 3);
        assert_eq!(system.hydro_plants.len(), 4);
        assert!(!system.hydro_unit_commitment_enabled);
    }

    #[test]
    fn reads_study_config_without_case_path() {
        let config_path = examples_24barras_path().join("study_config.json");

        let config = read_study_config(&config_path).expect("study config should be readable");
        assert_eq!(config.rede, 1);
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
        assert_eq!(system.buses.len(), 24);
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

    #[test]
    fn generates_valid_json_schema() {
        let schema = generate_study_config_schema();
        let parsed: serde_json::Value =
            serde_json::from_str(&schema).expect("schema should be valid JSON");
        assert!(parsed.get("$schema").is_some());
        assert!(parsed.get("properties").is_some());
    }
}
