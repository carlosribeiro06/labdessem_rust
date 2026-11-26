//! Plant geometry shared by the FPHA fit and max swallowing (RD-22): the forebay VHA table, the
//! tailrace polynomial, the net head they give and the errors of an invalid geometry table.

mod forebay;
mod tailrace;

use labdessem_core::entities::HydroPlant;
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

/// Net head of a plant: forebay level minus tailrace level minus hydraulic loss (MM eq. 9.30).
///
/// The tailrace polynomial and the loss are mandatory: each consumer applies its own
/// missing-data policy before it builds a model (RD-22).
#[derive(Debug, Clone)]
pub struct NetHeadModel {
    forebay: ForebayTable,
    tailrace: TailraceModel,
    loss_m: f64,
}

impl NetHeadModel {
    /// Net head model of a forebay table, a tailrace polynomial and a hydraulic loss (m).
    pub fn new(forebay: ForebayTable, tailrace: TailraceModel, loss_m: f64) -> Self {
        Self {
            forebay,
            tailrace,
            loss_m,
        }
    }

    /// Net head (m) at volume and outflow, floored at zero.
    pub fn net_head(&self, volume_hm3: f64, outflow_m3s: f64) -> f64 {
        let forebay_height = self.forebay.height(volume_hm3);
        let tailrace_level = self.tailrace.evaluate(outflow_m3s);
        let gross_head = forebay_height - tailrace_level;
        (gross_head - self.loss_m).max(0.0)
    }
}

/// Forebay table and tailrace polynomial of `plant`: the single `HydroPlant` -> geometry conversion
/// (RD-22).
///
/// Returns `Ok(None)` when the plant has no VHA table or no tailrace polynomial; each consumer applies
/// its own missing-data policy to that case. The VHA table is validated before the tailrace polynomial
/// is read, so a malformed table is reported even when the polynomial is missing.
///
/// # Errors
///
/// [`GeometryError`] when the VHA table is empty or repeats a volume.
pub fn plant_geometry(
    plant: &HydroPlant,
) -> Result<Option<(ForebayTable, TailraceModel)>, GeometryError> {
    let Some(table) = &plant.volume_level_table else {
        return Ok(None);
    };
    let rows: Vec<VhaRow> = table
        .iter()
        .map(|&(volume_hm3, height_m)| VhaRow {
            volume_hm3,
            height_m,
        })
        .collect();
    let forebay = ForebayTable::new(&rows, &plant.name)?;
    let Some(coefficients) = &plant.tailrace_coefficients else {
        return Ok(None);
    };
    Ok(Some((
        forebay,
        TailraceModel {
            coefficients: coefficients.to_vec(),
        },
    )))
}

#[cfg(test)]
#[allow(clippy::unwrap_used, clippy::expect_used, clippy::float_cmp)]
mod tests {
    use super::*;

    #[test]
    fn net_head_never_negative() {
        // Create scenario where tailrace > forebay
        let rows = vec![
            VhaRow {
                volume_hm3: 0.0,
                height_m: 100.0,
            },
            VhaRow {
                volume_hm3: 1000.0,
                height_m: 110.0,
            },
        ];
        let forebay = ForebayTable::new(&rows, "Low").unwrap();
        let tailrace = TailraceModel {
            coefficients: vec![150.0],
        };

        let model = NetHeadModel::new(forebay, tailrace, 5.0);

        assert!(model.net_head(500.0, 500.0) >= 0.0);
    }
}
