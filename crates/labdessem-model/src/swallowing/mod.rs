//! Maximum swallowing of a hydro plant (MM eq. 9.34), used by the simulation solve loop: the formula
//! and the pure plant helpers (RD-16).

mod formula;
mod plant;

pub use formula::calculate_max_swallowing;
pub use plant::{
    PlantReference, net_head_m, partial_effective_head_groups, plant_bound_hm3, plant_head_model,
};
