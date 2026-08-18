//! Shared hash primitives.
//!
//! Single home for the FNV-1a 64-bit hash used by cache keys, fingerprints,
//! router-map hashes, and obfuscation file hashes across the workspace.
//! Every site must fold through [`fnv1a64`] instead of inlining the offset
//! basis / prime constants, so the algorithm cannot drift between crates.

/// FNV-1a 64-bit hash of a byte slice.
///
/// Hashing a sequence of chunks with `fold` is equivalent to hashing their
/// concatenation, which is what multi-file / multi-field callers rely on.
pub fn fnv1a64(bytes: &[u8]) -> u64 {
    fnv1a64_fold(0xcbf29ce484222325, bytes)
}

/// Continue an FNV-1a hash from `state` over `bytes`.
///
/// `state` must originate from [`fnv1a64`] (or the offset basis); callers
/// must not serialise or truncate it between folds.
pub fn fnv1a64_fold(mut state: u64, bytes: &[u8]) -> u64 {
    for b in bytes {
        state ^= *b as u64;
        state = state.wrapping_mul(0x100000001b3);
    }
    state
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn fnv1a64_matches_reference_vectors() {
        // Reference vectors from the FNV-1a 64-bit test suite.
        assert_eq!(fnv1a64(b""), 0xcbf29ce484222325);
        assert_eq!(fnv1a64(b"a"), 0xaf63dc4c8601ec8c);
        assert_eq!(fnv1a64(b"foobar"), 0x85944171f73967e8);
    }

    #[test]
    fn fold_equals_concatenation() {
        let whole = fnv1a64(b"route_name/src_path");
        let folded = fnv1a64_fold(fnv1a64(b"route_name"), b"/src_path");
        assert_eq!(whole, folded);
    }
}
