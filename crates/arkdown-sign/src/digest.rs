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
