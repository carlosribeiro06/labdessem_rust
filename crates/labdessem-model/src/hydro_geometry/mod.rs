//! Plant geometry shared by the FPHA fit and max swallowing (RD-22): the forebay VHA table, the
//! tailrace polynomial and the errors of an invalid geometry table.

mod forebay;
mod tailrace;

use thiserror::Error;

pub use forebay::{ForebayTable, VhaRow};
pub use tailrace::TailraceModel;

/// Invalid plant geometry table.
#[derive(Debug, Clone, Error)]
pub enum GeometryError {
    #[error("hydro {hydro_name}: insufficient geometry points ({count} < 2)")]
    InsufficientPoints { hydro_name: String, count: usize },

    #[error("hydro {hydro_name}: duplicate volume {volume_hm3} in geometry table")]
    DuplicateVolume { hydro_name: String, volume_hm3: f64 },
}
