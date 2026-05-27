//! HAP signing block TLV assembly.
//!
//! Reference: `developtools_hapsigner` `SignHap.java`, `HapUtils.java`.

// ---------------------------------------------------------------------------
// Block type IDs
// ---------------------------------------------------------------------------

/// Main CMS SignedData block (HAP Signature Scheme V1).
pub const BLOCK_ID_SIGNATURE_V1: u32 = 0x20000000;
/// Proof-of-rotation certificate chain block.
pub const BLOCK_ID_PROOF_OF_ROTATION: u32 = 0x20000001;
/// Embedded `.p7b` provisioning profile block.
pub const BLOCK_ID_PROFILE: u32 = 0x20000002;
/// Build property metadata block.
pub const BLOCK_ID_PROPERTY: u32 = 0x20000003;

// ---------------------------------------------------------------------------
// Magic bytes
// ---------------------------------------------------------------------------

/// Signing block magic for `compatibleSdkVersion < 8` (V2).
pub const MAGIC_V2: &[u8; 16] = b"HAP Sig Block 42";
/// Signing block magic for `compatibleSdkVersion >= 8` (V3).
pub const MAGIC_V3: &[u8; 16] = b"<hap sign block>";

// ---------------------------------------------------------------------------
// Signature algorithm IDs
// ---------------------------------------------------------------------------

/// ECDSA P-256 + SHA-256 (most common for HarmonyOS builds).
pub const ALG_ECDSA_SHA256: u32 = 0x201;
/// RSA-PSS + SHA-256.
pub const ALG_RSA_PSS_SHA256: u32 = 0x101;

// ---------------------------------------------------------------------------
// Block assembly
// ---------------------------------------------------------------------------

/// Assemble the signing block from a list of `(type_id, value)` pairs.
///
/// Layout (little-endian):
/// ```text
/// For each block (12-byte TLV header):
///   u32: type
///   u32: length of value
///   u32: byte offset of value from start of data section
/// [concatenated block data values]
/// u32: block count
/// u64: total signing block size (includes this u64 + magic + u32 version)
/// 16 bytes: magic
/// u32: version
/// ```
pub fn build_signing_block(
    blocks: &[(u32, Vec<u8>)],
    compatible_sdk_version: u32,
) -> Vec<u8> {
    let header_size = blocks.len() * 12; // 12 bytes per TLV header
    let data_size: usize = blocks.iter().map(|(_, v)| v.len()).sum();

    // total = headers + data + u32(count) + u64(size) + 16(magic) + u32(version)
    let total_size: u64 = (header_size + data_size + 4 + 8 + 16 + 4) as u64;

    let mut buf = Vec::with_capacity(total_size as usize);

    // Write TLV headers.
    // Offsets are relative to the start of the data section (after all headers).
    let mut data_offset: u32 = 0;
    for (type_id, value) in blocks {
        buf.extend_from_slice(&type_id.to_le_bytes());
        buf.extend_from_slice(&(value.len() as u32).to_le_bytes());
        buf.extend_from_slice(&data_offset.to_le_bytes());
        data_offset += value.len() as u32;
    }

    // Write block data values.
    for (_, value) in blocks {
        buf.extend_from_slice(value);
    }

    // Block count.
    buf.extend_from_slice(&(blocks.len() as u32).to_le_bytes());

    // Total size (u64). Includes this field itself (8) + magic (16) + version (4) = 28.
    buf.extend_from_slice(&total_size.to_le_bytes());

    // Magic bytes.
    let magic = if compatible_sdk_version >= 8 {
        MAGIC_V3
    } else {
        MAGIC_V2
    };
    buf.extend_from_slice(magic);

    // Version.
    let version: u32 = if compatible_sdk_version >= 8 { 3 } else { 2 };
    buf.extend_from_slice(&version.to_le_bytes());

    buf
}

// ---------------------------------------------------------------------------
// Digest pair encoding
// ---------------------------------------------------------------------------

/// Encode `(alg_id, digest)` pairs as the "unsigned HAP digest" blob.
///
/// This blob becomes the `content` of the `encapContentInfo` in the CMS `SignedData`.
///
/// Format (little-endian):
/// ```text
/// u32: algorithm ID
/// u32: digest length
/// bytes: digest
/// ```
pub fn encode_digest_pairs(pairs: &[(u32, &[u8])]) -> Vec<u8> {
    let mut buf = Vec::new();
    for (alg_id, digest) in pairs {
        buf.extend_from_slice(&alg_id.to_le_bytes());
        buf.extend_from_slice(&(digest.len() as u32).to_le_bytes());
        buf.extend_from_slice(digest);
    }
    buf
}
