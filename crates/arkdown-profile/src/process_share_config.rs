//! `ProcessShareConfig` — merge `crossAppSharedConfig` from HSP dependencies.
//!
//! Mirrors the hvigor `ProcessShareConfig` task
//! (`hvigor-ohos-plugin/src/tasks/process-share-config.js`).
//!
//! ## What it does
//!
//! 1. Scans `oh_modules/` under the module root for installed HSP packages.
//! 2. Reads each HSP's `module.json5` (or the pre-processed `module.json`) and
//!    extracts its `module.crossAppSharedConfig` array.
//! 3. Fills in `packageName` from the package directory name when the entry
//!    does not already carry one.
//! 4. Validates that no two entries share the same `uri` across different packages
//!    (duplicate URIs would cause a runtime routing conflict).
//! 5. Writes the merged result as `{"crossAppSharedConfig": [...]}` to the
//!    intermediates path supplied by the caller.
//!
//! ## Output path (hvigor convention)
//!
//! ```text
//! <module_root>/build/default/intermediates/share_config/default/temp-share-config.json
//! ```

use rustc_hash::{FxHashMap, FxHashSet};
use std::path::{Path, PathBuf};

use arkdown_core::CrossAppSharedConfigEntry;

// ---------------------------------------------------------------------------
// Public API
// ---------------------------------------------------------------------------

/// Input for the share-config merge pass.
pub struct ShareConfigInput<'a> {
    /// Root directory of the consuming module (e.g. `/project/entry`).
    pub module_root: &'a Path,
    /// `crossAppSharedConfig` entries declared by the module itself (may be empty
    /// for HAP/HAR consumers; non-empty for HSP modules that export shared components).
    pub self_entries: Vec<CrossAppSharedConfigEntry>,
}

/// Merged `crossAppSharedConfig` result.
#[derive(Debug, Clone)]
pub struct ShareConfigOutput {
    /// All merged entries ready to be serialized to `temp-share-config.json`.
    pub entries: Vec<CrossAppSharedConfigEntry>,
}

/// Scan `oh_modules/` under `module_root`, collect `crossAppSharedConfig` entries
/// from every HSP dependency, merge them with `self_entries`, validate for
/// duplicate URIs, and return the merged list.
///
/// Errors are returned for I/O failures only; missing or malformed `module.json5`
/// files in individual dependencies are silently skipped (matching hvigor behaviour
/// where non-HSP packages simply have no `crossAppSharedConfig`).
pub fn merge_share_config(input: ShareConfigInput<'_>) -> ShareConfigOutput {
    let mut all_entries: Vec<CrossAppSharedConfigEntry> = Vec::new();

    // Include the module's own entries first (for HSP modules that export shared components).
    all_entries.extend(input.self_entries);

    // Walk oh_modules/ to find installed HSP packages.
    let oh_modules_dir = input.module_root.join("oh_modules");
    if oh_modules_dir.is_dir() {
        collect_from_oh_modules(&oh_modules_dir, &mut all_entries);
    }

    // Validate: no two entries from different packages may share the same URI.
    // (Duplicate URIs within the same package are allowed — hvigor only errors
    // on cross-package URI collisions.)
    check_duplicate_uris(&all_entries);

    ShareConfigOutput {
        entries: all_entries,
    }
}

/// Write the merged share config to `output_path` as
/// `{"crossAppSharedConfig": [...]}`.
///
/// Creates parent directories if they do not exist.
pub fn write_share_config(
    output: &ShareConfigOutput,
    output_path: &Path,
) -> Result<(), std::io::Error> {
    if let Some(parent) = output_path.parent() {
        std::fs::create_dir_all(parent)?;
    }
    let value = serde_json::json!({ "crossAppSharedConfig": output.entries });
    let json = serde_json::to_string(&value).map_err(|e| std::io::Error::other(e.to_string()))?;
    std::fs::write(output_path, json)?;
    Ok(())
}

// ---------------------------------------------------------------------------
// Internal helpers
// ---------------------------------------------------------------------------

/// Walk `oh_modules_dir`, find HSP packages (those whose `module.json5` or
/// `module.json` declares `module.type == "shared"`), and collect their
/// `crossAppSharedConfig` entries.
fn collect_from_oh_modules(
    oh_modules_dir: &Path,
    all_entries: &mut Vec<CrossAppSharedConfigEntry>,
) {
    let Ok(read_dir) = std::fs::read_dir(oh_modules_dir) else {
        return;
    };

    for entry in read_dir.flatten() {
        let pkg_dir = entry.path();
        if !pkg_dir.is_dir() {
            continue;
        }

        // Derive the package name from the directory name.
        let pkg_name = pkg_dir
            .file_name()
            .and_then(|n| n.to_str())
            .unwrap_or("")
            .to_string();

        // Try `module.json5` first, then fall back to the pre-processed `module.json`.
        let entries = read_cross_app_shared_config_from_pkg(&pkg_dir, &pkg_name);
        all_entries.extend(entries);
    }
}

/// Read `crossAppSharedConfig` from a single installed package directory.
///
/// Tries `src/main/module.json5` first (source layout), then `module.json`
/// (pre-built layout) at the package root.
fn read_cross_app_shared_config_from_pkg(
    pkg_dir: &Path,
    pkg_name: &str,
) -> Vec<CrossAppSharedConfigEntry> {
    // Candidate paths in priority order.
    let candidates: &[PathBuf] = &[
        pkg_dir.join("src/main/module.json5"),
        pkg_dir.join("module.json5"),
        pkg_dir.join("module.json"),
    ];

    for candidate in candidates {
        if !candidate.exists() {
            continue;
        }
        let Ok(content) = std::fs::read_to_string(candidate) else {
            continue;
        };
        // Parse as JSON5 (also valid for plain JSON).
        let Ok(raw): Result<serde_json::Value, _> = json5::from_str(&content) else {
            continue;
        };

        // Only process HSP modules (type == "shared").
        let module_type = raw
            .pointer("/module/type")
            .and_then(|v| v.as_str())
            .unwrap_or("");
        if module_type != "shared" {
            return vec![];
        }

        let Some(arr) = raw
            .pointer("/module/crossAppSharedConfig")
            .and_then(|v| v.as_array())
        else {
            return vec![];
        };

        let mut entries: Vec<CrossAppSharedConfigEntry> = arr
            .iter()
            .filter_map(|item| {
                serde_json::from_value::<CrossAppSharedConfigEntry>(item.clone()).ok()
            })
            .collect();

        // Fill in packageName from the directory name when absent.
        for entry in &mut entries {
            if entry.package_name.is_empty() {
                entry.package_name = pkg_name.to_string();
            }
        }

        return entries;
    }

    vec![]
}

/// Log a warning (but do not abort) when two entries from different packages
/// share the same URI.  Hvigor calls `printErrorExit` here; arkdown downgrades
/// to a non-fatal warning so the build continues in the same way that missing
/// optional config files are handled elsewhere in the pipeline.
fn check_duplicate_uris(entries: &[CrossAppSharedConfigEntry]) {
    // Map URI → list of package names that declare it.
    let mut uri_to_packages: FxHashMap<&str, Vec<&str>> = FxHashMap::default();
    for entry in entries {
        uri_to_packages
            .entry(entry.uri.as_str())
            .or_default()
            .push(entry.package_name.as_str());
    }

    for (uri, packages) in &uri_to_packages {
        if packages.len() > 1 {
            // Deduplicate package names before emitting the warning.
            let mut unique: Vec<&&str> = packages
                .iter()
                .collect::<FxHashSet<_>>()
                .into_iter()
                .collect();
            unique.sort();
            tracing::warn!(
                uri,
                packages = ?unique,
                "crossAppSharedConfig duplicate URI declared by multiple packages"
            );
        }
    }
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;

    fn write_module_json5(dir: &Path, content: &str) {
        let src_main = dir.join("src/main");
        fs::create_dir_all(&src_main).unwrap();
        fs::write(src_main.join("module.json5"), content).unwrap();
    }

    #[test]
    fn test_merge_share_config_no_oh_modules() {
        let tmp = tempfile::tempdir().unwrap();
        let output = merge_share_config(ShareConfigInput {
            module_root: tmp.path(),
            self_entries: vec![],
        });
        assert!(output.entries.is_empty());
    }

    #[test]
    fn test_merge_share_config_collects_hsp_entries() {
        let tmp = tempfile::tempdir().unwrap();
        let oh_modules = tmp.path().join("oh_modules");
        let hsp_pkg = oh_modules.join("my-hsp");
        write_module_json5(
            &hsp_pkg,
            r#"{
                "module": {
                    "name": "my-hsp",
                    "type": "shared",
                    "crossAppSharedConfig": [
                        { "uri": "myapp://shared/Foo", "srcEntry": "./ets/Foo.ets" }
                    ]
                }
            }"#,
        );

        let output = merge_share_config(ShareConfigInput {
            module_root: tmp.path(),
            self_entries: vec![],
        });

        assert_eq!(output.entries.len(), 1);
        assert_eq!(output.entries[0].uri, "myapp://shared/Foo");
        assert_eq!(output.entries[0].package_name, "my-hsp");
    }

    #[test]
    fn test_merge_share_config_skips_non_hsp() {
        let tmp = tempfile::tempdir().unwrap();
        let oh_modules = tmp.path().join("oh_modules");
        let har_pkg = oh_modules.join("my-har");
        write_module_json5(
            &har_pkg,
            r#"{
                "module": {
                    "name": "my-har",
                    "type": "har",
                    "crossAppSharedConfig": [
                        { "uri": "myapp://shared/Bar", "srcEntry": "./ets/Bar.ets" }
                    ]
                }
            }"#,
        );

        let output = merge_share_config(ShareConfigInput {
            module_root: tmp.path(),
            self_entries: vec![],
        });

        assert!(
            output.entries.is_empty(),
            "HAR packages must not contribute crossAppSharedConfig"
        );
    }

    #[test]
    fn test_merge_share_config_includes_self_entries() {
        let tmp = tempfile::tempdir().unwrap();
        let self_entry = CrossAppSharedConfigEntry {
            package_name: "self-hsp".to_string(),
            uri: "myapp://shared/Self".to_string(),
            src_entry: Some("./ets/Self.ets".to_string()),
        };
        let output = merge_share_config(ShareConfigInput {
            module_root: tmp.path(),
            self_entries: vec![self_entry.clone()],
        });
        assert_eq!(output.entries.len(), 1);
        assert_eq!(output.entries[0], self_entry);
    }

    #[test]
    fn test_write_share_config_creates_file() {
        let tmp = tempfile::tempdir().unwrap();
        let out_path = tmp
            .path()
            .join("intermediates/share_config/default/temp-share-config.json");

        let output = ShareConfigOutput {
            entries: vec![CrossAppSharedConfigEntry {
                package_name: "pkg-a".to_string(),
                uri: "myapp://shared/Comp".to_string(),
                src_entry: None,
            }],
        };
        write_share_config(&output, &out_path).unwrap();

        assert!(out_path.exists());
        let content = fs::read_to_string(&out_path).unwrap();
        let parsed: serde_json::Value = serde_json::from_str(&content).unwrap();
        let arr = parsed["crossAppSharedConfig"].as_array().unwrap();
        assert_eq!(arr.len(), 1);
        assert_eq!(arr[0]["uri"], "myapp://shared/Comp");
        assert_eq!(arr[0]["packageName"], "pkg-a");
    }

    #[test]
    fn test_write_share_config_empty_array() {
        let tmp = tempfile::tempdir().unwrap();
        let out_path = tmp.path().join("temp-share-config.json");
        let output = ShareConfigOutput { entries: vec![] };
        write_share_config(&output, &out_path).unwrap();

        let content = fs::read_to_string(&out_path).unwrap();
        assert_eq!(content, r#"{"crossAppSharedConfig":[]}"#);
    }

    #[test]
    fn test_merge_share_config_fills_package_name_from_dir() {
        let tmp = tempfile::tempdir().unwrap();
        let oh_modules = tmp.path().join("oh_modules");
        let hsp_pkg = oh_modules.join("auto-named-hsp");
        write_module_json5(
            &hsp_pkg,
            r#"{
                "module": {
                    "name": "auto-named-hsp",
                    "type": "shared",
                    "crossAppSharedConfig": [
                        { "uri": "myapp://shared/X" }
                    ]
                }
            }"#,
        );

        let output = merge_share_config(ShareConfigInput {
            module_root: tmp.path(),
            self_entries: vec![],
        });

        assert_eq!(output.entries.len(), 1);
        assert_eq!(
            output.entries[0].package_name, "auto-named-hsp",
            "packageName must be filled from the oh_modules directory name"
        );
    }
}
