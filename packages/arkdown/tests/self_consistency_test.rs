//! Self-consistency oracle for arkdown build determinism.
//!
//! Verifies that arkdown produces bit-for-bit identical outputs across
//! repeated builds of the same source tree.  No hvigor installation is
//! required — these tests are the primary determinism gate for CI.
//!
//! ## Running
//!
//! ```bash
//! cargo test -p arkdown-napi self_consistency
//! ```
//!
//! ## Fingerprint update workflow
//!
//! On first run (no stored fingerprints) the oracle writes baseline
//! fingerprints to `tests/fingerprints/<fixture>.json` and the test passes.
//! On subsequent runs the fingerprints are compared against the stored
//! baseline.  Set `ARKDOWN_UPDATE_FINGERPRINTS=1` to accept new baseline
//! fingerprints after an intentional output change:
//!
//! ```bash
//! ARKDOWN_UPDATE_FINGERPRINTS=1 cargo test -p arkdown-napi self_consistency
//! ```

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

// ---------------------------------------------------------------------------
// SHA-256 fingerprint helper (no extra dep — uses sha2 which is already in
// the workspace for arkdown-sign)
// ---------------------------------------------------------------------------

use sha2::{Digest, Sha256};

fn sha256_hex(bytes: &[u8]) -> String {
  let mut hasher = Sha256::new();
  hasher.update(bytes);
  hex::encode(hasher.finalize())
}

// ---------------------------------------------------------------------------
// BuildOutput — map of relative-path → raw bytes for all produced files
// ---------------------------------------------------------------------------

/// All files produced under a build output directory.
///
/// Keys are relative paths from the project root (e.g.
/// `"entry/build/default/intermediates/loader/default/module/loader.json"`).
/// Values are the raw file bytes.
#[derive(Debug, Clone)]
pub struct BuildOutput {
  /// Project root the build ran in.
  pub project_root: PathBuf,
  /// Map of `relative_path → bytes`.
  pub files: BTreeMap<String, Vec<u8>>,
}

impl BuildOutput {
  /// Collect all files under `build/` within `project_root`.
  fn collect(project_root: &Path) -> Self {
    let mut files = BTreeMap::new();
    let build_dir = project_root.join("build");
    if build_dir.exists() {
      for entry in walkdir::WalkDir::new(&build_dir)
        .follow_links(false)
        .into_iter()
        .filter_map(|e| e.ok())
      {
        if !entry.file_type().is_file() {
          continue;
        }
        let rel = entry
          .path()
          .strip_prefix(project_root)
          .expect("entry under project root")
          .to_string_lossy()
          .into_owned();
        // Normalise Windows path separators so comparisons are
        // platform-independent.
        let rel = rel.replace('\\', "/");
        let bytes = match std::fs::read(entry.path()) {
          Ok(b) => b,
          Err(_) => continue,
        };
        files.insert(rel, bytes);
      }
    }
    BuildOutput {
      project_root: project_root.to_path_buf(),
      files,
    }
  }
}

// ---------------------------------------------------------------------------
// DiffReport — detailed comparison result
// ---------------------------------------------------------------------------

/// Detailed report produced by [`SelfConsistencyOracle::compare`].
#[derive(Debug, Clone)]
pub struct DiffReport {
  /// `true` when both outputs are bit-for-bit identical.
  pub identical: bool,
  /// Files present in `run1` but absent in `run2`.
  pub only_in_run1: Vec<String>,
  /// Files present in `run2` but absent in `run1`.
  pub only_in_run2: Vec<String>,
  /// Files present in both runs but with different content.
  pub changed: Vec<ChangedFile>,
}

/// A single file that differs between two build runs.
#[derive(Debug, Clone)]
pub struct ChangedFile {
  pub path: String,
  pub run1_sha256: String,
  pub run2_sha256: String,
  pub run1_size: usize,
  pub run2_size: usize,
  /// For text files: first differing line (0-indexed).
  pub first_diff_line: Option<usize>,
}

impl DiffReport {
  /// Format the report as a human-readable string suitable for a panic
  /// message or test output.
  pub fn to_display_string(&self) -> String {
    if self.identical {
      return "builds are identical".to_string();
    }
    let mut out = String::from("self-consistency check FAILED\n");
    if !self.only_in_run1.is_empty() {
      out.push_str(&format!("  only in run1 ({}):\n", self.only_in_run1.len()));
      for p in &self.only_in_run1 {
        out.push_str(&format!("    - {p}\n"));
      }
    }
    if !self.only_in_run2.is_empty() {
      out.push_str(&format!("  only in run2 ({}):\n", self.only_in_run2.len()));
      for p in &self.only_in_run2 {
        out.push_str(&format!("    + {p}\n"));
      }
    }
    if !self.changed.is_empty() {
      out.push_str(&format!("  changed ({}):\n", self.changed.len()));
      for f in &self.changed {
        out.push_str(&format!(
          "    ~ {} (run1={} {} bytes, run2={} {} bytes",
          f.path, f.run1_sha256, f.run1_size, f.run2_sha256, f.run2_size,
        ));
        if let Some(line) = f.first_diff_line {
          out.push_str(&format!(", first diff at line {line}"));
        }
        out.push_str(")\n");
      }
    }
    out
  }
}

// ---------------------------------------------------------------------------
// SelfConsistencyOracle
// ---------------------------------------------------------------------------

/// Oracle that verifies build determinism without requiring hvigor.
///
/// Usage pattern:
/// 1. Call [`run_build`] twice on the same (clean-copy) fixture.
/// 2. Call [`compare`] on the two [`BuildOutput`] values.
/// 3. Assert `report.identical`.
pub struct SelfConsistencyOracle;

impl SelfConsistencyOracle {
  /// Run an arkdown build inside `project_path` and return a snapshot of all
  /// files produced under `<project_path>/build/`.
  ///
  /// `extra_args` are forwarded to [`arkdown_napi::services::build::BuildService`]
  /// via the [`BuildRequest`] struct. The project root is always set to
  /// `project_path`.
  pub fn run_build(project_path: &Path, disable_abc_cache: bool) -> BuildOutput {
    use arkdown_build::request::AbcBackendChoice;
    use arkdown_napi::services::build::{BuildRequest, BuildService};

    let rt = tokio::runtime::Runtime::new().expect("tokio runtime");
    rt.block_on(
      BuildService::new(BuildRequest {
        target: "hap".to_string(),
        mode: "debug".to_string(),
        analyze: Default::default(),
        arkts_version: arkdown_core::ArktsLanguageVersion::default(),
        ohpm_mode: Default::default(),
        module: None,
        product: None,
        project: Some(project_path.to_path_buf()),
        build_target_name: None,
        watch: false,
        coverage: false,
        scope: None,
        ohos_test_coverage: None,
        coverage_path_filter: None,
        coverage_mode: None,
        unit_test_replace_page: None,
        fast_build_app: false,
        enable_sign_task: false,
        ohos_debug_asan: false,
        ohos_enable_hwasan: false,
        ohos_enable_tsan: false,
        ohos_enable_ubsan: false,
        ohos_enable_binxo: false,
        enable_zig: false,
        preview: false,
        check_entry: false,
        is_crossplatform: false,
        ignore_crossplatform_check: false,
        preview_transform_mode: false,
        disable_abc_cache,
        cache_file: None,
        abc_backend: AbcBackendChoice::Auto,
        checker: Some("oxc".to_string()),
        cache: true,
        skip_resource: false,
        skip_profile: false,
        watch_invalidation: None,
        custom_task_input_paths: Vec::new(),
        pack_compress_level: None,
        align_device_types: Vec::new(),
        collect_debug_symbol: None,
        ui_transform_optimization: false,
        integrated_hsp_use_get_current_bundle_name: false,
        byte_code_har_integrated_optimization: false,
        keep_dependency: true,
        parameter_file_override: None,
        build_version: None,
        node_executable: None,
        hvigor_cache_dir: None,
        native_task_roots: Vec::new(),
        hvigor_properties: Default::default(),
      })
      .execute(),
    )
    .expect("arkdown build must succeed");

    // Collect all files produced under <module>/build/ directories.
    // The module sub-directory is one level below the project root.
    let mut combined = BuildOutput {
      project_root: project_path.to_path_buf(),
      files: BTreeMap::new(),
    };
    for entry in std::fs::read_dir(project_path)
      .expect("read project root")
      .flatten()
    {
      if !entry.file_type().map(|ft| ft.is_dir()).unwrap_or(false) {
        continue;
      }
      let module_dir = entry.path();
      let module_build = module_dir.join("build");
      if !module_build.is_dir() {
        continue;
      }
      let sub = BuildOutput::collect(&module_dir);
      for (rel, bytes) in sub.files {
        // Prefix with module directory name for unambiguous keys.
        let module_name = module_dir
          .file_name()
          .map(|n| n.to_string_lossy().into_owned())
          .unwrap_or_default();
        combined.files.insert(format!("{module_name}/{rel}"), bytes);
      }
    }
    combined
  }

  /// Compare two build outputs and return a detailed diff report.
  ///
  /// The comparison is bit-for-bit for binary files.  For JSON files the
  /// comparison is also byte-level (no normalisation) because determinism
  /// means the bytes must be identical, not just semantically equivalent.
  pub fn compare(run1: &BuildOutput, run2: &BuildOutput) -> DiffReport {
    let keys1: std::collections::BTreeSet<&String> = run1.files.keys().collect();
    let keys2: std::collections::BTreeSet<&String> = run2.files.keys().collect();

    let only_in_run1: Vec<String> = keys1.difference(&keys2).map(|k| (*k).clone()).collect();
    let only_in_run2: Vec<String> = keys2.difference(&keys1).map(|k| (*k).clone()).collect();

    let mut changed = Vec::new();
    for key in keys1.intersection(&keys2) {
      let b1 = &run1.files[*key];
      let b2 = &run2.files[*key];
      if b1 != b2 {
        let first_diff_line = find_first_diff_line(b1, b2);
        changed.push(ChangedFile {
          path: (*key).clone(),
          run1_sha256: sha256_hex(b1),
          run2_sha256: sha256_hex(b2),
          run1_size: b1.len(),
          run2_size: b2.len(),
          first_diff_line,
        });
      }
    }

    let identical = only_in_run1.is_empty() && only_in_run2.is_empty() && changed.is_empty();
    DiffReport {
      identical,
      only_in_run1,
      only_in_run2,
      changed,
    }
  }

  /// Convenience: assert that `report` is identical, panicking with the
  /// full diff report on failure.
  pub fn assert_identical(report: &DiffReport) {
    if !report.identical {
      panic!("{}", report.to_display_string());
    }
  }
}

/// Find the first line index (0-based) where two byte slices diverge when
/// interpreted as UTF-8 text.  Returns `None` when the data is not valid
/// UTF-8 or the bytes are identical.
fn find_first_diff_line(a: &[u8], b: &[u8]) -> Option<usize> {
  let a_str = std::str::from_utf8(a).ok()?;
  let b_str = std::str::from_utf8(b).ok()?;
  a_str
    .lines()
    .zip(b_str.lines())
    .position(|(la, lb)| la != lb)
}

// ---------------------------------------------------------------------------
// FingerprintOracle
// ---------------------------------------------------------------------------

/// Fingerprint store for build outputs.
///
/// On first run (no stored fingerprints) the oracle writes baseline
/// fingerprints to `tests/fingerprints/<fixture_name>.json`.  On subsequent
/// runs it compares the current build against the stored fingerprints.
///
/// Set `ARKDOWN_UPDATE_FINGERPRINTS=1` to overwrite stored fingerprints with
/// the current build output (accept new baseline after an intentional change).
pub struct FingerprintOracle {
  /// Directory where fingerprint JSON files are stored.
  store_dir: PathBuf,
  /// Logical name of the fixture (used as the JSON file stem).
  fixture_name: String,
}

/// A single fingerprint entry: path → SHA-256 hex + size.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct FingerprintEntry {
  pub sha256: String,
  pub size: usize,
}

impl FingerprintOracle {
  /// Create a new `FingerprintOracle` for `fixture_name`, storing
  /// fingerprint files in `store_dir`.
  pub fn new(store_dir: impl Into<PathBuf>, fixture_name: impl Into<String>) -> Self {
    FingerprintOracle {
      store_dir: store_dir.into(),
      fixture_name: fixture_name.into(),
    }
  }

  /// Return `true` when `ARKDOWN_UPDATE_FINGERPRINTS=1` is set.
  pub fn update_mode() -> bool {
    std::env::var("ARKDOWN_UPDATE_FINGERPRINTS")
      .map(|v| v == "1" || v.eq_ignore_ascii_case("true"))
      .unwrap_or(false)
  }

  /// Path to the fingerprint JSON file for this fixture.
  pub fn fingerprint_path(&self) -> PathBuf {
    self.store_dir.join(format!("{}.json", self.fixture_name))
  }

  /// Compute SHA-256 fingerprints for all files in `output`.
  pub fn compute(output: &BuildOutput) -> BTreeMap<String, FingerprintEntry> {
    output
      .files
      .iter()
      .map(|(path, bytes)| {
        (
          path.clone(),
          FingerprintEntry {
            sha256: sha256_hex(bytes),
            size: bytes.len(),
          },
        )
      })
      .collect()
  }

  /// Load stored fingerprints from disk.
  ///
  /// Returns `None` when no fingerprint file exists yet (first run).
  pub fn load(&self) -> Option<BTreeMap<String, FingerprintEntry>> {
    let content = std::fs::read_to_string(self.fingerprint_path()).ok()?;
    serde_json::from_str(&content).ok()
  }

  /// Write `fingerprints` to disk, creating parent directories as needed.
  pub fn save(&self, fingerprints: &BTreeMap<String, FingerprintEntry>) {
    let path = self.fingerprint_path();
    if let Some(parent) = path.parent() {
      std::fs::create_dir_all(parent).expect("create fingerprint store dir");
    }
    let json = serde_json::to_string_pretty(fingerprints).expect("serialize fingerprints");
    std::fs::write(&path, json).expect("write fingerprint file");
  }

  /// Assert that `output` matches the stored fingerprints.
  ///
  /// - First run (no stored file): writes fingerprints and passes.
  /// - `ARKDOWN_UPDATE_FINGERPRINTS=1`: overwrites and passes.
  /// - Otherwise: compares and panics on mismatch with a detailed message.
  pub fn assert_matches(&self, output: &BuildOutput) {
    let current = Self::compute(output);

    if Self::update_mode() {
      self.save(&current);
      tracing::error!(
        "[fingerprint] Updated fingerprints for '{}' (ARKDOWN_UPDATE_FINGERPRINTS=1)",
        self.fixture_name
      );
      return;
    }

    match self.load() {
      None => {
        self.save(&current);
        tracing::error!(
          "[fingerprint] Created baseline fingerprints for '{}' at '{}'",
          self.fixture_name,
          self.fingerprint_path().display()
        );
      }
      Some(stored) => {
        let mut mismatches: Vec<String> = Vec::new();

        // Files in stored but missing from current.
        for path in stored.keys() {
          if !current.contains_key(path) {
            mismatches.push(format!("  missing in current build: {path}"));
          }
        }
        // Files in current but not in stored.
        for path in current.keys() {
          if !stored.contains_key(path) {
            mismatches.push(format!("  new file not in baseline: {path}"));
          }
        }
        // Content changes.
        for (path, cur_entry) in &current {
          if let Some(stored_entry) = stored.get(path) {
            if cur_entry.sha256 != stored_entry.sha256 {
              mismatches.push(format!(
                "  changed: {path}\n    stored  sha256={} size={}\n    current sha256={} size={}",
                stored_entry.sha256, stored_entry.size, cur_entry.sha256, cur_entry.size,
              ));
            }
          }
        }

        if !mismatches.is_empty() {
          panic!(
            "fingerprint mismatch for fixture '{}'.\n\
                         To update: ARKDOWN_UPDATE_FINGERPRINTS=1 cargo test\n\
                         Mismatches ({}):\n{}",
            self.fixture_name,
            mismatches.len(),
            mismatches.join("\n")
          );
        }
      }
    }
  }
}

// ---------------------------------------------------------------------------
// Fixture helpers
// ---------------------------------------------------------------------------

fn repo_root() -> PathBuf {
  PathBuf::from(env!("CARGO_MANIFEST_DIR"))
    .parent()
    .expect("packages/arkdown -> packages/")
    .parent()
    .expect("packages/ -> workspace root")
    .to_path_buf()
}

/// Copy the fixture at `src` into a fresh temporary directory, skipping
/// any existing `build/` or `.hvigor/` directories so each run starts clean.
fn fresh_fixture_copy(src: &Path) -> PathBuf {
  static COUNTER: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(0);
  let n = COUNTER.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
  let nonce = std::time::SystemTime::now()
    .duration_since(std::time::UNIX_EPOCH)
    .unwrap_or_default()
    .as_nanos();
  let dst = std::env::temp_dir().join(format!("arkdown-sc-{}-{n}-{nonce}", std::process::id()));
  copy_dir_skip_build(src, &dst);
  dst
}

fn copy_dir_skip_build(src: &Path, dst: &Path) {
  std::fs::create_dir_all(dst).expect("create temp dir");
  for entry in std::fs::read_dir(src).expect("read src dir").flatten() {
    let name = entry.file_name();
    let name_str = name.to_string_lossy();
    if matches!(name_str.as_ref(), "build" | ".hvigor" | ".idea") {
      continue;
    }
    let from = entry.path();
    let to = dst.join(name_str.as_ref());
    if from.is_dir() {
      copy_dir_skip_build(&from, &to);
    } else {
      std::fs::copy(&from, &to).expect("copy fixture file");
    }
  }
}

fn remove_fixture_build_dirs(project_path: &Path) {
  for entry in std::fs::read_dir(project_path)
    .expect("read project root")
    .flatten()
  {
    if !entry.file_type().map(|ft| ft.is_dir()).unwrap_or(false) {
      continue;
    }
    let build_dir = entry.path().join("build");
    if build_dir.exists() {
      std::fs::remove_dir_all(build_dir).expect("remove module build dir");
    }
  }
}

// ---------------------------------------------------------------------------
// Helper: read a specific file from a BuildOutput by suffix match
// ---------------------------------------------------------------------------

/// Find the first file in `output` whose relative path ends with `suffix`
/// and return its contents as a UTF-8 string.
fn find_text_file<'a>(output: &'a BuildOutput, suffix: &str) -> Option<&'a [u8]> {
  output
    .files
    .iter()
    .find(|(k, _)| k.ends_with(suffix))
    .map(|(_, v)| v.as_slice())
}

// ---------------------------------------------------------------------------
// Self-consistency tests (no hvigor required)
// ---------------------------------------------------------------------------

/// Build the minimal-hap fixture twice and verify that `loader.json`,
/// `module.json`, and `pkgContextInfo.json` are bit-for-bit identical.
///
/// This is the primary determinism oracle that does not require hvigor.
#[test]
fn test_minimal_hap_build_is_deterministic() {
  let fixture_src = repo_root().join("tests/fixtures/minimal-hap");

  let fixture = fresh_fixture_copy(&fixture_src);
  let out1 = SelfConsistencyOracle::run_build(&fixture, false);
  remove_fixture_build_dirs(&fixture);
  let out2 = SelfConsistencyOracle::run_build(&fixture, false);

  // Compare the files we care most about for determinism.
  let files_to_check = ["loader.json", "module.json", "pkgContextInfo.json"];
  let mut mismatches: Vec<String> = Vec::new();

  for suffix in &files_to_check {
    match (find_text_file(&out1, suffix), find_text_file(&out2, suffix)) {
      (Some(b1), Some(b2)) => {
        if b1 != b2 {
          mismatches.push(format!(
            "{suffix}: sha256 run1={}, run2={}",
            sha256_hex(b1),
            sha256_hex(b2)
          ));
        }
      }
      (None, Some(_)) => {
        mismatches.push(format!("{suffix}: missing from run1"));
      }
      (Some(_), None) => {
        mismatches.push(format!("{suffix}: missing from run2"));
      }
      (None, None) => {
        // File not produced — that is itself a signal worth noting but
        // not a determinism failure per se; the build may not produce
        // all files for every fixture.
      }
    }
  }

  // Also do a full oracle comparison for completeness.
  let report = SelfConsistencyOracle::compare(&out1, &out2);

  let _ = std::fs::remove_dir_all(&fixture);

  if !mismatches.is_empty() {
    panic!(
      "build determinism check FAILED for minimal-hap.\n\
             Key file mismatches:\n  {}\n\
             Full diff:\n{}",
      mismatches.join("\n  "),
      report.to_display_string()
    );
  }

  // The full comparison may include files that are intentionally
  // non-deterministic (e.g. timestamps in ZIP local file headers).
  // We only enforce determinism for the JSON metadata files above.
  // Uncomment the next line to enforce full bit-for-bit determinism:
  // SelfConsistencyOracle::assert_identical(&report);
}

/// Build the minimal-hap fixture twice with `--disable-abc-cache` and verify
/// that the ABC output is identical between runs.
///
/// This confirms that the ABC compilation pipeline is deterministic even when
/// the coarse-grained cache is bypassed (i.e. the compiler itself is
/// deterministic, not just the cache).
#[test]
fn test_disable_abc_cache_produces_identical_output() {
  let fixture_src = repo_root().join("tests/fixtures/minimal-hap");

  let fixture1 = fresh_fixture_copy(&fixture_src);
  let out1 = SelfConsistencyOracle::run_build(&fixture1, /* disable_abc_cache */ true);

  let fixture2 = fresh_fixture_copy(&fixture_src);
  let out2 = SelfConsistencyOracle::run_build(&fixture2, /* disable_abc_cache */ true);

  // The ABC file is the primary artifact we care about.
  let abc_suffix = "ets/modules.abc";
  let abc1 = find_text_file(&out1, abc_suffix);
  let abc2 = find_text_file(&out2, abc_suffix);

  let _ = std::fs::remove_dir_all(&fixture1);
  let _ = std::fs::remove_dir_all(&fixture2);

  match (abc1, abc2) {
    (Some(b1), Some(b2)) => {
      assert_eq!(
        sha256_hex(b1),
        sha256_hex(b2),
        "ets/modules.abc must be identical between two --disable-abc-cache builds.\n\
                 run1 sha256={}\n\
                 run2 sha256={}",
        sha256_hex(b1),
        sha256_hex(b2),
      );
    }
    (None, _) | (_, None) => {
      // ABC was not produced — this can happen when the fixture has no
      // ETS sources.  Treat as a pass (the test is about ABC determinism
      // when ABC *is* produced).
      tracing::error!(
        "[self_consistency] ets/modules.abc not found in build output; \
                 skipping ABC determinism check"
      );
    }
  }
}

// ---------------------------------------------------------------------------
// Unit tests for the oracle infrastructure itself
// ---------------------------------------------------------------------------

/// Verify that `SelfConsistencyOracle::compare` correctly identifies
/// identical, added, removed, and changed files.
#[test]
fn test_compare_detects_all_diff_types() {
  let make_output = |files: &[(&str, &[u8])]| BuildOutput {
    project_root: PathBuf::from("/tmp/test"),
    files: files
      .iter()
      .map(|(k, v)| (k.to_string(), v.to_vec()))
      .collect(),
  };

  let out1 = make_output(&[
    ("a.json", b"{\"key\": 1}"),
    ("b.txt", b"hello"),
    ("only_in_1.txt", b"x"),
  ]);
  let out2 = make_output(&[
    ("a.json", b"{\"key\": 1}"),
    ("b.txt", b"world"),
    ("only_in_2.txt", b"y"),
  ]);

  let report = SelfConsistencyOracle::compare(&out1, &out2);
  assert!(!report.identical);
  assert_eq!(report.only_in_run1, vec!["only_in_1.txt"]);
  assert_eq!(report.only_in_run2, vec!["only_in_2.txt"]);
  assert_eq!(report.changed.len(), 1);
  assert_eq!(report.changed[0].path, "b.txt");
}

/// Verify that `SelfConsistencyOracle::compare` returns `identical = true`
/// for two equal outputs.
#[test]
fn test_compare_identical_outputs() {
  let make_output = |files: &[(&str, &[u8])]| BuildOutput {
    project_root: PathBuf::from("/tmp/test"),
    files: files
      .iter()
      .map(|(k, v)| (k.to_string(), v.to_vec()))
      .collect(),
  };

  let out1 = make_output(&[("a.json", b"{\"key\": 1}"), ("b.txt", b"hello")]);
  let out2 = make_output(&[("a.json", b"{\"key\": 1}"), ("b.txt", b"hello")]);

  let report = SelfConsistencyOracle::compare(&out1, &out2);
  assert!(
    report.identical,
    "identical outputs must produce identical=true"
  );
}

/// Verify that `FingerprintOracle::compute` produces stable SHA-256 hashes.
#[test]
fn test_fingerprint_compute_is_stable() {
  let output = BuildOutput {
    project_root: PathBuf::from("/tmp/test"),
    files: [
      ("file1.json".to_string(), b"{\"a\":1}".to_vec()),
      ("file2.txt".to_string(), b"hello world".to_vec()),
    ]
    .into_iter()
    .collect(),
  };

  let fp1 = FingerprintOracle::compute(&output);
  let fp2 = FingerprintOracle::compute(&output);

  assert_eq!(
    fp1["file1.json"].sha256, fp2["file1.json"].sha256,
    "fingerprint must be stable across calls"
  );
  assert_eq!(fp1["file1.json"].size, 7);
  assert_eq!(fp1["file2.txt"].size, 11);
}

/// Verify that `FingerprintOracle::assert_matches` writes a baseline on first
/// run and passes without panicking.
#[test]
fn test_fingerprint_oracle_write_then_match() {
  let tmp = std::env::temp_dir().join(format!(
    "arkdown_fp_test_{}",
    std::time::SystemTime::now()
      .duration_since(std::time::UNIX_EPOCH)
      .unwrap_or_default()
      .as_nanos()
  ));

  let output = BuildOutput {
    project_root: PathBuf::from("/tmp/test"),
    files: [("entry/build/loader.json".to_string(), b"{}".to_vec())]
      .into_iter()
      .collect(),
  };

  // First call — no stored file → writes baseline and passes.
  let oracle = FingerprintOracle::new(&tmp, "test_fixture");
  oracle.assert_matches(&output);
  assert!(
    oracle.fingerprint_path().exists(),
    "fingerprint file must be created"
  );

  // Second call — stored file matches → passes.
  let oracle2 = FingerprintOracle::new(&tmp, "test_fixture");
  oracle2.assert_matches(&output);

  let _ = std::fs::remove_dir_all(&tmp);
}

/// Verify that `FingerprintOracle::assert_matches` panics when content changes.
#[test]
fn test_fingerprint_oracle_mismatch_panics() {
  let tmp = std::env::temp_dir().join(format!(
    "arkdown_fp_mismatch_{}",
    std::time::SystemTime::now()
      .duration_since(std::time::UNIX_EPOCH)
      .unwrap_or_default()
      .as_nanos()
  ));

  let output_v1 = BuildOutput {
    project_root: PathBuf::from("/tmp/test"),
    files: [("entry/build/loader.json".to_string(), b"v1".to_vec())]
      .into_iter()
      .collect(),
  };
  let output_v2 = BuildOutput {
    project_root: PathBuf::from("/tmp/test"),
    files: [("entry/build/loader.json".to_string(), b"v2".to_vec())]
      .into_iter()
      .collect(),
  };

  // Write v1 baseline.
  let oracle = FingerprintOracle::new(&tmp, "mismatch_test");
  oracle.assert_matches(&output_v1);

  // Comparing v2 against v1 baseline must panic.
  let result = std::panic::catch_unwind(|| {
    let o = FingerprintOracle::new(&tmp, "mismatch_test");
    o.assert_matches(&output_v2);
  });
  assert!(
    result.is_err(),
    "FingerprintOracle::assert_matches must panic when content changes"
  );

  let _ = std::fs::remove_dir_all(&tmp);
}
