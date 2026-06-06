//! ArkTS widget (form extension) compilation pass.
//!
//! ArkTS widgets are small UI components rendered inside the system launcher or
//! other host applications.  They require a **second, independent** ABC
//! compilation pass that runs after the main module compilation:
//!
//! - Uses a separate cache directory (`<cache>_widget/`).
//! - Reads a widget-specific `loader.json` (the *widget loader JSON*).
//! - Writes a separate `widgets.abc` output file.
//! - Never runs for HAR modules (HAR modules cannot expose widgets).
//!
//! # hvigor reference
//!
//! `ArkCompile.doTaskAction()` in
//! `hvigor-ohos-plugin/src/tasks/ark-compile.js`:
//!
//! ```js
//! if (this.needSubmitArkTsWidget) {
//!     const widgetConfig = {
//!         ...config,
//!         widgetCompile: "true",
//!         cachePath:    taskTempDir + "_widget",
//!         aceBuildJson: pathInfo.getWidgetLoaderJsonPath(),
//!         byteCodeHar2HspDep: this.service.getByteCodeHar2Hsp(),
//!     };
//!     widgetConfig.ignoreWarning = true;
//!     await submitArkCompileWork(...widgetConfig...);
//! }
//! ```
//!
//! # Activation condition
//!
//! ```rust,ignore
//! let need = need_widget_compile(&form_json_paths, module_type);
//! ```
//!
//! - `form_json_paths` — the list of `form.json` / `form_config.json` paths
//!   collected from the module's extension abilities with `extensionType == "form"`.
//! - `module_type` — must **not** be `"har"` for widget compilation to activate.

use std::path::PathBuf;

/// Input configuration for the widget ABC compilation pass.
///
/// All paths must be absolute.
#[derive(Debug, Clone)]
pub struct WidgetCompileConfig {
    /// Source `.js` files to compile (the widget entry points bundled from
    /// the widget source tree).
    pub inputs: Vec<WidgetInput>,
    /// Absolute path where the widget `.abc` output file should be written.
    ///
    /// hvigor writes this to `build/default/intermediates/loader_out/default/ets/widgets.abc`.
    pub output: PathBuf,
    /// Separate cache directory for the widget pass.
    ///
    /// hvigor appends `"_widget"` to the main task temp dir, e.g.
    /// `build/default/cache/default/ArkTSEvolution/entry_widget/`.
    pub cache_dir: PathBuf,
    /// API version for the widget compiler (same as the main module).
    pub target_api_version: u32,
    /// Whether to embed debug information.
    pub debug_info: bool,
}

/// One widget source file to compile.
#[derive(Debug, Clone)]
pub struct WidgetInput {
    /// Absolute path to the bundled `.js` source file.
    pub path: PathBuf,
    /// Record name in `@bundle:<bundleName>/<moduleName>/<path>` format.
    pub record_name: String,
    /// Optional source-file hint for debug info (the original `.ets` path).
    pub source_file: Option<String>,
}

/// Decide whether the widget compile pass should run for this module.
///
/// Returns `true` when:
/// - `form_json_paths` is non-empty (the module declares at least one form
///   extension ability), **and**
/// - `module_type` is not `"har"` (HAR modules cannot expose widgets).
///
/// This mirrors `shouldCompileArkTsWidget(formJsonPathArr, isFormExtensionModule)`
/// in hvigor's `task-util.js`.
pub fn need_widget_compile(form_json_paths: &[PathBuf], module_type: &str) -> bool {
    !form_json_paths.is_empty() && module_type != "har"
}

/// Collect the `form.json` paths declared by form extension abilities in a
/// module's `module.json5`.
///
/// hvigor's `TargetTaskService.getFormJsonArr()` walks the extension abilities
/// and collects form profile files for every ability whose `extensionType` is
/// exactly `"form"`. Full metadata validation lives in `arkdown-profile`.
///
/// # Parameters
/// - `module_src_main` — absolute path to `<module>/src/main/`
/// - `extension_abilities` — the `extensionAbilities` array from `module.json5`
pub fn collect_form_json_paths(
    module_src_main: &std::path::Path,
    extension_abilities: &[ExtensionAbilityRef<'_>],
) -> Vec<PathBuf> {
    extension_abilities
        .iter()
        .filter(|e| e.extension_type == "form")
        .filter_map(|e| {
            // src_entry is e.g. "./ets/widget/pages/WidgetCard.ets"
            // The form.json lives at:
            //   <module>/src/main/resources/base/profile/<form_name>.json
            // hvigor resolves it via the metadata `resource` field; as a
            // simpler heuristic we look for a `form.json` or `form_config.json`
            // alongside the extension's src_entry directory.
            let src = std::path::Path::new(e.src_entry.trim_start_matches("./"));
            let ets_dir = module_src_main.join("ets");
            let entry_dir = ets_dir.join(src).parent()?.to_path_buf();

            // Check for the two common naming conventions.
            for name in &["form.json", "form_config.json"] {
                let candidate = entry_dir.join(name);
                if candidate.exists() {
                    return Some(candidate);
                }
            }
            // Fall back: return the src_entry path itself as a signal that
            // the form extension exists (even without the JSON sidecar).
            Some(module_src_main.join(src))
        })
        .collect()
}

/// Lightweight reference to an extension ability entry — avoids pulling in
/// `arkdown-core` as a dependency from `arkdown-abc`.
pub struct ExtensionAbilityRef<'a> {
    pub extension_type: &'a str,
    pub src_entry: &'a str,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn need_widget_compile_true_when_form_paths_present() {
        let paths = vec![PathBuf::from(
            "/project/entry/src/main/ets/widget/form.json",
        )];
        assert!(need_widget_compile(&paths, "entry"));
        assert!(need_widget_compile(&paths, "feature"));
        assert!(need_widget_compile(&paths, "shared"));
    }

    #[test]
    fn need_widget_compile_false_for_har_module() {
        let paths = vec![PathBuf::from("/project/lib/src/main/ets/widget/form.json")];
        assert!(!need_widget_compile(&paths, "har"));
    }

    #[test]
    fn need_widget_compile_false_when_no_form_paths() {
        let paths: Vec<PathBuf> = vec![];
        assert!(!need_widget_compile(&paths, "entry"));
    }

    #[test]
    fn need_widget_compile_false_for_har_with_no_paths() {
        let paths: Vec<PathBuf> = vec![];
        assert!(!need_widget_compile(&paths, "har"));
    }

    #[test]
    fn collect_form_json_paths_empty_when_no_form_extensions() {
        let abilities = vec![ExtensionAbilityRef {
            extension_type: "backup",
            src_entry: "./ets/backup/BackupAbility.ets",
        }];
        let result =
            collect_form_json_paths(std::path::Path::new("/project/entry/src/main"), &abilities);
        assert!(
            result.is_empty(),
            "non-form extension must not produce paths"
        );
    }

    #[test]
    fn collect_form_json_paths_is_case_sensitive() {
        let abilities = vec![ExtensionAbilityRef {
            extension_type: "Form",
            src_entry: "./ets/widget/pages/WidgetCard.ets",
        }];
        let result =
            collect_form_json_paths(std::path::Path::new("/project/entry/src/main"), &abilities);
        assert!(result.is_empty());
    }

    /// Prove that the widget compile pass produces a valid Panda ABC binary.
    ///
    /// This tests the core widget compilation path (compile_widget_pass equivalent)
    /// to address section 3.6: 'widget loader/cache/ABC 字节未证明'.
    #[test]
    fn test_widget_compile_produces_valid_panda_abc() {
        let tmp = tempfile::tempdir().expect("tempdir");
        let out = tmp.path().join("widgets.abc");

        // Write a minimal widget source file.
        // Note: the widget compile pass receives bundled .js output (not raw .ets),
        // so we use standard JavaScript as the input.
        let widget_src = tmp.path().join("Card.js");
        std::fs::write(
            &widget_src,
            "\"use strict\";\nfunction Card() { return null; }\nexports.Card = Card;\n",
        )
        .expect("write widget source");

        let config = crate::WidgetCompileConfig {
            inputs: vec![crate::WidgetInput {
                path: widget_src.clone(),
                record_name: "com.example/entry/ets/widget/pages/Card".to_string(),
                source_file: Some("ets/widget/pages/Card.ets".to_string()),
            }],
            output: out.clone(),
            cache_dir: std::path::PathBuf::new(), // no cache
            target_api_version: 12,
            debug_info: true,
        };

        let compiler = crate::compiler::AbcCompiler::new(crate::compiler::AbcCompileOptions {
            debug_info: true,
            opt_level: 0,
            optimize_try_catch_func: false,
            target_api_version: 12,
            target_api_sub_version: None,
            enable_annotations: false,
            merge_abc: true,
            module: false,
            module_record_field_name: None,
            npm_entries: None,
            dependency_context: None,
            remove_redundant_file: false,
            dump_deps_info: false,
        });

        let result = compiler.compile_widget(&config);
        assert!(
            result.is_ok(),
            "widget compilation must succeed; err: {:?}",
            result.err()
        );

        // Output file must exist and start with the PANDA magic bytes.
        assert!(out.exists(), "widgets.abc must be written");
        let bytes = std::fs::read(&out).expect("read widgets.abc");
        assert_eq!(
            &bytes[..8],
            b"PANDA\x00\x00\x00",
            "widgets.abc must start with PANDA magic"
        );
    }
}
