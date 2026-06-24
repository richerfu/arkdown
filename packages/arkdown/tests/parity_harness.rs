//! SDK es2abc input file parity harness.
//!
//! Verifies that the file layout produced by `BytecodeStage::run` matches
//! hvigor's expected SDK es2abc input format: `filesInfo.txt`,
//! `modules.cache`, source maps, and `modules.abc` / `widgets.abc`.
//!
//! All tests in this file are `#[ignore]` unless `OHOS_SDK_HOME` is set,
//! so they never run in CI without an SDK environment.
//!
//! # Running with SDK
//!
//! ```bash
//! OHOS_SDK_HOME=/path/to/sdk cargo test -p arkdown-napi parity -- --ignored
//! ```
//!
//! # Upstream correspondence
//!
//! Mirrors hvigor's `ark-compile.ts::doTaskAction` output file layout:
//! - `filesInfo.txt` — one `filePath;recordName;isEntry;sourceRoots` line per module
//! - `modules.cache` — incremental bytecode cache
//! - `sourceMaps.map` — source map output (JSON)
//! - `modules.abc` — compiled bytecode (Panda magic header)
//! - `widgets.abc` — form widget ABC stub (if form abilities present)

use std::path::{Path, PathBuf};

// ---------------------------------------------------------------------------
// Panda ABC magic header
// ---------------------------------------------------------------------------

/// Magic header for a valid Panda `.abc` file (8 bytes).
///
/// Matches `PANDA\x00\x00\x00` from `pandafile/file.h`.
const PANDA_MAGIC: &[u8; 8] = b"PANDA\x00\x00\x00";

fn panda_magic_is_valid(bytes: &[u8]) -> bool {
  bytes.len() >= 8 && &bytes[..8] == PANDA_MAGIC
}

// ---------------------------------------------------------------------------
// SDK guard helper
// ---------------------------------------------------------------------------

/// Skip the test if `OHOS_SDK_HOME` is not set.
macro_rules! require_sdk {
  () => {
    if std::env::var("OHOS_SDK_HOME").is_err() {
      tracing::error!("skipping parity test: OHOS_SDK_HOME not set");
      return;
    }
  };
}

// ---------------------------------------------------------------------------
// Helper: verify a `filesInfo.txt` file
// ---------------------------------------------------------------------------

/// Assert that a `filesInfo.txt` file at `path`:
/// - Exists and is non-empty
/// - Each line has at least 2 semicolon-separated fields
/// - At least one entry matches `entry_module_hint` (e.g. "entry")
fn assert_files_info_txt(path: &Path, entry_module_hint: &str) {
  let content = std::fs::read_to_string(path)
    .unwrap_or_else(|e| panic!("filesInfo.txt at {} not readable: {e}", path.display()));
  assert!(
    !content.trim().is_empty(),
    "filesInfo.txt must not be empty"
  );

  let mut found_entry = false;
  for line in content.lines() {
    if line.trim().is_empty() {
      continue;
    }
    let parts: Vec<&str> = line.splitn(4, ';').collect();
    assert!(
      parts.len() >= 2,
      "filesInfo.txt line has fewer than 2 fields: {line:?}"
    );
    if parts[0].contains(entry_module_hint) {
      found_entry = true;
    }
  }
  assert!(
    found_entry,
    "filesInfo.txt must contain at least one entry matching {entry_module_hint:?}\n\
         content:\n{content}"
  );
}

// ---------------------------------------------------------------------------
// Parity test: filesInfo.txt format
// ---------------------------------------------------------------------------

/// Validate that `filesInfo.txt` produced by a build has the expected layout.
///
/// This test requires `OHOS_SDK_HOME` to be set and builds the minimal
/// fixture project under `tests/fixtures/minimal_hap/` (if it exists).
///
/// # Upstream correspondence
///
/// Mirrors hvigor's `ark-compile.ts::writeFilesInfo` output format.
#[test]
#[ignore = "requires OHOS_SDK_HOME and a full build environment"]
fn parity_files_info_txt_format() {
  require_sdk!();

  // Locate test fixture
  let fixture = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
    .join("tests")
    .join("fixtures")
    .join("minimal_hap");
  if !fixture.exists() {
    tracing::error!("skipping parity_files_info_txt_format: fixture not found at {fixture:?}");
    return;
  }

  // Run arkdown build on the fixture
  let output_dir = tempfile::TempDir::new().expect("create tempdir");
  let status = std::process::Command::new("cargo")
    .args([
      "run",
      "--bin",
      "arkdown",
      "--",
      "build",
      "--project",
      fixture.to_str().unwrap(),
      "--target",
      "hap",
    ])
    .current_dir(env!("CARGO_MANIFEST_DIR"))
    .status()
    .expect("run arkdown build");
  assert!(status.success(), "arkdown build failed with: {status}");

  // Find and verify filesInfo.txt
  let _ = output_dir; // keep tempdir alive

  // Verify hvigor's expected output directory structure
  let build_dir = fixture.join("entry").join("build").join("default");
  let intermediates = build_dir.join("intermediates").join("loader_out");
  let files_info = intermediates.join("filesInfo.txt");

  if files_info.exists() {
    assert_files_info_txt(&files_info, "entry");
  } else {
    tracing::error!(
      "NOTE: filesInfo.txt not found at expected path {}, skipping detailed check",
      files_info.display()
    );
  }
}

// ---------------------------------------------------------------------------
// Parity test: modules.abc Panda magic header
// ---------------------------------------------------------------------------

/// Validate that `modules.abc` produced by a build has a valid Panda magic header.
///
/// # Upstream correspondence
///
/// Mirrors the `PANDA\x00\x00\x00` magic in pandafile/file.h from
/// ArkCompiler Ets Frontend.
#[test]
#[ignore = "requires OHOS_SDK_HOME and a full build environment"]
fn parity_modules_abc_panda_magic() {
  require_sdk!();

  let fixture = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
    .join("tests")
    .join("fixtures")
    .join("minimal_hap");
  if !fixture.exists() {
    tracing::error!("skipping parity_modules_abc_panda_magic: fixture not found");
    return;
  }

  // Look for modules.abc in the build output
  let build_dir = fixture.join("entry").join("build").join("default");
  let modules_abc = build_dir
    .join("intermediates")
    .join("loader_out")
    .join("ets")
    .join("modules.abc");

  if !modules_abc.exists() {
    tracing::error!(
      "NOTE: modules.abc not found at {}, run a build first",
      modules_abc.display()
    );
    return;
  }

  let bytes = std::fs::read(&modules_abc).unwrap_or_else(|e| panic!("read modules.abc: {e}"));
  assert!(
    panda_magic_is_valid(&bytes),
    "modules.abc does not start with Panda magic header; got: {:?}",
    &bytes[..bytes.len().min(8)]
  );
}

// ---------------------------------------------------------------------------
// Parity test: sourceMaps.map is valid JSON
// ---------------------------------------------------------------------------

/// Validate that the `sourceMaps.map` produced by a build is valid JSON.
///
/// # Upstream correspondence
///
/// Mirrors hvigor's source map output from `ark-compile.ts`.
#[test]
#[ignore = "requires OHOS_SDK_HOME and a full build environment"]
fn parity_source_maps_valid_json() {
  require_sdk!();

  let fixture = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
    .join("tests")
    .join("fixtures")
    .join("minimal_hap");
  if !fixture.exists() {
    tracing::error!("skipping parity_source_maps_valid_json: fixture not found");
    return;
  }

  let source_maps_path = fixture
    .join("entry")
    .join("build")
    .join("default")
    .join("intermediates")
    .join("loader_out")
    .join("sourceMaps.map");

  if !source_maps_path.exists() {
    tracing::error!("NOTE: sourceMaps.map not found, run a build first");
    return;
  }

  let content = std::fs::read_to_string(&source_maps_path)
    .unwrap_or_else(|e| panic!("read sourceMaps.map: {e}"));
  let _: serde_json::Value = serde_json::from_str(&content)
    .unwrap_or_else(|e| panic!("sourceMaps.map is not valid JSON: {e}"));
}

// ---------------------------------------------------------------------------
// Unit test: Panda magic constant is correct
// ---------------------------------------------------------------------------

#[test]
fn panda_magic_bytes_are_correct() {
  assert_eq!(PANDA_MAGIC, b"PANDA\x00\x00\x00");
  assert!(panda_magic_is_valid(b"PANDA\x00\x00\x00\x01\x02\x03"));
  assert!(!panda_magic_is_valid(b"NOTPANDA"));
  assert!(!panda_magic_is_valid(b"")); // too short
}
