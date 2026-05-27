//! Build a CMS SignedData DER structure for HAP signing.
//!
//! Built manually using the `der` crate's `Encode` trait for x509 types.
//! The `cms` builder feature is intentionally avoided to keep dependency
//! surface small.
//!
//! Reference: RFC 5652 (CMS), `developtools_hapsigner` `BcPkcs7Generator.java`.

use crate::error::SignError;
use crate::signing_block::{ALG_ECDSA_SHA256, ALG_RSA_PSS_SHA256};
use der::{Decode, Encode};
use sha2::{Digest, Sha256};
use x509_cert::Certificate;

// ---------------------------------------------------------------------------
// Hard-coded OID DER bytes (tag 0x06 + length + content)
// These are verbatim BER/DER encodings of the well-known OIDs.
// ---------------------------------------------------------------------------

/// `id-data` (1.2.840.113549.1.7.1)
const OID_ID_DATA: &[u8] = &[0x06, 0x09, 0x2a, 0x86, 0x48, 0x86, 0xf7, 0x0d, 0x01, 0x07, 0x01];

/// `id-signedData` (1.2.840.113549.1.7.2)
const OID_ID_SIGNED_DATA: &[u8] =
    &[0x06, 0x09, 0x2a, 0x86, 0x48, 0x86, 0xf7, 0x0d, 0x01, 0x07, 0x02];

/// `id-sha256` (2.16.840.1.101.3.4.2.1)
const OID_SHA256: &[u8] = &[0x06, 0x09, 0x60, 0x86, 0x48, 0x01, 0x65, 0x03, 0x04, 0x02, 0x01];

/// `id-ecdsaWithSHA256` (1.2.840.10045.4.3.2)
const OID_ECDSA_WITH_SHA256: &[u8] =
    &[0x06, 0x08, 0x2a, 0x86, 0x48, 0xce, 0x3d, 0x04, 0x03, 0x02];

/// `id-RSASSA-PSS` (1.2.840.113549.1.1.10)
const OID_RSASSA_PSS: &[u8] =
    &[0x06, 0x09, 0x2a, 0x86, 0x48, 0x86, 0xf7, 0x0d, 0x01, 0x01, 0x0a];

/// `id-contentType` (1.2.840.113549.1.9.3)
const OID_CONTENT_TYPE: &[u8] =
    &[0x06, 0x09, 0x2a, 0x86, 0x48, 0x86, 0xf7, 0x0d, 0x01, 0x09, 0x03];

/// `id-signingTime` (1.2.840.113549.1.9.5)
const OID_SIGNING_TIME: &[u8] =
    &[0x06, 0x09, 0x2a, 0x86, 0x48, 0x86, 0xf7, 0x0d, 0x01, 0x09, 0x05];

/// `id-messageDigest` (1.2.840.113549.1.9.4)
const OID_MESSAGE_DIGEST: &[u8] =
    &[0x06, 0x09, 0x2a, 0x86, 0x48, 0x86, 0xf7, 0x0d, 0x01, 0x09, 0x04];

// ---------------------------------------------------------------------------
// DER primitive helpers
// ---------------------------------------------------------------------------

/// Encode an ASN.1 length field.
fn encode_length(len: usize) -> Vec<u8> {
    if len < 0x80 {
        vec![len as u8]
    } else if len <= 0xFF {
        vec![0x81, len as u8]
    } else if len <= 0xFFFF {
        vec![0x82, (len >> 8) as u8, (len & 0xFF) as u8]
    } else if len <= 0xFFFFFF {
        vec![
            0x83,
            (len >> 16) as u8,
            (len >> 8) as u8,
            (len & 0xFF) as u8,
        ]
    } else {
        vec![
            0x84,
            (len >> 24) as u8,
            (len >> 16) as u8,
            (len >> 8) as u8,
            (len & 0xFF) as u8,
        ]
    }
}

/// Wrap `data` in a DER TLV with the given tag.
fn tlv(tag: u8, data: &[u8]) -> Vec<u8> {
    let mut result = Vec::with_capacity(1 + 4 + data.len());
    result.push(tag);
    result.extend_from_slice(&encode_length(data.len()));
    result.extend_from_slice(data);
    result
}

/// Build a SEQUENCE TLV.
#[inline]
fn sequence(inner: &[u8]) -> Vec<u8> {
    tlv(0x30, inner)
}

/// Build a SET TLV.
#[inline]
fn set(inner: &[u8]) -> Vec<u8> {
    tlv(0x31, inner)
}

/// Build a context-tagged IMPLICIT TLV (e.g., `[0] IMPLICIT`).
#[inline]
fn implicit_ctx(tag_num: u8, inner: &[u8]) -> Vec<u8> {
    tlv(0xA0 | tag_num, inner)
}

/// Build a context-tagged EXPLICIT TLV (e.g., `[0] EXPLICIT`).
#[inline]
fn explicit_ctx(tag_num: u8, inner: &[u8]) -> Vec<u8> {
    // Explicit: constructed bit (0x20) + context class (0x80) + tag number
    tlv(0xA0 | 0x20 | tag_num, inner)
}

/// Encode an OCTET STRING TLV.
#[inline]
fn octet_string(bytes: &[u8]) -> Vec<u8> {
    tlv(0x04, bytes)
}

/// Encode `INTEGER 1` as DER.
fn integer_one() -> Vec<u8> {
    vec![0x02, 0x01, 0x01]
}

/// Concatenate multiple byte slices.
fn concat(parts: &[&[u8]]) -> Vec<u8> {
    let total: usize = parts.iter().map(|p| p.len()).sum();
    let mut result = Vec::with_capacity(total);
    for p in parts {
        result.extend_from_slice(p);
    }
    result
}

// ---------------------------------------------------------------------------
// UTC time formatting (no external dependency)
// ---------------------------------------------------------------------------

/// Format the current UTC time as a DER UTCTime TLV (tag 0x17).
fn current_utctime_der() -> Vec<u8> {
    use std::time::{SystemTime, UNIX_EPOCH};
    let secs = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs();
    let (year, month, day, hour, min, sec) = unix_secs_to_utc(secs);
    let yy = year % 100;
    let s = format!(
        "{:02}{:02}{:02}{:02}{:02}{:02}Z",
        yy, month, day, hour, min, sec
    );
    tlv(0x17, s.as_bytes())
}

fn unix_secs_to_utc(mut secs: u64) -> (u32, u8, u8, u8, u8, u8) {
    let second = (secs % 60) as u8;
    secs /= 60;
    let minute = (secs % 60) as u8;
    secs /= 60;
    let hour = (secs % 24) as u8;
    let mut days = (secs / 24) as u32;

    let mut year = 1970u32;
    loop {
        let diy = days_in_year(year);
        if days < diy {
            break;
        }
        days -= diy;
        year += 1;
    }

    let leap = is_leap_year(year);
    let month_lens: [u32; 12] = [
        31,
        if leap { 29 } else { 28 },
        31,
        30,
        31,
        30,
        31,
        31,
        30,
        31,
        30,
        31,
    ];
    let mut month = 1u8;
    for (i, &ml) in month_lens.iter().enumerate() {
        if days < ml {
            month = (i + 1) as u8;
            break;
        }
        days -= ml;
    }
    let day = (days + 1) as u8;
    (year, month, day, hour, minute, second)
}

fn is_leap_year(y: u32) -> bool {
    y % 4 == 0 && (y % 100 != 0 || y % 400 == 0)
}

fn days_in_year(y: u32) -> u32 {
    if is_leap_year(y) { 366 } else { 365 }
}

// ---------------------------------------------------------------------------
// Signing
// ---------------------------------------------------------------------------

fn sign_ecdsa(key_der: &[u8], data: &[u8]) -> Result<Vec<u8>, SignError> {
    use p256::pkcs8::DecodePrivateKey;
    use signature::Signer;

    let signing_key = p256::ecdsa::SigningKey::from_pkcs8_der(key_der)
        .map_err(|e| SignError::Pkcs8Error(format!("ECDSA key load: {e}")))?;

    let sig: p256::ecdsa::DerSignature = signing_key.sign(data);
    Ok(sig.as_bytes().to_vec())
}

fn sign_rsa_pss(key_der: &[u8], data: &[u8]) -> Result<Vec<u8>, SignError> {
    use rsa::pkcs8::DecodePrivateKey;
    use signature::{RandomizedSigner, SignatureEncoding};

    let rsa_key = rsa::RsaPrivateKey::from_pkcs8_der(key_der)
        .map_err(|e| SignError::Pkcs8Error(format!("RSA key load: {e}")))?;
    let signing_key = rsa::pss::SigningKey::<sha2::Sha256>::new(rsa_key);
    let sig = signing_key.sign_with_rng(&mut rand::rngs::OsRng, data);
    Ok(sig.to_vec())
}

// ---------------------------------------------------------------------------
// Public API
// ---------------------------------------------------------------------------

/// Build a CMS SignedData DER blob for the given content and signing key.
///
/// # Arguments
/// - `content`: the digest-pairs blob from `signing_block::encode_digest_pairs`
/// - `private_key_der`: PKCS#8 DER-encoded private key (EC or RSA)
/// - `cert_chain`: DER-encoded X.509 certificates, leaf-first
/// - `alg_id`: `ALG_ECDSA_SHA256` or `ALG_RSA_PSS_SHA256`
pub fn build_cms_signed_data(
    content: &[u8],
    private_key_der: &[u8],
    cert_chain: &[Vec<u8>],
    alg_id: u32,
) -> Result<Vec<u8>, SignError> {
    if cert_chain.is_empty() {
        return Err(SignError::NoCertificate);
    }

    // Parse leaf certificate to extract issuer and serial number for SignerInfo.
    let leaf_cert = Certificate::from_der(&cert_chain[0])
        .map_err(|e| SignError::DerError(format!("cert parse: {e}")))?;
    let issuer_der = leaf_cert
        .tbs_certificate
        .issuer
        .to_der()
        .map_err(|e| SignError::DerError(format!("issuer encode: {e}")))?;
    let serial_der = leaf_cert
        .tbs_certificate
        .serial_number
        .to_der()
        .map_err(|e| SignError::DerError(format!("serial encode: {e}")))?;

    // -----------------------------------------------------------------------
    // 1. digestAlgorithms SET
    // -----------------------------------------------------------------------
    // AlgorithmIdentifier { OID(SHA-256), NULL }
    let sha256_alg_id = sequence(&concat(&[OID_SHA256, &[0x05, 0x00]])); // OID + NULL
    let digest_algorithms = set(&sha256_alg_id);

    // -----------------------------------------------------------------------
    // 2. encapContentInfo
    // -----------------------------------------------------------------------
    // ContentInfo { OID(id-data), [0] EXPLICIT { OCTET STRING(content) } }
    let econtent_inner = concat(&[OID_ID_DATA, &explicit_ctx(0, &octet_string(content))]);
    let encap_content_info = sequence(&econtent_inner);

    // -----------------------------------------------------------------------
    // 3. Certificates ([0] IMPLICIT set of DER certs)
    // -----------------------------------------------------------------------
    let mut certs_inner = Vec::new();
    for cert_der in cert_chain {
        certs_inner.extend_from_slice(cert_der);
    }
    let certificates = implicit_ctx(0, &certs_inner);

    // -----------------------------------------------------------------------
    // 4. Signed attributes (for computing signature)
    // -----------------------------------------------------------------------
    let content_digest = {
        let mut h = Sha256::new();
        h.update(content);
        h.finalize()
    };

    // Attribute: contentType = id-data
    let attr_content_type = sequence(&concat(&[
        OID_CONTENT_TYPE,
        &set(OID_ID_DATA),
    ]));

    // Attribute: signingTime = UTCTime(now)
    let signing_time = current_utctime_der();
    let attr_signing_time = sequence(&concat(&[OID_SIGNING_TIME, &set(&signing_time)]));

    // Attribute: messageDigest = SHA-256(content)
    let attr_message_digest = sequence(&concat(&[
        OID_MESSAGE_DIGEST,
        &set(&octet_string(content_digest.as_slice())),
    ]));

    // Concatenate signed attrs (order: contentType, signingTime, messageDigest)
    let signed_attrs_inner = concat(&[&attr_content_type, &attr_signing_time, &attr_message_digest]);

    // For signing: encode as SET (tag 0x31)
    let signed_attrs_for_signing = set(&signed_attrs_inner);

    // For embedding in SignerInfo: encode as [0] IMPLICIT (tag 0xA0)
    let signed_attrs_encoded = implicit_ctx(0, &signed_attrs_inner);

    // -----------------------------------------------------------------------
    // 5. Compute signature
    // -----------------------------------------------------------------------
    let signature_bytes = match alg_id {
        ALG_ECDSA_SHA256 => sign_ecdsa(private_key_der, &signed_attrs_for_signing)?,
        ALG_RSA_PSS_SHA256 => sign_rsa_pss(private_key_der, &signed_attrs_for_signing)?,
        _ => sign_ecdsa(private_key_der, &signed_attrs_for_signing)?,
    };

    // -----------------------------------------------------------------------
    // 6. signatureAlgorithm
    // -----------------------------------------------------------------------
    let sig_alg_oid = match alg_id {
        ALG_RSA_PSS_SHA256 => OID_RSASSA_PSS,
        _ => OID_ECDSA_WITH_SHA256,
    };
    let signature_algorithm = sequence(sig_alg_oid);

    // -----------------------------------------------------------------------
    // 7. IssuerAndSerialNumber
    // -----------------------------------------------------------------------
    let issuer_and_serial = sequence(&concat(&[&issuer_der, &serial_der]));

    // -----------------------------------------------------------------------
    // 8. SignerInfo
    // -----------------------------------------------------------------------
    // digestAlgorithm AlgorithmIdentifier(SHA-256)
    let digest_alg_for_signer = sequence(&concat(&[OID_SHA256, &[0x05, 0x00]]));

    let signer_info_inner = concat(&[
        &integer_one(),             // version
        &issuer_and_serial,         // sid: IssuerAndSerialNumber
        &digest_alg_for_signer,     // digestAlgorithm
        &signed_attrs_encoded,      // signedAttrs [0] IMPLICIT
        &signature_algorithm,       // signatureAlgorithm
        &octet_string(&signature_bytes), // signature
    ]);
    let signer_info = sequence(&signer_info_inner);

    // -----------------------------------------------------------------------
    // 9. SignedData
    // -----------------------------------------------------------------------
    let signer_infos = set(&signer_info);

    let signed_data_inner = concat(&[
        &integer_one(),        // version
        &digest_algorithms,   // digestAlgorithms
        &encap_content_info,  // encapContentInfo
        &certificates,        // [0] IMPLICIT certs
        &signer_infos,        // signerInfos
    ]);
    let signed_data = sequence(&signed_data_inner);

    // -----------------------------------------------------------------------
    // 10. ContentInfo wrapping
    // -----------------------------------------------------------------------
    let content_info_inner = concat(&[OID_ID_SIGNED_DATA, &explicit_ctx(0, &signed_data)]);
    let content_info = sequence(&content_info_inner);

    Ok(content_info)
}
