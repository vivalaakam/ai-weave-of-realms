//! Deterministic pseudo-random number generator based on the Keccak256 hash chain.
//!
//! [`SeededRng`] produces a deterministic sequence of random values from a
//! 32-byte seed.  Each call advances an internal position within the current
//! 32-byte hash state; when the state is exhausted it is rehashed in place.
//!
//! Helper functions [`keccak256`] and [`derive_seed`] allow constructing
//! child seeds for independent sub-systems (e.g. one seed per map chunk).

use serde::{Deserialize, Serialize};
use sha3::{Digest, Keccak256};

// ─── SeededRng ────────────────────────────────────────────────────────────────

/// A deterministic pseudo-random number generator backed by the Keccak256 hash.
///
/// The internal state is a 32-byte array that is rehashed every time all bytes
/// have been consumed.  Two instances created with the same seed will always
/// produce the same sequence of values.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SeededRng {
    state: [u8; 32],
    position: usize,
}

impl SeededRng {
    /// Creates a new [`SeededRng`] from a UTF-8 seed string.
    ///
    /// The string is hashed with Keccak256 to produce the initial 32-byte state.
    pub fn new(seed: &str) -> Self {
        let mut hasher = Keccak256::new();
        hasher.update(seed.as_bytes());
        let state: [u8; 32] = hasher.finalize().into();
        Self { state, position: 0 }
    }

    /// Derives a new [`SeededRng`] by mixing the current state with an
    /// additional seed string.
    ///
    /// Useful for spawning independent child generators (e.g. per-chunk RNGs).
    pub fn update(&self, seed: &str) -> Self {
        let mut hasher = Keccak256::new();
        hasher.update(self.state);
        hasher.update(seed.as_bytes());
        let new_state: [u8; 32] = hasher.finalize().into();
        Self {
            state: new_state,
            position: 0,
        }
    }

    /// Derives a child [`SeededRng`] bound to a specific hero.
    ///
    /// Given the same base state and `hero_id`, this always returns the same
    /// child generator — useful for giving each hero a reproducible personal RNG.
    pub fn derive_for_hero(&self, hero_id: u32) -> Self {
        self.update(&format!("hero_{hero_id}"))
    }

    /// Creates a [`SeededRng`] directly from a raw 32-byte seed.
    pub fn from_bytes(bytes: [u8; 32]) -> Self {
        Self {
            state: bytes,
            position: 0,
        }
    }

    /// Returns the current raw 32-byte state of the generator.
    ///
    /// Can be used to snapshot and restore the generator state.
    pub fn state(&self) -> [u8; 32] {
        self.state
    }

    // ── Internal helpers ──────────────────────────────────────────────────────

    /// Rehashes the internal state when all 32 bytes have been consumed.
    fn rehash(&mut self) {
        let mut hasher = Keccak256::new();
        hasher.update(self.state);
        self.state = hasher.finalize().into();
        self.position = 0;
    }

    /// Consumes and returns the next byte from the internal state.
    fn next_u8(&mut self) -> u8 {
        if self.position >= 32 {
            self.rehash();
        }
        let val = self.state[self.position];
        self.position += 1;
        val
    }

    /// Assembles the next four bytes into a `u32` (big-endian).
    fn next_u32(&mut self) -> u32 {
        let b0 = self.next_u8() as u32;
        let b1 = self.next_u8() as u32;
        let b2 = self.next_u8() as u32;
        let b3 = self.next_u8() as u32;
        (b0 << 24) | (b1 << 16) | (b2 << 8) | b3
    }

    /// Assembles the next eight bytes into a `u64`.
    fn next_u64(&mut self) -> u64 {
        let hi = self.next_u32() as u64;
        let lo = self.next_u32() as u64;
        (hi << 32) | lo
    }

    // ── Public sampling API ───────────────────────────────────────────────────

    /// Returns a random `f64` in the half-open range `[0.0, 1.0)`.
    pub fn next_f64(&mut self) -> f64 {
        (self.next_u64() as f64) / (u64::MAX as f64)
    }

    /// Returns a random `u32` in the half-open range `[range.start, range.end)`.
    ///
    /// Returns `range.start` if the range is empty.
    pub fn random_range_u32(&mut self, range: std::ops::Range<u32>) -> u32 {
        let len = range.end - range.start;
        if len == 0 {
            return range.start;
        }
        range.start + (self.next_u32() % len)
    }

    /// Returns a random `u8` in the half-open range `[range.start, range.end)`.
    ///
    /// Returns `range.start` if the range is empty.
    pub fn random_range_u8(&mut self, range: std::ops::Range<u8>) -> u8 {
        let len = range.end - range.start;
        if len == 0 {
            return range.start;
        }
        range.start + (self.next_u8() % len)
    }

    /// Returns a random `i32` in the half-open range `[range.start, range.end)`.
    ///
    /// Returns `range.start` if the range is empty.
    pub fn random_range_i32(&mut self, range: std::ops::Range<i32>) -> i32 {
        let len = (range.end - range.start) as u32;
        if len == 0 {
            return range.start;
        }
        range.start + (self.next_u32() % len) as i32
    }

    /// Returns a random `usize` in the half-open range `[range.start, range.end)`.
    ///
    /// Returns `range.start` if the range is empty.
    pub fn random_range_usize(&mut self, range: std::ops::Range<usize>) -> usize {
        let len = range.end - range.start;
        if len == 0 {
            return range.start;
        }
        range.start + (self.next_u64() as usize % len)
    }

    /// Returns a random `f64` in the half-open range `[range.start, range.end)`.
    pub fn random_range_f64(&mut self, range: std::ops::Range<f64>) -> f64 {
        let len = range.end - range.start;
        range.start + self.next_f64() * len
    }

    /// Returns a random `f64` in the closed range `[range.start, range.end]`.
    pub fn random_range_f64_inclusive(&mut self, range: std::ops::RangeInclusive<f64>) -> f64 {
        let start = *range.start();
        let end = *range.end();
        let len = end - start;
        start + self.next_f64() * len
    }

    /// Returns `true` with the given `probability` (a value in `[0.0, 1.0]`).
    ///
    /// A `probability` of `1.0` always returns `true`; `0.0` always returns `false`.
    pub fn random_bool(&mut self, probability: f64) -> bool {
        self.next_f64() < probability
    }
}

// ─── Free functions ───────────────────────────────────────────────────────────

/// Hashes a UTF-8 string with Keccak256 and returns the 32-byte digest.
///
/// Useful for producing a deterministic seed from a human-readable phrase.
pub fn keccak256(seed: &str) -> [u8; 32] {
    let mut hasher = Keccak256::new();
    hasher.update(seed.as_bytes());
    hasher.finalize().into()
}

/// Derives a child seed by hashing a parent seed together with arbitrary context bytes.
///
/// # Arguments
/// * `base`    - Parent 32-byte seed (e.g. map seed).
/// * `context` - Distinguishing context (e.g. `b"chunk_3_7"`).
///
/// # Returns
/// A new 32-byte seed that is deterministically unique for this `(base, context)` pair.
pub fn derive_seed(base: &[u8; 32], context: &[u8]) -> [u8; 32] {
    let mut hasher = Keccak256::new();
    hasher.update(base);
    hasher.update(context);
    hasher.finalize().into()
}

// ─── Tests ────────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    fn init_tracing() {
        let _ = tracing_subscriber::fmt().with_test_writer().try_init();
    }

    #[test]
    fn same_seed_produces_same_sequence() {
        init_tracing();
        let mut a = SeededRng::new("test-seed");
        let mut b = SeededRng::new("test-seed");
        for _ in 0..100 {
            assert_eq!(a.next_f64(), b.next_f64());
        }
    }

    #[test]
    fn different_seeds_produce_different_sequences() {
        init_tracing();
        let mut a = SeededRng::new("seed-a");
        let mut b = SeededRng::new("seed-b");
        let values_a: Vec<f64> = (0..10).map(|_| a.next_f64()).collect();
        let values_b: Vec<f64> = (0..10).map(|_| b.next_f64()).collect();
        assert_ne!(values_a, values_b);
    }

    #[test]
    fn random_range_u32_stays_in_range() {
        init_tracing();
        let mut rng = SeededRng::new("range-test");
        for _ in 0..1000 {
            let v = rng.random_range_u32(10..20);
            assert!((10..20).contains(&v), "value {v} out of range 10..20");
        }
    }

    #[test]
    fn derive_seed_is_deterministic() {
        init_tracing();
        let base = keccak256("map-seed");
        let a = derive_seed(&base, b"chunk_0_0");
        let b = derive_seed(&base, b"chunk_0_0");
        assert_eq!(a, b);
    }

    #[test]
    fn derive_seed_differs_by_context() {
        init_tracing();
        let base = keccak256("map-seed");
        let a = derive_seed(&base, b"chunk_0_0");
        let b = derive_seed(&base, b"chunk_0_1");
        assert_ne!(a, b);
    }

    #[test]
    fn rehash_occurs_after_32_bytes() {
        init_tracing();
        let seed = keccak256("rehash-test");
        let mut rng = SeededRng::from_bytes(seed);
        // Consume all 32 bytes + a few more to trigger rehash
        for _ in 0..40 {
            let _ = rng.random_range_u8(0..255);
        }
    }
}
