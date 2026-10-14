//! Model constraints organized by domain.
//!
//! Constraints are split into submodules by type:
//! - [`thermal`] - Thermal unit commitment, ramps, min up/down times
//! - [`hydro`] - Hydraulic balance, FPHA, turbining limits
//! - [`demand`] - Demand balance and interchange limits
//! - [`limits`] - Operational limits and future cost cuts

mod constraint_set;
mod demand;
mod hydro;
mod limits;
mod thermal;
mod types;

use crate::variables::Variable;

pub use constraint_set::ConstraintSet;
pub use demand::{build_demand_balance_constraints, build_interchange_limit_constraints};
pub use hydro::{
    build_hydro_balance_constraints, build_hydro_commitment_channeling_constraints,
    build_hydro_fpha_constraints, build_hydro_generation_turbining_coupling_constraints,
    build_hydro_productivity_constraints, build_hydro_spillage_nonnegativity_constraints,
    build_hydro_turbining_limit_constraints,
};
pub use limits::{build_future_cost_constraints, build_operational_limit_constraints};
pub use thermal::{build_thermal_min_up_down_constraints, build_thermal_ramp_constraints};
pub use types::{ConstraintSense, LinearConstraint, LinearTerm};

pub(crate) fn term(variable: &Variable, coefficient: f64) -> LinearTerm {
    LinearTerm {
        variable: variable.name.clone(),
        coefficient,
    }
}

pub(crate) fn display_period(period: usize) -> usize {
    period + 1
}
