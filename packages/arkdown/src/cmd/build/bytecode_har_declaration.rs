//! ByteCode HAR `declarationEntry` collection.
//!
//! When arkdown builds a HAR target, it must write
//! `oh-package.json5#metadata.declarationEntry` — a list of OHM URLs that
//! describes the reachable module entry points for workers, runtimeOnly
//! sources/packages, ability, startup tasks, and insight intent entries.
//! Downstream HAP/HSP consumers read this list from `byteCodeHarInfo` in
//! their `loader.json`.
//!
//! # Upstream correspondence
//!
//! | Type/function | hvigor equivalent |
//! |---------------|-------------------|
//! | `ByteCodeHarDeclarationCollector` | `process-har-artifacts.ts::processDeclarationEntry` |
//! | `declarationEntry` field | `metadata.declarationEntry` in `oh-package.json5` |
//! | loader consumer | `generate-loader-json.ts#setByteCodeHarInfoAndOtherCompileFiles` |
//!
//! # Architecture
//!
//! The collector takes the current module's [`LoaderJson`] (which already
//! contains the resolved `compile_entry`, `workers`, and ability lists) and
//! extracts the OHM URL entry points that belong in `declarationEntry`.
//!
//! In hvigor's model, `declarationEntry` is populated by
//! `processDeclarationEntry()` which calls:
//! - `collectConfigOhmurl(workers, ...)` — worker ability OHM URLs
//! - `collectRuntimeOnlyOhmurl(runtimeOnly, ...)` — runtimeOnly sources/packages
//! - `collectByteCodeHarAbilityOhmurl(...)` — ability srcEntry OHM URLs
//! - `collectByteCodeHarStartupTasks(...)` — startup task OHM URLs
//! - `collectByteCodeHarInsightIntentSrcEntry(...)` — insight intent OHM URLs
//!
//! In arkdown, `LoaderJson::compile_entry` already contains the union of these
//! entry points for the current module.  With `useNormalizedOHMUrl` enabled
//! (the only supported mode), these are already OHM URLs.

use std::collections::BTreeSet;

use anyhow::Result;

// ---------------------------------------------------------------------------
// ByteCodeHarDeclarationEntry
// ---------------------------------------------------------------------------

/// The `declarationEntry` list for a ByteCode HAR being built.
///
/// A sorted, deduplicated set of OHM URL strings representing all reachable
/// module entry points (workers, abilities, startup tasks, insight intents,
/// runtimeOnly sources/packages) in the HAR.
///
/// Written to `oh-package.json5#metadata.declarationEntry` in the HAR
/// archive so downstream consumers can find entry points without re-scanning
/// the HAR's source files.
#[derive(Debug, Clone, Default)]
pub struct ByteCodeHarDeclarationEntry {
  /// Sorted, deduplicated OHM URL entry points for this ByteCode HAR.
  pub ohm_urls: Vec<String>,
}

impl ByteCodeHarDeclarationEntry {
  /// Return `true` when there are no entry points to declare.
  pub fn is_empty(&self) -> bool {
    self.ohm_urls.is_empty()
  }
}

// ---------------------------------------------------------------------------
// ByteCodeHarDeclarationCollector
// ---------------------------------------------------------------------------

/// Collects `declarationEntry` for a HAR being built by arkdown.
///
/// Reads entry points from the module's processed [`LoaderJson`] (which
/// already contains the resolved OHM URLs when `useNormalizedOHMUrl` is
/// enabled) and returns them as the HAR's `declarationEntry`.
///
/// # Usage
///
/// Call [`ByteCodeHarDeclarationCollector::collect`] after the profile stage
/// produces a [`LoaderJson`] for the HAR module.  If the result is non-empty,
/// write it to `oh-package.json5#metadata.declarationEntry` via
/// [`ByteCodeHarDeclarationCollector::write_to_package_metadata`].
pub struct ByteCodeHarDeclarationCollector;

impl ByteCodeHarDeclarationCollector {
  /// Collect the `declarationEntry` OHM URLs from ability/module entry
  /// points and worker entry points.
  ///
  /// # Arguments
  ///
  /// - `compile_entry`: `LoaderJson::compile_entry` — ability, extension-ability,
  ///   and module srcEntry OHM URLs (with `useNormalizedOHMUrl`, already in OHM format).
  /// - `workers`: `LoaderJson::workers` — WebWorker entry OHM URLs.
  ///
  /// De-duplicates and sorts for deterministic output.
  pub fn collect(compile_entry: &[String], workers: &[String]) -> ByteCodeHarDeclarationEntry {
    let mut seen: BTreeSet<String> = BTreeSet::new();

    for entry in compile_entry {
      if !entry.trim().is_empty() {
        seen.insert(entry.clone());
      }
    }
    for worker in workers {
      if !worker.trim().is_empty() {
        seen.insert(worker.clone());
      }
    }

    ByteCodeHarDeclarationEntry {
      ohm_urls: seen.into_iter().collect(),
    }
  }

  /// Convenience wrapper that collects from a `LoaderJson`.
  pub fn collect_from_loader(
    loader_json: &arkdown_profile::LoaderJson,
  ) -> ByteCodeHarDeclarationEntry {
    Self::collect(&loader_json.compile_entry, &loader_json.workers)
  }

  /// Write the collected `declarationEntry` into
  /// `oh-package.json5#metadata.declarationEntry`.
  ///
  /// If `entry.is_empty()` this is a no-op (empty `declarationEntry` is
  /// valid and means the HAR has no specially tracked entry points).
  ///
  /// # File format
  ///
  /// Reads the existing `oh-package.json5` (or creates a minimal skeleton
  /// if absent), merges the `declarationEntry` list into
  /// `metadata.declarationEntry`, and writes back as JSON5.
  pub fn write_to_package_metadata(
    entry: &ByteCodeHarDeclarationEntry,
    oh_package_json5_path: &std::path::Path,
  ) -> Result<()> {
    use serde_json::Value;

    // Read existing file (may not exist for newly-generated HARs).
    let existing = if oh_package_json5_path.exists() {
      let text = fs_err::read_to_string(oh_package_json5_path)?;
      json5::from_str::<Value>(&text).unwrap_or(Value::Object(serde_json::Map::new()))
    } else {
      Value::Object(serde_json::Map::new())
    };

    let Value::Object(mut obj) = existing else {
      anyhow::bail!(
        "oh-package.json5 at {} is not a JSON object",
        oh_package_json5_path.display()
      );
    };

    // Ensure `metadata` object exists.
    let metadata = obj
      .entry("metadata")
      .or_insert_with(|| Value::Object(serde_json::Map::new()));
    let Value::Object(meta_obj) = metadata else {
      anyhow::bail!(
        "oh-package.json5 'metadata' field at {} is not an object",
        oh_package_json5_path.display()
      );
    };

    // Write declarationEntry as a JSON array.
    let declaration_value: Value = entry
      .ohm_urls
      .iter()
      .map(|s| Value::String(s.clone()))
      .collect::<Vec<_>>()
      .into();
    meta_obj.insert("declarationEntry".to_string(), declaration_value);

    // Write back as pretty JSON (JSON5-compatible).
    let out = serde_json::to_string_pretty(&Value::Object(obj))
      .map_err(|e| anyhow::anyhow!("serialize oh-package.json5 failed: {e}"))?;
    if let Some(parent) = oh_package_json5_path.parent() {
      std::fs::create_dir_all(parent)?;
    }
    fs_err::write(oh_package_json5_path, out)?;

    Ok(())
  }
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
  use super::*;
  use tempfile::TempDir;

  #[test]
  fn collect_deduplicates_and_sorts() {
    let compile_entry = vec![
      "@bundle:com.example/entry/pages/Index".to_string(),
      "@bundle:com.example/entry/EntryAbility".to_string(),
    ];
    let workers = vec!["@bundle:com.example/entry/workers/Worker".to_string()];
    let entry = ByteCodeHarDeclarationCollector::collect(&compile_entry, &workers);
    assert_eq!(entry.ohm_urls.len(), 3);
    // sorted
    assert!(entry.ohm_urls.windows(2).all(|w| w[0] <= w[1]));
  }

  #[test]
  fn collect_deduplicates_overlap_between_compile_entry_and_workers() {
    let url = "@bundle:com.example/entry/WorkerAbility".to_string();
    let entry = ByteCodeHarDeclarationCollector::collect(&[url.clone()], &[url.clone()]);
    assert_eq!(entry.ohm_urls, vec![url]);
  }

  #[test]
  fn collect_empty_gives_empty_entry() {
    let entry = ByteCodeHarDeclarationCollector::collect(&[], &[]);
    assert!(entry.is_empty());
  }

  #[test]
  fn write_and_read_back_declaration_entry() {
    let tmp = TempDir::new().unwrap();
    let pkg_path = tmp.path().join("oh-package.json5");

    // Write initial content
    std::fs::write(&pkg_path, r#"{"name": "my-har", "version": "1.0.0"}"#).unwrap();

    let entry = ByteCodeHarDeclarationEntry {
      ohm_urls: vec![
        "@bundle:com.example/entry/EntryAbility".to_string(),
        "@bundle:com.example/entry/workers/Worker".to_string(),
      ],
    };
    ByteCodeHarDeclarationCollector::write_to_package_metadata(&entry, &pkg_path).unwrap();

    // Read back and verify
    let text = std::fs::read_to_string(&pkg_path).unwrap();
    let parsed: serde_json::Value = serde_json::from_str(&text).unwrap();
    let decl = parsed["metadata"]["declarationEntry"].as_array().unwrap();
    assert_eq!(decl.len(), 2);
    assert_eq!(
      decl[0].as_str().unwrap(),
      "@bundle:com.example/entry/EntryAbility"
    );
  }

  #[test]
  fn write_creates_metadata_when_absent() {
    let tmp = TempDir::new().unwrap();
    let pkg_path = tmp.path().join("oh-package.json5");
    std::fs::write(&pkg_path, r#"{"name": "no-metadata"}"#).unwrap();

    let entry = ByteCodeHarDeclarationEntry {
      ohm_urls: vec!["@bundle:com.example/entry/Ability".to_string()],
    };
    ByteCodeHarDeclarationCollector::write_to_package_metadata(&entry, &pkg_path).unwrap();

    let text = std::fs::read_to_string(&pkg_path).unwrap();
    let parsed: serde_json::Value = serde_json::from_str(&text).unwrap();
    assert!(parsed["metadata"]["declarationEntry"].is_array());
  }
}
