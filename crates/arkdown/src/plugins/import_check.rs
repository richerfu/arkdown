//! Import validity and cross-module import guard plugin.
//!
//! Validates that modules do not import across HAP/HSP/HAR package boundaries
//! in ways that would produce invalid OHM URL references at runtime. Reports
//! diagnostics for imports that cross module roots without going through the
//! published `oh_modules` interface.

use std::borrow::Cow;
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};

use arkdown_path::lexical_normalize;
use rolldown_common::{ModuleInfo, NormalModule};
use rolldown_plugin::{HookBuildEndArgs, HookNoopReturn, HookUsage, Plugin, PluginContext};

/// Collects cross-boundary import violations found during the bundle.
#[derive(Debug, Default)]
struct Violations {
    entries: Vec<ImportViolation>,
}

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
/// At `build_end` all accumulated violations are emitted as a single
/// aggregated error so that the user sees all problems at once.
pub struct ImportCheckPlugin {
    /// Absolute module root for the OHOS module being bundled.
    module_root: PathBuf,
    /// Resolved package roots that are legal import targets for this module.
    allowed_package_roots: Vec<PathBuf>,
    /// Collected violations, shared across hook invocations.
    violations: Arc<Mutex<Violations>>,
}

impl std::fmt::Debug for ImportCheckPlugin {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("ImportCheckPlugin")
            .field("module_root", &self.module_root)
            .field("allowed_package_roots", &self.allowed_package_roots)
            .finish_non_exhaustive()
    }
}

impl ImportCheckPlugin {
    /// Create a new `ImportCheckPlugin` for the given module root.
    pub fn new(module_root: PathBuf) -> Self {
        Self::with_allowed_package_roots(module_root, Vec::new())
    }

    /// Create a new `ImportCheckPlugin` with dependency package roots that may
    /// be imported through their package interface.
    pub fn with_allowed_package_roots(
        module_root: PathBuf,
        allowed_package_roots: Vec<PathBuf>,
    ) -> Self {
        Self {
            module_root: lexical_normalize(&module_root),
            allowed_package_roots: allowed_package_roots
                .into_iter()
                .filter(|root| !root.as_os_str().is_empty())
                .map(|root| lexical_normalize(&root))
                .collect(),
            violations: Arc::new(Mutex::new(Violations::default())),
        }
    }

    /// Return a snapshot of all violations collected so far.
    pub fn violations_snapshot(&self) -> Vec<ImportViolation> {
        self.violations
            .lock()
            .map(|v| v.entries.clone())
            .unwrap_or_default()
    }

    /// Check whether an import from `importer` to `importee_resolved` crosses
    /// the module root boundary without going through `oh_modules/`.
    fn check_boundary(
        module_root: &Path,
        allowed_package_roots: &[PathBuf],
        importer: &str,
        importee_resolved: &str,
    ) -> Option<ImportViolation> {
        // Only check local filesystem paths.
        let importee_raw = Path::new(importee_resolved);
        if !importee_raw.is_absolute() {
            return None;
        }
        let module_root = lexical_normalize(module_root);
        let importee_path = lexical_normalize(importee_raw);

        // Imports that stay within the module root are allowed.
        if importee_path.starts_with(&module_root) {
            return None;
        }
        // Package-name imports can resolve directly to a workspace file: HAR
        // package root. Those are still package-interface imports, not raw
        // relative cross-module source imports.
        if allowed_package_roots
            .iter()
            .any(|root| importee_path.starts_with(root))
        {
            return None;
        }

        // The importer must itself be inside the module root for the check to
        // be meaningful. External importers are handled by the resolver.
        if !lexical_normalize(Path::new(importer)).starts_with(&module_root) {
            return None;
        }
        Some(ImportViolation {
            importer: importer.to_owned(),
            importee: importee_resolved.to_owned(),
            reason: format!(
                "import crosses module boundary: '{}' is outside module root '{}'",
                importee_resolved,
                module_root.display(),
            ),
        })
    }
}

impl Plugin for ImportCheckPlugin {
    fn name(&self) -> Cow<'static, str> {
        Cow::Borrowed("import-check")
    }

    fn register_hook_usage(&self) -> HookUsage {
        HookUsage::ModuleParsed | HookUsage::BuildEnd
    }

    fn module_parsed(
        &self,
        _ctx: &PluginContext,
        module_info: Arc<ModuleInfo>,
        _normal_module: &NormalModule,
    ) -> impl std::future::Future<Output = HookNoopReturn> + Send {
        let importer = module_info.id.as_str().to_owned();
        let imported_ids: Vec<String> = module_info
            .imported_ids
            .iter()
            .map(|id| id.as_str().to_owned())
            .collect();
        let dynamically_imported_ids: Vec<String> = module_info
            .dynamically_imported_ids
            .iter()
            .map(|id| id.as_str().to_owned())
            .collect();
        let module_root = self.module_root.clone();
        let allowed_package_roots = self.allowed_package_roots.clone();
        let violations = Arc::clone(&self.violations);

        async move {
            let mut new_violations: Vec<ImportViolation> = Vec::new();
            for importee in imported_ids.iter().chain(dynamically_imported_ids.iter()) {
                if let Some(v) = ImportCheckPlugin::check_boundary(
                    &module_root,
                    &allowed_package_roots,
                    &importer,
                    importee,
                ) {
                    new_violations.push(v);
                }
            }
            if !new_violations.is_empty() {
                violations
                    .lock()
                    .map_err(|e| anyhow::anyhow!("import-check: failed to lock violations: {e}"))?
                    .entries
                    .extend(new_violations);
            }
            Ok(())
        }
    }

    fn build_end(
        &self,
        _ctx: &PluginContext,
        _args: Option<&HookBuildEndArgs<'_>>,
    ) -> impl std::future::Future<Output = HookNoopReturn> + Send {
        let violations = Arc::clone(&self.violations);
        async move {
            let entries = violations
                .lock()
                .map_err(|e| {
                    anyhow::anyhow!("import-check: failed to lock violations at build_end: {e}")
                })?
                .entries
                .clone();
            if entries.is_empty() {
                return Ok(());
            }
            let details = entries
                .iter()
                .map(|v| format!("  [{}] → {}: {}", v.importer, v.importee, v.reason))
                .collect::<Vec<_>>()
                .join("\n");
            let msg = format!(
                "import-check: {} cross-boundary import(s) detected:\n{}\n",
                entries.len(),
                details
            );
            Err(anyhow::anyhow!("{}", msg))
        }
    }
}

#[cfg(test)]
mod tests {
    use super::ImportCheckPlugin;

    #[test]
    fn allows_resolved_file_har_package_roots() {
        let tmp = tempfile::tempdir().expect("tempdir");
        let module_root = tmp.path().join("products/phone");
        let dep_root = tmp.path().join("commons/aspect");
        let importer = module_root.join("src/main/ets/pages/Home.ets");
        let importee = dep_root.join("Index.ets");

        let violation = ImportCheckPlugin::check_boundary(
            &module_root,
            &[dep_root],
            importer.to_str().expect("utf8 importer"),
            importee.to_str().expect("utf8 importee"),
        );

        assert!(
            violation.is_none(),
            "declared file: HAR package roots are legal package-interface imports"
        );
    }

    #[test]
    fn rejects_unlisted_workspace_roots() {
        let tmp = tempfile::tempdir().expect("tempdir");
        let module_root = tmp.path().join("products/phone");
        let importer = module_root.join("src/main/ets/pages/Home.ets");
        let importee = tmp.path().join("commons/aspect/Index.ets");

        let violation = ImportCheckPlugin::check_boundary(
            &module_root,
            &[],
            importer.to_str().expect("utf8 importer"),
            importee.to_str().expect("utf8 importee"),
        );

        assert!(
            violation.is_some(),
            "workspace files outside the module remain blocked unless resolved as package roots"
        );
    }

    #[test]
    fn rejects_unlisted_oh_modules_roots() {
        let tmp = tempfile::tempdir().expect("tempdir");
        let module_root = tmp.path().join("entry");
        let importer = module_root.join("src/main/ets/pages/Home.ets");
        let importee = tmp.path().join("oh_modules/@ohos/hypium/index.ets");

        let violation = ImportCheckPlugin::check_boundary(
            &module_root,
            &[],
            importer.to_str().expect("utf8 importer"),
            importee.to_str().expect("utf8 importee"),
        );

        assert!(
            violation.is_some(),
            "oh_modules paths are legal only when their package root comes from the dependency graph"
        );
    }
}
