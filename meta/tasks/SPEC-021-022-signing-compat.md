# SPEC-021: HAP Signing (Pure Rust)

# SPEC-022: hvigorw Compatibility Wrapper

---

## SPEC-021: HAP Signing

**Crate**: `arkdown-sign`
**Priority**: P1
**Blocked by**: SPEC-012 (packager)
**Blocks**: —

### Summary

Signs `.hap`/`.hsp` in **pure Rust** — no Java subprocess, no SDK binary dependency.
Ports `developtools_hapsigner` (Apache 2.0) from Java/BouncyCastle to Rust.

Reference implementations:
- `https://github.com/openharmony/developtools_hapsigner` — Java hapsigner source
- `https://github.com/openharmony/security_code_signature` — verification side

---

## HAP Signing Block Binary Format

The signing block is inserted between the last local file entry and the Central Directory.

### Overall signing block layout (little-endian)

```
For each optional block (TLV header, 12 bytes each):
  u32: type      (e.g. 0x20000000 for main sig, 0x20000002 for profile)
  u32: length    (byte length of this block's value)
  u32: offset    (byte offset from start of block-data section)

Block data section (concatenated values):
  [block 0 bytes]
  [block 1 bytes]
  ...

u32:    block count
u64:    total signing block size (includes this u64 + magic + u32 version)
16 bytes: magic
  V2 (compatibleSdkVersion < 8): b"HAP Sig Block 42"
  V3 (compatibleSdkVersion >= 8): b"<hap sign block>"
u32:    version (2 for V2, 3 for V3)
```

### Block IDs

| Constant                         | Value        | Meaning                              |
| -------------------------------- | ------------ | ------------------------------------ |
| `HAP_SIGNATURE_SCHEME_V1`        | `0x20000000` | Main CMS SignedData block            |
| `PROOF_OF_ROTATION`              | `0x20000001` | Proof-of-rotation certificate chain  |
| `PROFILE_BLOCK`                  | `0x20000002` | Embedded `.p7b` profile              |
| `PROPERTY_BLOCK`                 | `0x20000003` | Build property metadata              |

---

## Content Digest Algorithm

Split the unsigned HAP into four logical sections:

1. All local file entry bytes (from offset 0 up to CD offset, minus any existing signing block)
2. Signing block placeholder (zero-length initially; signing block bytes during verification)
3. Central Directory bytes
4. EOCD bytes (with the CD offset field zeroed out for signing, restored for verification)

Process each section as a stream of **1 MB chunks** (`CHUNK_SIZE = 1_048_576`).

```
For each chunk:
  write byte 0xa5                        (second-level chunk marker)
  write u32 LE: chunk_content_size       (size of the chunk's actual data)
  write chunk bytes

Then prefix everything with:
  byte 0x5a                              (first-level marker)
  u32 LE: total_chunk_count              (across all sections)

Final: SHA-256 of [0x5a | u32(count) | 0xa5 | u32(size0) | chunk0 | 0xa5 | u32(size1) | chunk1 | ...]
```

Optional blocks' `.value` bytes are also fed into `sha256.update()` before finalising,
after all content chunks are processed.

---

## CMS / PKCS#7 SignedData Structure

The `HAP_SIGNATURE_SCHEME_V1` block value is a DER-encoded CMS `SignedData`:

```
ContentInfo {
  contentType: id-signedData (1.2.840.113549.1.7.2)
  content: SignedData {
    version: 1
    digestAlgorithms: SET { AlgorithmIdentifier(SHA-256) }
    encapContentInfo: ContentInfo {
      contentType: id-data (1.2.840.113549.1.7.1)
      content: OCTET STRING { encoded (alg_id, digest) pairs — see below }
    }
    certificates: [0] IMPLICIT SEQUENCE { leaf cert, intermediate certs... }
    signerInfos: SET {
      SignerInfo {
        version: 1
        sid: IssuerAndSerialNumber { issuer, serialNumber }
        digestAlgorithm: AlgorithmIdentifier(SHA-256)
        signedAttrs: [0] IMPLICIT SET {
          Attribute(id-contentType = id-data)
          Attribute(id-signingTime = UTCTime(now))
          Attribute(id-messageDigest = SHA-256(encapContentInfo.content))
        }
        signatureAlgorithm: AlgorithmIdentifier(id-ecPublicKey or rsaPSS)
        signature: OCTET STRING { sign(DER(signedAttrs), privateKey) }
      }
    }
  }
}
```

### Content of `encapContentInfo.content` (the "unsigned HAP digest" blob)

```
For each signature algorithm (usually just one):
  u32 LE: algorithm ID  (e.g. 0x201 for ECDSA_WITH_SHA256)
  u32 LE: digest length
  bytes:  SHA-256 content digest (computed above)
```

---

## Signature Algorithm IDs

| ID      | Algorithm                    | JCA name                  |
| ------- | ---------------------------- | ------------------------- |
| `0x101` | RSA-PSS + SHA-256            | `SHA256withRSA/PSS`       |
| `0x102` | RSA-PSS + SHA-384            | `SHA384withRSA/PSS`       |
| `0x103` | RSA-PSS + SHA-512            | `SHA512withRSA/PSS`       |
| `0x104` | RSA PKCS#1 v1.5 + SHA-256   | `SHA256withRSA`           |
| `0x201` | ECDSA + SHA-256 (P-256)     | `SHA256withECDSA`         |
| `0x202` | ECDSA + SHA-384              | `SHA384withECDSA`         |
| `0x203` | ECDSA + SHA-512              | `SHA512withECDSA`         |

Most HarmonyOS builds use `0x201` (ECDSA_WITH_SHA256).

---

## ZIP Alignment

Before signing, the ZIP must be aligned:

- Entries sorted: runnable uncompressed (.abc, .so) → bitmap → other uncompressed → compressed
- `.abc` and `.so` files (stored, not deflate): **4096-byte** alignment
- First compressed-method entry after runnable block: 4096-byte alignment
- All other files: **4-byte** alignment (or configurable)
- Alignment achieved by padding the `ZipEntry` local file header's extra field

---

## Files to Create / Rewrite

```
crates/arkdown-sign/src/
├── lib.rs          ← HapSigner public API
├── error.rs        ← SignError
├── zip.rs          ← ZIP parse, sort, align, signing-block insertion
├── digest.rs       ← HAP content digest (chunked SHA-256)
├── signing_block.rs← signing block TLV assembly + magic
├── pkcs7.rs        ← CMS SignedData builder (pure Rust)
└── keystore.rs     ← PKCS#12 loading → (PrivateKey, cert chain)
```

### Cargo.toml additions (workspace)

```toml
# Crypto
p256        = { version = "0.13", features = ["ecdsa"] }
rsa         = { version = "0.9",  features = ["sha2"] }
sha2        = { version = "0.10" }
signature   = { version = "2" }
# ASN.1 / CMS
der         = { version = "0.7" }
x509-cert   = { version = "0.2" }
pkcs12      = { version = "0.1" }
cms         = { version = "0.2" }
const-oid   = { version = "0.9" }
spki        = { version = "0.7" }
# Random (for signing time)
rand        = { version = "0.8" }
```

---

## Public API

```rust
pub struct HapSigner {
    key:   PrivateKeyInfo,   // from PKCS#12
    certs: Vec<Certificate>, // full chain
    profile: Vec<u8>,        // raw .p7b bytes
    alg_id: SignatureAlgorithmId,
}

pub enum SignatureAlgorithmId {
    EcdsaWithSha256 = 0x201,
    RsaPssWithSha256 = 0x101,
    // ...
}

impl HapSigner {
    /// Load from PKCS#12 keystore.
    pub fn from_pkcs12(
        p12_bytes: &[u8],
        store_password: &str,
        key_alias: &str,
        key_password: &str,
        cert_path: &Path,
        profile_path: &Path,
        alg: SignatureAlgorithmId,
    ) -> Result<Self, SignError>;

    /// Sign an unsigned HAP/HSP in place (overwrites output_path).
    pub fn sign(&self, unsigned_hap: &[u8]) -> Result<Vec<u8>, SignError>;
}
```

---

## Acceptance Criteria

- `sign()` produces a HAP accepted by `hdc install` on OHOS device
- No Java, no SDK binary invocation
- Password inputs as plaintext (DevEco-encrypted passwords: accept via
  `ARKDOWN_KEY_PASSWORD` / `ARKDOWN_STORE_PASSWORD` env vars as plaintext override)
- Magic bytes match expected V2/V3 values
- SHA-256 chunk digest reproducible against reference Java tool output

---

## Design References

- `developtools_hapsigner` source:
  - `hap/sign/SignHap.java` — signing block assembly + digest
  - `hap/sign/BcPkcs7Generator.java` — CMS SignedData via BouncyCastle
  - `hap/utils/HapUtils.java` — magic constants, block IDs, chunk digest
  - `zip/Zip.java` — ZIP parsing, alignment, signing block location
  - `entity/SignatureAlgorithm.java` — algorithm IDs

---

## Rolldown Alignment

`arkdown-sign` has no rolldown equivalent. rolldown produces JS files; it does not sign ZIP archives. HAP signing is unique to the OHOS ecosystem.

| arkdown concept | closest analogue | note |
| --- | --- | --- |
| `HapSigner::sign()` | npm `publish` code signing / macOS codesign | Both produce signed artifacts; completely different formats and protocols |
| PKCS#12 keystore loading | Node.js `crypto` module PKCS#12 support | Same standard; arkdown uses `pkcs12` Rust crate (no Java/BouncyCastle) |
| CMS SignedData (PKCS#7) | no JS bundler equivalent | Standard cryptographic format; arkdown uses `cms` Rust crate |
| ZIP alignment (4096-byte for `.abc`/`.so`) | no rolldown equivalent | OHOS-specific requirement for mmap-based loading |
| HAP signing block insertion | no rolldown equivalent | OHOS-specific: signing block is inserted between last local file entry and Central Directory |

---

## Test Requirements (SPEC-021)

- V3 magic bytes: produced HAP bytes at signing block offset contain `b"<hap sign block>"` (for `compatibleSdkVersion >= 8`)
- V2 magic bytes: produced HAP bytes at signing block offset contain `b"HAP Sig Block 42"` (for `compatibleSdkVersion < 8`)
- Signing block `HAP_SIGNATURE_SCHEME_V1` block ID `0x20000000` is present in the signing block
- Profile block `PROFILE_BLOCK` block ID `0x20000002` is present when a `.p7b` profile is provided
- Content digest: SHA-256 of the chunked content is reproducible — signing the same unsigned HAP twice with the same key produces the same digest (but different CMS signature due to `signingTime`)
- Content digest chunk size: each chunk is exactly 1 MB (`1_048_576` bytes) except the final chunk which may be smaller
- Type markers: first byte of content digest input is `0x5a`; each chunk prefixed with `0xa5`
- ZIP alignment: `.abc` entries have 4096-byte alignment in the produced signed HAP
- ZIP alignment: `.so` entries have 4096-byte alignment
- ZIP alignment: non-runnable entries have 4-byte alignment
- ECDSA P-256 signature: `sign()` with algorithm `EcdsaWithSha256` produces a valid signature verifiable with the public key from the certificate
- RSA-PSS signature: `sign()` with algorithm `RsaPssWithSha256` produces a valid RSA-PSS signature
- `HapSigner::from_pkcs12()` with incorrect store password returns `Err(SignError::KeystoreDecryptFailed)`
- `sign()` output accepted by `hdc install` on an OHOS device (acceptance criterion; requires device)
- No Java subprocess is invoked at any point — verified by asserting no `Command::new("java")` exists in the crate

---

## SPEC-022: hvigorw Compatibility Wrapper

**Crate**: `arkdown-cli` (extension)
**Priority**: P1
**Blocked by**: SPEC-013

### Summary

Provide a `hvigorw`-compatible wrapper script so projects can swap out hvigor without changing their CI pipelines. Maps hvigor task invocations to `arkdown build` subcommands.

### Command Mapping

```
hvigorw assembleHap --mode debug    →  arkdown build --target hap --mode debug
hvigorw assembleHsp --mode release  →  arkdown build --target hsp --mode release
hvigorw assembleHar                 →  arkdown build --target har
hvigorw clean                       →  arkdown clean
hvigorw test                        →  arkdown test
```

### Implementation

Add a `hvigorw` binary target to `arkdown-cli/Cargo.toml`:

```toml
[[bin]]
name = "hvigorw"
path = "src/hvigorw.rs"
```

`hvigorw.rs` parses the hvigor task name and flags, maps them to `arkdown` subcommands, and calls the same pipeline as `arkdown build`.

### Rolldown Alignment

No rolldown equivalent. This is a compatibility shim specific to OHOS project tooling.

### Test Requirements

- `hvigorw assembleHap --mode debug` exits with code 0 and produces a `.hap` (same as `arkdown build --target hap --mode debug`)
- `hvigorw assembleHsp --mode release` → `arkdown build --target hsp --mode release` (same output)
- `hvigorw clean` → deletes `build/` directory
- Unknown hvigor task name (e.g., `hvigorw unknownTask`) → exits with code 2 and prints a descriptive error
