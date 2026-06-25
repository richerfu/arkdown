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
const OID_ID_DATA: &[u8] = &[
    0x06, 0x09, 0x2a, 0x86, 0x48, 0x86, 0xf7, 0x0d, 0x01, 0x07, 0x01,
];

/// `id-signedData` (1.2.840.113549.1.7.2)
const OID_ID_SIGNED_DATA: &[u8] = &[
    0x06, 0x09, 0x2a, 0x86, 0x48, 0x86, 0xf7, 0x0d, 0x01, 0x07, 0x02,
];

/// `id-sha256` (2.16.840.1.101.3.4.2.1)
const OID_SHA256: &[u8] = &[
    0x06, 0x09, 0x60, 0x86, 0x48, 0x01, 0x65, 0x03, 0x04, 0x02, 0x01,
];

/// `id-ecdsaWithSHA256` (1.2.840.10045.4.3.2)
const OID_ECDSA_WITH_SHA256: &[u8] = &[0x06, 0x08, 0x2a, 0x86, 0x48, 0xce, 0x3d, 0x04, 0x03, 0x02];

/// `id-RSASSA-PSS` (1.2.840.113549.1.1.10)
const OID_RSASSA_PSS: &[u8] = &[
    0x06, 0x09, 0x2a, 0x86, 0x48, 0x86, 0xf7, 0x0d, 0x01, 0x01, 0x0a,
];

/// `id-mgf1` (1.2.840.113549.1.1.8)
const OID_MGF1: &[u8] = &[
    0x06, 0x09, 0x2a, 0x86, 0x48, 0x86, 0xf7, 0x0d, 0x01, 0x01, 0x08,
];

/// `id-contentType` (1.2.840.113549.1.9.3)
const OID_CONTENT_TYPE: &[u8] = &[
    0x06, 0x09, 0x2a, 0x86, 0x48, 0x86, 0xf7, 0x0d, 0x01, 0x09, 0x03,
];

/// `id-signingTime` (1.2.840.113549.1.9.5)
const OID_SIGNING_TIME: &[u8] = &[
    0x06, 0x09, 0x2a, 0x86, 0x48, 0x86, 0xf7, 0x0d, 0x01, 0x09, 0x05,
];

/// `id-messageDigest` (1.2.840.113549.1.9.4)
const OID_MESSAGE_DIGEST: &[u8] = &[
    0x06, 0x09, 0x2a, 0x86, 0x48, 0x86, 0xf7, 0x0d, 0x01, 0x09, 0x04,
];

/// hapsigner code-sign ownerID (1.3.6.1.4.1.2011.2.376.1.4.1)
const OID_CODE_SIGN_OWNER_ID: &[u8] = &[
    0x06, 0x0d, 0x2b, 0x06, 0x01, 0x04, 0x01, 0x8f, 0x5b, 0x02, 0x82, 0x78, 0x01, 0x04, 0x01,
];

/// hapsigner code-sign pluginId (1.3.6.1.4.1.2011.2.376.1.4.2)
const OID_CODE_SIGN_PLUGIN_ID: &[u8] = &[
    0x06, 0x0d, 0x2b, 0x06, 0x01, 0x04, 0x01, 0x8f, 0x5b, 0x02, 0x82, 0x78, 0x01, 0x04, 0x02,
];

#[derive(Debug, Clone, Copy)]
struct CodeSignAttributes<'a> {
    owner_id: Option<&'a str>,
    plugin_id: Option<&'a str>,
}

pub(crate) struct CodeSignSignedDataBuilder {
    signer: CmsSigningContext,
}

struct CmsSigningContext {
    issuer_and_serial: Vec<u8>,
    certificates: Vec<u8>,
    signature_algorithm: Vec<u8>,
    private_key: CmsPrivateKey,
    verifier: SignatureVerifier,
}

enum CmsPrivateKey {
    Ecdsa(p256::ecdsa::SigningKey),
    RsaPss(rsa::pss::SigningKey<Sha256>),
}

enum SignatureVerifier {
    EcdsaP256(Vec<u8>),
    EcdsaP384(Vec<u8>),
    RsaPss(rsa::pss::VerifyingKey<Sha256>),
}

// ---------------------------------------------------------------------------
// DER helper namespace
// ---------------------------------------------------------------------------

/// Namespace for DER primitive helpers and signing helpers.
///
/// All methods are static (no `self`); the struct is never instantiated.
struct Der;

impl Der {
    // -----------------------------------------------------------------------
    // ASN.1 / DER primitives
    // -----------------------------------------------------------------------

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
        result.extend_from_slice(&Der::encode_length(data.len()));
        result.extend_from_slice(data);
        result
    }

    /// Build a SEQUENCE TLV.
    #[inline]
    fn sequence(inner: &[u8]) -> Vec<u8> {
        Der::tlv(0x30, inner)
    }

    /// Build a SET TLV.
    #[inline]
    fn set(inner: &[u8]) -> Vec<u8> {
        Der::tlv(0x31, inner)
    }

    /// Build a context-tagged IMPLICIT TLV (e.g., `[0] IMPLICIT`).
    #[inline]
    fn implicit_ctx(tag_num: u8, inner: &[u8]) -> Vec<u8> {
        Der::tlv(0xA0 | tag_num, inner)
    }

    /// Build a context-tagged EXPLICIT TLV (e.g., `[0] EXPLICIT`).
    #[inline]
    fn explicit_ctx(tag_num: u8, inner: &[u8]) -> Vec<u8> {
        // Explicit: constructed bit (0x20) + context class (0x80) + tag number
        Der::tlv(0xA0 | 0x20 | tag_num, inner)
    }

    /// Encode an OCTET STRING TLV.
    #[inline]
    fn octet_string(bytes: &[u8]) -> Vec<u8> {
        Der::tlv(0x04, bytes)
    }

    /// Encode a DER UTF8String TLV.
    #[inline]
    fn utf8_string(value: &str) -> Vec<u8> {
        Der::tlv(0x0c, value.as_bytes())
    }

    /// Encode `INTEGER 1` as DER.
    fn integer_one() -> Vec<u8> {
        vec![0x02, 0x01, 0x01]
    }

    fn integer_u8(value: u8) -> Vec<u8> {
        vec![0x02, 0x01, value]
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

    // -----------------------------------------------------------------------
    // UTC time formatting (no external dependency)
    // -----------------------------------------------------------------------

    /// Format the current UTC time as a DER UTCTime TLV (tag 0x17).
    fn current_utctime_der() -> Vec<u8> {
        use std::time::{SystemTime, UNIX_EPOCH};
        let secs = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap_or_default()
            .as_secs();
        let (year, month, day, hour, min, sec) = Der::unix_secs_to_utc(secs);
        let yy = year % 100;
        let s = format!(
            "{:02}{:02}{:02}{:02}{:02}{:02}Z",
            yy, month, day, hour, min, sec
        );
        Der::tlv(0x17, s.as_bytes())
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
            let diy = Der::days_in_year(year);
            if days < diy {
                break;
            }
            days -= diy;
            year += 1;
        }

        let leap = Der::is_leap_year(year);
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
        y.is_multiple_of(4) && (!y.is_multiple_of(100) || y.is_multiple_of(400))
    }

    fn days_in_year(y: u32) -> u32 {
        if Der::is_leap_year(y) {
            366
        } else {
            365
        }
    }

    // -----------------------------------------------------------------------
    // Signing
    // -----------------------------------------------------------------------

    fn rsa_pss_sha256_params() -> Vec<u8> {
        let sha256_algorithm = Der::sequence(&Der::concat(&[OID_SHA256, &[0x05, 0x00]]));
        let mgf1_algorithm = Der::sequence(&Der::concat(&[OID_MGF1, &sha256_algorithm]));
        Der::sequence(&Der::concat(&[
            &Der::explicit_ctx(0, &sha256_algorithm),
            &Der::explicit_ctx(1, &mgf1_algorithm),
            &Der::explicit_ctx(2, &Der::integer_u8(32)),
        ]))
    }
}

impl CodeSignSignedDataBuilder {
    pub(crate) fn new(
        private_key_der: &[u8],
        cert_chain: &[Vec<u8>],
        alg_id: u32,
    ) -> Result<Self, SignError> {
        Ok(Self {
            signer: CmsSigningContext::new(private_key_der, cert_chain, alg_id)?,
        })
    }

    pub(crate) fn build(
        &self,
        content: &[u8],
        owner_id: Option<&str>,
        plugin_id: Option<&str>,
    ) -> Result<Vec<u8>, SignError> {
        self.signer.build_signed_data(
            content,
            false,
            Some(CodeSignAttributes {
                owner_id,
                plugin_id,
            }),
        )
    }
}

impl CmsSigningContext {
    fn new(private_key_der: &[u8], cert_chain: &[Vec<u8>], alg_id: u32) -> Result<Self, SignError> {
        if cert_chain.is_empty() {
            return Err(SignError::NoCertificate);
        }

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
        let issuer_and_serial = Der::sequence(&Der::concat(&[&issuer_der, &serial_der]));

        let mut certs_inner = Vec::new();
        for cert_der in cert_chain {
            certs_inner.extend_from_slice(cert_der);
        }
        let certificates = Der::implicit_ctx(0, &certs_inner);

        let signature_algorithm = match alg_id {
            ALG_RSA_PSS_SHA256 => Der::sequence(&Der::concat(&[
                OID_RSASSA_PSS,
                &Der::rsa_pss_sha256_params(),
            ])),
            ALG_ECDSA_SHA256 => Der::sequence(OID_ECDSA_WITH_SHA256),
            _ => {
                return Err(SignError::Config(format!(
                    "unsupported HAP signature algorithm id: 0x{alg_id:x}"
                )));
            }
        };

        Ok(Self {
            issuer_and_serial,
            certificates,
            signature_algorithm,
            private_key: Self::load_private_key(private_key_der, alg_id)?,
            verifier: Self::load_verifier(&leaf_cert, alg_id)?,
        })
    }

    fn load_private_key(private_key_der: &[u8], alg_id: u32) -> Result<CmsPrivateKey, SignError> {
        match alg_id {
            ALG_ECDSA_SHA256 => {
                use p256::pkcs8::DecodePrivateKey;

                let signing_key = p256::ecdsa::SigningKey::from_pkcs8_der(private_key_der)
                    .map_err(|e| SignError::Pkcs8Error(format!("ECDSA key load: {e}")))?;
                Ok(CmsPrivateKey::Ecdsa(signing_key))
            }
            ALG_RSA_PSS_SHA256 => {
                use rsa::pkcs8::DecodePrivateKey;

                let rsa_key = rsa::RsaPrivateKey::from_pkcs8_der(private_key_der)
                    .map_err(|e| SignError::Pkcs8Error(format!("RSA key load: {e}")))?;
                Ok(CmsPrivateKey::RsaPss(rsa::pss::SigningKey::<Sha256>::new(
                    rsa_key,
                )))
            }
            _ => Err(SignError::Config(format!(
                "unsupported HAP signature algorithm id: 0x{alg_id:x}"
            ))),
        }
    }

    fn load_verifier(leaf_cert: &Certificate, alg_id: u32) -> Result<SignatureVerifier, SignError> {
        match alg_id {
            ALG_ECDSA_SHA256 => {
                let public_key = leaf_cert
                    .tbs_certificate
                    .subject_public_key_info
                    .subject_public_key
                    .as_bytes()
                    .ok_or_else(|| {
                        SignError::DerError(
                            "leaf certificate public key has unused bits".to_string(),
                        )
                    })?
                    .to_vec();
                match public_key.len() {
                    65 => Ok(SignatureVerifier::EcdsaP256(public_key)),
                    97 => Ok(SignatureVerifier::EcdsaP384(public_key)),
                    len => Err(SignError::SigningFailed(format!(
                        "unsupported ECDSA public key length for SHA256withECDSA: {len}"
                    ))),
                }
            }
            ALG_RSA_PSS_SHA256 => {
                use rsa::pkcs8::DecodePublicKey;

                let spki_der = leaf_cert
                    .tbs_certificate
                    .subject_public_key_info
                    .to_der()
                    .map_err(|e| SignError::DerError(format!("leaf SPKI encode: {e}")))?;
                let public_key = rsa::RsaPublicKey::from_public_key_der(&spki_der)
                    .map_err(|e| SignError::DerError(format!("leaf RSA public key parse: {e}")))?;
                Ok(SignatureVerifier::RsaPss(
                    rsa::pss::VerifyingKey::<Sha256>::new(public_key),
                ))
            }
            _ => Err(SignError::Config(format!(
                "unsupported HAP signature algorithm id: 0x{alg_id:x}"
            ))),
        }
    }

    fn build_signed_data(
        &self,
        content: &[u8],
        embed_content: bool,
        code_sign_attributes: Option<CodeSignAttributes<'_>>,
    ) -> Result<Vec<u8>, SignError> {
        build_cms_signed_data_internal(content, self, embed_content, code_sign_attributes)
    }

    fn sign(&self, data: &[u8]) -> Result<Vec<u8>, SignError> {
        match &self.private_key {
            CmsPrivateKey::Ecdsa(signing_key) => {
                use signature::Signer;

                let sig: p256::ecdsa::DerSignature = signing_key.sign(data);
                Ok(sig.as_bytes().to_vec())
            }
            CmsPrivateKey::RsaPss(signing_key) => {
                use signature::{RandomizedSigner, SignatureEncoding};

                let sig = signing_key.sign_with_rng(&mut rand::rngs::OsRng, data);
                Ok(sig.to_vec())
            }
        }
    }

    fn verify_generated_signature(
        &self,
        data: &[u8],
        signature_bytes: &[u8],
    ) -> Result<(), SignError> {
        match &self.verifier {
            SignatureVerifier::EcdsaP256(public_key) => {
                let verifier = ring::signature::UnparsedPublicKey::new(
                    &ring::signature::ECDSA_P256_SHA256_ASN1,
                    public_key,
                );
                verifier.verify(data, signature_bytes).map_err(|_| {
                    SignError::SigningFailed(
                        "generated ECDSA signature did not verify with leaf certificate"
                            .to_string(),
                    )
                })
            }
            SignatureVerifier::EcdsaP384(public_key) => {
                let verifier = ring::signature::UnparsedPublicKey::new(
                    &ring::signature::ECDSA_P384_SHA256_ASN1,
                    public_key,
                );
                verifier.verify(data, signature_bytes).map_err(|_| {
                    SignError::SigningFailed(
                        "generated ECDSA signature did not verify with leaf certificate"
                            .to_string(),
                    )
                })
            }
            SignatureVerifier::RsaPss(verifier) => {
                use signature::Verifier;

                let signature = rsa::pss::Signature::try_from(signature_bytes).map_err(|e| {
                    SignError::SigningFailed(format!("RSA-PSS signature parse: {e}"))
                })?;
                verifier.verify(data, &signature).map_err(|e| {
                    SignError::SigningFailed(format!(
                        "generated RSA-PSS signature did not verify with leaf certificate: {e}"
                    ))
                })
            }
        }
    }
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
    let signer = CmsSigningContext::new(private_key_der, cert_chain, alg_id)?;
    signer.build_signed_data(content, true, None)
}

fn build_cms_signed_data_internal(
    content: &[u8],
    signer: &CmsSigningContext,
    embed_content: bool,
    code_sign_attributes: Option<CodeSignAttributes<'_>>,
) -> Result<Vec<u8>, SignError> {
    // -----------------------------------------------------------------------
    // 1. digestAlgorithms SET
    // -----------------------------------------------------------------------
    // AlgorithmIdentifier { OID(SHA-256), NULL }
    let sha256_alg_id = Der::sequence(&Der::concat(&[OID_SHA256, &[0x05, 0x00]])); // OID + NULL
    let digest_algorithms = Der::set(&sha256_alg_id);

    // -----------------------------------------------------------------------
    // 2. encapContentInfo
    // -----------------------------------------------------------------------
    // ContentInfo { OID(id-data), [0] EXPLICIT { OCTET STRING(content) } }
    // hapsigner code signing uses detached content:
    // `new ContentInfo(PKCSObjectIdentifiers.data, null)`.
    let econtent_inner = if embed_content {
        Der::concat(&[
            OID_ID_DATA,
            &Der::explicit_ctx(0, &Der::octet_string(content)),
        ])
    } else {
        OID_ID_DATA.to_vec()
    };
    let encap_content_info = Der::sequence(&econtent_inner);

    // -----------------------------------------------------------------------
    // 3. Certificates ([0] IMPLICIT set of DER certs)
    // -----------------------------------------------------------------------
    let certificates = &signer.certificates;

    // -----------------------------------------------------------------------
    // 4. Signed attributes (for computing signature)
    // -----------------------------------------------------------------------
    let content_digest = {
        let mut h = Sha256::new();
        h.update(content);
        h.finalize()
    };

    // Attribute: contentType = id-data
    let attr_content_type =
        Der::sequence(&Der::concat(&[OID_CONTENT_TYPE, &Der::set(OID_ID_DATA)]));

    // Attribute: signingTime = UTCTime(now)
    let signing_time = Der::current_utctime_der();
    let attr_signing_time =
        Der::sequence(&Der::concat(&[OID_SIGNING_TIME, &Der::set(&signing_time)]));

    // Attribute: messageDigest = SHA-256(content)
    let attr_message_digest = Der::sequence(&Der::concat(&[
        OID_MESSAGE_DIGEST,
        &Der::set(&Der::octet_string(content_digest.as_slice())),
    ]));

    // Java hapsigner uses `new DERSet(new AttributeTable(tab).toASN1EncodableVector())`.
    // DER SET elements are sorted by their full DER encoding before signing.
    let mut signed_attrs = vec![attr_content_type, attr_signing_time, attr_message_digest];
    if let Some(attributes) = code_sign_attributes {
        if let Some(owner_id) = attributes.owner_id {
            signed_attrs.push(Der::sequence(&Der::concat(&[
                OID_CODE_SIGN_OWNER_ID,
                &Der::set(&Der::utf8_string(owner_id)),
            ])));
        }
        if let Some(plugin_id) = attributes.plugin_id {
            signed_attrs.push(Der::sequence(&Der::concat(&[
                OID_CODE_SIGN_PLUGIN_ID,
                &Der::set(&Der::utf8_string(plugin_id)),
            ])));
        }
    }
    signed_attrs.sort();
    let signed_attr_parts = signed_attrs.iter().map(Vec::as_slice).collect::<Vec<_>>();
    let signed_attrs_inner = Der::concat(&signed_attr_parts);

    // For signing: encode as SET (tag 0x31)
    let signed_attrs_for_signing = Der::set(&signed_attrs_inner);

    // For embedding in SignerInfo: encode as [0] IMPLICIT (tag 0xA0)
    let signed_attrs_encoded = Der::implicit_ctx(0, &signed_attrs_inner);

    // -----------------------------------------------------------------------
    // 5. Compute signature
    // -----------------------------------------------------------------------
    let signature_bytes = signer.sign(&signed_attrs_for_signing)?;
    signer.verify_generated_signature(&signed_attrs_for_signing, &signature_bytes)?;

    // -----------------------------------------------------------------------
    // 8. SignerInfo
    // -----------------------------------------------------------------------
    // digestAlgorithm AlgorithmIdentifier(SHA-256)
    let digest_alg_for_signer = Der::sequence(&Der::concat(&[OID_SHA256, &[0x05, 0x00]]));

    let signer_info_inner = Der::concat(&[
        &Der::integer_one(),                  // version
        &signer.issuer_and_serial,            // sid: IssuerAndSerialNumber
        &digest_alg_for_signer,               // digestAlgorithm
        &signed_attrs_encoded,                // signedAttrs [0] IMPLICIT
        &signer.signature_algorithm,          // signatureAlgorithm
        &Der::octet_string(&signature_bytes), // signature
    ]);
    let signer_info = Der::sequence(&signer_info_inner);

    // -----------------------------------------------------------------------
    // 9. SignedData
    // -----------------------------------------------------------------------
    let signer_infos = Der::set(&signer_info);

    let signed_data_inner = Der::concat(&[
        &Der::integer_one(), // version
        &digest_algorithms,  // digestAlgorithms
        &encap_content_info, // encapContentInfo
        &certificates,       // [0] IMPLICIT certs
        &signer_infos,       // signerInfos
    ]);
    let signed_data = Der::sequence(&signed_data_inner);

    // -----------------------------------------------------------------------
    // 10. ContentInfo wrapping
    // -----------------------------------------------------------------------
    let content_info_inner =
        Der::concat(&[OID_ID_SIGNED_DATA, &Der::explicit_ctx(0, &signed_data)]);
    let content_info = Der::sequence(&content_info_inner);

    Ok(content_info)
}
