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

/// Streaming HAP content digest computer.
///
/// The caller supplies the logical section lengths up front, matching
/// `developtools_hapsigner`'s first-level chunk count, then feeds each section
/// in order. Pieces inside a section are chunked as if they were one contiguous
/// byte slice, so ZIP entries can be hashed without first concatenating all
/// local headers and file data into a second full-size buffer.
pub struct HapDigestComputer {
    hasher: Sha256,
}

impl HapDigestComputer {
    pub fn new(section_lengths: &[usize]) -> Self {
        let total_chunks = section_lengths
            .iter()
            .map(|section_len| section_len.div_ceil(CHUNK_SIZE) as u32)
            .sum::<u32>();

        let mut hasher = Sha256::new();
        hasher.update([CONTENT_TYPE_BYTE]);
        hasher.update(total_chunks.to_le_bytes());

        Self { hasher }
    }

    pub fn update_section(&mut self, section: &[u8]) {
        self.update_section_pieces(section.len(), |push| push(section));
    }

    pub fn update_section_pieces<F>(&mut self, expected_len: usize, feed: F)
    where
        F: FnOnce(&mut dyn FnMut(&[u8])),
    {
        self.try_update_section_pieces(expected_len, |push| {
            feed(push);
            Ok::<(), ()>(())
        })
        .expect("infallible HAP digest section feeder failed");
    }

    pub fn try_update_section_pieces<F, E>(&mut self, expected_len: usize, feed: F) -> Result<(), E>
    where
        F: FnOnce(&mut dyn FnMut(&[u8])) -> Result<(), E>,
    {
        let mut observed_len = 0usize;
        let mut pending = Vec::with_capacity(CHUNK_SIZE);

        {
            let hasher = &mut self.hasher;
            let mut push = |mut piece: &[u8]| {
                observed_len += piece.len();

                while !piece.is_empty() {
                    if pending.is_empty() && piece.len() >= CHUNK_SIZE {
                        let (chunk, rest) = piece.split_at(CHUNK_SIZE);
                        Self::update_chunk(hasher, chunk);
                        piece = rest;
                        continue;
                    }

                    let available = CHUNK_SIZE - pending.len();
                    let take = available.min(piece.len());
                    pending.extend_from_slice(&piece[..take]);
                    piece = &piece[take..];

                    if pending.len() == CHUNK_SIZE {
                        Self::update_chunk(hasher, &pending);
                        pending.clear();
                    }
                }
            };

            feed(&mut push)?;

            if !pending.is_empty() {
                Self::update_chunk(hasher, &pending);
            }
        }

        debug_assert_eq!(
            observed_len, expected_len,
            "HAP digest section feeder length mismatch"
        );
        Ok(())
    }

    pub fn finalize(self) -> [u8; 32] {
        self.hasher.finalize().into()
    }

    /// Append a HAP optional signing block value to the top-level digest.
    ///
    /// `developtools_hapsigner` hashes optional block values after the
    /// first-level chunk digest bytes, without wrapping them in chunk records.
    pub fn update_optional_block(&mut self, value: &[u8]) {
        self.hasher.update(value);
    }

    fn update_chunk(hasher: &mut Sha256, chunk: &[u8]) {
        // Mirrors developtools_hapsigner `HapUtils.computeDigests`: hash each
        // chunk record first, then append that fixed-size chunk digest to the
        // top-level `0x5a | chunkCount | chunkDigests...` input.
        let mut chunk_hasher = Sha256::new();
        chunk_hasher.update([CHUNK_TYPE_BYTE]);
        chunk_hasher.update((chunk.len() as u32).to_le_bytes());
        chunk_hasher.update(chunk);
        hasher.update(chunk_hasher.finalize());
    }
}

/// Compute the HAP content digest over the four logical sections.
///
/// The digest algorithm (from `developtools_hapsigner`):
///
/// 1. Split each section into CHUNK_SIZE chunks.
/// 2. For each chunk compute `SHA-256(0xa5 | u32_LE(chunk_len) | chunk_bytes)`.
/// 3. Final hash input:
///    `0x5a | u32_LE(total_chunk_count) | chunk_digest_0 | chunk_digest_1 | ...`
///
/// `signing_block_bytes` is empty (`&[]`) during initial signing and contains
/// the signing block during *verification*.
pub fn compute_hap_digest(
    entries_bytes: &[u8],
    signing_block_bytes: &[u8],
    cd_bytes: &[u8],
    eocd_bytes: &[u8],
) -> [u8; 32] {
    let mut computer = HapDigestComputer::new(&[
        entries_bytes.len(),
        signing_block_bytes.len(),
        cd_bytes.len(),
        eocd_bytes.len(),
    ]);
    computer.update_section(entries_bytes);
    computer.update_section(signing_block_bytes);
    computer.update_section(cd_bytes);
    computer.update_section(eocd_bytes);
    computer.finalize()
}

#[cfg(test)]
mod tests {
    use super::*;
    use sha2::{Digest, Sha256};

    // ── Type byte constants ────────────────────────────────────────────────

    #[test]
    fn test_content_type_byte_is_0x5a() {
        assert_eq!(
            CONTENT_TYPE_BYTE, 0x5a,
            "top-level prefix byte must be 0x5a"
        );
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

        assert_eq!(
            d, expected,
            "empty-sections digest must equal SHA-256(0x5a || u32_LE(0))"
        );
    }

    #[test]
    fn test_empty_sections_produce_32_byte_output() {
        let d = compute_hap_digest(&[], &[], &[], &[]);
        assert_eq!(d.len(), 32);
    }

    // ── Single-chunk digest ──────────────────────────────────────────────

    #[test]
    fn test_single_byte_payload_produces_one_chunk() {
        // One byte in entries_bytes -> exactly one chunk digest.
        // chunk digest: SHA-256(0xa5 | u32_LE(1) | 0x??)
        // top-level: SHA-256(0x5a | u32_LE(1) | chunk_digest)
        let payload = b"X";
        let d = compute_hap_digest(payload, &[], &[], &[]);

        let chunk_len = 1u32;
        let mut chunk_record = vec![0xa5];
        chunk_record.extend_from_slice(&chunk_len.to_le_bytes());
        chunk_record.extend_from_slice(payload);
        let chunk_digest = Sha256::digest(&chunk_record);

        let mut h = Sha256::new();
        h.update([0x5a_u8]);
        h.update(1u32.to_le_bytes()); // total_chunks = 1
        h.update(chunk_digest);
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

    #[test]
    fn test_piecewise_section_matches_contiguous_section_across_chunk_boundary() {
        let first = vec![0x11u8; CHUNK_SIZE - 3];
        let second = vec![0x22u8; 8];
        let mut contiguous = Vec::with_capacity(first.len() + second.len());
        contiguous.extend_from_slice(&first);
        contiguous.extend_from_slice(&second);

        let expected = compute_hap_digest(&contiguous, &[], b"cd", b"eocd");

        let mut computer = HapDigestComputer::new(&[contiguous.len(), 0, 2, 4]);
        computer.update_section_pieces(contiguous.len(), |push| {
            push(&first);
            push(&second);
        });
        computer.update_section(&[]);
        computer.update_section(b"cd");
        computer.update_section(b"eocd");

        assert_eq!(computer.finalize(), expected);
    }
}
