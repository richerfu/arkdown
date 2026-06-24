//! Integrated HSP (embedded remote HSP) processing.
//!
//! When a HAP depends on a remote HSP that is marked `isIntegratedHsp: true`, the HSP must be
//! embedded directly inside the HAP archive. Before embedding, the HSP's `pack.info` is
//! "normalized" — its `bundleName` and `versionCode` are patched to match the host HAP.
//!
//! This ports `ProcessIntegratedHsp` and `generateIntegratedHspCommand` from
//! `hvigor-ohos-plugin/src/tasks/process-integrated-hsp.js` and
//! `hvigor-ohos-plugin/src/utils/integrated-hsp-utils.js`.
//!
//! # Layout inside the HAP
//!
//! ```text
//! entry.hap
//! └── libs/
//!     └── <hsp_dir_name>/
//!         └── <hsp_file_name>.hsp   ← normalized HSP (patched pack.info)
//! ```

use std::io::Read;
use std::path::{Path, PathBuf};

use arkdown_path::ModuleSpecifierText;

use crate::error::PackError;
use crate::zip_writer;

/// Descriptor for a single remote HSP to be integrated into a HAP.
#[derive(Debug, Clone)]
pub struct IntegratedHspEntry {
    /// Directory name used inside the HAP `libs/` tree (e.g. `"mylib@1.0.0"`).
    pub hsp_dir_name: String,
    /// Filename of the `.hsp` archive (e.g. `"mylib.hsp"`).
    pub hsp_file_name: String,
    /// Path to the pre-built `.hsp` file on disk.
    pub hsp_path: PathBuf,
}

/// Session-independent candidate used to build integrated HSP pack entries.
#[derive(Debug, Clone)]
pub struct IntegratedHspCandidate {
    pub alias: String,
    pub package_root: PathBuf,
    pub version: String,
}

/// Converts dependency graph candidates into normalized HSP package entries.
pub struct IntegratedHspCollector<'a> {
    candidates: &'a [IntegratedHspCandidate],
}

impl<'a> IntegratedHspCollector<'a> {
    pub fn new(candidates: &'a [IntegratedHspCandidate]) -> Self {
        Self { candidates }
    }

    pub fn collect(&self) -> Vec<IntegratedHspEntry> {
        let mut entries = Vec::new();
        for candidate in self.candidates {
            let package_name = IntegratedHspPackageName::new(&candidate.alias);
            let archive_name = IntegratedHspArchiveName::new(package_name.short_name());
            let hsp_file_name = archive_name.hsp_file_name();
            let hsp_path = candidate.package_root.join(&hsp_file_name);
            if !hsp_path.exists() {
                tracing::warn!(
                    dep = candidate.alias,
                    "K5: integrated HSP dependency has no pre-built .hsp file; skipping"
                );
                continue;
            }
            entries.push(IntegratedHspEntry {
                hsp_dir_name: archive_name.versioned_dir(&candidate.version),
                hsp_file_name,
                hsp_path,
            });
        }
        entries
    }

    pub fn collect_normalized(
        &self,
        bundle_name: &str,
        version_code: u64,
        output_dir: &Path,
    ) -> Result<Vec<IntegratedHspEntry>, PackError> {
        let entries = self.collect();
        if entries.is_empty() {
            return Ok(Vec::new());
        }
        let normalized_paths =
            normalize_integrated_hsps(&entries, bundle_name, version_code, output_dir)?;
        Ok(entries
            .into_iter()
            .zip(normalized_paths)
            .map(|(mut entry, path)| {
                entry.hsp_path = path;
                entry
            })
            .collect())
    }
}

struct IntegratedHspPackageName<'a> {
    alias: &'a str,
}

impl<'a> IntegratedHspPackageName<'a> {
    fn new(alias: &'a str) -> Self {
        Self { alias }
    }

    fn short_name(&self) -> &'a str {
        ModuleSpecifierText::new(self.alias)
            .package_module_name_candidate()
            .unwrap_or(self.alias)
    }
}

struct IntegratedHspArchiveName<'a> {
    name: &'a str,
}

impl<'a> IntegratedHspArchiveName<'a> {
    fn new(name: &'a str) -> Self {
        Self { name }
    }

    fn hsp_file_name(&self) -> String {
        let mut file_name = String::with_capacity(self.name.len() + ".hsp".len());
        file_name.push_str(self.name);
        file_name.push_str(".hsp");
        file_name
    }

    fn versioned_dir(&self, version: &str) -> String {
        let mut dir = String::with_capacity(self.name.len() + 1 + version.len());
        dir.push_str(self.name);
        dir.push('@');
        dir.push_str(version);
        dir
    }
}

impl IntegratedHspEntry {
    /// Normalize this HSP: patch `pack.info` and write the result to
    /// `output_dir/<hsp_dir_name>/<hsp_file_name>`.
    fn normalize(
        &self,
        bundle_name: &str,
        version_code: u64,
        output_dir: &Path,
    ) -> Result<PathBuf, PackError> {
        // --- open source HSP ---
        let src_file = fs_err::File::open(&self.hsp_path)?;
        let mut src_archive = zip::ZipArchive::new(src_file)?;

        // --- locate pack.info ---
        // We need to verify it exists before we start writing the output.
        let has_pack_info = (0..src_archive.len()).any(|i| {
            src_archive
                .by_index(i)
                .map(|e| e.name() == "pack.info")
                .unwrap_or(false)
        });
        if !has_pack_info {
            return Err(PackError::MissingInput("pack.info".to_string()));
        }

        // --- create output directory and file ---
        let out_dir = output_dir.join(&self.hsp_dir_name);
        fs_err::create_dir_all(&out_dir)?;
        let out_path = out_dir.join(&self.hsp_file_name);
        let mut writer = zip_writer::create_zip(&out_path)?;

        // --- copy entries, patching pack.info ---
        for i in 0..src_archive.len() {
            let mut src_entry = src_archive.by_index(i)?;
            let name = src_entry.name().to_owned();
            let compression = src_entry.compression();

            if name == "pack.info" {
                // Read, parse, patch, re-serialize.
                let mut raw = Vec::new();
                src_entry.read_to_end(&mut raw)?;
                let patched = Self::patch_pack_info(&raw, bundle_name, version_code)?;
                zip_writer::add_bytes_deflate(&mut writer, &name, &patched)?;
            } else {
                let mut buf = Vec::new();
                src_entry.read_to_end(&mut buf)?;
                Self::write_normalized_entry(&mut writer, &name, compression, &buf)?;
            }
        }

        writer.finish()?;
        Ok(out_path)
    }

    /// Patch `pack.info` JSON bytes:
    /// - `summary.app.bundleName` → `bundle_name`
    /// - `summary.app.version.code` → `version_code`
    ///
    /// Returns the patched JSON as compact bytes with a trailing newline (matching hvigor output).
    fn patch_pack_info(
        raw: &[u8],
        bundle_name: &str,
        version_code: u64,
    ) -> Result<Vec<u8>, PackError> {
        let mut value: serde_json::Value =
            serde_json::from_slice(raw).map_err(|e| PackError::MalformedPackInfo(e.to_string()))?;

        if let Some(summary) = value.get_mut("summary") {
            if let Some(app) = summary.get_mut("app") {
                app["bundleName"] = serde_json::json!(bundle_name);
                if let Some(version) = app.get_mut("version") {
                    version["code"] = serde_json::json!(version_code);
                }
            }
        }

        let mut out =
            serde_json::to_vec(&value).map_err(|e| PackError::MalformedPackInfo(e.to_string()))?;
        out.push(b'\n');
        Ok(out)
    }

    fn write_normalized_entry<W: std::io::Write>(
        writer: &mut zip_writer::RawZipWriter<W>,
        name: &str,
        compression: zip::CompressionMethod,
        content: &[u8],
    ) -> Result<(), PackError> {
        match compression {
            zip::CompressionMethod::Stored => zip_writer::add_bytes_stored(writer, name, content),
            zip::CompressionMethod::Deflated => {
                zip_writer::add_bytes_deflate(writer, name, content)
            }
            method => Err(PackError::UnsupportedZipCompression {
                entry: name.to_string(),
                method: format!("{method:?}"),
            }),
        }
    }
}

/// Processes a list of remote HSPs for integration into a HAP.
///
/// For each entry:
/// 1. Opens the source `.hsp` zip.
/// 2. Reads and patches `pack.info` — replaces `summary.app.bundleName` and
///    `summary.app.version.code` with the host `bundle_name` / `version_code`.
/// 3. Writes the normalized HSP to `output_dir/<hsp_dir_name>/<hsp_file_name>`.
///
/// Returns the list of output paths in the same order as `entries`.
///
/// # Errors
///
/// Returns `PackError::MissingInput("pack.info")` if an HSP zip does not contain `pack.info`.
pub fn normalize_integrated_hsps(
    entries: &[IntegratedHspEntry],
    bundle_name: &str,
    version_code: u64,
    output_dir: &Path,
) -> Result<Vec<PathBuf>, PackError> {
    let mut output_paths = Vec::with_capacity(entries.len());
    for entry in entries {
        let out_path = entry.normalize(bundle_name, version_code, output_dir)?;
        output_paths.push(out_path);
    }
    Ok(output_paths)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Write;
    use zip::write::SimpleFileOptions;
    use zip::CompressionMethod;

    /// Build a minimal in-memory HSP zip and return its bytes.
    fn make_hsp_bytes(pack_info_json: &str, extra_entries: &[(&str, &[u8])]) -> Vec<u8> {
        let buf = Vec::new();
        let cursor = std::io::Cursor::new(buf);
        let mut zw = zip::ZipWriter::new(cursor);

        let opts = SimpleFileOptions::default().compression_method(CompressionMethod::Deflated);
        zw.start_file("pack.info", opts).unwrap();
        zw.write_all(pack_info_json.as_bytes()).unwrap();

        for (name, data) in extra_entries {
            let opts = SimpleFileOptions::default().compression_method(CompressionMethod::Stored);
            zw.start_file(*name, opts).unwrap();
            zw.write_all(data).unwrap();
        }

        zw.finish().unwrap().into_inner()
    }

    #[test]
    fn test_normalize_patches_bundle_name_and_version_code() {
        let tmp = tempfile::tempdir().unwrap();

        let pack_info = r#"{
            "summary": {
                "app": {
                    "bundleName": "com.remote.hsp",
                    "bundleType": "shared",
                    "version": { "code": 1, "name": "1.0.0" }
                },
                "modules": []
            },
            "packages": []
        }"#;

        let hsp_bytes = make_hsp_bytes(pack_info, &[("ets/modules.abc", b"PAND")]);
        let hsp_path = tmp.path().join("mylib.hsp");
        std::fs::write(&hsp_path, &hsp_bytes).unwrap();

        let entry = IntegratedHspEntry {
            hsp_dir_name: "mylib@1.0.0".to_string(),
            hsp_file_name: "mylib.hsp".to_string(),
            hsp_path,
        };

        let out_dir = tmp.path().join("integrated");
        let paths = normalize_integrated_hsps(&[entry], "com.host.app", 2000000, &out_dir).unwrap();

        assert_eq!(paths.len(), 1);
        let out_path = &paths[0];
        assert!(out_path.exists());
        assert_eq!(out_path, &out_dir.join("mylib@1.0.0").join("mylib.hsp"));

        // Verify the output is a valid zip with patched pack.info.
        let out_file = std::fs::File::open(out_path).unwrap();
        let mut out_archive = zip::ZipArchive::new(out_file).unwrap();

        let mut pack_info_entry = out_archive.by_name("pack.info").unwrap();
        let mut content = String::new();
        pack_info_entry.read_to_string(&mut content).unwrap();
        let v: serde_json::Value = serde_json::from_str(&content).unwrap();

        assert_eq!(
            v["summary"]["app"]["bundleName"].as_str().unwrap(),
            "com.host.app"
        );
        assert_eq!(
            v["summary"]["app"]["version"]["code"].as_u64().unwrap(),
            2000000
        );
        // bundleType must be preserved.
        assert_eq!(
            v["summary"]["app"]["bundleType"].as_str().unwrap(),
            "shared"
        );
    }

    #[test]
    fn test_normalize_preserves_other_entries() {
        let tmp = tempfile::tempdir().unwrap();

        let pack_info =
            r#"{"summary":{"app":{"bundleName":"com.r","version":{"code":1}}},"packages":[]}"#;
        let abc_bytes = b"PANDA\x00\x00\x00";
        let hsp_bytes = make_hsp_bytes(pack_info, &[("ets/modules.abc", abc_bytes)]);
        let hsp_path = tmp.path().join("lib.hsp");
        std::fs::write(&hsp_path, &hsp_bytes).unwrap();

        let entry = IntegratedHspEntry {
            hsp_dir_name: "lib@2.0.0".to_string(),
            hsp_file_name: "lib.hsp".to_string(),
            hsp_path,
        };

        let out_dir = tmp.path().join("out");
        let paths = normalize_integrated_hsps(&[entry], "com.host", 9999, &out_dir).unwrap();

        let out_file = std::fs::File::open(&paths[0]).unwrap();
        let mut out_archive = zip::ZipArchive::new(out_file).unwrap();

        // ets/modules.abc must be preserved verbatim.
        let mut abc_entry = out_archive.by_name("ets/modules.abc").unwrap();
        let mut buf = Vec::new();
        abc_entry.read_to_end(&mut buf).unwrap();
        assert_eq!(&buf, abc_bytes);
    }

    #[test]
    fn test_normalize_missing_pack_info_returns_error() {
        let tmp = tempfile::tempdir().unwrap();

        // Build an HSP zip with no pack.info.
        let buf = Vec::new();
        let cursor = std::io::Cursor::new(buf);
        let mut zw = zip::ZipWriter::new(cursor);
        let opts = SimpleFileOptions::default().compression_method(CompressionMethod::Stored);
        zw.start_file("ets/modules.abc", opts).unwrap();
        zw.write_all(b"PAND").unwrap();
        let hsp_bytes = zw.finish().unwrap().into_inner();

        let hsp_path = tmp.path().join("nopackinfo.hsp");
        std::fs::write(&hsp_path, &hsp_bytes).unwrap();

        let entry = IntegratedHspEntry {
            hsp_dir_name: "lib@1.0.0".to_string(),
            hsp_file_name: "nopackinfo.hsp".to_string(),
            hsp_path,
        };

        let result = normalize_integrated_hsps(&[entry], "com.host", 1, &tmp.path().join("out"));
        assert!(result.is_err());
        assert!(
            matches!(result.unwrap_err(), PackError::MissingInput(ref s) if s == "pack.info"),
            "expected MissingInput(pack.info)"
        );
    }

    #[test]
    fn test_normalize_empty_entries_returns_empty_vec() {
        let tmp = tempfile::tempdir().unwrap();
        let paths = normalize_integrated_hsps(&[], "com.host", 1, tmp.path()).unwrap();
        assert!(paths.is_empty());
    }
}
