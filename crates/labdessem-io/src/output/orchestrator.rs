//! Orchestrator for writing all simulation results.

use std::collections::HashMap;
use std::path::Path;

use labdessem_core::system::System;

use crate::error::IoError;

use super::{
    types::OutputData, write_added_flow_cut_lines_csv, write_cmb_csv, write_cmosist_csv,
    write_future_cost_eco_csv, write_hydro_csv, write_network_csv, write_network_infeasibility_csv,
    write_operational_limit_infeasibility_csv, write_operational_limits_csv,
    write_process_iterations_csv, write_pumping_csv, write_renewable_csv, write_thermal_csv,
};

/// Write all simulation results to CSV files.
///
/// Calls all individual writers in sequence, conditional on system flags.
///
/// # Arguments
/// * `system` - The system definition
/// * `data` - Aggregated output data from the simulation
/// * `variable_values` - Map of variable names to values from LP-CALC-CMO solution
/// * `constraint_duals` - Map of constraint names to dual values from LP-CALC-CMO solution
/// * `output_dir` - Directory to write output files
/// * `network_enabled` - Whether network treatment is enabled
///
/// # Errors
/// Returns `IoError` if any writer fails.
pub fn write_all_results(
    system: &System,
    data: &OutputData,
    variable_values: &HashMap<String, f64>,
    constraint_duals: &HashMap<String, f64>,
    output_dir: impl AsRef<Path>,
    network_enabled: bool,
) -> Result<(), IoError> {
    let output_dir = output_dir.as_ref();
    std::fs::create_dir_all(output_dir)?;

    write_hydro_csv(system, variable_values, constraint_duals, output_dir)?;
    write_thermal_csv(system, variable_values, output_dir)?;
    write_pumping_csv(system, variable_values, output_dir)?;
    write_renewable_csv(system, variable_values, output_dir)?;
    write_cmosist_csv(
        system,
        constraint_duals,
        data.cmo_values.as_ref(),
        output_dir,
    )?;
    write_process_iterations_csv(&data.steps, output_dir)?;
    write_operational_limits_csv(system, output_dir)?;
    write_operational_limit_infeasibility_csv(&data.operational_limit_infeasibilities, output_dir)?;

    if system.future_cost_enabled {
        write_future_cost_eco_csv(system, output_dir)?;
    }

    if network_enabled {
        write_network_csv(&data.final_line_flows, output_dir)?;
        write_added_flow_cut_lines_csv(&data.added_flow_cut_lines, output_dir)?;
        write_network_infeasibility_csv(&data.network_infeasibilities, output_dir)?;
        write_cmb_csv(&data.cmb_records, output_dir)?;
    }

    Ok(())
}

#[cfg(test)]
#[allow(clippy::expect_used, clippy::unwrap_used)]
mod tests {
    use super::*;
    use labdessem_core::system::StudyHorizon;
    use std::collections::HashMap;
    use tempfile::TempDir;

    fn minimal_system() -> System {
        System {
            horizon: StudyHorizon {
                periods: 1,
                period_duration_hours: 1.0,
                original_periods: 1,
                original_period_durations_hours: vec![1.0],
                internal_to_original_period: vec![1],
                internal_subperiod_index: vec![1],
            },
            thermal_unit_commitment_enabled: false,
            hydro_unit_commitment_enabled: false,
            ton_residual_enabled: false,
            fpha_enabled: false,
            future_cost_enabled: false,
            residual_costs: vec![],
            future_cost_cuts: vec![],
            submarkets: vec![],
            interchange_limits: vec![],
            operational_limits: vec![],
            buses: vec![],
            branches: vec![],
            thermal_plants: vec![],
            hydro_plants: vec![],
            pumping_plants: vec![],
            renewable_plants: vec![],
            inflow_history: vec![],
        }
    }

    #[test]
    fn write_all_results_creates_output_directory() {
        let temp_dir = TempDir::new().expect("failed to create temp dir");
        let output_dir = temp_dir.path().join("output");

        let system = minimal_system();

        let data = OutputData::default();
        let variable_values = HashMap::new();
        let constraint_duals = HashMap::new();

        write_all_results(
            &system,
            &data,
            &variable_values,
            &constraint_duals,
            &output_dir,
            false,
        )
        .expect("write_all_results should succeed");

        assert!(output_dir.exists());
    }

    #[test]
    fn write_all_results_creates_core_files() {
        let temp_dir = TempDir::new().expect("failed to create temp dir");
        let output_dir = temp_dir.path().join("output");

        let system = minimal_system();

        let data = OutputData::default();
        let variable_values = HashMap::new();
        let constraint_duals = HashMap::new();

        write_all_results(
            &system,
            &data,
            &variable_values,
            &constraint_duals,
            &output_dir,
            false,
        )
        .expect("write_all_results should succeed");

        assert!(output_dir.join("resultado_hidreletricas.csv").exists());
        assert!(output_dir.join("resultado_termicas.csv").exists());
        assert!(output_dir.join("resultado_elevatorias.csv").exists());
        assert!(output_dir.join("resultado_renovaveis.csv").exists());
        assert!(output_dir.join("resultado_cmosist.csv").exists());
        assert!(output_dir.join("resultado_processo_iter.csv").exists());
        assert!(output_dir.join("resultado_rest_lim.csv").exists());
        assert!(output_dir.join("resultado_inviabilidade_lim.csv").exists());
    }

    #[test]
    fn write_all_results_creates_network_files_when_enabled() {
        let temp_dir = TempDir::new().expect("failed to create temp dir");
        let output_dir = temp_dir.path().join("output");

        let system = minimal_system();

        let data = OutputData::default();
        let variable_values = HashMap::new();
        let constraint_duals = HashMap::new();

        write_all_results(
            &system,
            &data,
            &variable_values,
            &constraint_duals,
            &output_dir,
            true,
        )
        .expect("write_all_results should succeed");

        assert!(output_dir.join("resultado_rede.csv").exists());
        assert!(output_dir.join("resultado_linhas_adicionadas.csv").exists());
        assert!(output_dir.join("resultado_inviabilidade_rede.csv").exists());
        assert!(output_dir.join("resultado_cmobar.csv").exists());
    }

    #[test]
    fn write_all_results_skips_network_files_when_disabled() {
        let temp_dir = TempDir::new().expect("failed to create temp dir");
        let output_dir = temp_dir.path().join("output");

        let system = minimal_system();

        let data = OutputData::default();
        let variable_values = HashMap::new();
        let constraint_duals = HashMap::new();

        write_all_results(
            &system,
            &data,
            &variable_values,
            &constraint_duals,
            &output_dir,
            false,
        )
        .expect("write_all_results should succeed");

        assert!(!output_dir.join("resultado_rede.csv").exists());
        assert!(!output_dir.join("resultado_linhas_adicionadas.csv").exists());
        assert!(!output_dir.join("resultado_inviabilidade_rede.csv").exists());
        assert!(!output_dir.join("resultado_cmobar.csv").exists());
    }
}
