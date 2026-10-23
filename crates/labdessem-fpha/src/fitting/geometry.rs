//! Forebay geometry: VHA table and fitting bounds.

use crate::error::FphaFittingError;

/// Row of the VHA (Volume-Height-Area) table.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct VhaRow {
    pub volume_hm3: f64,
    pub height_m: f64,
    pub area_km2: f64,
}

/// Forebay height interpolation table.
///
/// Stores VHA rows sorted by ascending volume. `height(v)` performs linear
/// interpolation with clamp-not-extrapolate at boundaries.
#[derive(Debug, Clone)]
pub struct ForebayTable {
    rows: Vec<VhaRow>,
    hydro_name: String,
}

impl ForebayTable {
    /// Construct from VHA rows, sorting by volume and validating.
    pub fn new(rows: &[VhaRow], hydro_name: &str) -> Result<Self, FphaFittingError> {
        if rows.len() < 2 {
            return Err(FphaFittingError::InsufficientPoints {
                hydro_name: hydro_name.to_owned(),
                count: rows.len(),
            });
        }

        let mut sorted: Vec<VhaRow> = rows.to_vec();
        sorted.sort_by(|a, b| a.volume_hm3.total_cmp(&b.volume_hm3));

        // Check for duplicate volumes
        for w in sorted.windows(2) {
            if (w[0].volume_hm3 - w[1].volume_hm3).abs() < 1e-9 {
                return Err(FphaFittingError::DuplicateVolume {
                    hydro_name: hydro_name.to_owned(),
                    volume_hm3: w[0].volume_hm3,
                });
            }
        }

        Ok(Self {
            rows: sorted,
            hydro_name: hydro_name.to_owned(),
        })
    }

    /// Forebay height (m) at reservoir volume (hm³), clamped to table bounds.
    pub fn height(&self, volume_hm3: f64) -> f64 {
        let n = self.rows.len();
        let v_min = self.rows[0].volume_hm3;
        let v_max = self.rows[n - 1].volume_hm3;
        let v = volume_hm3.clamp(v_min, v_max);

        let upper = self.rows.partition_point(|r| r.volume_hm3 <= v);
        let hi = upper.min(n - 1).max(1);
        let lo = hi - 1;

        let v_lo = self.rows[lo].volume_hm3;
        let v_hi = self.rows[hi].volume_hm3;
        let h_lo = self.rows[lo].height_m;
        let h_hi = self.rows[hi].height_m;

        let span = v_hi - v_lo;
        if span <= 0.0 {
            h_lo
        } else {
            let t = (v - v_lo) / span;
            h_lo + t * (h_hi - h_lo)
        }
    }

    /// Minimum volume in the table (hm³).
    pub fn v_min(&self) -> f64 {
        self.rows[0].volume_hm3
    }

    /// Maximum volume in the table (hm³).
    pub fn v_max(&self) -> f64 {
        self.rows[self.rows.len() - 1].volume_hm3
    }

    /// Hydro plant name.
    pub fn hydro_name(&self) -> &str {
        &self.hydro_name
    }
}

/// Fitting bounds for the (V, Q, S) discretization grid.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct FittingBounds {
    pub v_min: f64,
    pub v_max: f64,
    pub n_volume_points: usize,
    pub n_flow_points: usize,
    pub n_spillage_points: usize,
    pub max_planes_per_hydro: usize,
    pub single_volume: bool,
}

impl Default for FittingBounds {
    fn default() -> Self {
        Self {
            v_min: 0.0,
            v_max: 0.0,
            n_volume_points: 10,
            n_flow_points: 10,
            n_spillage_points: 2,
            max_planes_per_hydro: 15,
            single_volume: false,
        }
    }
}

#[cfg(test)]
#[allow(clippy::unwrap_used, clippy::expect_used, clippy::float_cmp)]
mod tests {
    use super::*;

    fn test_rows() -> Vec<VhaRow> {
        vec![
            VhaRow { volume_hm3: 100.0, height_m: 850.0, area_km2: 10.0 },
            VhaRow { volume_hm3: 500.0, height_m: 880.0, area_km2: 20.0 },
            VhaRow { volume_hm3: 1000.0, height_m: 900.0, area_km2: 30.0 },
        ]
    }

    #[test]
    fn interpolates_interior_point() {
        let table = ForebayTable::new(&test_rows(), "Test").unwrap();
        let h = table.height(300.0);
        let expected = 850.0 + (300.0 - 100.0) / (500.0 - 100.0) * (880.0 - 850.0);
        assert!((h - expected).abs() < 1e-9);
    }

    #[test]
    fn clamps_below_minimum() {
        let table = ForebayTable::new(&test_rows(), "Test").unwrap();
        assert!((table.height(50.0) - 850.0).abs() < 1e-9);
    }

    #[test]
    fn clamps_above_maximum() {
        let table = ForebayTable::new(&test_rows(), "Test").unwrap();
        assert!((table.height(1500.0) - 900.0).abs() < 1e-9);
    }

    #[test]
    fn exact_table_point() {
        let table = ForebayTable::new(&test_rows(), "Test").unwrap();
        assert!((table.height(500.0) - 880.0).abs() < 1e-9);
    }

    #[test]
    fn rejects_duplicate_volumes() {
        let rows = vec![
            VhaRow { volume_hm3: 100.0, height_m: 850.0, area_km2: 10.0 },
            VhaRow { volume_hm3: 100.0, height_m: 855.0, area_km2: 11.0 },
        ];
        let result = ForebayTable::new(&rows, "Dup");
        assert!(matches!(result, Err(FphaFittingError::DuplicateVolume { .. })));
    }

    #[test]
    fn rejects_insufficient_points() {
        let rows = vec![VhaRow { volume_hm3: 100.0, height_m: 850.0, area_km2: 10.0 }];
        let result = ForebayTable::new(&rows, "Short");
        assert!(matches!(result, Err(FphaFittingError::InsufficientPoints { .. })));
    }

    #[test]
    fn sorts_unsorted_input() {
        let rows = vec![
            VhaRow { volume_hm3: 1000.0, height_m: 900.0, area_km2: 30.0 },
            VhaRow { volume_hm3: 100.0, height_m: 850.0, area_km2: 10.0 },
            VhaRow { volume_hm3: 500.0, height_m: 880.0, area_km2: 20.0 },
        ];
        let table = ForebayTable::new(&rows, "Unsorted").unwrap();
        assert!((table.height(300.0) - 865.0).abs() < 1e-9);
    }
}
