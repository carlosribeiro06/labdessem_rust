//! CSV output writers for simulation results.

pub mod common;
pub mod helpers;
pub mod hydro;
pub mod limits;
pub mod pumping;
pub mod renewable;
pub mod summary;
pub mod thermal;
pub mod types;

pub use common::{csv_with_header, volume_hm3_to_flow_m3s, write_csv_file};
pub use helpers::{
    average_internal_values, hydro_generation_max_at_point_mw, hydro_pumping_out_hm3,
    hydro_total_turbining_hm3, hydro_upstream_inflow_hm3, original_period_duration_hours,
    original_period_internal_indices, sum_internal_values, thermal_residence_times,
};
pub use hydro::write_hydro_csv;
pub use limits::write_operational_limits_csv;
pub use pumping::write_pumping_csv;
pub use renewable::write_renewable_csv;
pub use summary::{write_cmosist_csv, write_future_cost_eco_csv};
pub use thermal::write_thermal_csv;
pub use types::*;
