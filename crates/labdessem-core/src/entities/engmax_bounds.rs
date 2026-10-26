//! EngMax bounds storage for inter-stage updates.

use std::collections::HashMap;

/// Storage for dynamic maximum swallowing bounds by (plant, unit, period).
#[derive(Debug, Clone, Default)]
pub struct EngMaxBounds {
    bounds: HashMap<(usize, usize, usize), f64>,
}

impl EngMaxBounds {
    /// Create with capacity hint.
    #[must_use]
    pub fn new(capacity: usize) -> Self {
        Self {
            bounds: HashMap::with_capacity(capacity),
        }
    }

    /// Get bound for (plant, unit, period) in hm3.
    #[must_use]
    pub fn get(&self, plant: usize, unit: usize, period: usize) -> Option<f64> {
        self.bounds.get(&(plant, unit, period)).copied()
    }

    /// Set bound for (plant, unit, period) in hm3.
    pub fn set(&mut self, plant: usize, unit: usize, period: usize, value: f64) {
        self.bounds.insert((plant, unit, period), value);
    }

    /// Number of stored bounds.
    #[must_use]
    pub fn len(&self) -> usize {
        self.bounds.len()
    }

    /// Check if empty.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.bounds.is_empty()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn new_creates_with_capacity() {
        let bounds = EngMaxBounds::new(100);
        assert!(bounds.is_empty());
    }

    #[test]
    fn set_and_get_returns_value() {
        let mut bounds = EngMaxBounds::new(10);
        bounds.set(0, 0, 0, 50.5);
        assert_eq!(bounds.get(0, 0, 0), Some(50.5));
    }

    #[test]
    fn get_missing_key_returns_none() {
        let bounds = EngMaxBounds::new(10);
        assert_eq!(bounds.get(0, 0, 1), None);
    }

    #[test]
    fn len_tracks_entries() {
        let mut bounds = EngMaxBounds::new(10);
        assert_eq!(bounds.len(), 0);
        bounds.set(0, 0, 0, 50.0);
        assert_eq!(bounds.len(), 1);
        bounds.set(0, 0, 1, 60.0);
        assert_eq!(bounds.len(), 2);
    }
}
