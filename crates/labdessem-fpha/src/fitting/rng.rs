//! Deterministic PRNG for FPHA plane reduction.

/// FNV-1a 64-bit hash.
pub fn fnv1a64(data: &[u8]) -> u64 {
    const FNV_OFFSET: u64 = 0xcbf29ce484222325;
    const FNV_PRIME: u64 = 0x00000100000001B3;
    let mut hash = FNV_OFFSET;
    for &byte in data {
        hash ^= u64::from(byte);
        hash = hash.wrapping_mul(FNV_PRIME);
    }
    hash
}

/// SplitMix64 PRNG for deterministic sampling.
pub struct SplitMix64 {
    state: u64,
}

impl SplitMix64 {
    pub fn new(seed: u64) -> Self {
        Self { state: seed }
    }

    pub fn next_u64(&mut self) -> u64 {
        self.state = self.state.wrapping_add(0x9e3779b97f4a7c15);
        let mut z = self.state;
        z = (z ^ (z >> 30)).wrapping_mul(0xbf58476d1ce4e5b9);
        z = (z ^ (z >> 27)).wrapping_mul(0x94d049bb133111eb);
        z ^ (z >> 31)
    }

    /// Returns a uniform f64 in [0, 1).
    pub fn next_unit_f64(&mut self) -> f64 {
        (self.next_u64() >> 11) as f64 / ((1_u64 << 53) as f64)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn splitmix64_is_deterministic() {
        let mut rng1 = SplitMix64::new(12345);
        let mut rng2 = SplitMix64::new(12345);
        for _ in 0..100 {
            assert_eq!(rng1.next_u64(), rng2.next_u64());
        }
    }

    #[test]
    fn unit_f64_is_in_range() {
        let mut rng = SplitMix64::new(42);
        for _ in 0..1000 {
            let x = rng.next_unit_f64();
            assert!((0.0..1.0).contains(&x));
        }
    }

    #[test]
    fn fnv1a64_different_inputs_different_hashes() {
        let h1 = fnv1a64(b"hello");
        let h2 = fnv1a64(b"world");
        assert_ne!(h1, h2);
    }
}
