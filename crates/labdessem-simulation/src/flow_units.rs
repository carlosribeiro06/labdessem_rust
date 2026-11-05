//! Unit conversions shared by the EngMax recalculation and the stage helpers (ADR-014).

/// Convert volume in hm³ to flow in m³/s given duration in hours.
pub(crate) fn volume_hm3_to_flow_m3s(volume_hm3: f64, duration_hours: f64) -> f64 {
    if duration_hours.abs() < 1e-12 {
        return 0.0;
    }
    volume_hm3 * 1_000_000.0 / (duration_hours * 3600.0)
}

#[cfg(test)]
mod tests {
    use super::volume_hm3_to_flow_m3s;

    #[test]
    fn volume_hm3_to_flow_m3s_converts_one_hour() {
        // 0.36 hm3 = 360 000 m3 over 1 h = 3600 s: 100 m3/s
        let flow = volume_hm3_to_flow_m3s(0.36, 1.0);
        assert!((flow - 100.0).abs() <= 1e-9 * 100.0, "got {flow}");
    }
}
