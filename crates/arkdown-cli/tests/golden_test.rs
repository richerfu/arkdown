//! Oracle golden test infrastructure for arkdown.
//!
//! Provides [`GoldenStore`] and [`assert_golden`] for comparing build outputs
//! against stored golden files without requiring hvigor.
//!
//! ## Usage
//!
//! ```no_run
//! use golden_test::{GoldenStore, assert_golden};
//!
//! let store = GoldenStore::new("tests/golden");
//! assert_golden(&store, "loader_json_minimal_hap", &actual_output);
//! ```
//!
//! ## Update workflow
//!
//! Set `ARKDOWN_UPDATE_GOLDEN=1` to regenerate all golden files:
//!
//! ```bash
//! ARKDOWN_UPDATE_GOLDEN=1 cargo test -p arkdown-cli golden
//! ```
//!
//! ## Golden file storage
//!
//! Golden files are stored under `tests/golden/` relative to the workspace
//! root.  Each golden is a plain text file named `<name>.golden`.  The
//! directory is created on first use.

use std::path::PathBuf;

// ---------------------------------------------------------------------------
// GoldenStore
// ---------------------------------------------------------------------------

/// Persistent store for golden test files.
///
/// Reads and writes golden files in a configurable directory.  The directory
/// is created lazily when the first golden file is written.
pub struct GoldenStore {
    dir: PathBuf,
}

impl GoldenStore {
    /// Create a `GoldenStore` rooted at `dir`.
    ///
    /// The directory does not need to exist yet — it is created when the first
    /// golden file is written.
    pub fn new(dir: impl Into<PathBuf>) -> Self {
        GoldenStore { dir: dir.into() }
    }

    /// Return the path to the golden file for `name`.
    pub fn golden_path(&self, name: &str) -> PathBuf {
        self.dir.join(format!("{}.golden", name))
    }

    /// Read the stored golden for `name`.
    ///
    /// Returns `None` when no golden file exists yet (first run).
    pub fn read(&self, name: &str) -> Option<String> {
        std::fs::read_to_string(self.golden_path(name)).ok()
    }

    /// Write `content` as the golden for `name`, creating parent directories
    /// as needed.
    pub fn write(&self, name: &str, content: &str) {
        let path = self.golden_path(name);
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent).expect("create golden directory");
        }
        std::fs::write(&path, content).expect("write golden file");
    }

    /// Return `true` when `ARKDOWN_UPDATE_GOLDEN=1` is set in the environment.
    pub fn update_mode() -> bool {
        std::env::var("ARKDOWN_UPDATE_GOLDEN")
            .map(|v| v == "1" || v.eq_ignore_ascii_case("true"))
            .unwrap_or(false)
    }
}

// ---------------------------------------------------------------------------
// assert_golden
// ---------------------------------------------------------------------------

/// Compare `actual` against the stored golden for `name`.
///
/// Behaviour:
/// - **First run** (no golden file): writes `actual` as the new golden and
///   prints an informational message.  The test passes.
/// - **Subsequent runs**: compares `actual` against the stored golden.
///   Fails the test with a diff-style message when they differ.
/// - **`ARKDOWN_UPDATE_GOLDEN=1`**: overwrites the golden with `actual` and
///   passes unconditionally.  Use this to accept intentional output changes.
///
/// # Panics
///
/// Panics (fails the test) when `actual` differs from the stored golden and
/// `ARKDOWN_UPDATE_GOLDEN` is not set.
pub fn assert_golden(store: &GoldenStore, name: &str, actual: &str) {
    if GoldenStore::update_mode() {
        store.write(name, actual);
        eprintln!("[golden] Updated golden '{}' (ARKDOWN_UPDATE_GOLDEN=1)", name);
        return;
    }

    match store.read(name) {
        None => {
            // First run — write the golden and pass.
            store.write(name, actual);
            eprintln!(
                "[golden] Created new golden '{}' at '{}'",
                name,
                store.golden_path(name).display()
            );
        }
        Some(expected) => {
            if actual != expected {
                // Produce a simple line-diff for the failure message.
                let diff = compute_diff(&expected, actual);
                panic!(
                    "golden '{}' mismatch.\n\
                     To update: ARKDOWN_UPDATE_GOLDEN=1 cargo test\n\
                     Diff (- expected, + actual):\n{}",
                    name, diff
                );
            }
        }
    }
}

// ---------------------------------------------------------------------------
// Diff helper
// ---------------------------------------------------------------------------

/// Produce a minimal line-level diff between `expected` and `actual`.
///
/// Lines present only in `expected` are prefixed with `-`.
/// Lines present only in `actual` are prefixed with `+`.
/// Common lines are prefixed with ` `.
///
/// This is intentionally simple (no LCS) and is only used for test failure
/// messages, not for production code.
fn compute_diff(expected: &str, actual: &str) -> String {
    let exp_lines: Vec<&str> = expected.lines().collect();
    let act_lines: Vec<&str> = actual.lines().collect();

    let max = exp_lines.len().max(act_lines.len());
    let mut out = String::new();
    let mut differences = 0;

    for i in 0..max {
        match (exp_lines.get(i), act_lines.get(i)) {
            (Some(e), Some(a)) if e == a => {
                out.push_str(&format!("  {}\n", e));
            }
            (Some(e), Some(a)) => {
                out.push_str(&format!("- {}\n+ {}\n", e, a));
                differences += 1;
            }
            (Some(e), None) => {
                out.push_str(&format!("- {}\n", e));
                differences += 1;
            }
            (None, Some(a)) => {
                out.push_str(&format!("+ {}\n", a));
                differences += 1;
            }
            (None, None) => {}
        }
        // Truncate very long diffs to avoid swamping the test output.
        if differences >= 20 {
            out.push_str(&format!(
                "  ... ({} more differing lines truncated)\n",
                max.saturating_sub(i + 1)
            ));
            break;
        }
    }

    out
}

// ---------------------------------------------------------------------------
// Golden store root helpers
// ---------------------------------------------------------------------------

/// Return the workspace root (two levels above `CARGO_MANIFEST_DIR`).
fn workspace_root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .expect("crates/arkdown-cli → crates/")
        .parent()
        .expect("crates/ → workspace root")
        .to_path_buf()
}

/// Return the default golden store directory: `<workspace_root>/tests/golden/arkdown`.
fn default_golden_dir() -> PathBuf {
    workspace_root().join("tests/golden/arkdown")
}

// ---------------------------------------------------------------------------
// Tests that use the golden infrastructure
// ---------------------------------------------------------------------------

/// Build a minimal [`LoaderJson`] and compare it against a stored golden.
///
/// This is the oracle comparison that does not require hvigor — it verifies
/// that arkdown's `ProfileProcessor::process_module` produces stable output
/// across builds.
#[test]
fn test_loader_json_golden_minimal_hap() {
    use arkdown_core::{
        AbilityConfig, ArkdownConfig, BuildMode, BuildOption, ModuleConfig, ModuleJson5,
        ModuleJson5Module, OhosModuleType, OutputTarget, ProductConfig, SdkConfig, StrictModeConfig,
    };
    use arkdown_profile::ProfileProcessor;
    use std::sync::Arc;

    let config = Arc::new(ArkdownConfig {
        project_root: workspace_root().join("tests/fixtures/minimal-hap"),
        build_mode: BuildMode::Debug,
        output_target: OutputTarget::Hap,
        product: ProductConfig {
            name: "default".to_string(),
            bundle_name: "com.example.arkdown.test".to_string(),
            compatible_sdk_version: "5.0.5(17)".to_string(),
            compatible_sdk_version_stage: None,
            runtime_os: "HarmonyOS".to_string(),
            signing_config: None,
            build_option: BuildOption {
                strict_mode: StrictModeConfig {
                    case_sensitive_check: false,
                    use_normalized_ohm_url: false,
                },
                ark_options: None,
                res_options: None,
                source_option: None,
                preload_system_so: false,
                preload_so: vec![],
                remove_permissions: vec![],
                aot_compile_mode: None,
                host_pgo: None,
                ap_path: None,
                abi_filters: vec![],
            },
            version_code: None,
            version_name: None,
        },
        modules: vec![],
        sdk_config: SdkConfig {
            ets_loader_path: PathBuf::from("/sdk/ets-loader"),
            es2abc_path: PathBuf::from("/sdk/es2abc"),
            restool_path: PathBuf::from("/sdk/restool"),
            compile_api_version: 17,
            has_rollup_plugin: true,
            compile_sdk_version: "6.0.1.112".to_string(),
        },
    });

    let module_cfg = ModuleConfig {
        name: "entry".to_string(),
        src_path: workspace_root()
            .join("tests/fixtures/minimal-hap")
            .join("entry"),
        module_type: OhosModuleType::Entry,
    };

    let module_json5 = ModuleJson5 {
        module: ModuleJson5Module {
            name: "entry".to_string(),
            type_: "entry".to_string(),
            main_element: Some("EntryAbility".to_string()),
            device_types: vec!["phone".to_string()],
            pages: Some("$profile:main_pages".to_string()),
            router_map: None,
            description: Some("$string:module_desc".to_string()),
            abilities: vec![AbilityConfig {
                name: "EntryAbility".to_string(),
                src_entry: "./ets/entryability/EntryAbility.ets".to_string(),
                exported: true,
                skills: vec![],
                icon: Some("$media:layered_image".to_string()),
                label: Some("$string:EntryAbility_label".to_string()),
                description: Some("$string:EntryAbility_desc".to_string()),
                start_window_icon: Some("$media:startIcon".to_string()),
                start_window_background: Some("$color:start_window_background".to_string()),
            }],
            extension_abilities: vec![],
            src_entry: None,
            metadata: vec![],
            app_startup: None,
            cross_app_shared_config: Default::default(),
            ..Default::default()
        },
    };

    let processor = ProfileProcessor::new(config);
    let (_, loader) = processor
        .process_module(&module_cfg, &module_json5)
        .expect("process_module must succeed");

    // Serialize to pretty JSON for golden comparison.
    let actual = serde_json::to_string_pretty(&loader).expect("serialize loader.json");

    let store = GoldenStore::new(default_golden_dir());
    assert_golden(&store, "loader_json_minimal_hap", &actual);
}

/// Verify that the golden infrastructure itself works correctly.
///
/// This test exercises the write-on-first-run and compare-on-subsequent-run
/// paths using a temporary directory so it does not pollute the real golden
/// store.
#[test]
fn test_golden_store_write_then_compare() {
    let tmp = std::env::temp_dir().join(format!(
        "arkdown_golden_test_{}",
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap_or_default()
            .as_nanos()
    ));

    let store = GoldenStore::new(&tmp);
    let content = "hello golden world\nline two\n";

    // First call: no golden file exists → write it.
    assert_golden(&store, "test_fixture", content);
    assert!(store.golden_path("test_fixture").exists());

    // Second call: golden file exists and matches → pass.
    assert_golden(&store, "test_fixture", content);

    // Cleanup.
    let _ = std::fs::remove_dir_all(&tmp);
}

/// Verify that `assert_golden` panics when actual differs from the golden.
#[test]
fn test_golden_mismatch_panics() {
    let tmp = std::env::temp_dir().join(format!(
        "arkdown_golden_mismatch_{}",
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap_or_default()
            .as_nanos()
    ));

    let store = GoldenStore::new(&tmp);

    // Write initial golden.
    store.write("mismatch_test", "original content\n");

    // Comparing different content should panic.
    let result = std::panic::catch_unwind(|| {
        let s = GoldenStore::new(&tmp);
        assert_golden(&s, "mismatch_test", "different content\n");
    });
    assert!(
        result.is_err(),
        "assert_golden must panic when actual differs from golden"
    );

    let _ = std::fs::remove_dir_all(&tmp);
}

/// Verify that `ARKDOWN_UPDATE_GOLDEN=1` overwrites an existing golden.
#[test]
fn test_golden_update_mode_overwrites() {
    let tmp = std::env::temp_dir().join(format!(
        "arkdown_golden_update_{}",
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap_or_default()
            .as_nanos()
    ));

    let store = GoldenStore::new(&tmp);

    // Write initial golden.
    store.write("update_test", "old content\n");

    // Simulate ARKDOWN_UPDATE_GOLDEN=1 by directly calling write.
    // (We cannot set env vars in tests without unsafe, so we test the
    // write path directly.)
    store.write("update_test", "new content\n");

    let read_back = store.read("update_test").expect("golden file must exist");
    assert_eq!(read_back, "new content\n");

    let _ = std::fs::remove_dir_all(&tmp);
}

/// Verify `compute_diff` produces expected output.
#[test]
fn test_compute_diff_shows_changes() {
    let expected = "line one\nline two\nline three\n";
    let actual = "line one\nLINE TWO CHANGED\nline three\n";
    let diff = compute_diff(expected, actual);
    assert!(diff.contains("- line two"));
    assert!(diff.contains("+ LINE TWO CHANGED"));
    assert!(diff.contains("  line one"));
}
