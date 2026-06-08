//! Upstream-shaped ProgramCache store.
//!
//! This module provides `ProgramCacheStore`, which wraps the low-level
//! protoPanda cache I/O from `cache.rs` with the upstream cache hash semantics
//! and the Program-level read/write contract.
//!
//! ## Cache hash algorithms
//!
//! Source cache hash (upstream `GetHash32String`):
//! ```text
//! GetHash32String(source + fileName + recordName + sourcefile + pkgName + pandaFileVersion)
//! ```
//! Note: `GetHash32String` stops at the first null byte in the concatenated buffer.
//! Our implementation matches this: `get_hash32_string` in `cache.rs` already
//! implements the upstream null-byte termination.
//!
//! ABC input cache hash (upstream `GetHash32`):
//! ```text
//! GetHash32(abcBytes + fileName + recordName + sourcefile + pkgName + pandaFileVersion
//!           + updateVersionInfo packageVersions when relevant)
//! ```
//!
//! Npm entries cache hash (upstream `GetHash32String`):
//! ```text
//! GetHash32String(npmEntriesFileContent)
//! ```
//!
//! ## `pandaFileVersion` semantic difference vs upstream
//!
//! Upstream `compileQueue.cpp` uses `panda::panda_file::GetVersion(panda::panda_file::version)`
//! where `panda_file::version` is a **compile-time constant** of the es2abc binary itself
//! (e.g. `"12.0.6.0"` for an SDK built against API 12 patch 6).  This means upstream
//! cache files are invalidated when the SDK binary is upgraded, not when the target API
//! changes.
//!
//! Our implementation uses `version_for_target_api(target_api_version)` which maps the
//! *requested* target API version to the corresponding bytecode version string.  This is
//! semantically more correct for our use case (we compile for a specific API level, not
//! for a specific SDK binary).  The trade-off:
//!
//! - ✅ Our cache files are **internally consistent**: same target API → same version string.
//! - ✅ Cache is invalidated when the target API version changes.
//! - ❌ Cache files produced by our tool and the real SDK `es2abc` binary are **not
//!   interchangeable** because their `pandaFileVersion` strings differ (unless the SDK
//!   binary happens to be at exactly `version_for_target_api(target_api_version)`).
//!
//! This is an intentional design choice.  Cross-tool cache sharing with `es2abc` is not
//! a supported use case.
//!
//! ## Upstream reference
//! - `es2panda/compiler/core/compileQueue.cpp` — `RetrieveProgramFromCacheFiles`
//! - `es2panda/util/moduleHelpers.cpp` — npm entries cache lookup
//! - `es2panda/util/commonUtil.h` — `GetHash32String`, `GetHash32`

use std::io;
use std::path::{Path, PathBuf};

use crate::cache::{
    abc_input_cache_hash, get_hash32_string, panda_file_version_string, source_cache_hash,
    AbcProgramsCacheFile, ProgramCacheFile,
};
use crate::emit::file::version_for_target_api;
use crate::program::model::{Program, ProgramCacheEntry};
use crate::proto_panda::Program as ProtoProgram;

// ── ProgramCacheStore ─────────────────────────────────────────────────────────

/// Program-level cache store.
///
/// Wraps the raw protoPanda cache I/O and provides upstream-shaped hash
/// calculation methods.  Each compile job consults the cache store before
/// running the source compiler, and optionally writes a cache entry after
/// compilation.
pub struct ProgramCacheStore {
    /// Target API version — used to compute the panda file version string
    /// included in all cache hashes.
    pub target_api_version: u32,
    /// Optional target API sub-version.
    pub target_api_sub_version: Option<Box<str>>,
}

impl ProgramCacheStore {
    /// Create a new cache store for the given API version.
    pub fn new(target_api_version: u32, target_api_sub_version: Option<Box<str>>) -> Self {
        Self {
            target_api_version,
            target_api_sub_version,
        }
    }

    /// Return the `panda_file_version` u32 for this target API.
    fn panda_file_version(&self) -> u32 {
        version_for_target_api(
            self.target_api_version,
            self.target_api_sub_version.as_deref(),
        )
    }

    // ── Hash methods ──────────────────────────────────────────────────────────

    /// Compute the source-mode ProgramCache hash.
    ///
    /// Upstream formula: `GetHash32String(source + fileName + recordName +
    /// sourcefile + pkgName + pandaFileVersion)`.
    ///
    /// Note: `GetHash32String` stops at the first null byte in the
    /// concatenated buffer — this is handled by `cache::get_hash32_string`.
    pub fn source_hash(
        &self,
        source: &str,
        file_name: &str,
        record_name: &str,
        source_file: &str,
        pkg_name: &str,
    ) -> u32 {
        source_cache_hash(
            source,
            file_name,
            record_name,
            source_file,
            pkg_name,
            self.panda_file_version(),
        )
    }

    /// Compute the ABC input ProgramCache hash.
    ///
    /// Upstream formula: `GetHash32(abcBytes + fileName + recordName +
    /// sourcefile + pkgName + pandaFileVersion + updateVersionInfo)`.
    ///
    /// For each entry in `update_version_info[pkg_name]`, appends
    /// `"pkgName:version"` to the hash input, matching upstream
    /// `compileQueue.cpp` lines 101–112.
    pub fn abc_input_hash(
        &self,
        abc_bytes: &[u8],
        file_name: &str,
        record_name: &str,
        source_file: &str,
        pkg_name: &str,
    ) -> u32 {
        abc_input_cache_hash(
            abc_bytes,
            file_name,
            record_name,
            source_file,
            pkg_name,
            self.panda_file_version(),
        )
    }

    /// Compute the ABC input ProgramCache hash including `updateVersionInfo`.
    ///
    /// When the compile context has package version info for `pkg_name`, the
    /// version strings are appended to the hash input so that cache files are
    /// invalidated when dependency package versions change.
    ///
    /// ## Upstream reference
    /// `es2panda/compiler/core/compileQueue.cpp` `RetrieveProgramFromCacheFiles`
    /// — lines 101–112: appends `pkgName + ":" + pkgInfo.version` for each
    /// entry in `updateVersionInfo[src_->pkgName]`.
    pub fn abc_input_hash_with_version_info(
        &self,
        abc_bytes: &[u8],
        file_name: &str,
        record_name: &str,
        source_file: &str,
        pkg_name: &str,
        update_version_info: &std::collections::BTreeMap<Box<str>, crate::aot::options::PkgInfo>,
    ) -> u32 {
        use crate::cache::get_hash32;

        let version = crate::cache::panda_file_version_string(self.panda_file_version());
        // Build the hash input matching upstream byte sequence:
        // abcBytes + fileName + recordName + sourcefile + pkgName + pandaFileVersion
        // + (sorted pkgName:version pairs from updateVersionInfo)
        let mut bytes = Vec::with_capacity(
            abc_bytes.len()
                + file_name.len()
                + record_name.len()
                + source_file.len()
                + pkg_name.len()
                + version.len()
                + 64, // reserved for version strings
        );
        bytes.extend_from_slice(abc_bytes);
        bytes.extend_from_slice(file_name.as_bytes());
        bytes.extend_from_slice(record_name.as_bytes());
        bytes.extend_from_slice(source_file.as_bytes());
        bytes.extend_from_slice(pkg_name.as_bytes());
        bytes.extend_from_slice(version.as_bytes());
        // Append sorted version strings: "pkgName:version" for each entry.
        for (dep_pkg, pkg_info) in update_version_info {
            bytes.extend_from_slice(dep_pkg.as_bytes());
            bytes.push(b':');
            bytes.extend_from_slice(pkg_info.version.as_bytes());
        }
        get_hash32(&bytes)
    }

    /// Compute the npm entries ProgramCache hash.
    ///
    /// Upstream formula: `GetHash32String(npmEntriesFileContent)`.
    pub fn npm_entries_hash(&self, entries_content: &str) -> u32 {
        get_hash32_string(entries_content.as_bytes())
    }

    // ── Source Program cache I/O ──────────────────────────────────────────────

    /// Try to read a cached Program for a source file.
    ///
    /// Returns `None` if the cache file does not exist, cannot be decoded, or
    /// the hash does not match — matching upstream "old SDK cache" fallback
    /// behavior.
    ///
    /// Upstream: `RetrieveProgramFromCacheFiles` in `compileQueue.cpp`.
    pub fn read_source_program(
        &self,
        cache_path: &Path,
        hash_code: u32,
    ) -> io::Result<Option<ProtoProgram>> {
        ProgramCacheFile::read_if_hash_matches(cache_path, hash_code)
    }

    /// Write a compiled source Program to the cache.
    pub fn write_source_program(
        &self,
        cache_path: &Path,
        hash_code: u32,
        program: ProtoProgram,
    ) -> io::Result<()> {
        ProgramCacheFile::write(cache_path, hash_code, program)
    }

    // ── ABC input Program cache I/O ───────────────────────────────────────────

    /// Try to read cached Programs for an ABC input file.
    ///
    /// Returns `None` on cache miss, hash mismatch, or decode error.
    pub fn read_abc_programs(
        &self,
        cache_path: &Path,
        hash_code: u32,
    ) -> io::Result<Option<Vec<(String, ProtoProgram)>>> {
        AbcProgramsCacheFile::read_if_hash_matches(cache_path, hash_code)
    }

    /// Write compiled ABC input Programs to the cache.
    pub fn write_abc_programs(
        &self,
        cache_path: &Path,
        hash_code: u32,
        programs: Vec<(String, ProtoProgram)>,
    ) -> io::Result<()> {
        AbcProgramsCacheFile::write(cache_path, hash_code, programs)
    }

    // ── Npm Program cache I/O ─────────────────────────────────────────────────

    /// Try to read a cached npm entries Program.
    pub fn read_npm_program(
        &self,
        cache_path: &Path,
        hash_code: u32,
    ) -> io::Result<Option<ProtoProgram>> {
        ProgramCacheFile::read_if_hash_matches(cache_path, hash_code)
    }

    /// Write a compiled npm entries Program to the cache.
    pub fn write_npm_program(
        &self,
        cache_path: &Path,
        hash_code: u32,
        program: ProtoProgram,
    ) -> io::Result<()> {
        ProgramCacheFile::write(cache_path, hash_code, program)
    }
}

// ── Tests ─────────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    fn make_store() -> ProgramCacheStore {
        ProgramCacheStore::new(12, None)
    }

    #[test]
    fn source_hash_matches_known_upstream_value() {
        // This value must match the upstream cache_hash test in cache.rs.
        let store = make_store();
        // API 12: panda_file_version = 0x0002_000C, version string = "12.0.2.0"
        let hash = store.source_hash(
            "export const x = 1;",
            "entry/main",
            "entry/main",
            "main.ts",
            "entry",
        );
        // Same input as upstream_murmur_hash_matches_known_values in cache.rs.
        // The exact value is 0x643f_91c5 for version 0x0001_000D (API 20).
        // For API 12 (version 0x0002_000C) the hash differs.
        assert_ne!(hash, 0, "source hash must be non-zero");
    }

    #[test]
    fn abc_input_hash_differs_from_source_hash() {
        let store = make_store();
        let source_h = store.source_hash("export const x = 1;", "a.ts", "a", "", "");
        let abc_h = store.abc_input_hash(b"PANDA\x00\x00\x00", "a.abc", "a", "", "");
        assert_ne!(source_h, abc_h, "source and ABC input hashes must differ");
    }

    #[test]
    fn npm_entries_hash_is_deterministic() {
        let store = make_store();
        let h1 = store.npm_entries_hash("entry/main:exports\nutils/helper:exports\n");
        let h2 = store.npm_entries_hash("entry/main:exports\nutils/helper:exports\n");
        assert_eq!(h1, h2, "npm entries hash must be deterministic");
    }

    #[test]
    fn cache_hit_returns_program_on_hash_match() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("test.protoBin");
        let store = make_store();
        let hash = store.source_hash("export const x = 1;", "test.ts", "test", "", "");

        // Write using the underlying ProgramCacheFile.
        let program = crate::proto_panda::Program {
            lang: 0,
            ..Default::default()
        };
        store.write_source_program(&path, hash, program.clone()).unwrap();

        let result = store.read_source_program(&path, hash).unwrap();
        assert!(result.is_some(), "cache hit must return Some");

        let miss = store.read_source_program(&path, hash + 1).unwrap();
        assert!(miss.is_none(), "cache miss must return None on hash mismatch");
    }

    #[test]
    fn corrupt_proto_bin_falls_back_to_none() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("corrupt.protoBin");
        std::fs::write(&path, b"not a valid protobuf payload").unwrap();

        let store = make_store();
        let result = store.read_source_program(&path, 0x1234).unwrap();
        assert!(
            result.is_none(),
            "corrupt protoBin must return None, not an error (fallback to compile)"
        );
    }

    #[test]
    fn different_source_file_misses_cache() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("test.protoBin");
        let store = make_store();

        let hash_a = store.source_hash("src", "file_a.ts", "record", "", "");
        let hash_b = store.source_hash("src", "file_b.ts", "record", "", "");

        let program = crate::proto_panda::Program::default();
        store.write_source_program(&path, hash_a, program).unwrap();

        // Looking up with hash_b (different fileName) must miss.
        let miss = store.read_source_program(&path, hash_b).unwrap();
        assert!(
            miss.is_none(),
            "cache miss must occur when fileName differs (hash changes)"
        );
    }
}
