//! Incremental build cache for the ABC compiler (SPEC-025).
//!
//! ## Upstream vs. arkdown cache semantics
//!
//! Upstream `es2abc` caches a per-file `pandasm::Program` IR in a `.protoBin`
//! binary (serialized via protobuf). For merged-ABC builds, a `modules.cache`
//! text manifest maps each source file to its `.protoBin` path:
//!
//! ```text
//! src/path/file1.js;cache/file1.protoBin
//! src/path/file2.js;cache/file2.protoBin
//! ```
//!
//! `--cache-file @path/to/modules.cache` reads this manifest at startup; a
//! bare `--cache-file path/to/file.protoBin` (no `@`) sets the cache for the
//! single input file.
//!
//! ## arkdown cache implementation
//!
//! arkdown does not implement the upstream protobuf Program IR serialization
//! (which would require a complete pandasm Program roundtrip). Instead, it
//! caches the **compiled batch ABC** keyed by a SHA-256 hash of all record
//! sources + compile options.
//!
//! The cache layout inside `<cache_dir>/`:
//!
//! ```text
//! modules.cache    — text manifest, semicolon-separated: hash;proto_bin_name
//! modules.protoBin — binary: ARKD magic + u32 hash_len + hash_bytes +
//!                                       u32 abc_len  + abc_bytes
//! ```
//!
//! The `modules.cache` manifest uses the same semicolon-separated line format
//! as upstream but stores the batch hash in the first column (instead of a
//! per-file source path). The `.protoBin` binary format starts with the
//! `ARKD` magic (4 bytes) to distinguish it from upstream protobuf wire
//! encoding and from the old JSON `modules.abc.cache` format.
//!
//! ## Removed: old JSON whole-ABC cache
//!
//! The previous `modules.abc.cache` JSON format (hex-encoded bytecode) has
//! been removed. Stale `modules.abc.cache` files are silently ignored. The
//! replacement `ModulesCache` / `modules.protoBin` format is binary and
//! version-aware.

use std::io;
use std::path::{Path, PathBuf};

use sha2::{Digest, Sha256};

/// Magic bytes for arkdown `.protoBin` cache files.
///
/// Chosen to be distinct from both upstream protobuf wire encoding (which
/// would begin with a field tag varint) and the old JSON format (`{`).
const ARKDOWN_PROTO_BIN_MAGIC: &[u8; 4] = b"ARKD";

// ── Internal manifest helpers ─────────────────────────────────────────────

struct ManifestEntry {
    hash: String,
    proto_bin_name: String,
}

fn parse_manifest(path: &Path) -> Vec<ManifestEntry> {
    let content = match std::fs::read_to_string(path) {
        Ok(s) => s,
        Err(_) => return Vec::new(),
    };
    content
        .lines()
        .filter_map(|line| {
            let mut parts = line.splitn(2, ';');
            let hash = parts.next()?.trim().to_string();
            let name = parts.next()?.trim().to_string();
            if hash.is_empty() || name.is_empty() {
                None
            } else {
                Some(ManifestEntry {
                    hash,
                    proto_bin_name: name,
                })
            }
        })
        .collect()
}

fn write_manifest(dir: &Path, entries: &[ManifestEntry]) -> io::Result<()> {
    let manifest_path = dir.join(ModulesCache::MANIFEST_NAME);
    let content: String = entries
        .iter()
        .map(|e| format!("{};{}\n", e.hash, e.proto_bin_name))
        .collect();
    std::fs::write(manifest_path, content)
}

// ── .protoBin binary format ───────────────────────────────────────────────

/// Encode `(hash_hex, abc_bytes)` into the ARKD binary format:
/// `ARKD` magic (4) + hash_len u32 LE (4) + hash_bytes + abc_len u32 LE (4) + abc_bytes.
fn encode_proto_bin(hash_hex: &str, abc: &[u8]) -> Vec<u8> {
    let hash_bytes = hash_hex.as_bytes();
    let mut out = Vec::with_capacity(4 + 4 + hash_bytes.len() + 4 + abc.len());
    out.extend_from_slice(ARKDOWN_PROTO_BIN_MAGIC);
    out.extend_from_slice(&(hash_bytes.len() as u32).to_le_bytes());
    out.extend_from_slice(hash_bytes);
    out.extend_from_slice(&(abc.len() as u32).to_le_bytes());
    out.extend_from_slice(abc);
    out
}

/// Decode an ARKD binary, returning `(hash_hex, abc_bytes)` on success.
fn decode_proto_bin(data: &[u8]) -> io::Result<(String, Vec<u8>)> {
    if data.len() < 8 {
        return Err(io::Error::new(io::ErrorKind::InvalidData, "proto_bin too short"));
    }
    if &data[0..4] != ARKDOWN_PROTO_BIN_MAGIC {
        return Err(io::Error::new(io::ErrorKind::InvalidData, "bad ARKD magic"));
    }
    let hash_len = u32::from_le_bytes(data[4..8].try_into().unwrap()) as usize;
    let hash_end = 8 + hash_len;
    if data.len() < hash_end + 4 {
        return Err(io::Error::new(
            io::ErrorKind::InvalidData,
            "proto_bin truncated before abc_len",
        ));
    }
    let hash_hex = String::from_utf8_lossy(&data[8..hash_end]).into_owned();
    let abc_len = u32::from_le_bytes(data[hash_end..hash_end + 4].try_into().unwrap()) as usize;
    let abc_start = hash_end + 4;
    if data.len() < abc_start + abc_len {
        return Err(io::Error::new(
            io::ErrorKind::InvalidData,
            "proto_bin truncated in abc data",
        ));
    }
    Ok((hash_hex, data[abc_start..abc_start + abc_len].to_vec()))
}

// ── Public API ────────────────────────────────────────────────────────────

/// Per-batch incremental build cache.
///
/// Layout inside `cache_dir/`:
///
/// ```text
/// modules.cache    — semicolon-separated manifest: hash;proto_bin_filename
/// modules.protoBin — ARKD binary: magic + hash + compiled ABC bytes
/// ```
///
/// This aligns with the upstream `--cache-file @modules.cache` convention
/// while using an ARKD-magic binary format (not upstream protobuf) for the
/// actual cache payload. The hash covers all record sources + compile options,
/// so any change to any record or option causes a full recompile.
pub struct ModulesCache {
    dir: PathBuf,
    entries: Vec<ManifestEntry>,
}

impl ModulesCache {
    /// Name of the manifest file inside the cache directory.
    pub const MANIFEST_NAME: &'static str = "modules.cache";
    /// Default name for the batch protoBin inside the cache directory.
    const PROTO_BIN_NAME: &'static str = "modules.protoBin";

    /// Load (or initialise an empty) cache from `dir`.
    pub fn load(dir: &Path) -> Self {
        let manifest_path = dir.join(Self::MANIFEST_NAME);
        let entries = parse_manifest(&manifest_path);
        Self {
            dir: dir.to_path_buf(),
            entries,
        }
    }

    /// Look up a batch hash.  Returns the compiled ABC bytes on a cache hit.
    pub fn lookup(&self, hash: &str) -> Option<Vec<u8>> {
        let entry = self.entries.iter().find(|e| e.hash == hash)?;
        let proto_bin_path = self.dir.join(&entry.proto_bin_name);
        let data = std::fs::read(&proto_bin_path).ok()?;
        let (stored_hash, abc) = decode_proto_bin(&data).ok()?;
        // Double-check hash inside the binary (defence against partial writes).
        if stored_hash != hash {
            return None;
        }
        Some(abc)
    }

    /// Store a batch `hash` → `abc` mapping.  Writes the `.protoBin` file and
    /// updates the manifest.  Errors are returned but are non-fatal to the
    /// caller.
    pub fn store(&mut self, hash: String, abc: &[u8]) -> io::Result<()> {
        std::fs::create_dir_all(&self.dir)?;
        let proto_bin_name = Self::PROTO_BIN_NAME.to_string();
        let proto_bin_path = self.dir.join(&proto_bin_name);
        let data = encode_proto_bin(&hash, abc);
        std::fs::write(&proto_bin_path, data)?;
        // Upsert manifest entry.
        if let Some(e) = self
            .entries
            .iter_mut()
            .find(|e| e.proto_bin_name == proto_bin_name)
        {
            e.hash = hash;
        } else {
            self.entries.push(ManifestEntry {
                hash,
                proto_bin_name,
            });
        }
        write_manifest(&self.dir, &self.entries)
    }

    /// Return the expected path to the `modules.cache` manifest inside `dir`.
    pub fn manifest_path_for(dir: &Path) -> PathBuf {
        dir.join(Self::MANIFEST_NAME)
    }

    /// Return the number of entries in the loaded manifest.
    pub fn entry_count(&self) -> usize {
        self.entries.len()
    }
}

/// Derive a SHA-256 cache key from a slice of `(record_name, source)` pairs
/// and compile options.
///
/// The key covers:
/// - All record names and source texts (in order)
/// - `module` flag
/// - `debug_info` flag
/// - `target_api_version` (to avoid reusing stale cache across API bumps)
pub fn derive_key(
    modules: &[(&str, &str)],
    module_mode: bool,
    debug_info: bool,
    target_api_version: u32,
) -> String {
    let mut hasher = Sha256::new();
    for (record_name, source) in modules {
        hasher.update(record_name.as_bytes());
        hasher.update(b"\x00");
        hasher.update(source.as_bytes());
        hasher.update(b"\x00");
    }
    hasher.update(if module_mode {
        b"module=1"
    } else {
        b"module=0"
    });
    hasher.update(if debug_info { b"debug=1" } else { b"debug=0" });
    // Include target API version so cache entries are not shared across API bumps.
    hasher.update(format!("api={target_api_version}").as_bytes());
    hex::encode(hasher.finalize())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_derive_key_deterministic() {
        let modules = vec![("entry/main", "export const x = 1;")];
        let k1 = derive_key(&modules, true, false, 12);
        let k2 = derive_key(&modules, true, false, 12);
        assert_eq!(k1, k2, "key must be deterministic");
    }

    #[test]
    fn test_derive_key_changes_on_source_change() {
        let m1 = vec![("entry/main", "export const x = 1;")];
        let m2 = vec![("entry/main", "export const x = 2;")];
        let k1 = derive_key(&m1, true, false, 12);
        let k2 = derive_key(&m2, true, false, 12);
        assert_ne!(k1, k2, "key must change when source changes");
    }

    #[test]
    fn test_derive_key_changes_on_api_version_change() {
        let modules = vec![("entry/main", "export const x = 1;")];
        let k_v12 = derive_key(&modules, true, false, 12);
        let k_v13 = derive_key(&modules, true, false, 13);
        assert_ne!(
            k_v12, k_v13,
            "key must change when target_api_version changes"
        );
    }

    #[test]
    fn test_modules_cache_round_trip() {
        let dir = tempfile::tempdir().unwrap();
        let abc_bytes = b"PANDA\x00\x00\x00fake_bytecode";
        let hash = "a1b2c3d4e5f6a1b2c3d4e5f6a1b2c3d4e5f6a1b2c3d4e5f6a1b2c3d4e5f6a1b2";

        let mut cache = ModulesCache::load(dir.path());
        cache.store(hash.to_string(), abc_bytes).unwrap();

        let loaded = ModulesCache::load(dir.path());
        let hit = loaded.lookup(hash).unwrap();
        assert_eq!(hit, abc_bytes);

        // Verify both manifest and protoBin files exist.
        assert!(dir.path().join(ModulesCache::MANIFEST_NAME).exists());
        assert!(dir.path().join("modules.protoBin").exists());
    }

    #[test]
    fn test_modules_cache_miss_returns_none() {
        let dir = tempfile::tempdir().unwrap();
        let cache = ModulesCache::load(dir.path());
        assert!(cache.lookup("nonexistent_hash").is_none());
    }

    #[test]
    fn test_modules_cache_hash_mismatch_returns_none() {
        let dir = tempfile::tempdir().unwrap();
        let abc_bytes = b"PANDA\x00\x00\x00";
        let hash_a = "aaaa";
        let hash_b = "bbbb";

        let mut cache = ModulesCache::load(dir.path());
        cache.store(hash_a.to_string(), abc_bytes).unwrap();

        // Looking up with a different hash should miss even if the file exists.
        let loaded = ModulesCache::load(dir.path());
        assert!(loaded.lookup(hash_b).is_none());
    }

    #[test]
    fn test_proto_bin_round_trip() {
        let hash = "deadbeef";
        let abc = b"PANDA\x00\x00\x00test";
        let encoded = encode_proto_bin(hash, abc);
        let (got_hash, got_abc) = decode_proto_bin(&encoded).unwrap();
        assert_eq!(got_hash, hash);
        assert_eq!(got_abc, abc);
    }

    #[test]
    fn test_proto_bin_bad_magic_rejected() {
        let mut bad = encode_proto_bin("hash", b"abc");
        bad[0] = b'X'; // corrupt magic
        assert!(decode_proto_bin(&bad).is_err());
    }
}
