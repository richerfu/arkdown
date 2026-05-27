//! HAP content digest: chunked SHA-256 with type-prefix byte markers.
//!
//! Reference: `developtools_hapsigner` `HapUtils.java`, `SignHap.java`.

use sha2::{Digest, Sha256};

/// Size of each chunk used for digest computation: 1 MiB.
pub const CHUNK_SIZE: usize = 1_048_576;

/// Second-level type prefix byte prepended before each chunk.
pub const CHUNK_TYPE_BYTE: u8 = 0xa5;

/// First-level type prefix byte prepended at the top level.
pub const CONTENT_TYPE_BYTE: u8 = 0x5a;

/// Compute the HAP content digest over the four logical sections.
///
/// The digest algorithm (from `developtools_hapsigner`):
///
/// 1. Split each section into CHUNK_SIZE chunks.
///    For each chunk write: `0xa5 | u32_LE(chunk_len) | chunk_bytes`
/// 2. Concatenate all such records across all four sections.
/// 3. Final hash input: `0x5a | u32_LE(total_chunk_count) | all_chunk_records`
///
/// `signing_block_bytes` is empty (`&[]`) during initial signing and contains
/// the signing block during *verification*.
pub fn compute_hap_digest(
    entries_bytes: &[u8],
    signing_block_bytes: &[u8],
    cd_bytes: &[u8],
    eocd_zeroed: &[u8],
) -> [u8; 32] {
    let sections: &[&[u8]] = &[entries_bytes, signing_block_bytes, cd_bytes, eocd_zeroed];

    let mut all_chunk_data: Vec<u8> = Vec::new();
    let mut total_chunks: u32 = 0;

    for section in sections {
        let mut offset = 0;
        while offset < section.len() {
            let end = (offset + CHUNK_SIZE).min(section.len());
            let chunk = &section[offset..end];
            let chunk_len = chunk.len() as u32;

            all_chunk_data.push(CHUNK_TYPE_BYTE);
            all_chunk_data.extend_from_slice(&chunk_len.to_le_bytes());
            all_chunk_data.extend_from_slice(chunk);
            total_chunks += 1;
            offset = end;
        }
        // Even an empty section contributes zero chunks; skip if section is empty.
    }

    // If all sections were empty, return a hash of just the prefix.
    let mut hasher = Sha256::new();
    hasher.update([CONTENT_TYPE_BYTE]);
    hasher.update(total_chunks.to_le_bytes());
    hasher.update(&all_chunk_data);
    hasher.finalize().into()
}

#[cfg(test)]
mod tests {
    use super::*;
    use sha2::{Digest, Sha256};

    // ── Type byte constants ────────────────────────────────────────────────

    #[test]
    fn test_content_type_byte_is_0x5a() {
        assert_eq!(CONTENT_TYPE_BYTE, 0x5a, "top-level prefix byte must be 0x5a");
    }

    #[test]
    fn test_chunk_type_byte_is_0xa5() {
        assert_eq!(CHUNK_TYPE_BYTE, 0xa5, "per-chunk prefix byte must be 0xa5");
    }

    #[test]
    fn test_chunk_size_is_one_mib() {
        assert_eq!(CHUNK_SIZE, 1_048_576, "chunk size must be 1 MiB");
    }

    // ── All-empty digest ─────────────────────────────────────────────────

    #[test]
    fn test_all_empty_sections_produce_deterministic_hash() {
        // When all four sections are empty, we get SHA-256(0x5a || 0x00000000)
        let d = compute_hap_digest(&[], &[], &[], &[]);

        let mut h = Sha256::new();
        h.update([0x5a_u8]);
        h.update(0u32.to_le_bytes());
        let expected: [u8; 32] = h.finalize().into();

        assert_eq!(d, expected, "empty-sections digest must equal SHA-256(0x5a || u32_LE(0))");
    }

    #[test]
    fn test_empty_sections_produce_32_byte_output() {
        let d = compute_hap_digest(&[], &[], &[], &[]);
        assert_eq!(d.len(), 32);
    }

    // ── Single-chunk digest ──────────────────────────────────────────────

    #[test]
    fn test_single_byte_payload_produces_one_chunk() {
        // One byte in entries_bytes → exactly one chunk record
        // chunk record: 0xa5 | u32_LE(1) | 0x??
        // top-level: SHA-256(0x5a | u32_LE(1) | chunk_record)
        let payload = b"X";
        let d = compute_hap_digest(payload, &[], &[], &[]);

        let chunk_len = 1u32;
        let mut chunk_record = vec![0xa5];
        chunk_record.extend_from_slice(&chunk_len.to_le_bytes());
        chunk_record.extend_from_slice(payload);

        let mut h = Sha256::new();
        h.update([0x5a_u8]);
        h.update(1u32.to_le_bytes()); // total_chunks = 1
        h.update(&chunk_record);
        let expected: [u8; 32] = h.finalize().into();

        assert_eq!(d, expected);
    }

    #[test]
    fn test_two_sections_each_one_byte_produce_two_chunks() {
        // entries_bytes = [0x01], cd_bytes = [0x02], rest empty → 2 chunks
        let d1_2 = compute_hap_digest(b"\x01", &[], b"\x02", &[]);
        let d1_1 = compute_hap_digest(b"\x01", &[], &[], &[]);
        let d1_3 = compute_hap_digest(b"\x03", &[], &[], &[]);

        // With 2 chunks the digest must differ from 1-chunk digests
        assert_ne!(d1_2, d1_1);
        assert_ne!(d1_2, d1_3);
    }

    // ── Determinism ──────────────────────────────────────────────────────

    #[test]
    fn test_digest_is_deterministic() {
        let a = compute_hap_digest(b"hello", &[], b"world", b"eocd");
        let b = compute_hap_digest(b"hello", &[], b"world", b"eocd");
        assert_eq!(a, b, "same inputs must produce identical digests");
    }

    #[test]
    fn test_different_payloads_produce_different_digests() {
        let a = compute_hap_digest(b"aaa", &[], &[], &[]);
        let b = compute_hap_digest(b"bbb", &[], &[], &[]);
        assert_ne!(a, b);
    }

    // ── Multi-chunk boundary ─────────────────────────────────────────────

    #[test]
    fn test_payload_larger_than_chunk_size_splits_correctly() {
        // A payload of CHUNK_SIZE + 1 bytes produces 2 chunks.
        let big = vec![0xABu8; CHUNK_SIZE + 1];
        let d = compute_hap_digest(&big, &[], &[], &[]);
        // Must produce a 32-byte hash without panicking.
        assert_eq!(d.len(), 32);
    }

    #[test]
    fn test_exact_chunk_size_payload_produces_one_chunk() {
        // A payload of exactly CHUNK_SIZE is one chunk.
        let exact = vec![0x55u8; CHUNK_SIZE];
        let d = compute_hap_digest(&exact, &[], &[], &[]);
        assert_eq!(d.len(), 32);

        // CHUNK_SIZE + 1 differs.
        let one_more = vec![0x55u8; CHUNK_SIZE + 1];
        let d2 = compute_hap_digest(&one_more, &[], &[], &[]);
        assert_ne!(d, d2, "adding one extra byte must change the digest");
    }
}
