//! Production function φ(v, q, s) = K · η · q · h_net.

use super::geometry::ForebayTable;
use crate::types::TailraceModel;

/// Gravitational constant for power calculation.
const K_GRAVITY: f64 = 0.00981; // MW / (m³/s · m)

/// Source of tailrace level for production evaluation.
#[derive(Debug, Clone)]
pub enum TailraceSource {
    /// Use entity-level tailrace model.
    Entity(Option<TailraceModel>),
    /// Use fitted families (placeholder for backwater coupling).
    Families,
}

/// Evaluable production function for one hydro plant.
///
/// Computes generation = K · η · q · (forebay - tailrace - losses)
/// capped at installed capacity.
#[derive(Debug, Clone)]
pub struct ProductionFunction {
    forebay: ForebayTable,
    tailrace: TailraceSource,
    hydraulic_losses_m: f64,
    efficiency: f64,
    max_turbined_m3s: f64,
    max_generation_mw: f64,
    hydro_name: String,
}

impl ProductionFunction {
    /// Create a new production function.
    pub fn new(
        forebay: ForebayTable,
        tailrace: TailraceSource,
        hydraulic_losses_m: Option<f64>,
        efficiency: Option<f64>,
        max_turbined_m3s: f64,
        max_generation_mw: f64,
        hydro_name: String,
    ) -> Self {
        Self {
            forebay,
            tailrace,
            hydraulic_losses_m: hydraulic_losses_m.unwrap_or(0.0),
            efficiency: efficiency.unwrap_or(1.0),
            max_turbined_m3s,
            max_generation_mw,
            hydro_name,
        }
    }

    /// Net head (m) at volume and outflow.
    pub fn net_head(&self, volume_hm3: f64, outflow_m3s: f64) -> f64 {
        let forebay_height = self.forebay.height(volume_hm3);
        let tailrace_level = self.tailrace_level(outflow_m3s);
        let gross_head = forebay_height - tailrace_level;
        (gross_head - self.hydraulic_losses_m).max(0.0)
    }

    /// Tailrace level (m) at outflow.
    fn tailrace_level(&self, outflow_m3s: f64) -> f64 {
        match &self.tailrace {
            TailraceSource::Entity(Some(model)) => model.evaluate(outflow_m3s),
            TailraceSource::Entity(None) => 0.0,
            TailraceSource::Families => 0.0, // Placeholder
        }
    }

    /// Evaluate generation (MW) without capacity cap.
    pub fn evaluate(&self, volume_hm3: f64, turbined_m3s: f64, spillage_m3s: f64) -> f64 {
        let q = turbined_m3s.clamp(0.0, self.max_turbined_m3s);
        let outflow = q + spillage_m3s;
        let h_net = self.net_head(volume_hm3, outflow);
        K_GRAVITY * self.efficiency * q * h_net
    }

    /// Evaluate generation (MW) capped at installed capacity.
    pub fn evaluate_capped(&self, volume_hm3: f64, turbined_m3s: f64, spillage_m3s: f64) -> f64 {
        self.evaluate(volume_hm3, turbined_m3s, spillage_m3s)
            .min(self.max_generation_mw)
    }

    /// Override max generation (for testing capacity ceiling).
    pub fn with_max_generation_mw(mut self, mw: f64) -> Self {
        self.max_generation_mw = mw;
        self
    }

    /// Maximum turbined flow (m³/s).
    pub fn max_turbined_m3s(&self) -> f64 {
        self.max_turbined_m3s
    }

    /// Maximum generation (MW).
    pub fn max_generation_mw(&self) -> f64 {
        self.max_generation_mw
    }

    /// Hydro plant name.
    pub fn hydro_name(&self) -> &str {
        &self.hydro_name
    }

    /// Reference to the forebay table.
    pub fn forebay(&self) -> &ForebayTable {
        &self.forebay
    }
}

#[cfg(test)]
#[allow(clippy::unwrap_used, clippy::expect_used, clippy::float_cmp)]
mod tests {
    use super::*;
    use crate::fitting::geometry::VhaRow;

    fn simple_forebay() -> ForebayTable {
        let rows = vec![
            VhaRow { volume_hm3: 0.0, height_m: 380.0, area_km2: 0.0 },
            VhaRow { volume_hm3: 30_000.0, height_m: 410.0, area_km2: 0.0 },
        ];
        ForebayTable::new(&rows, "Simple").unwrap()
    }

    fn constant_tailrace() -> TailraceSource {
        TailraceSource::Entity(Some(TailraceModel::Constant { level_m: 300.0 }))
    }

    fn polynomial_tailrace() -> TailraceSource {
        TailraceSource::Entity(Some(TailraceModel::Polynomial {
            coefficients: vec![300.0, 0.001, -1e-8],
        }))
    }

    #[test]
    fn evaluate_mid_volume_mid_flow() {
        let pf = ProductionFunction::new(
            simple_forebay(),
            constant_tailrace(),
            Some(2.0),
            Some(0.92),
            3000.0,
            5000.0,
            "Test".to_owned(),
        );

        let g = pf.evaluate(15000.0, 1500.0, 0.0);

        // Forebay: 380 + (15000/30000) * 30 = 395 m
        // Net head: 395 - 300 - 2 = 93 m
        // Generation: 0.00981 * 0.92 * 1500 * 93 ≈ 1258.9 MW
        let expected = K_GRAVITY * 0.92 * 1500.0 * 93.0;
        assert!((g - expected).abs() < 1.0);
    }

    #[test]
    fn evaluate_capped_at_max_generation() {
        let pf = ProductionFunction::new(
            simple_forebay(),
            constant_tailrace(),
            Some(2.0),
            Some(0.92),
            3000.0,
            1000.0, // Low cap
            "Capped".to_owned(),
        );

        let g = pf.evaluate_capped(15000.0, 1500.0, 0.0);
        assert!((g - 1000.0).abs() < 1e-9);
    }

    #[test]
    fn polynomial_tailrace_increases_with_outflow() {
        let pf = ProductionFunction::new(
            simple_forebay(),
            polynomial_tailrace(),
            None,
            None,
            3000.0,
            5000.0,
            "Poly".to_owned(),
        );

        let g_low_q = pf.evaluate(15000.0, 500.0, 0.0);
        let g_high_q = pf.evaluate(15000.0, 2000.0, 0.0);

        // Higher turbining means higher tailrace, lower net head, so
        // generation per m³/s decreases (but total may still increase).
        let efficiency_low = g_low_q / 500.0;
        let efficiency_high = g_high_q / 2000.0;
        assert!(efficiency_low > efficiency_high);
    }

    #[test]
    fn zero_turbining_yields_zero_generation() {
        let pf = ProductionFunction::new(
            simple_forebay(),
            constant_tailrace(),
            Some(2.0),
            Some(0.92),
            3000.0,
            5000.0,
            "Zero".to_owned(),
        );

        let g = pf.evaluate(15000.0, 0.0, 0.0);
        assert!((g - 0.0).abs() < 1e-9);
    }

    #[test]
    fn spillage_affects_tailrace() {
        let pf = ProductionFunction::new(
            simple_forebay(),
            polynomial_tailrace(),
            None,
            None,
            3000.0,
            5000.0,
            "Spill".to_owned(),
        );

        let g_no_spill = pf.evaluate(15000.0, 1000.0, 0.0);
        let g_with_spill = pf.evaluate(15000.0, 1000.0, 500.0);

        // Spillage increases outflow, raises tailrace, reduces net head
        assert!(g_no_spill > g_with_spill);
    }

    #[test]
    fn net_head_never_negative() {
        // Create scenario where tailrace > forebay
        let rows = vec![
            VhaRow { volume_hm3: 0.0, height_m: 100.0, area_km2: 0.0 },
            VhaRow { volume_hm3: 1000.0, height_m: 110.0, area_km2: 0.0 },
        ];
        let forebay = ForebayTable::new(&rows, "Low").unwrap();
        let tailrace = TailraceSource::Entity(Some(TailraceModel::Constant { level_m: 150.0 }));

        let pf = ProductionFunction::new(
            forebay,
            tailrace,
            Some(5.0),
            Some(1.0),
            1000.0,
            500.0,
            "NegHead".to_owned(),
        );

        let h = pf.net_head(500.0, 500.0);
        assert!(h >= 0.0);
    }
}
