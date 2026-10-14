//! Internal helper functions for system validation.

use std::collections::HashSet;

use crate::{
    entities::Bus,
    error::CoreError,
    ids::{BusId, SubmarketId},
};

pub(super) fn validate_unique_ids(
    ids: impl IntoIterator<Item = usize>,
    message: &str,
) -> Result<(), CoreError> {
    let mut seen = HashSet::new();
    for id in ids {
        if !seen.insert(id) {
            return Err(CoreError::validation(message));
        }
    }

    Ok(())
}

pub(super) fn validate_bus_and_submarket_membership(
    entity_id: usize,
    entity_name: &str,
    bus_id: BusId,
    submarket_id: SubmarketId,
    buses: &[Bus],
    bus_ids: &HashSet<BusId>,
    submarket_ids: &HashSet<SubmarketId>,
) -> Result<(), CoreError> {
    if !bus_ids.contains(&bus_id) {
        return Err(CoreError::validation(format!(
            "{entity_name} {entity_id} references unknown bus {bus_id:?}"
        )));
    }
    if !submarket_ids.contains(&submarket_id) {
        return Err(CoreError::validation(format!(
            "{entity_name} {entity_id} references unknown submarket {submarket_id:?}"
        )));
    }

    // Rationale: expect() is safe because bus existence was checked by contains() above
    #[allow(clippy::expect_used)]
    let bus = buses
        .iter()
        .find(|bus| bus.id == bus_id)
        .expect("bus id should exist after membership check");

    if bus.submarket_id != submarket_id {
        return Err(CoreError::validation(format!(
            "{entity_name} {entity_id} has inconsistent bus/submarket mapping"
        )));
    }

    Ok(())
}
