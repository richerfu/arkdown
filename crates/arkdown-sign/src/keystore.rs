//! PKCS#12 and PEM keystore loading.
//!
//! Uses the `p12` crate for PKCS#12 parsing.

use crate::error::SignError;

/// A loaded signing key + certificate chain.
pub struct SigningKey {
    /// PKCS#8 DER-encoded private key.
    pub private_key_der: Vec<u8>,
    /// DER-encoded X.509 certificates, leaf first.
    pub cert_chain: Vec<Vec<u8>>,
}

impl SigningKey {
    /// Load from a PKCS#12 (.p12 / .pfx) file.
    ///
    /// `key_alias` is currently unused (the p12 crate does not filter by alias),
    /// but kept in the signature for API compatibility with the Java tool.
    pub fn from_pkcs12(
        p12_bytes: &[u8],
        _store_password: &str,
        _key_alias: &str,
        key_password: &str,
    ) -> Result<Self, SignError> {
        use p12::PFX;

        let pfx = PFX::parse(p12_bytes)
            .map_err(|e| SignError::KeystoreParseError(format!("PKCS#12 parse: {e:?}")))?;

        // Extract private key(s)
        let keys = pfx
            .key_bags(key_password)
            .map_err(|e| SignError::KeystoreParseError(format!("key bags: {e:?}")))?;

        if keys.is_empty() {
            return Err(SignError::NoPrivateKey);
        }
        let private_key_der = keys.into_iter().next().unwrap();

        // Extract certificate(s) — leaf first (same order p12 typically stores them)
        let certs = pfx
            .cert_x509_bags(key_password)
            .map_err(|e| SignError::KeystoreParseError(format!("cert bags: {e:?}")))?;

        if certs.is_empty() {
            return Err(SignError::NoCertificate);
        }

        Ok(SigningKey {
            private_key_der,
            cert_chain: certs,
        })
    }

    /// Load from a separate PEM private key + PEM certificate.
    ///
    /// The PEM private key must be PKCS#8 (`BEGIN PRIVATE KEY`) format.
    /// The PEM certificate must be X.509 (`BEGIN CERTIFICATE`) format.
    pub fn from_pem(private_key_pem: &str, cert_pem: &str) -> Result<Self, SignError> {
        // Decode raw PKCS#8 DER from PEM (base64 between BEGIN/END markers).
        let private_key_der = pkcs8_der_from_pem(private_key_pem)?;
        let cert_der = cert_der_from_pem(cert_pem)?;

        Ok(SigningKey {
            private_key_der,
            cert_chain: vec![cert_der],
        })
    }
}

// ---------------------------------------------------------------------------
// PEM helpers
// ---------------------------------------------------------------------------

/// Extract PKCS#8 DER bytes from a PEM-encoded private key string.
fn pkcs8_der_from_pem(pem: &str) -> Result<Vec<u8>, SignError> {
    // Minimal PEM base64 decoder — look for BEGIN/END markers.
    let pem = pem.trim();
    let b64 = pem
        .lines()
        .filter(|l| !l.starts_with("-----"))
        .collect::<Vec<_>>()
        .join("");
    base64_decode(&b64).map_err(|e| SignError::PemError(format!("base64 decode: {e}")))
}

/// Extract DER bytes from a PEM-encoded certificate string.
fn cert_der_from_pem(pem: &str) -> Result<Vec<u8>, SignError> {
    let pem = pem.trim();
    let b64 = pem
        .lines()
        .filter(|l| !l.starts_with("-----"))
        .collect::<Vec<_>>()
        .join("");
    base64_decode(&b64).map_err(|e| SignError::PemError(format!("base64 decode: {e}")))
}

/// Minimal base64 decoder (standard alphabet).
fn base64_decode(input: &str) -> Result<Vec<u8>, String> {
    const TABLE: &[u8; 64] =
        b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/";

    let mut lookup = [0xFFu8; 256];
    for (i, &c) in TABLE.iter().enumerate() {
        lookup[c as usize] = i as u8;
    }

    let bytes: Vec<u8> = input
        .bytes()
        .filter(|&b| b != b'\n' && b != b'\r' && b != b' ')
        .collect();

    let mut output = Vec::with_capacity((bytes.len() * 3) / 4);
    let mut i = 0;
    while i < bytes.len() {
        let b0 = bytes.get(i).copied().unwrap_or(b'=');
        let b1 = bytes.get(i + 1).copied().unwrap_or(b'=');
        let b2 = bytes.get(i + 2).copied().unwrap_or(b'=');
        let b3 = bytes.get(i + 3).copied().unwrap_or(b'=');

        let v0 = if b0 == b'=' { 0u8 } else { lookup[b0 as usize] };
        let v1 = if b1 == b'=' { 0u8 } else { lookup[b1 as usize] };
        let v2 = if b2 == b'=' { 0u8 } else { lookup[b2 as usize] };
        let v3 = if b3 == b'=' { 0u8 } else { lookup[b3 as usize] };

        if v0 == 0xFF || v1 == 0xFF {
            return Err(format!("invalid base64 char at position {i}"));
        }

        output.push((v0 << 2) | (v1 >> 4));
        if b2 != b'=' {
            if v2 == 0xFF {
                return Err(format!("invalid base64 char at position {}", i + 2));
            }
            output.push((v1 << 4) | (v2 >> 2));
        }
        if b3 != b'=' {
            if v3 == 0xFF {
                return Err(format!("invalid base64 char at position {}", i + 3));
            }
            output.push((v2 << 6) | v3);
        }
        i += 4;
    }
    Ok(output)
}
