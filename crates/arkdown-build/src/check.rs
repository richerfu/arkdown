//! Native OXC-backed standalone ArkTS checking.

use std::path::{Path, PathBuf};
use std::sync::Arc;

use anyhow::Result;
use walkdir::WalkDir;

use arkdown::plugins::ets_transform::{ArktsSourceChecker, ComponentMetadata};
use arkdown_core::{ArkdownConfig, BuildMode, ModuleConfig, OutputTarget};
use arkdown_path::ArtifactPathSerializer;
use arkdown_session::{BuildSession, BuildSessionBuilder, ModuleCompileContext};

use crate::package_resolution::{LocalPackageSourceRootInventory, ModulePackageResolutionPlanner};
use crate::project_checker::ProjectSemanticChecker;
use crate::source_snapshot::TransformSourceSnapshotBuilder;

#[derive(Debug)]
pub struct CheckRequest {
    /// Build target context: hap, hsp, har
    pub target: String,
    /// Scope to a specific module
    pub module: Option<String>,
    /// Project root (default: current directory)
    pub project: Option<PathBuf>,
    /// Enable ArkUI-X cross-platform API validation.
    pub is_crossplatform: bool,
    /// Downgrade cross-platform API diagnostics to warnings.
    pub ignore_crossplatform_check: bool,
    /// Native ArkTS TypeScript service resolved from the loaded N-API package.
    pub typescript_executable: Option<PathBuf>,
}

pub struct CheckApplication {
    args: CheckRequest,
}

impl CheckApplication {
    pub fn new(args: CheckRequest) -> Self {
        Self { args }
    }

    pub fn execute(self) -> Result<()> {
        let project_root =
            self.args.project.clone().unwrap_or_else(|| {
                std::env::current_dir().expect("failed to get current directory")
            });
        let target = Self::parse_target(&self.args.target);
        let config = Arc::new(
            arkdown_config::load_config(&project_root, BuildMode::Debug, target, None, None)
                .map_err(|error| anyhow::anyhow!("config error: {error}"))?,
        );
        // Checking follows the configured product language (default 1.1).
        // Product-configured 1.2 is supported without an explicit CLI override;
        // selecting it here is not a parity gap in the version-selection policy.
        let session = Arc::new(
            BuildSessionBuilder::new(config.clone())
                .with_arkts_language_version(config.product.arkts_version)
                .build()
                .map_err(|error| anyhow::anyhow!("check session: {error}"))?,
        );
        let mut summary = CheckSummary::default();
        for module in &config.modules {
            if self
                .args
                .module
                .as_deref()
                .is_some_and(|name| name != module.name)
            {
                continue;
            }
            let module_context = session.module_context(&module.name).ok_or_else(|| {
                anyhow::anyhow!(
                    "check session is missing module context for '{}'",
                    module.name
                )
            })?;
            let module_config = crate::planning::ModuleBuildConfiguration::resolve_build(
                config.as_ref(),
                session.build_options.as_ref(),
                module,
                None,
                &Default::default(),
            )?;
            let module_summary = CheckModuleRunner {
                config: module_config.config.as_ref(),
                session: session.as_ref(),
                module,
                module_context: &module_context,
                is_crossplatform: self.args.is_crossplatform,
                ignore_crossplatform_check: self.args.ignore_crossplatform_check,
                typescript_executable: self.args.typescript_executable.as_deref(),
            }
            .run()?;
            summary.merge(module_summary);
        }
        if summary.errors > 0 {
            let details = if summary.diagnostics.is_empty() {
                String::new()
            } else {
                format!("\n{}", summary.diagnostics.join("\n"))
            };
            anyhow::bail!(
                "ARKDOWN_ETS_CHECK_FAILED: {} error(s) across {} source file(s){}",
                summary.errors,
                summary.files,
                details
            );
        }
        tracing::info!(files = summary.files, "check passed");
        Ok(())
    }

    fn parse_target(target: &str) -> OutputTarget {
        match target {
            "hsp" => OutputTarget::Hsp,
            "har" => OutputTarget::Har,
            _ => OutputTarget::Hap,
        }
    }
}

struct CheckModuleRunner<'a> {
    config: &'a ArkdownConfig,
    session: &'a BuildSession,
    module: &'a ModuleConfig,
    module_context: &'a ModuleCompileContext,
    is_crossplatform: bool,
    ignore_crossplatform_check: bool,
    typescript_executable: Option<&'a Path>,
}

impl<'a> CheckModuleRunner<'a> {
    fn run(&self) -> Result<CheckSummary> {
        let sources = CheckSourceInventory::new(
            &self.module_context.module.source_roots,
            &self.session.source_store,
        )
        .collect()?;
        let mut summary = CheckSummary {
            files: sources.len(),
            ..CheckSummary::default()
        };
        let local_sources = LocalPackageSourceRootInventory::default();
        let package_resolution =
            ModulePackageResolutionPlanner::new(self.module_context, &local_sources).build()?;
        let module_layout = crate::layout::ModuleBuildLayout::for_output_target_product(
            &self.config.output_target,
            &self.module_context.module.root,
            &self.config.product.name,
            "default",
        );
        let project_path = self.module_context.module.src_main.join("ets");
        let compile_mode = self
            .module_context
            .module
            .module_json
            .as_ref()
            .and_then(|manifest| manifest.module.compile_mode.as_deref())
            .unwrap_or("esmodule");
        let semantic =
            ProjectSemanticChecker::new(&self.config.project_root, &sources, &package_resolution)
                .with_typescript_executable(self.typescript_executable)
                .with_compile_mode(compile_mode)
                .with_sdk_options(&self.session.sdk)?
                .with_sdk_declaration_roots(
                    &self.session.source_store,
                    self.session.sdk.global_module_paths.as_ref(),
                )
                .with_builtin_declarations(&self.config.sdk_config.ets_loader_path)
                .with_module_config(
                    self.config,
                    self.module,
                    self.module_context,
                    &self.session.project.bundle_type,
                )
                .with_compile_inputs(
                    &project_path,
                    None,
                    "ohpm",
                    None,
                    [
                        self.module_context.module.src_main.clone(),
                        module_layout
                            .build_profile_for_module(&self.module.module_type, "default")
                            .parent()
                            .map(Path::to_path_buf)
                            .unwrap_or_default(),
                        self.module_context.module.root.join("oh_modules"),
                        self.config.project_root.join("oh_modules"),
                    ],
                    std::iter::empty(),
                );
        let semantic = if let Some(capabilities) = self
            .module_context
            .module
            .module_json
            .as_ref()
            .and_then(|manifest| {
                self.session
                    .sdk
                    .system_capabilities
                    .select(&manifest.module.device_types)
            }) {
            semantic.with_sdk_validation_context(
                self.is_crossplatform,
                self.ignore_crossplatform_check,
                capabilities.intersection.into_iter().map(String::from),
                capabilities.union.into_iter().map(String::from),
            )
        } else {
            semantic.with_sdk_validation_context(
                self.is_crossplatform,
                self.ignore_crossplatform_check,
                std::iter::empty(),
                std::iter::empty(),
            )
        }
        .run()?;
        for diagnostic in semantic.diagnostics {
            tracing::error!(module = %self.module.name, diagnostic, "ArkTS native semantic diagnostic");
            summary
                .diagnostics
                .push(format!("[{}] {diagnostic}", self.module.name));
        }
        summary.errors += semantic.errors;
        for warning in semantic.warnings {
            tracing::warn!(module = %self.module.name, diagnostic = warning, "ArkTS native checker warning");
        }
        self.run_oxc_structural_checker(&sources, &mut summary)?;
        Ok(summary)
    }

    fn run_oxc_structural_checker(
        &self,
        sources: &[PathBuf],
        summary: &mut CheckSummary,
    ) -> Result<()> {
        let component_metadata = Arc::new(
            ComponentMetadata::from_ets_loader_path_with_external_declaration_roots(
                &self.config.sdk_config.ets_loader_path,
                self.session.sdk.global_module_paths.as_ref(),
            )?,
        );
        let module_roots: Vec<(String, PathBuf)> = self
            .config
            .modules
            .iter()
            .map(|module| (module.name.clone(), module.src_path.clone()))
            .collect();
        let mut sdk_declaration_roots = self
            .config
            .sdk_config
            .ets_api_dirs_for(self.session.arkts_language_version)
            .into_iter()
            .chain(
                self.config
                    .sdk_config
                    .ets_kits_dirs_for(self.session.arkts_language_version),
            )
            .chain(
                self.config
                    .sdk_config
                    .ets_arkts_dirs_for(self.session.arkts_language_version),
            )
            .collect::<Vec<_>>();
        for root in self.session.sdk.global_module_paths.iter() {
            if !sdk_declaration_roots.contains(root) {
                sdk_declaration_roots.push(root.clone());
            }
        }
        let project_sources = TransformSourceSnapshotBuilder::new(&module_roots, &[], &[], &[])
            .with_source_store(&self.session.source_store)
            .with_sdk_declaration_roots(&sdk_declaration_roots)
            .build()?;
        let checker = ArktsSourceChecker::new(component_metadata)
            .with_arkts_language_version(self.session.arkts_language_version)
            .with_project_sources(project_sources.clone(), module_roots);
        for path in sources
            .iter()
            .filter(|path| CheckSourceInventory::is_arkts_source(path))
        {
            let Some(source) = project_sources.source_text(path) else {
                continue;
            };
            let source_id = match ArtifactPathSerializer::path_to_slash_string(path) {
                Ok(source_id) => source_id,
                Err(error) => {
                    tracing::error!(path = %path.display(), error = %error, "check cannot encode source path");
                    summary.errors += 1;
                    summary
                        .diagnostics
                        .push(format!("[{}] {error}", path.display()));
                    continue;
                }
            };
            let report = checker.check(&source_id, &source);
            for diagnostic in report.diagnostics() {
                if diagnostic.is_error() {
                    tracing::error!(path = %path.display(), diagnostic = %diagnostic, "ArkTS OXC check failed");
                    summary
                        .diagnostics
                        .push(format!("[{}] {diagnostic}", path.display()));
                } else {
                    tracing::warn!(path = %path.display(), diagnostic = %diagnostic, "ArkTS OXC check warning");
                }
            }
            summary.errors += report.error_count();
        }
        Ok(())
    }
}

struct CheckSourceInventory<'a> {
    roots: &'a [PathBuf],
    source_store: &'a arkdown_session::SourceStore,
}

impl<'a> CheckSourceInventory<'a> {
    fn new(roots: &'a [PathBuf], source_store: &'a arkdown_session::SourceStore) -> Self {
        Self {
            roots,
            source_store,
        }
    }

    fn collect(&self) -> Result<Vec<PathBuf>> {
        let mut sources = Vec::new();
        for root in self.roots {
            if self.source_store.covers(root) {
                sources.extend(
                    self.source_store
                        .text_sources_under(root)
                        .into_iter()
                        .map(|(path, _)| path)
                        .filter(|path| Self::is_checkable_source(path))
                        .map(Path::to_path_buf),
                );
                continue;
            }
            if !root.is_dir() {
                continue;
            }
            for entry in WalkDir::new(root).follow_links(false) {
                let entry = entry
                    .map_err(|error| anyhow::anyhow!("ARKDOWN_ETS_CHECK_SOURCE_WALK: {}", error))?;
                if entry.file_type().is_file() && Self::is_checkable_source(entry.path()) {
                    sources.push(entry.path().to_path_buf());
                }
            }
        }
        sources.sort();
        sources.dedup();
        Ok(sources)
    }

    fn is_checkable_source(path: &Path) -> bool {
        if path
            .file_name()
            .and_then(|name| name.to_str())
            .is_some_and(|name| name.ends_with(".d.ts") || name.ends_with(".d.ets"))
        {
            return false;
        }
        matches!(
            path.extension().and_then(|extension| extension.to_str()),
            Some("ets" | "ts" | "js")
        )
    }

    fn is_arkts_source(path: &Path) -> bool {
        matches!(
            path.extension().and_then(|extension| extension.to_str()),
            Some("ets" | "ts")
        ) && !path
            .file_name()
            .and_then(|name| name.to_str())
            .is_some_and(|name| name.ends_with(".d.ets") || name.ends_with(".d.ts"))
    }
}

#[derive(Default)]
struct CheckSummary {
    files: usize,
    errors: usize,
    diagnostics: Vec<String>,
}

impl CheckSummary {
    fn merge(&mut self, other: Self) {
        self.files += other.files;
        self.errors += other.errors;
        self.diagnostics.extend(other.diagnostics);
    }
}
