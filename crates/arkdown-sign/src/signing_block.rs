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
/// Code signing block ID stored inside the property block value.
pub const BLOCK_ID_CODE_SIGN: u32 = 0x30000001;

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
///   u32: byte offset of value from start of the signing block payload
/// [concatenated block data values]
/// u32: block count
/// u64: total signing block size (includes this u64 + magic + u32 version)
/// 16 bytes: magic
/// u32: version
/// ```
pub fn build_signing_block(blocks: &[(u32, Vec<u8>)], compatible_sdk_version: u32) -> Vec<u8> {
    let header_size = blocks.len() * 12; // 12 bytes per TLV header
    let data_size: usize = blocks.iter().map(|(_, v)| v.len()).sum();

    // total = headers + data + u32(count) + u64(size) + 16(magic) + u32(version)
    let total_size: u64 = (header_size + data_size + 4 + 8 + 16 + 4) as u64;

    let mut buf = Vec::with_capacity(total_size as usize);

    // Write TLV headers.
    // Upstream `developtools_hapsigner` `SignHap.generateHapSigningBlock`
    // stores offsets from the start of this payload, not from the value
    // section. Install-time profile parsing uses the same absolute offsets.
    let mut data_offset: u32 = 0;
    for (type_id, value) in blocks {
        buf.extend_from_slice(&type_id.to_le_bytes());
        buf.extend_from_slice(&(value.len() as u32).to_le_bytes());
        let block_offset = header_size as u32 + data_offset;
        buf.extend_from_slice(&block_offset.to_le_bytes());
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
/// u32: content version (=2)
/// u32: block number (=1)
/// For each digest pair:
///   u32: pair length (= u32 alg id + u32 digest length + digest bytes)
///   u32: algorithm ID
///   u32: digest length
///   bytes: digest
/// ```
pub fn encode_digest_pairs(pairs: &[(u32, &[u8])]) -> Vec<u8> {
    let mut buf = Vec::new();
    buf.extend_from_slice(&2u32.to_le_bytes());
    buf.extend_from_slice(&1u32.to_le_bytes());
    for (alg_id, digest) in pairs {
        let pair_len = (4 + 4 + digest.len()) as u32;
        buf.extend_from_slice(&pair_len.to_le_bytes());
        buf.extend_from_slice(&alg_id.to_le_bytes());
        buf.extend_from_slice(&(digest.len() as u32).to_le_bytes());
        buf.extend_from_slice(digest);
    }
    buf
}

#[cfg(test)]
mod tests {
    use super::*;

    // ── Magic constant byte values ──────────────────────────────────────

    #[test]
    fn test_magic_v2_exact_bytes() {
        assert_eq!(MAGIC_V2, b"HAP Sig Block 42");
        assert_eq!(MAGIC_V2.len(), 16);
    }

    #[test]
    fn test_magic_v3_exact_bytes() {
        assert_eq!(MAGIC_V3, b"<hap sign block>");
        assert_eq!(MAGIC_V3.len(), 16);
    }

    #[test]
    fn test_magic_v2_and_v3_differ() {
        assert_ne!(MAGIC_V2, MAGIC_V3, "V2 and V3 magic must be distinct");
    }

    // ── Block ID constants ──────────────────────────────────────────────

    #[test]
    fn test_block_id_signature_v1_value() {
        assert_eq!(BLOCK_ID_SIGNATURE_V1, 0x20000000);
    }

    #[test]
    fn test_block_id_proof_of_rotation_value() {
        assert_eq!(BLOCK_ID_PROOF_OF_ROTATION, 0x20000001);
    }

    #[test]
    fn test_block_id_profile_value() {
        assert_eq!(BLOCK_ID_PROFILE, 0x20000002);
    }

    #[test]
    fn test_block_id_property_value() {
        assert_eq!(BLOCK_ID_PROPERTY, 0x20000003);
    }

    // ── Algorithm ID constants ──────────────────────────────────────────

    #[test]
    fn test_alg_ecdsa_sha256_value() {
        assert_eq!(ALG_ECDSA_SHA256, 0x201);
    }

    #[test]
    fn test_alg_rsa_pss_sha256_value() {
        assert_eq!(ALG_RSA_PSS_SHA256, 0x101);
    }

    // ── build_signing_block magic selection ────────────────────────────

    #[test]
    fn test_sdk_lt_8_selects_v2_magic() {
        let block = build_signing_block(&[(BLOCK_ID_SIGNATURE_V1, vec![0xAA; 4])], 7);
        let len = block.len();
        // magic is at: block.len() - 4(version) - 16(magic)
        let magic_start = len - 4 - 16;
        assert_eq!(&block[magic_start..magic_start + 16], MAGIC_V2.as_slice());
    }

    #[test]
    fn test_sdk_gte_8_selects_v3_magic() {
        let block = build_signing_block(&[(BLOCK_ID_SIGNATURE_V1, vec![0xBB; 4])], 8);
        let len = block.len();
        let magic_start = len - 4 - 16;
        assert_eq!(&block[magic_start..magic_start + 16], MAGIC_V3.as_slice());
    }

    #[test]
    fn test_sdk_version_12_selects_v3_magic() {
        let block = build_signing_block(&[(BLOCK_ID_SIGNATURE_V1, vec![1u8])], 12);
        let len = block.len();
        let magic_start = len - 4 - 16;
        assert_eq!(&block[magic_start..magic_start + 16], MAGIC_V3.as_slice());
    }

    // ── Version field ──────────────────────────────────────────────────

    #[test]
    fn test_sdk_lt_8_sets_version_2() {
        let block = build_signing_block(&[(BLOCK_ID_SIGNATURE_V1, vec![1u8])], 5);
        let version = u32::from_le_bytes(block[block.len() - 4..].try_into().unwrap());
        assert_eq!(version, 2);
    }

    #[test]
    fn test_sdk_gte_8_sets_version_3() {
        let block = build_signing_block(&[(BLOCK_ID_SIGNATURE_V1, vec![1u8])], 12);
        let version = u32::from_le_bytes(block[block.len() - 4..].try_into().unwrap());
        assert_eq!(version, 3);
    }

    // ── Total size field ───────────────────────────────────────────────

    #[test]
    fn test_total_size_field_correct_for_two_blocks() {
        let data1 = vec![0u8; 50];
        let data2 = vec![0u8; 100];
        let block = build_signing_block(
            &[(BLOCK_ID_SIGNATURE_V1, data1), (BLOCK_ID_PROFILE, data2)],
            12,
        );
        // total = 2*12 (headers) + 50+100 (data) + 4 (count) + 8 (size) + 16 (magic) + 4 (version)
        let expected: u64 = (2 * 12 + 150 + 4 + 8 + 16 + 4) as u64;
        let total_start = block.len() - 4 - 16 - 8;
        let total = u64::from_le_bytes(block[total_start..total_start + 8].try_into().unwrap());
        assert_eq!(total, expected);
    }

    #[test]
    fn test_offsets_match_hapsigner_absolute_payload_offsets() {
        let profile = vec![0xAA, 0xBB, 0xCC];
        let signature = vec![0x11, 0x22, 0x33, 0x44, 0x55];
        let block = build_signing_block(
            &[
                (BLOCK_ID_PROFILE, profile.clone()),
                (BLOCK_ID_SIGNATURE_V1, signature.clone()),
            ],
            20,
        );

        let first_offset = u32::from_le_bytes(block[8..12].try_into().unwrap()) as usize;
        let second_offset = u32::from_le_bytes(block[20..24].try_into().unwrap()) as usize;

        assert_eq!(first_offset, 24);
        assert_eq!(second_offset, 24 + profile.len());
        assert_eq!(
            &block[first_offset..first_offset + profile.len()],
            profile.as_slice()
        );
        assert_eq!(
            &block[second_offset..second_offset + signature.len()],
            signature.as_slice()
        );
    }

    // ── encode_digest_pairs ────────────────────────────────────────────

    #[test]
    fn test_encode_digest_pairs_empty_has_hapsigner_header() {
        let encoded = encode_digest_pairs(&[]);
        assert_eq!(encoded, [2u32.to_le_bytes(), 1u32.to_le_bytes()].concat());
    }

    #[test]
    fn test_encode_digest_pairs_single_matches_hapsigner_tlv() {
        let digest = [0xDEu8; 32];
        let encoded = encode_digest_pairs(&[(ALG_ECDSA_SHA256, &digest)]);
        assert_eq!(encoded.len(), 8 + 4 + 4 + 4 + 32);
        assert_eq!(&encoded[0..4], &2u32.to_le_bytes());
        assert_eq!(&encoded[4..8], &1u32.to_le_bytes());
        assert_eq!(&encoded[8..12], &40u32.to_le_bytes());
        assert_eq!(&encoded[12..16], &ALG_ECDSA_SHA256.to_le_bytes());
        assert_eq!(&encoded[16..20], &32u32.to_le_bytes());
        assert_eq!(&encoded[20..52], &digest[..]);
    }

    #[test]
    fn test_encode_digest_pairs_multiple() {
        let d1 = [0x11u8; 32];
        let d2 = [0x22u8; 32];
        let encoded = encode_digest_pairs(&[(ALG_ECDSA_SHA256, &d1), (ALG_RSA_PSS_SHA256, &d2)]);
        assert_eq!(encoded.len(), 8 + 2 * (4 + 4 + 4 + 32));
        assert_eq!(&encoded[0..4], &2u32.to_le_bytes());
        assert_eq!(&encoded[4..8], &1u32.to_le_bytes());
        let second_start = 8 + 44;
        assert_eq!(
            &encoded[second_start..second_start + 4],
            &40u32.to_le_bytes()
        );
        assert_eq!(
            &encoded[second_start + 4..second_start + 8],
            &ALG_RSA_PSS_SHA256.to_le_bytes()
        );
        assert_eq!(
            &encoded[second_start + 8..second_start + 12],
            &32u32.to_le_bytes()
        );
        assert_eq!(&encoded[second_start + 12..second_start + 44], &d2[..]);
    }
}
