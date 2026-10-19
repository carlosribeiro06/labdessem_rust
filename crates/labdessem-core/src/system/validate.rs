//! System validation logic.
//!
//! This module provides the entry point for system validation.
//! The actual validation logic is currently in `System::validate()`,
//! but this module serves as the public API for validation.

use super::System;
use crate::error::CoreError;

/// Validate all invariants of a constructed system.
///
/// This function validates:
/// - Study horizon consistency
/// - Unique IDs for all entity types
/// - Valid cross-references (bus_id, submarket_id, etc.)
/// - Hydro cascade consistency (upstream/downstream links)
/// - Array dimensions match horizon.periods
/// - Operational limit validity
///
/// # Errors
///
/// Returns [`CoreError`] describing the first validation failure.
pub fn validate_system(system: &System) -> Result<(), CoreError> {
    system.validate()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::entities::{Bus, Submarket};
    use crate::ids::{BusId, SubmarketId};
    use crate::system::StudyHorizon;

    fn minimal_horizon() -> StudyHorizon {
        StudyHorizon {
            periods: 2,
            period_duration_hours: 1.0,
            original_periods: 2,
            original_period_durations_hours: vec![1.0, 1.0],
            internal_to_original_period: vec![1, 2],
            internal_subperiod_index: vec![1, 1],
        }
    }

    fn valid_system() -> System {
        System {
            horizon: minimal_horizon(),
            thermal_unit_commitment_enabled: false,
            hydro_unit_commitment_enabled: false,
            ton_residual_enabled: false,
            fpha_enabled: true,
            future_cost_enabled: false,
            residual_costs: vec![],
            future_cost_cuts: vec![],
            submarkets: vec![Submarket {
                id: SubmarketId(1),
                name: "SE".into(),
                demand_mw: vec![100.0, 110.0],
            }],
            interchange_limits: vec![],
            operational_limits: vec![],
            buses: vec![Bus {
                id: BusId(1),
                name: "BUS-1".into(),
                submarket_id: SubmarketId(1),
                angle_reference: true,
                demand_mw: vec![100.0, 110.0],
            }],
            branches: vec![],
            thermal_plants: vec![],
            hydro_plants: vec![],
            pumping_plants: vec![],
            renewable_plants: vec![],
        }
    }

    #[test]
    fn validates_minimal_system() {
        let system = valid_system();
        assert!(validate_system(&system).is_ok());
    }

    #[test]
    fn rejects_empty_submarkets() {
        let mut system = valid_system();
        system.submarkets.clear();
        assert!(validate_system(&system).is_err());
    }

    #[test]
    fn rejects_empty_buses() {
        let mut system = valid_system();
        system.buses.clear();
        assert!(validate_system(&system).is_err());
    }

    #[test]
    fn rejects_dimension_mismatch() {
        let mut system = valid_system();
        system.submarkets[0].demand_mw = vec![100.0]; // Only 1 period, should be 2
        assert!(validate_system(&system).is_err());
    }
}
