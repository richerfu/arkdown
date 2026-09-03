//! Regression coverage for the explicit SDK comparison backend.
//!
//! Hvigor's ArkLint worker delegates this work to
//! `ets-loader/lib/ets_checker.js::etsStandaloneChecker`. The fixture is
//! copied before mutation so the checked-in source remains a valid build.

use std::fs;
use std::path::{Component, Path, PathBuf};
use std::process::Command;

use arkdown_core::{BuildMode, OutputTarget};
use walkdir::WalkDir;

#[test]
fn check_reports_sdk_typescript_semantic_diagnostics() {
  let fixture = repo_root().join("tests/fixtures/minimal-hap");
  if !sdk_ets_checker_available(&fixture) {
    eprintln!("skipping SDK semantic check regression: ets-loader is unavailable");
    return;
  }

  let temp = tempfile::tempdir().expect("temporary semantic-check fixture");
  copy_fixture(&fixture, temp.path()).expect("copy minimal HAP fixture");
  let source = temp.path().join("entry/src/main/ets/pages/Index.ets");
  let original = fs::read_to_string(&source).expect("read copied ArkTS source");
  fs::write(
    &source,
    format!("{original}\nconst semanticTypeError: number = \"not-a-number\";\n"),
  )
  .expect("inject TypeScript semantic error");

  let output = Command::new(env!("CARGO_BIN_EXE_arkdown"))
    .args(["check", "--project"])
    .arg(temp.path())
    .args(["--checker", "sdk"])
    .output()
    .expect("run arkdown check");
  let diagnostics = format!(
    "{}{}",
    String::from_utf8_lossy(&output.stdout),
    String::from_utf8_lossy(&output.stderr)
  );

  assert!(
    !output.status.success(),
    "SDK etsChecker must reject a TypeScript type mismatch; output:\n{diagnostics}"
  );
  assert!(
    diagnostics.contains("ARKDOWN_ETS_CHECK_FAILED"),
    "check must report its semantic-check failure boundary; output:\n{diagnostics}"
  );
  assert!(
    diagnostics.contains("not assignable to type 'number'")
      || diagnostics.contains("not assignable to type \"number\"")
      || diagnostics.contains("Type 'string'"),
    "failure must originate from the SDK TypeScript checker; output:\n{diagnostics}"
  );
}

#[test]
fn check_defaults_to_native_oxc_without_a_node_runtime() {
  let fixture = repo_root().join("tests/fixtures/minimal-hap");
  let output = Command::new(env!("CARGO_BIN_EXE_arkdown"))
    .args(["check", "--project"])
    .arg(&fixture)
    .env("NODE_HOME", fixture.join("missing-node-runtime"))
    .output()
    .expect("run native arkdown check");
  let diagnostics = format!(
    "{}{}",
    String::from_utf8_lossy(&output.stdout),
    String::from_utf8_lossy(&output.stderr)
  );

  assert!(
    output.status.success(),
    "default OXC check must not require Node: {diagnostics}"
  );
}

fn sdk_ets_checker_available(project_root: &Path) -> bool {
  arkdown_config::load_config(
    project_root,
    BuildMode::Debug,
    OutputTarget::Hap,
    None,
    None,
  )
  .map(|config| {
    config
      .sdk_config
      .ets_loader_path
      .join("compile_plugin.js")
      .is_file()
  })
  .unwrap_or(false)
}

fn repo_root() -> PathBuf {
  PathBuf::from(env!("CARGO_MANIFEST_DIR"))
    .parent()
    .expect("crates directory")
    .parent()
    .expect("repository root")
    .to_path_buf()
}

fn copy_fixture(source: &Path, destination: &Path) -> std::io::Result<()> {
  for entry in WalkDir::new(source).follow_links(false) {
    let entry = entry?;
    let relative = entry
      .path()
      .strip_prefix(source)
      .expect("fixture entry is nested");
    if relative.as_os_str().is_empty() || is_generated_fixture_path(relative) {
      continue;
    }
    let output = destination.join(relative);
    if entry.file_type().is_dir() {
      fs::create_dir_all(output)?;
    } else if entry.file_type().is_file() {
      if let Some(parent) = output.parent() {
        fs::create_dir_all(parent)?;
      }
      fs::copy(entry.path(), output)?;
    }
  }
  Ok(())
}

fn is_generated_fixture_path(relative: &Path) -> bool {
  relative.components().any(|component| {
    matches!(component, Component::Normal(name) if name == "build" || name == ".hvigor")
  })
}
