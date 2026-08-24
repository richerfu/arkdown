//! Import validity and cross-module import guard plugin.
//!
//! Validates that modules do not import across HAP/HSP/HAR package boundaries
//! in ways that would produce invalid OHM URL references at runtime. Reports
//! diagnostics for imports that cross module roots without going through the
//! published `oh_modules` interface.

use std::borrow::Cow;
use std::path::{Path, PathBuf};
use std::sync::Arc;

use arkdown_path::{lexical_normalize, ModuleSpecifierText, SpecifierKind};
use rolldown_common::{LogWithoutPlugin, ModuleInfo, NormalModule};
use rolldown_plugin::{HookNoopReturn, HookUsage, Plugin, PluginContext};

/// A single cross-module-boundary import that bypasses the `oh_modules` interface.
#[derive(Debug, Clone)]
pub struct ImportViolation {
    /// The importing module's resolved path.
    pub importer: String,
    /// The imported specifier as written in source.
    pub importee: String,
    /// Human-readable reason this import is invalid.
    pub reason: String,
}

/// Rolldown plugin adapter that checks for cross-boundary imports.
///
/// Each module is checked when it is parsed. If a local (non-`oh_modules`)
/// import crosses the module root boundary — i.e. it resolves to a path
/// outside the current module's source tree without going through
/// `oh_modules/` — the import is recorded as a violation.
///
/// At `build_end` all accumulated violations are emitted together. Hvigor
/// warns by default and upgrades them to a build error when effective
/// `strictMode.noExternalImportByPath` is true.
pub struct ImportCheckPlugin {
    /// Absolute roots from hvigor `projectConfig.allModulePaths`.
    module_roots: Arc<[PathBuf]>,
    /// Effective hvigor `strictMode.noExternalImportByPath` policy.
    no_external_import_by_path: bool,
}

impl std::fmt::Debug for ImportCheckPlugin {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("ImportCheckPlugin")
            .field("module_roots", &self.module_roots)
            .field(
                "no_external_import_by_path",
                &self.no_external_import_by_path,
            )
            .finish_non_exhaustive()
    }
}

impl ImportCheckPlugin {
    /// Create the plugin with hvigor's effective strict-mode severity.
    pub fn with_policy(module_roots: Vec<PathBuf>, no_external_import_by_path: bool) -> Self {
        Self {
            module_roots: module_roots
                .into_iter()
                .filter(|root| !root.as_os_str().is_empty())
                .map(|root| lexical_normalize(&root))
                .collect::<Vec<_>>()
                .into(),
            no_external_import_by_path,
        }
    }

    /// Mirror `import-check-plugin.ts::resolveId`: inspect the source
    /// specifier, not the resolver's output path. Bare package imports remain
    /// legal when they resolve into `.ohpm`, while absolute imports and
    /// relative imports that escape their owning project module are reported.
    fn check_request(
        module_roots: &[PathBuf],
        importer: &str,
        request: &str,
    ) -> Option<ImportViolation> {
        if request.contains("oh_modules") {
            return Some(ImportViolation {
                importer: importer.to_owned(),
                importee: request.to_owned(),
                reason: "cannot import files through an explicit oh_modules path".to_string(),
            });
        }

        let specifier = ModuleSpecifierText::new(request);
        if matches!(specifier.kind(), SpecifierKind::AbsolutePath) {
            return Some(ImportViolation {
                importer: importer.to_owned(),
                importee: request.to_owned(),
                reason: "avoid absolute paths in imports".to_string(),
            });
        }
        if !specifier.is_relative() {
            return None;
        }

        let importer_path = lexical_normalize(Path::new(importer));
        let module_root = module_roots
            .iter()
            .find(|root| importer_path.starts_with(root))?;
        let importee_path = lexical_normalize(
            &importer_path
                .parent()
                .unwrap_or(importer_path.as_path())
                .join(request),
        );
        if importee_path.starts_with(module_root) {
            return None;
        }

        Some(ImportViolation {
            importer: importer.to_owned(),
            importee: request.to_owned(),
            reason: format!(
                "relative import crosses module boundary: '{}' is outside module root '{}'",
                request,
                module_root.display(),
            ),
        })
    }

    fn diagnostic(entries: &[ImportViolation]) -> String {
        let details = entries
            .iter()
            .map(|violation| {
                format!(
                    "  [{}] → {}: {}",
                    violation.importer, violation.importee, violation.reason
                )
            })
            .collect::<Vec<_>>()
            .join("\n");
        format!(
            "import-check: {} invalid import path(s) detected:\n{}\n",
            entries.len(),
            details
        )
    }
}

impl Plugin for ImportCheckPlugin {
    fn name(&self) -> Cow<'static, str> {
        Cow::Borrowed("import-check")
    }

    fn register_hook_usage(&self) -> HookUsage {
        HookUsage::ModuleParsed
    }

    fn module_parsed(
        &self,
        _ctx: &PluginContext,
        module_info: Arc<ModuleInfo>,
        normal_module: &NormalModule,
    ) -> impl std::future::Future<Output = HookNoopReturn> + Send {
        let importer = module_info.id.as_str().to_owned();
        let requests = normal_module
            .ecma_view
            .import_records
            .iter()
            .filter(|record| record.resolved_module.is_some())
            .map(|record| record.module_request.to_string())
            .collect::<Vec<_>>();
        let module_roots = Arc::clone(&self.module_roots);
        let no_external_import_by_path = self.no_external_import_by_path;
        let ctx = _ctx.clone();

        async move {
            let entries = requests
                .iter()
                .filter_map(|request| {
                    ImportCheckPlugin::check_request(&module_roots, &importer, request)
                })
                .collect::<Vec<_>>();
            if entries.is_empty() {
                return Ok(());
            }
            let message = ImportCheckPlugin::diagnostic(&entries);
            if no_external_import_by_path {
                Err(anyhow::anyhow!(message))
            } else {
                ctx.warn(LogWithoutPlugin {
                    message,
                    code: Some("IMPORT_OUTSIDE_FILE_USING_RELATIVE_PATH".to_string()),
                    ids: Some(entries.into_iter().map(|entry| entry.importer).collect()),
                    ..Default::default()
                });
                Ok(())
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::ImportCheckPlugin;

    #[test]
    fn allows_bare_package_requests_that_resolve_outside_the_module() {
        let tmp = tempfile::tempdir().expect("tempdir");
        let module_root = tmp.path().join("entry");
        let importer = module_root.join("src/main/ets/pages/Home.ets");

        let violation = ImportCheckPlugin::check_request(
            &[module_root],
            importer.to_str().expect("utf8 importer"),
            "@example/mrouter",
        );

        assert!(
            violation.is_none(),
            "hvigor validates the original bare package request, not its resolved .ohpm path"
        );
    }

    #[test]
    fn rejects_relative_requests_that_escape_the_owning_module() {
        let tmp = tempfile::tempdir().expect("tempdir");
        let module_root = tmp.path().join("entry");
        let importer = module_root.join("src/main/ets/pages/Home.ets");

        let violation = ImportCheckPlugin::check_request(
            &[module_root],
            importer.to_str().expect("utf8 importer"),
            "../../../../../commons/aspect/Index",
        );

        assert!(
            violation.is_some(),
            "relative source requests must not cross the hvigor allModulePaths boundary"
        );
    }

    #[test]
    fn rejects_requests_that_spell_an_oh_modules_path() {
        let tmp = tempfile::tempdir().expect("tempdir");
        let module_root = tmp.path().join("entry");
        let importer = module_root.join("src/main/ets/pages/Home.ets");

        let violation = ImportCheckPlugin::check_request(
            &[module_root],
            importer.to_str().expect("utf8 importer"),
            "../../../../../oh_modules/@ohos/hypium/index",
        );

        assert!(
            violation.is_some(),
            "hvigor rejects explicit oh_modules path imports even when they resolve"
        );
    }

    #[test]
    fn allows_sibling_target_source_sets_below_the_hvigor_module_root() {
        let tmp = tempfile::tempdir().expect("tempdir");
        let module_root = tmp.path().join("entry");
        let importer = module_root.join("src/main/ets/pages/File.ets");

        let violation = ImportCheckPlugin::check_request(
            &[module_root],
            importer.to_str().expect("utf8 importer"),
            "../../../ohosTest/ets/test/utils/Reflection",
        );

        assert!(
            violation.is_none(),
            "Hvigor allModulePaths treats src/main and src/ohosTest as source sets of one module"
        );
    }
}
