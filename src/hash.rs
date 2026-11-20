//! Step 1: hashing.
//!
//! Every routing method needs the same building block: a function that turns
//! a key (or a shard name) into a number between 0 and 2^32 - 1. We treat that
//! number as a position on a circle (the "ring").
//!
//! Requirements for the hash:
//! - Deterministic: the same key always gives the same number, on every
//!   machine and every run. (Rust's built-in `HashMap` hasher is randomly
//!   seeded per process, so we can't use it for routing.)
//! - Well spread: similar keys like "user1" and "user2" must land far apart,
//!   otherwise they would all pile onto the same shard.
//!
//! We use FNV-1a (simple, fast) followed by the MurmurHash3 finalizer
//! (`fmix32`), which scrambles the bits so the output is evenly spread.

/// A position on the ring. The ring has 2^32 positions: 0 ..= u32::MAX.
pub type Position = u32;

/// Total number of positions on the ring (2^32). Stored as u64 because it
/// does not fit in a u32.
pub const RING_SIZE: u64 = 1 << 32;

/// FNV-1a, 32-bit version.
///
/// For each byte: XOR it into the hash, then multiply by the FNV prime.
/// `wrapping_mul` lets the multiplication overflow and wrap around instead of
/// panicking, which is exactly what the algorithm expects.
fn fnv1a_32(bytes: &[u8]) -> u32 {
    const OFFSET_BASIS: u32 = 0x811c_9dc5;
    const PRIME: u32 = 0x0100_0193;

    let mut h = OFFSET_BASIS;
    for &b in bytes {
        h ^= b as u32;
        h = h.wrapping_mul(PRIME);
    }
    h
}

/// MurmurHash3's 32-bit finalizer ("avalanche" step).
///
/// FNV-1a alone leaves similar inputs with similar high bits. This mixes every
/// input bit into every output bit, so flipping one input bit changes about
/// half of the output bits.
pub fn fmix32(mut h: u32) -> u32 {
    h ^= h >> 16;
    h = h.wrapping_mul(0x85eb_ca6b);
    h ^= h >> 13;
    h = h.wrapping_mul(0xc2b2_ae35);
    h ^= h >> 16;
    h
}

/// Hash any key to a ring position.
///
/// `impl AsRef<[u8]>` means callers can pass `&str`, `String`, `&[u8]` or
/// `Vec<u8>` without converting first.
pub fn hash_key(key: impl AsRef<[u8]>) -> Position {
    fmix32(fnv1a_32(key.as_ref()))
}

/// The i-th probe position for an already-hashed key (used in step 5).
///
/// Probe 0 is the key's own hash, so multi-probe with k = 1 behaves exactly
/// like the classic ring. Later probes add a multiple of the golden-ratio
/// constant and re-mix, giving k independent-looking positions per key.
pub fn probe(key_hash: Position, i: u32) -> Position {
    if i == 0 {
        key_hash
    } else {
        fmix32(key_hash.wrapping_add(i.wrapping_mul(0x9e37_79b9)))
    }
}

/// Clockwise distance from `from` to `to` on the ring.
///
/// If `to` is ahead of `from`, it's just `to - from`. If `to` is behind,
/// we have to go past the top of the ring and wrap around to 0.
/// u32 wrapping subtraction does exactly that for free.
pub fn clockwise_distance(from: Position, to: Position) -> u32 {
    to.wrapping_sub(from)
}

/// Express a position as a percentage of the way around the ring.
/// Handy for printing and for comparing with the web simulation.
pub fn ring_percent(pos: Position) -> f64 {
    pos as f64 / RING_SIZE as f64 * 100.0
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn same_key_same_hash() {
        assert_eq!(hash_key("user123"), hash_key("user123"));
        assert_eq!(hash_key("user123"), hash_key(String::from("user123")));
    }

    #[test]
    fn similar_keys_land_far_apart() {
        let a = hash_key("user1");
        let b = hash_key("user2");
        // At least ~1% of the ring apart. Without fmix32 they would be close.
        assert!(clockwise_distance(a, b).min(clockwise_distance(b, a)) > (RING_SIZE / 100) as u32);
    }

    #[test]
    fn hashes_are_evenly_spread() {
        // Drop 100,000 keys into 16 equal slices of the ring.
        // Each slice should get close to 100,000 / 16 = 6,250.
        let mut buckets = [0u32; 16];
        for i in 0..100_000 {
            let pos = hash_key(format!("user{i}"));
            buckets[(pos >> 28) as usize] += 1; // top 4 bits pick 1 of 16 slices
        }
        for count in buckets {
            assert!((5_900..=6_600).contains(&count), "uneven bucket: {count}");
        }
    }

    #[test]
    fn probe_zero_is_the_key_hash() {
        let h = hash_key("user123");
        assert_eq!(probe(h, 0), h);
        assert_ne!(probe(h, 1), h);
        assert_ne!(probe(h, 1), probe(h, 2));
    }

    #[test]
    fn distance_wraps_around_the_top() {
        assert_eq!(clockwise_distance(10, 30), 20);
        // From near the end of the ring, past u32::MAX, to position 5.
        assert_eq!(clockwise_distance(u32::MAX - 4, 5), 10);
    }
}
