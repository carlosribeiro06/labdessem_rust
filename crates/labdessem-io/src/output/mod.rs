//! Result CSV writers.
//!
//! ## Entry Point
//!
//! Use [`write_all_results`] to write all simulation outputs at once.
//!
//! ## Individual Writers
//!
//! | Writer | Output File |
//! |--------|-------------|
//! | [`write_hydro_csv`] | resultado_hidreletricas.csv |
//! | [`write_thermal_csv`] | resultado_termicas.csv |
//! | [`write_pumping_csv`] | resultado_elevatorias.csv |
//! | [`write_renewable_csv`] | resultado_renovaveis.csv |
//! | [`write_cmosist_csv`] | resultado_cmosist.csv |
//! | [`write_process_iterations_csv`] | resultado_processo_iter.csv |
//! | [`write_operational_limits_csv`] | resultado_rest_lim.csv |
//! | [`write_operational_limit_infeasibility_csv`] | resultado_inviabilidade_lim.csv |
//! | [`write_future_cost_eco_csv`] | resultado_fcf_eco.csv |
//! | [`write_network_csv`] | resultado_rede.csv |
//! | [`write_added_flow_cut_lines_csv`] | resultado_linhas_adicionadas.csv |
//! | [`write_network_infeasibility_csv`] | resultado_inviabilidade_rede.csv |
//! | [`write_cmb_csv`] | resultado_cmobar.csv |

pub mod common;
pub mod fpha_computed;
pub mod helpers;
pub mod hydro;
pub mod infeasibility;
pub mod limits;
pub mod network;
pub mod orchestrator;
pub mod process;
pub mod pumping;
pub mod renewable;
pub mod summary;
pub mod thermal;
pub mod types;

pub use common::{csv_with_header, volume_hm3_to_flow_m3s, write_csv_file};
pub use fpha_computed::{write_fpha_computed, ComputedFphaRecord};
pub use helpers::{
    average_internal_values, hydro_generation_max_at_point_mw, hydro_pumping_out_hm3,
    hydro_total_turbining_hm3, hydro_upstream_inflow_hm3, operational_limit_variable_label,
    original_period_duration_hours, original_period_internal_indices, sum_internal_values,
    thermal_residence_times,
};
pub use hydro::write_hydro_csv;
pub use infeasibility::{
    write_network_infeasibility_csv, write_operational_limit_infeasibility_csv,
};
pub use limits::write_operational_limits_csv;
pub use network::{write_added_flow_cut_lines_csv, write_cmb_csv, write_network_csv};
pub use orchestrator::write_all_results;
pub use process::write_process_iterations_csv;
pub use pumping::write_pumping_csv;
pub use renewable::write_renewable_csv;
pub use summary::{write_cmosist_csv, write_future_cost_eco_csv};
pub use thermal::write_thermal_csv;
pub use types::*;
