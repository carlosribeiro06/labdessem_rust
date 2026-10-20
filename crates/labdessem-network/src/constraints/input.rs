//! Input types for flow constraint generation.
//!
//! These types decouple the network constraint builder from simulation-specific
//! data structures, enabling a clean boundary between crates.

use labdessem_core::ids::BusId;

/// Entry for a generator connected to a bus.
#[derive(Debug, Clone, PartialEq)]
pub struct GeneratorEntry {
    pub bus_id: BusId,
    pub variable_name: String,
}

impl GeneratorEntry {
    #[must_use]
    pub fn new(bus_id: BusId, variable_name: impl Into<String>) -> Self {
        Self {
            bus_id,
            variable_name: variable_name.into(),
        }
    }
}

/// Entry for a pumping plant with consumption factor.
#[derive(Debug, Clone, PartialEq)]
pub struct PumpingEntry {
    pub bus_id: BusId,
    pub variable_name: String,
    pub specific_consumption_mw_per_m3s: f64,
}

impl PumpingEntry {
    #[must_use]
    pub fn new(
        bus_id: BusId,
        variable_name: impl Into<String>,
        specific_consumption_mw_per_m3s: f64,
    ) -> Self {
        Self {
            bus_id,
            variable_name: variable_name.into(),
            specific_consumption_mw_per_m3s,
        }
    }
}

/// Input data for building flow cut constraints.
///
/// Gathers all generator and pumping entries needed to construct
/// the linearized flow constraint for a given period.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct FlowCutInput {
    pub thermal_entries: Vec<GeneratorEntry>,
    pub hydro_entries: Vec<GeneratorEntry>,
    pub renewable_entries: Vec<GeneratorEntry>,
    pub pumping_entries: Vec<PumpingEntry>,
    pub flow_conversion: f64,
}

impl FlowCutInput {
    #[must_use]
    pub fn new(flow_conversion: f64) -> Self {
        Self {
            flow_conversion,
            ..Default::default()
        }
    }

    pub fn add_thermal(&mut self, entry: GeneratorEntry) {
        self.thermal_entries.push(entry);
    }

    pub fn add_hydro(&mut self, entry: GeneratorEntry) {
        self.hydro_entries.push(entry);
    }

    pub fn add_renewable(&mut self, entry: GeneratorEntry) {
        self.renewable_entries.push(entry);
    }

    pub fn add_pumping(&mut self, entry: PumpingEntry) {
        self.pumping_entries.push(entry);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_generator_entry_construction() {
        let entry = GeneratorEntry::new(BusId(1), "gt_1_gen");
        assert_eq!(entry.bus_id, BusId(1));
        assert_eq!(entry.variable_name, "gt_1_gen");
    }

    #[test]
    fn test_pumping_entry_construction() {
        let entry = PumpingEntry::new(BusId(2), "pump_1", 1.5);
        assert_eq!(entry.bus_id, BusId(2));
        assert_eq!(entry.variable_name, "pump_1");
        assert!((entry.specific_consumption_mw_per_m3s - 1.5).abs() < 1e-9);
    }

    #[test]
    fn test_flow_cut_input_building() {
        let mut input = FlowCutInput::new(0.0036);
        input.add_thermal(GeneratorEntry::new(BusId(1), "th_1"));
        input.add_hydro(GeneratorEntry::new(BusId(2), "hy_1"));
        input.add_renewable(GeneratorEntry::new(BusId(3), "ren_1"));
        input.add_pumping(PumpingEntry::new(BusId(4), "pump_1", 1.0));

        assert_eq!(input.thermal_entries.len(), 1);
        assert_eq!(input.hydro_entries.len(), 1);
        assert_eq!(input.renewable_entries.len(), 1);
        assert_eq!(input.pumping_entries.len(), 1);
        assert!((input.flow_conversion - 0.0036).abs() < 1e-9);
    }
}
