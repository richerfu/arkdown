//! OXC-native project semantic checking for the CompileArkTS root set.
//!
//! Upstream evidence: `ets_checker.ts::serviceChecker` creates one program
//! from `rootFileNames`, resolves package modules, then obtains syntactic and
//! semantic diagnostics before the source transform runs.

use std::path::{Path, PathBuf};

use anyhow::Result;
use arkdown_ets_checker::{check, CheckOptions};
use arkdown_resolver::{PackageResolutionPlan, PkgBriefInfo};
use arkdown_session::SourceStore;

#[derive(Debug, Default)]
pub struct ProjectSemanticCheckReport {
    pub errors: usize,
    pub diagnostics: Vec<String>,
}

pub struct ProjectSemanticChecker<'a> {
    project_root: &'a Path,
    root_files: &'a [PathBuf],
    package_resolution: &'a PackageResolutionPlan,
    sdk_declarations: Vec<PathBuf>,
}

impl<'a> ProjectSemanticChecker<'a> {
    pub fn new(
        project_root: &'a Path,
        root_files: &'a [PathBuf],
        package_resolution: &'a PackageResolutionPlan,
    ) -> Self {
        Self {
            project_root,
            root_files,
            package_resolution,
            sdk_declarations: Vec::new(),
        }
    }

    /// Attach the SDK declaration inventory captured by [`SourceStore`] at
    /// session startup. `ets_checker.ts::serviceChecker` receives these roots
    /// through TypeScript's configured module resolution; the native checker
    /// maps the same declaration filenames to exact module specifiers without
    /// invoking the SDK checker process.
    pub fn with_sdk_declaration_roots(
        mut self,
        source_store: &SourceStore,
        roots: &[PathBuf],
    ) -> Self {
        self.sdk_declarations = roots
            .iter()
            .flat_map(|root| source_store.text_sources_under(root))
            .map(|(path, _)| path.to_path_buf())
            .filter(|path| Self::declaration_specifier(path).is_some())
            .collect();
        self.sdk_declarations.sort();
        self.sdk_declarations.dedup();
        self
    }

    pub fn run(&self) -> Result<ProjectSemanticCheckReport> {
        let mut roots = self
            .root_files
            .iter()
            .filter(|path| Self::is_checkable_root(path))
            .cloned()
            .collect::<Vec<_>>();
        roots.sort();
        roots.dedup();
        if roots.is_empty() {
            return Ok(ProjectSemanticCheckReport::default());
        }

        let (module_paths, aliases) = self.resolver_inputs();
        let result = check(CheckOptions {
            project_root: self.project_root.to_path_buf(),
            root_files: roots,
            tsconfig_path: None,
            module_paths,
            aliases,
            strict_null_checks: false,
            // `compiler/tsconfig.json` used by `serviceChecker` sets this.
            skip_lib_check: true,
            // `compiler/src/interop/main.js` sets
            // `projectConfig.isolatedDeclarations = false`.
            report_isolated_declaration_diagnostics: false,
        })
        .map_err(|error| anyhow::anyhow!("ARKDOWN_OXC_CHECKER_PROGRAM: {error}"))?;

        let mut report = ProjectSemanticCheckReport::default();
        for file in result.files {
            for diagnostic in file.diagnostics {
                let offset = diagnostic
                    .labels
                    .first()
                    .map_or(0usize, |label| label.offset().try_into().unwrap_or(0));
                let (line, column) = Self::line_column(&file.source_text, offset);
                report.diagnostics.push(format!(
                    "{}:{line}:{column} {} {}",
                    file.path.display(),
                    diagnostic.code,
                    diagnostic.message
                ));
                report.errors += 1;
            }
        }
        Ok(report)
    }

    fn resolver_inputs(&self) -> (Vec<PathBuf>, Vec<(String, PathBuf)>) {
        let mut module_paths = vec![self.project_root.join("oh_modules")];
        let mut aliases = Vec::new();
        for (specifier, info) in self.package_resolution.sorted_entries() {
            if let Some(root) = Self::package_container(&info.pkg_root) {
                if !module_paths.contains(&root) {
                    module_paths.push(root);
                }
            }
            aliases.push((specifier.to_string(), info.pkg_root.clone()));
            aliases.push((format!("{specifier}$"), Self::package_type_entry(info)));
        }
        for declaration in &self.sdk_declarations {
            if let Some(specifier) = Self::declaration_specifier(declaration) {
                aliases.push((format!("{specifier}$"), declaration.clone()));
            }
        }
        module_paths.sort();
        module_paths.dedup();
        aliases.sort_by(|left, right| left.0.cmp(&right.0));
        aliases.dedup_by(|left, right| left.0 == right.0);
        (module_paths, aliases)
    }

    fn declaration_specifier(path: &Path) -> Option<&str> {
        let name = path.file_name()?.to_str()?;
        name.strip_suffix(".d.ets")
            .or_else(|| name.strip_suffix(".d.ts"))
    }

    fn package_type_entry(info: &PkgBriefInfo) -> PathBuf {
        let declaration = info.manifest.declaration_entry(&info.pkg_root);
        if declaration.is_file() {
            return declaration;
        }
        if let Some(main) = info.manifest.hvigor_main_field() {
            let entry = info.pkg_root.join(main);
            if entry.exists() {
                return entry;
            }
        }
        info.source_roots
            .first()
            .cloned()
            .unwrap_or_else(|| info.pkg_root.clone())
    }

    fn package_container(package_root: &Path) -> Option<PathBuf> {
        package_root.ancestors().find_map(|ancestor| {
            ancestor
                .file_name()
                .and_then(|name| name.to_str())
                .is_some_and(|name| matches!(name, "oh_modules" | "node_modules"))
                .then(|| ancestor.to_path_buf())
        })
    }

    fn is_checkable_root(path: &Path) -> bool {
        if !path.is_file() {
            return false;
        }
        let name = path
            .file_name()
            .and_then(|name| name.to_str())
            .unwrap_or_default();
        !name.ends_with(".d.ts")
            && !name.ends_with(".d.ets")
            && matches!(
                path.extension().and_then(|extension| extension.to_str()),
                Some("ets" | "ts" | "tsx" | "mts" | "cts")
            )
    }

    fn line_column(source: &str, offset: usize) -> (usize, usize) {
        let prefix = &source[..offset.min(source.len())];
        let line = prefix.bytes().filter(|byte| *byte == b'\n').count() + 1;
        let column = prefix
            .rfind('\n')
            .map_or(offset + 1, |newline| offset - newline);
        (line, column)
    }
}
