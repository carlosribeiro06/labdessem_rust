//! CAD_FPHA_CONFIG.csv reader.

use std::collections::HashMap;
use std::path::Path;

use csv::ReaderBuilder;
use serde::Deserialize;

use crate::config::{FphaMode, FphaPlantConfig};
use crate::error::FphaFittingError;
use crate::fitting::PlaneReductionConfig;

/// Raw CSV row from CAD_FPHA_CONFIG.csv.
#[derive(Debug, Clone, Deserialize)]
pub struct FphaConfigRow {
    #[serde(alias = "Codigo", alias = "CODIGO", alias = "codigo")]
    pub codigo: i32,
    #[serde(alias = "Nome", alias = "NOME", alias = "nome")]
    pub nome: String,
    #[serde(alias = "FphaMode", alias = "FPHAMODE", alias = "fpha_mode")]
    pub fpha_mode: String,
    #[serde(alias = "MaxPlanes", alias = "MAXPLANES", alias = "max_planes", default)]
    pub max_planes: usize,
    #[serde(alias = "VolumePoints", alias = "VOLUMEPOINTS", alias = "volume_points", default)]
    pub volume_points: usize,
    #[serde(alias = "FlowPoints", alias = "FLOWPOINTS", alias = "flow_points", default)]
    pub flow_points: usize,
    #[serde(alias = "ReductionMethod", alias = "REDUCTIONMETHOD", alias = "reduction_method", default)]
    pub reduction_method: String,
    #[serde(alias = "ReductionTolerance", alias = "REDUCTIONTOLERANCE", alias = "reduction_tolerance", default)]
    pub reduction_tolerance: f64,
}

impl FphaConfigRow {
    fn to_config(&self) -> FphaPlantConfig {
        let mode = match self.fpha_mode.to_ascii_lowercase().as_str() {
            "computed" | "compute" | "1" => FphaMode::Computed,
            _ => FphaMode::Precomputed,
        };

        let reduction_config = match self.reduction_method.to_ascii_lowercase().as_str() {
            "distance" => PlaneReductionConfig::Distance {
                tolerance_pct: self.reduction_tolerance.max(0.1),
                n_samples: 64,
            },
            _ => PlaneReductionConfig::Angle {
                tolerance_deg: self.reduction_tolerance.max(0.1),
            },
        };

        FphaPlantConfig {
            mode,
            max_planes: self.max_planes.max(1),
            volume_points: self.volume_points.max(2),
            flow_points: self.flow_points.max(2),
            reduction_config,
        }
    }
}

/// Read CAD_FPHA_CONFIG.csv and build per-plant configs.
pub fn read_fpha_config(
    path: impl AsRef<Path>,
) -> Result<HashMap<i32, FphaPlantConfig>, FphaFittingError> {
    let path = path.as_ref();
    let mut reader = ReaderBuilder::new()
        .delimiter(b';')
        .has_headers(true)
        .flexible(true)
        .from_path(path)
        .map_err(|e| FphaFittingError::IoError {
            path: path.display().to_string(),
            detail: e.to_string(),
        })?;

    let mut configs = HashMap::new();

    for result in reader.deserialize() {
        let row: FphaConfigRow = result.map_err(|e| FphaFittingError::IoError {
            path: path.display().to_string(),
            detail: e.to_string(),
        })?;

        configs.insert(row.codigo, row.to_config());
    }

    Ok(configs)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Write;
    use tempfile::NamedTempFile;

    fn write_test_csv(content: &str) -> NamedTempFile {
        let mut file = NamedTempFile::new().unwrap();
        file.write_all(content.as_bytes()).unwrap();
        file
    }

    #[test]
    fn reads_computed_mode() {
        let csv = "Codigo;Nome;FphaMode;MaxPlanes;VolumePoints;FlowPoints;ReductionMethod;ReductionTolerance\n\
                   1;FURNAS;computed;10;6;6;angle;1.0";
        let file = write_test_csv(csv);
        let configs = read_fpha_config(file.path()).unwrap();

        let config = configs.get(&1).unwrap();
        assert_eq!(config.mode, FphaMode::Computed);
        assert_eq!(config.max_planes, 10);
        assert_eq!(config.volume_points, 6);
    }

    #[test]
    fn defaults_to_precomputed() {
        let csv = "Codigo;Nome;FphaMode;MaxPlanes;VolumePoints;FlowPoints;ReductionMethod;ReductionTolerance\n\
                   1;TEST;precomputed;0;0;0;;0";
        let file = write_test_csv(csv);
        let configs = read_fpha_config(file.path()).unwrap();

        let config = configs.get(&1).unwrap();
        assert_eq!(config.mode, FphaMode::Precomputed);
    }

    #[test]
    fn parses_distance_reduction() {
        let csv = "Codigo;Nome;FphaMode;MaxPlanes;VolumePoints;FlowPoints;ReductionMethod;ReductionTolerance\n\
                   1;TEST;computed;8;4;4;distance;0.5";
        let file = write_test_csv(csv);
        let configs = read_fpha_config(file.path()).unwrap();

        let config = configs.get(&1).unwrap();
        assert!(matches!(config.reduction_config, PlaneReductionConfig::Distance { .. }));
    }
}
