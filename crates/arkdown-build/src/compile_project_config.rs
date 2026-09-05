//! Source-owned `projectConfig` construction for ArkTS compilation and plugins.
//!
//! Rust owns the immutable configuration snapshot exposed to ArkTS compilation
//! and to user-authored compile plugins. No checker or JavaScript process is
//! started from this module.
//!
//! Upstream evidence:
//! - `hvigor-ohos-plugin/tasks/worker/util.js::generateLoaderEnv`

use std::collections::{BTreeMap, BTreeSet};
use std::path::{Component, Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};

use anyhow::{Context, Result};
use serde::Serialize;
use serde_json::{json, Map, Value};
use sha2::{Digest, Sha256};

use arkdown_config::build_profile::parse_api_version;
use arkdown_core::{ArkdownConfig, BuildMode, BuildOption, ModuleConfig, OhosModuleType};
use arkdown_path::{lexical_normalize, ArtifactPathSerializer};
use arkdown_profile::LoaderJson;
use arkdown_resolver::PackageResolutionPlan;
use arkdown_session::{BuildSession, ModuleCompileContext};

static PROJECT_CONFIG_WORKSPACE_SEQUENCE: AtomicU64 = AtomicU64::new(0);

/// Filesystem paths represented in the compile-plugin `projectConfig`.
#[derive(Debug, Clone)]
pub struct CompileProjectPaths {
    pub ace_profile_path: PathBuf,
    pub ace_module_json_path: PathBuf,
    pub app_resource_path: PathBuf,
    pub raw_file_resource_path: PathBuf,
    pub ace_module_root: PathBuf,
    pub ace_super_visual_path: PathBuf,
    pub ace_build_json_path: PathBuf,
    pub cache_path: PathBuf,
    pub ace_module_build: PathBuf,
    pub generated_profile_path: PathBuf,
    pub resource_table_typescript_path: PathBuf,
    workspace: PathBuf,
}

/// Generated build artifacts represented in the compile configuration.
#[derive(Debug, Clone)]
pub struct CompileProjectBuildPaths {
    pub ace_profile_path: PathBuf,
    pub ace_module_json_path: PathBuf,
    pub app_resource_path: PathBuf,
    pub raw_file_resource_path: PathBuf,
    pub ace_module_root: PathBuf,
    pub ace_super_visual_path: PathBuf,
    pub ace_build_json_path: PathBuf,
    pub cache_path: PathBuf,
    pub ace_module_build: PathBuf,
    pub generated_profile_path: PathBuf,
    pub resource_table_typescript_path: PathBuf,
}

impl CompileProjectPaths {
    /// Construct paths for one CompileArkTS target.
    pub fn build(input: CompileProjectBuildPaths) -> Self {
        let CompileProjectBuildPaths {
            ace_profile_path,
            ace_module_json_path,
            app_resource_path,
            raw_file_resource_path,
            ace_module_root,
            ace_super_visual_path,
            ace_build_json_path,
            cache_path,
            ace_module_build,
            generated_profile_path,
            resource_table_typescript_path,
        } = input;
        let workspace = Self::next_workspace(&cache_path);
        Self {
            ace_profile_path,
            ace_module_json_path,
            app_resource_path,
            raw_file_resource_path,
            ace_module_root,
            ace_super_visual_path,
            ace_build_json_path,
            cache_path,
            ace_module_build,
            generated_profile_path,
            resource_table_typescript_path,
            workspace,
        }
    }

    fn next_workspace(parent: &Path) -> PathBuf {
        let sequence = PROJECT_CONFIG_WORKSPACE_SEQUENCE.fetch_add(1, Ordering::Relaxed);
        parent.join(format!(
            "arkdown-compile-project-{}-{sequence}",
            std::process::id()
        ))
    }
}

/// Typed inputs collected before exposing the compile configuration.
pub struct CompileProjectConfigInput<'a> {
    pub config: &'a ArkdownConfig,
    pub session: &'a BuildSession,
    pub module: &'a ModuleConfig,
    pub module_context: &'a ModuleCompileContext,
    pub package_resolution: &'a PackageResolutionPlan,
    pub module_build_option: &'a BuildOption,
    pub loader_json: Option<&'a LoaderJson>,
    pub target_name: &'a str,
    pub paths: CompileProjectPaths,
    pub entries: Vec<PathBuf>,
    pub coverage_enabled: bool,
    pub coverage_path_filter: Option<&'a str>,
    pub coverage_mode: Option<&'a str>,
    pub debug_line: bool,
    pub preview_mode: bool,
    pub check_entry: bool,
    pub watch_mode: bool,
    pub byte_code_har_integrated_optimization: bool,
    pub single_file_emit: bool,
    pub rollup_cache_use_source_hash: bool,
    pub is_crossplatform: bool,
    pub ignore_crossplatform_check: bool,
}

struct CompileProjectModuleLayout<'a> {
    root: &'a Path,
}

impl<'a> CompileProjectModuleLayout<'a> {
    fn new(root: &'a Path) -> Self {
        Self { root }
    }

    fn mock_config_json5(&self) -> PathBuf {
        self.root.join("src").join("mock").join("mock-config.json5")
    }

    fn ets_source_root(&self) -> PathBuf {
        self.root.join("src").join("main").join("ets")
    }

    fn ets_source_root_artifact_path(&self) -> Result<String> {
        let relative = self
            .ets_source_root()
            .strip_prefix(self.root)
            .map_err(|error| anyhow::anyhow!("derive ETS source root: {error}"))?
            .to_path_buf();
        Ok(arkdown_path::ArtifactPathSerializer::rel_to_slash(
            &relative,
        ))
    }
}

/// Collect the same root source set that latest `rollup-plugin-ets-checker`
/// passes to `serviceChecker`: CompileArkTS entries plus `oh-exports` code
/// roots. `LoaderJson::compile_entry` is intentionally retained in its
/// src/main-relative form for Arkdown's bundler, so this owner resolves it
/// once for the SDK service.
pub struct CompileProjectEntryCollector<'a> {
    loader_json: &'a LoaderJson,
    source_root: PathBuf,
}

impl<'a> CompileProjectEntryCollector<'a> {
    pub fn new(loader_json: &'a LoaderJson, ace_module_root: &Path) -> Self {
        Self {
            loader_json,
            source_root: ace_module_root
                .parent()
                .map(Path::to_path_buf)
                .unwrap_or_else(|| PathBuf::from(&loader_json.module_root_path)),
        }
    }

    pub fn collect(&self) -> Vec<PathBuf> {
        let mut entries = Vec::new();
        let mut seen = BTreeSet::new();
        for entry in self
            .loader_json
            .hvigor_compile_entry
            .iter()
            .chain(self.loader_json.compile_entry.iter())
            .chain(self.loader_json.oh_exports.iter())
        {
            let path = PathBuf::from(entry);
            let path = if path.is_absolute() {
                path
            } else {
                self.source_root.join(path)
            };
            let path = lexical_normalize(&path);
            if seen.insert(path.clone()) {
                entries.push(path);
            }
        }
        entries
    }
}

/// Rust-owned builder for the compile-plugin `projectConfig` snapshot.
pub struct CompileProjectConfigBuilder<'a> {
    input: CompileProjectConfigInput<'a>,
}

impl<'a> CompileProjectConfigBuilder<'a> {
    pub fn new(input: CompileProjectConfigInput<'a>) -> Self {
        Self { input }
    }

    /// Return the exact Rust-owned ArkTS compile configuration used by both the
    /// JavaScript/TypeScript compile plugins. Keeping a single serialized
    /// snapshot prevents the plugin boundary from reconstructing a smaller,
    /// divergent configuration.
    pub fn project_config_value(&self) -> Result<Value> {
        serde_json::to_value(self.project_config()?).map_err(Into::into)
    }

    /// `ArkCompile.initDefaultArkCompileConfig().mockParams`, sourced from the
    /// immutable session dependency graph instead of re-running OHPM lookup in
    /// later build stages.
    fn mock_params(&self) -> Result<Value> {
        let module_layout = CompileProjectModuleLayout::new(&self.input.module_context.module.root);
        let mock_config_path = module_layout.mock_config_json5();
        let mut mock_config_key_to_module_info = Map::new();
        let mut source_to_module_id = Map::new();

        if let Some(source) = self
            .input
            .session
            .source_store
            .source_by_path(&mock_config_path)
        {
            let content = match source.cached_content() {
                Some(arkdown_session::SourceContent::Text(content)) => content.as_ref(),
                _ => {
                    anyhow::bail!(
            "ARKDOWN_ETS_CHECKER_MOCK_CONFIG_CONTENT: mock config {} was not captured as UTF-8 text",
            mock_config_path.display()
          )
                }
            };
            let entries = json5::from_str::<indexmap::IndexMap<String, Value>>(content)
                .with_context(|| format!("parse mock config {}", mock_config_path.display()))?;
            for (key, entry) in entries {
                if let Some(source) = entry.get("source").and_then(Value::as_str) {
                    if !source.is_empty() {
                        let source_path = self.input.module_context.module.root.join(source);
                        if self
                            .input
                            .session
                            .source_store
                            .source_by_path(&source_path)
                            .is_some()
                        {
                            source_to_module_id.insert(
                                source.to_string(),
                                Value::String(self.path(&source_path)?),
                            );
                        }
                    }
                }
                if key.starts_with('.') {
                    continue;
                }
                let mut components = key.split(['/', '\\']);
                let Some(first) = components.next() else {
                    continue;
                };
                let package_key = if first.starts_with('@') {
                    components
                        .next()
                        .map(|second| format!("{first}/{second}"))
                        .unwrap_or_else(|| first.to_string())
                } else {
                    first.to_string()
                };
                // `ModuleTaskService.getAllDependencies()` is folded into a Map by
                // dependency name, so a later edge with the same alias wins. Preserve
                // that traversal contract instead of guessing a package directory.
                let Some(edge) = self
                    .input
                    .module_context
                    .deps
                    .edges
                    .iter()
                    .rev()
                    .find(|edge| edge.alias == package_key)
                else {
                    continue;
                };
                let Some(node) = self.input.module_context.deps.nodes.get(&edge.package_name)
                else {
                    continue;
                };
                let package_root = &edge.package_root;
                let consumed = if package_key.starts_with('@') { 2 } else { 1 };
                let suffix = key
                    .split(['/', '\\'])
                    .skip(consumed)
                    .fold(PathBuf::new(), |path, component| path.join(component));
                let file_path = if suffix.as_os_str().is_empty() {
                    node.package
                        .as_ref()
                        .and_then(|package| package.package_json.main.as_deref())
                        .map_or_else(
                            || package_root.join("index.js"),
                            |main| package_root.join(main),
                        )
                } else {
                    package_root.join(suffix)
                };
                mock_config_key_to_module_info.insert(
                    key,
                    json!({
                      "meta": {
                        "pkgPath": self.path(package_root)?,
                        "pkgName": node.package_name,
                      },
                      "filePath": self.path(&file_path)?,
                    }),
                );
            }
        }

        Ok(json!({
          "decorator": "@MockSetup",
          "packageName": "@ohos/hamock",
          "etsSourceRootPath": module_layout.ets_source_root_artifact_path()?,
          "mockConfigPath": mock_config_path.is_file().then(|| self.path(&mock_config_path)).transpose()?,
          "mockConfigKey2ModuleInfo": mock_config_key_to_module_info,
          "source2ModuleIdMap": source_to_module_id,
        }))
    }

    fn project_config(&self) -> Result<CompileProjectConfig> {
        let module_package = self
            .input
            .module_context
            .module
            .package
            .as_ref()
            .ok_or_else(|| {
                anyhow::anyhow!(
          "ARKDOWN_ETS_CHECKER_PACKAGE_METADATA: module '{}' requires oh-package.json5 metadata",
          self.input.module.name
        )
            })?;
        let component_metadata = self
            .input
            .session
            .sdk
            .compile_plugin()
            .component_metadata()
            .ok_or_else(|| {
                anyhow::anyhow!(
                    "ARKDOWN_ETS_CHECKER_SDK_METADATA: cannot read ETS component version from {}",
                    self.input.config.sdk_config.ets_loader_path.display()
                )
            })?;
        let compatible_sdk_version =
            parse_api_version(&self.input.config.product.compatible_sdk_version)
                .unwrap_or(self.input.config.sdk_config.compile_api_version);
        let module_type = self.input.module.module_type.to_string();
        let package_manager_type = self
            .input
            .loader_json
            .map(|loader| loader.package_manager_type.clone())
            .unwrap_or_else(|| "ohpm".to_string());
        let build_mode = match self.input.config.build_mode {
            BuildMode::Debug => "Debug",
            BuildMode::Release => "Release",
            BuildMode::Test => "Test",
        }
        .to_string();
        let project_path = self.input.paths.ace_module_root.clone();
        let module_root = self.input.module_context.module.root.clone();
        let resource_table_hash = self.input.session.fingerprints.resources.to_hex();
        let ark_options = self.input.module_build_option.ark_options.as_ref();
        let is_bytecode_har = matches!(
            self.input.module_build_option.har_artifact_type,
            arkdown_core::HarArtifactType::Bytecode
        );
        let mut extras = BTreeMap::new();
        extras.insert("ohPackagePathMap".to_string(), self.oh_package_path_map()?);
        extras.insert(
            "dependencyAliasMap".to_string(),
            self.dependency_alias_map(),
        );
        let permission = self
            .input
            .module_context
            .module
            .module_json
            .as_ref()
            .map(|module_json| {
                json!({
                  "requestPermissions": module_json.module.request_permissions,
                  "definePermissions": module_json.module.define_permissions,
                })
            })
            .unwrap_or_else(|| {
                json!({
                  "requestPermissions": [],
                  "definePermissions": [],
                })
            });
        extras.insert("permission".to_string(), permission);
        extras.insert("useGetCurrentBundleName".to_string(), Value::Bool(false));
        // Standalone checking does not call `main.js::initProjectConfig()`, while
        // the CompileArkTS bridge does. Keep the effective flag in the shared
        // project model so both official entry points parse SDK annotations the
        // same way. Upstream: ets-loader/main.js::initProjectConfig().
        extras.insert("allowEtsAnnotations".to_string(), Value::Bool(true));
        extras.insert("projectArkOption".to_string(), Value::Object(Map::new()));
        extras.insert("sourceMapDir".to_string(), Value::String(String::new()));
        extras.insert(
            "branchElimination".to_string(),
            Value::Bool(
                self.input.config.build_mode.is_release()
                    && ark_options
                        .and_then(|options| options.branch_elimination)
                        .unwrap_or(false),
            ),
        );
        extras.insert("tsImportSendable".to_string(), Value::Bool(false));
        extras.insert("resolveConflictMode".to_string(), Value::Bool(true));
        extras.insert("depName2RootPath".to_string(), self.dependency_root_map()?);
        // SDK `import_path_expand.js` calls `depName2DepInfo.has(pkgName)` and
        // reads `entry.pkgRootPath`; an empty object makes `{}.has` throw.
        extras.insert(
            "depName2DepInfo".to_string(),
            self.dependency_name2_dep_info(),
        );
        extras.insert("depName2OhExports".to_string(), Value::Object(Map::new()));
        extras.insert(
            "rootPathSet".to_string(),
            Value::Array(vec![Value::String(
                self.path(&self.input.config.project_root)?,
            )]),
        );
        extras.insert("useNativeResolver".to_string(), Value::Bool(true));
        extras.insert("shouldEmitJs".to_string(), Value::Bool(true));
        extras.insert("useTscResolve".to_string(), Value::Bool(false));
        extras.insert(
            "singleFileEmit".to_string(),
            Value::Bool(self.input.single_file_emit),
        );
        extras.insert("userProjectConfig".to_string(), self.user_project_config());
        extras.insert(
            "executionMode".to_string(),
            Value::String("memory".to_string()),
        );
        extras.insert(
            "arkCompileCachePath".to_string(),
            Value::String(
                self.path(
                    self.input
                        .paths
                        .cache_path
                        .parent()
                        .unwrap_or(&self.input.paths.cache_path),
                )?,
            ),
        );
        extras.insert(
            "isHarDeduplicateEnabled".to_string(),
            Value::Bool(self.input.module_build_option.deduplicate_har),
        );
        extras.insert(
            "autoLazyImport".to_string(),
            Value::Bool(
                ark_options
                    .and_then(|options| options.auto_lazy_import)
                    .unwrap_or(false),
            ),
        );
        extras.insert("allowEmptyBundleName".to_string(), Value::Bool(false));
        extras.insert(
            "reExportCheckMode".to_string(),
            Value::String(
                ark_options
                    .and_then(|options| options.re_export_check_mode.clone())
                    .unwrap_or_else(|| "noCheck".to_string()),
            ),
        );
        extras.insert("usePathPlaceholder".to_string(), Value::Bool(false));
        extras.insert("writeRollupCache".to_string(), Value::Bool(true));
        let expand_import_path =
            ark_options.and_then(|options| options.expand_import_path.as_ref());
        extras.insert(
            "isExpandImportEnabled".to_string(),
            Value::Bool(
                expand_import_path
                    .map(|expand| expand.enable)
                    .unwrap_or(false),
            ),
        );
        extras.insert(
            "expandImportExclude".to_string(),
            Value::String(
                expand_import_path
                    .map(|expand| expand.exclude.join(","))
                    .unwrap_or_default(),
            ),
        );
        let auto_lazy_filter = ark_options.and_then(|options| options.auto_lazy_filter.as_ref());
        extras.insert(
            "lazyImportExclude".to_string(),
            Value::String(
                auto_lazy_filter
                    .map(|filter| filter.exclude.join(","))
                    .unwrap_or_default(),
            ),
        );
        extras.insert(
            "lazyImportInclude".to_string(),
            Value::String(
                auto_lazy_filter
                    .map(|filter| filter.include.join(","))
                    .unwrap_or_default(),
            ),
        );
        extras.insert(
            "ignoreCrossplatformCheck".to_string(),
            Value::Bool(self.input.ignore_crossplatform_check),
        );
        extras.insert(
            "skipOhModulesLint".to_string(),
            Value::Bool(
                ark_options
                    .and_then(|options| options.skip_oh_modules_lint)
                    .unwrap_or(false),
            ),
        );
        extras.insert("enableStrictCheckOHModule".to_string(), Value::Bool(false));
        extras.insert(
            "disableSendableCheckRules".to_string(),
            Value::Array(Vec::new()),
        );
        extras.insert(
            "useDeclarationFileSignature".to_string(),
            Value::Bool(false),
        );
        extras.insert(
            "integratedHsp".to_string(),
            Value::Bool(
                ark_options
                    .and_then(|options| options.integrated_hsp)
                    .unwrap_or(false),
            ),
        );
        extras.insert(
            "isPreloadSoEnabled".to_string(),
            Value::Bool(self.input.module_build_option.preload_system_so),
        );
        extras.insert("strictMode".to_string(), self.strict_mode());
        extras.insert(
            "useNormalizedOHMUrl".to_string(),
            Value::Bool(
                self.input
                    .module_build_option
                    .strict_mode
                    .use_normalized_ohm_url,
            ),
        );
        extras.insert(
            "caseSensitiveCheck".to_string(),
            Value::Bool(
                self.input
                    .module_build_option
                    .strict_mode
                    .case_sensitive_check,
            ),
        );
        extras.insert("strictCheckerOnly".to_string(), Value::Bool(false));
        extras.insert("ignoreWarning".to_string(), Value::Bool(false));
        extras.insert("mixCompile".to_string(), Value::Bool(false));
        extras.insert(
            "aceModuleJsonPath".to_string(),
            Value::String(self.path(&self.input.paths.ace_module_json_path)?),
        );
        extras.insert(
            "appResource".to_string(),
            Value::String(self.path(&self.input.paths.app_resource_path)?),
        );
        extras.insert(
            "rawFileResource".to_string(),
            Value::String(self.path(&self.input.paths.raw_file_resource_path)?),
        );
        extras.insert(
            "resourceTableHash".to_string(),
            Value::String(resource_table_hash),
        );
        extras.insert(
            "runtimeOS".to_string(),
            Value::String(self.input.config.product.runtime_os.clone()),
        );
        extras.insert(
            "sdkInfo".to_string(),
            Value::String(format!(
                "{}:{}:{}:{}",
                !arkdown_core::is_harmony_os(&self.input.config.product.runtime_os),
                self.input.config.sdk_config.compile_api_version,
                component_metadata.version,
                component_metadata.release_type
            )),
        );
        extras.insert(
            "aceModuleRoot".to_string(),
            Value::String(self.path(&self.input.paths.ace_module_root)?),
        );
        extras.insert(
            "projectPath".to_string(),
            Value::String(self.path(&project_path)?),
        );
        extras.insert(
            "compileMode".to_string(),
            Value::String(self.compile_mode()),
        );
        extras.insert(
            "aceSuperVisualPath".to_string(),
            Value::String(self.path(&self.input.paths.ace_super_visual_path)?),
        );
        extras.insert(
            "aceBuildJson".to_string(),
            Value::String(self.path(&self.input.paths.ace_build_json_path)?),
        );
        extras.insert(
            "cachePath".to_string(),
            Value::String(self.path(&self.input.paths.cache_path)?),
        );
        extras.insert(
            "aceModuleBuild".to_string(),
            Value::String(self.path(&self.input.paths.ace_module_build)?),
        );
        extras.insert(
            "supportChunks".to_string(),
            Value::Bool(
                self.input
                    .loader_json
                    .map(|loader| loader.support_chunks)
                    .unwrap_or_else(|| ark_options.is_some_and(|options| options.support_chunks)),
            ),
        );
        extras.insert(
            "shouldCollectDirectImportees".to_string(),
            Value::Bool(false),
        );
        extras.insert(
            "isByteCodeHarOptimize".to_string(),
            Value::Bool(self.input.byte_code_har_integrated_optimization),
        );
        extras.insert(
            "resourceTableTs".to_string(),
            Value::String(self.path(&self.input.paths.resource_table_typescript_path)?),
        );
        extras.insert(
            "moduleMetaInfoFiles".to_string(),
            self.module_meta_info_files()?,
        );
        extras.insert(
            "pkgNameToPkgBriefInfo".to_string(),
            self.package_brief_info()?,
        );
        extras.insert("projectModel".to_string(), self.project_model()?);
        extras.insert(
            "pkgJsonFileHash".to_string(),
            Value::String(self.input.session.fingerprints.package_graph.to_hex()),
        );
        extras.insert("allModulePaths".to_string(), self.all_module_paths()?);
        extras.insert("routerMap".to_string(), Value::Object(Map::new()));
        extras.insert("compileBlockPkg".to_string(), Value::Array(Vec::new()));
        extras.insert("mockParams".to_string(), self.mock_params()?);
        extras.insert("copyCodeResourceEnable".to_string(), Value::Bool(false));
        extras.insert(
            "copyCodeResourceExcludes".to_string(),
            Value::Array(Vec::new()),
        );
        extras.insert("uiTransformOptimization".to_string(), Value::Bool(false));
        extras.insert("sdkPath".to_string(), Value::String(self.sdk_root_path()?));
        extras.insert(
            "needPreloadSoFileSet".to_string(),
            Value::Object(Map::new()),
        );
        extras.insert(
            "preloadSoFilePath".to_string(),
            Value::String(self.path(&self.input.paths.workspace.join("preload.json"))?),
        );
        extras.insert(
            "useSourceHash".to_string(),
            Value::Bool(self.input.rollup_cache_use_source_hash),
        );
        extras.insert("localModuleParamMap".to_string(), Value::Object(Map::new()));
        extras.insert("otherPaths".to_string(), self.other_paths(&project_path)?);
        extras.insert(
            "globalModulePaths".to_string(),
            Value::Array(
                self.input
                    .session
                    .sdk
                    .global_module_paths
                    .iter()
                    .map(|path| self.path(path).map(Value::String))
                    .collect::<Result<Vec<_>>>()?,
            ),
        );
        extras.insert(
            "resolveModulePaths".to_string(),
            self.resolve_module_paths()?,
        );
        extras.insert(
            "compileHar".to_string(),
            Value::Bool(matches!(self.input.module.module_type, OhosModuleType::Har)),
        );
        extras.insert(
            "compileShared".to_string(),
            Value::Bool(matches!(
                self.input.module.module_type,
                OhosModuleType::Shared
            )),
        );
        extras.insert("byteCodeHar".to_string(), Value::Bool(is_bytecode_har));
        extras.insert("useTsHar".to_string(), Value::Bool(false));
        extras.insert("entryObj".to_string(), self.entry_obj()?);

        Ok(CompileProjectConfig {
            module_type,
            perf: 0,
            target_name: format!(".{0}", self.input.target_name),
            package_manager_type,
            local_properties_path: self
                .path(&self.input.config.project_root.join("local.properties"))?,
            is_preview: self.input.preview_mode,
            check_entry: self.input.check_entry,
            is_ohos_test: self.input.target_name == "ohosTest",
            is_local_test: matches!(
                self.input.config.output_target,
                arkdown_core::OutputTarget::Test
            ),
            build_mode,
            watch_mode: self.input.watch_mode.to_string(),
            ace_profile_path: self.path(&self.input.paths.ace_profile_path)?,
            ets_loader_path: self.path(&self.input.config.sdk_config.ets_loader_path)?,
            module_path: self.path(&module_root)?,
            test_framework_par: json!({
              "testMode": if matches!(self.input.config.output_target, arkdown_core::OutputTarget::Test) {
                "unitTest"
              } else if self.input.coverage_enabled || self.input.target_name == "ohosTest" {
                "ohosTest"
              } else {
                ""
              },
              "coveragePathFilter": self.input.coverage_path_filter,
              "coverageMode": self.input.coverage_mode,
            }),
            need_coverage_insert: self.input.coverage_enabled,
            debug_line: self.input.debug_line,
            project_top_dir: self.path(&self.input.config.project_root)?,
            compile_sdk_version: self.input.config.sdk_config.compile_api_version,
            compatible_sdk_version,
            origin_compatible_sdk_version: self.input.config.product.compatible_sdk_version.clone(),
            compatible_sdk_version_stage: self
                .input
                .config
                .product
                .compatible_sdk_version_stage
                .clone(),
            bundle_name: self.input.config.product.bundle_name.clone(),
            ets_loader_version: component_metadata.version.clone(),
            ets_loader_release_type: component_metadata.release_type.clone(),
            aot_compile_mode: if self.input.coverage_enabled {
                String::new()
            } else {
                self.input
                    .module_build_option
                    .aot_compile_mode
                    .clone()
                    .unwrap_or_else(|| "type".to_string())
            },
            ap_path: self
                .input
                .module_build_option
                .ap_path
                .as_ref()
                .map(|path| self.path(path))
                .transpose()?
                .unwrap_or_default(),
            entry_module_name: self.input.module.name.clone(),
            entry_module_version: module_package.version.clone(),
            entry_package_name: if self.input.target_name == "ohosTest"
                && self.input.coverage_enabled
            {
                format!("{}_test", module_package.name)
            } else {
                module_package.name.clone()
            },
            all_module_name_hash: self.all_module_name_hash()?,
            external_api_paths: self.external_api_paths()?,
            compiler_types: self
                .input
                .loader_json
                .map(|loader| loader.compiler_types.clone())
                .unwrap_or_else(|| {
                    ark_options
                        .map(|options| options.types.clone())
                        .unwrap_or_default()
                }),
            is_crossplatform: self.input.is_crossplatform,
            build_generated_profile_path: self.path(&self.input.paths.generated_profile_path)?,
            bundle_type: self.input.session.project.bundle_type.clone(),
            api_version: self.input.config.sdk_config.compile_api_version,
            need_complete_sources_map: false,
            is_fa_mode: false,
            build_dir: self.path(&module_root.join("build"))?,
            device_types: self.device_types(),
            extras,
        })
    }

    fn compile_mode(&self) -> String {
        self.input
            .loader_json
            .map(|loader| loader.compile_mode.clone())
            .unwrap_or_else(|| "esmodule".to_string())
    }

    fn device_types(&self) -> Vec<String> {
        let types = self
            .input
            .module_context
            .module
            .module_json
            .as_ref()
            .map(|module| module.module.device_types.clone())
            .unwrap_or_default();
        if types.is_empty() {
            vec!["phone".to_string()]
        } else {
            types
        }
    }

    fn user_project_config(&self) -> Value {
        let ark_options = self.input.module_build_option.ark_options.as_ref();
        let expand_import_path =
            ark_options.and_then(|options| options.expand_import_path.as_ref());
        let auto_lazy_filter = ark_options.and_then(|options| options.auto_lazy_filter.as_ref());
        json!({
          "autoLazyImport": ark_options.and_then(|options| options.auto_lazy_import).unwrap_or(false),
          "allowEmptyBundleName": false,
          "reExportCheckMode": ark_options
            .and_then(|options| options.re_export_check_mode.clone())
            .unwrap_or_else(|| "noCheck".to_string()),
          "executionMode": "memory",
          "usePathPlaceholder": false,
          "writeRollupCache": true,
          "isExpandImportEnabled": expand_import_path.map(|expand| expand.enable).unwrap_or(false),
          "expandImportExclude": expand_import_path
            .map(|expand| expand.exclude.join(","))
            .unwrap_or_default(),
          "lazyImportExclude": auto_lazy_filter
            .map(|filter| filter.exclude.join(","))
            .unwrap_or_default(),
          "lazyImportInclude": auto_lazy_filter
            .map(|filter| filter.include.join(","))
            .unwrap_or_default(),
          "ignoreCrossplatformCheck": false,
          "isHarDeduplicateEnabled": self.input.module_build_option.deduplicate_har,
          "skipOhModulesLint": ark_options
            .and_then(|options| options.skip_oh_modules_lint)
            .unwrap_or(false),
          "enableStrictCheckOHModule": false,
          "disableSendableCheckRules": [],
          "useDeclarationFileSignature": false,
          "integratedHsp": ark_options.and_then(|options| options.integrated_hsp).unwrap_or(false),
          "isPreloadSoEnabled": self.input.module_build_option.preload_system_so,
        })
    }

    fn strict_mode(&self) -> Value {
        json!({
          "caseSensitiveCheck": self.input.module_build_option.strict_mode.case_sensitive_check,
          "useNormalizedOHMUrl": self.input.module_build_option.strict_mode.use_normalized_ohm_url,
        })
    }

    fn oh_package_path_map(&self) -> Result<Value> {
        let mut output = Map::new();
        for node in self.input.module_context.deps.nodes.values() {
            let Some(root) = node.package_root.as_ref() else {
                continue;
            };
            output.insert(node.package_name.clone(), Value::String(self.path(root)?));
            if node.alias != node.package_name {
                output.insert(node.alias.clone(), Value::String(self.path(root)?));
            }
        }
        Ok(Value::Object(output))
    }

    /// `depName2DepInfo`: `{ [pkgName]: { pkgRootPath } }` consumed by the SDK
    /// `import_path_expand.js` (`.has(pkgName)` + `entry.pkgRootPath`).
    fn dependency_name2_dep_info(&self) -> Value {
        let mut output = Map::new();
        for node in self.input.module_context.deps.nodes.values() {
            let Some(root) = node.package_root.as_ref() else {
                continue;
            };
            let path = self.path(root).unwrap_or_default();
            output.insert(
                node.package_name.clone(),
                Value::Object(Map::from_iter([(
                    "pkgRootPath".to_string(),
                    Value::String(path),
                )])),
            );
            if node.alias != node.package_name {
                output.insert(
                    node.alias.clone(),
                    Value::Object(Map::from_iter([(
                        "pkgRootPath".to_string(),
                        Value::String(self.path(root).unwrap_or_default()),
                    )])),
                );
            }
        }
        Value::Object(output)
    }

    fn dependency_alias_map(&self) -> Value {
        let mut output = Map::new();
        for node in self.input.module_context.deps.nodes.values() {
            if node.alias != node.package_name {
                output.insert(node.alias.clone(), Value::String(node.package_name.clone()));
            }
        }
        Value::Object(output)
    }

    fn dependency_root_map(&self) -> Result<Value> {
        let mut output = Map::new();
        for node in self.input.module_context.deps.nodes.values() {
            let Some(root) = node.package_root.as_ref() else {
                continue;
            };
            let path = Value::String(self.path(root)?);
            output.insert(node.package_name.clone(), path.clone());
            if node.alias != node.package_name {
                output.insert(node.alias.clone(), path);
            }
        }
        Ok(Value::Object(output))
    }

    fn package_brief_info(&self) -> Result<Value> {
        let mut output = Map::new();
        for (specifier, package) in self.input.package_resolution.sorted_entries() {
            let source_roots = package
                .source_roots
                .iter()
                .map(|source_root| {
                    self.relative_path(&package.pkg_root, source_root)
                        .map(|path| ArtifactPathSerializer::rel_to_slash(&path))
                })
                .collect::<Result<Vec<_>>>()?;
            output.insert(
                specifier.to_string(),
                json!({
                  "pkgRoot": self.path(&package.pkg_root)?,
                  "sourceRoots": source_roots,
                  "pkgName": package.pkg_name,
                }),
            );
        }
        Ok(Value::Object(output))
    }

    fn project_model(&self) -> Result<Value> {
        let mut output = Map::new();
        for module in &self.input.config.modules {
            let module_root = self.path(&module.src_path)?;
            let source_main = self.path(&module.src_path.join("src").join("main"))?;
            let details = json!({
              "moduleName": module.name,
              "modulePkgPath": module_root,
              "belongProjectPath": self.path(&self.input.config.project_root)?,
            });
            output.insert(source_main, details.clone());
            output.insert(module_root, details);
        }
        let project_path = self.path(&self.input.config.project_root)?;
        let project_name = self
            .input
            .config
            .project_root
            .file_name()
            .and_then(|name| name.to_str())
            .ok_or_else(|| {
                anyhow::anyhow!(
          "ARKDOWN_ETS_CHECKER_PROJECT_NAME: project root has no UTF-8 final component: {}",
          self.input.config.project_root.display()
        )
            })?;
        output.insert(
            project_path.clone(),
            json!({
              "moduleName": project_name,
              "modulePkgPath": project_path,
              "belongProjectPath": self.path(&self.input.config.project_root)?,
            }),
        );
        Ok(Value::Object(output))
    }

    fn module_meta_info_files(&self) -> Result<Value> {
        let file_path = self.path(&self.input.paths.resource_table_typescript_path)?;
        let ohmurl = if self
            .input
            .module_build_option
            .strict_mode
            .use_normalized_ohm_url
        {
            format!(
                "&{}/build/generated/r/ResourceTable&",
                self.input.module.name
            )
        } else {
            format!(
                "{}/{}/build/generated/r/ResourceTable",
                self.input.config.product.bundle_name, self.input.module.name
            )
        };
        Ok(Value::Array(vec![
            json!({ "filePath": file_path, "ohmurl": ohmurl }),
        ]))
    }

    fn all_module_paths(&self) -> Result<Value> {
        Ok(Value::Array(
            self.input
                .config
                .modules
                .iter()
                .map(|module| self.path(&module.src_path).map(Value::String))
                .collect::<Result<Vec<_>>>()?,
        ))
    }

    fn other_paths(&self, project_path: &Path) -> Result<Value> {
        let mut output = Map::new();
        for (specifier, package) in self.input.package_resolution.sorted_entries() {
            if package.source_roots.is_empty() {
                continue;
            }
            let values = package
                .source_roots
                .iter()
                .map(|source_root| {
                    self.relative_glob(project_path, source_root)
                        .map(Value::String)
                })
                .collect::<Result<Vec<_>>>()?;
            output.insert(format!("{specifier}/*"), Value::Array(values));
        }
        Ok(Value::Object(output))
    }

    fn resolve_module_paths(&self) -> Result<Value> {
        let module_root = &self.input.module_context.module.root;
        Ok(Value::Array(
            [
                self.input.module_context.module.src_main.clone(),
                self.input.paths.generated_profile_path.clone(),
                module_root.join("oh_modules"),
                self.input.config.project_root.join("oh_modules"),
            ]
            .iter()
            .map(|path| self.path(path).map(Value::String))
            .collect::<Result<Vec<_>>>()?,
        ))
    }

    fn entry_obj(&self) -> Result<Value> {
        let mut output = Map::new();
        for (index, entry) in self.input.entries.iter().enumerate() {
            output.insert(
                format!("entry_{index:05}"),
                Value::String(self.path(entry)?),
            );
        }
        Ok(Value::Object(output))
    }

    fn external_api_paths(&self) -> Result<Vec<String>> {
        self.input
            .session
            .sdk
            .external_api_paths
            .iter()
            .map(|path| self.path(path))
            .collect()
    }

    fn sdk_root_path(&self) -> Result<String> {
        let sdk_root = self
            .input
            .session
            .sdk
            .sdk_root
            .parent()
            .and_then(Path::parent)
            .ok_or_else(|| {
                anyhow::anyhow!(
                    "ARKDOWN_ETS_CHECKER_SDK_ROOT: cannot derive SDK root from {}",
                    self.input.session.sdk.sdk_root.display()
                )
            })?;
        self.path(sdk_root)
    }

    fn all_module_name_hash(&self) -> Result<String> {
        let mut names = String::new();
        for module in &self.input.config.modules {
            let module_name = self
                .input
                .session
                .project
                .modules
                .get(&module.name)
                .and_then(|snapshot| snapshot.module_json.as_ref())
                .map(|module_json| module_json.module.name.as_str())
                .filter(|name| !name.is_empty())
                .unwrap_or(module.name.as_str());
            names.push_str(module_name);
        }
        let project_name = self
            .input
            .config
            .project_root
            .file_name()
            .and_then(|name| name.to_str())
            .ok_or_else(|| {
                anyhow::anyhow!(
          "ARKDOWN_ETS_CHECKER_PROJECT_NAME: project root has no UTF-8 final component: {}",
          self.input.config.project_root.display()
        )
            })?;
        names.push_str(project_name);
        Ok(format!("{:x}", Sha256::digest(names.as_bytes())))
    }

    fn relative_glob(&self, from: &Path, to: &Path) -> Result<String> {
        let relative = self.relative_path(from, to)?;
        let mut text = ArtifactPathSerializer::rel_to_slash(&relative);
        if text.is_empty() || text == "." {
            text = ".".to_string();
        }
        if !text.ends_with('/') {
            text.push('/');
        }
        text.push('*');
        Ok(text)
    }

    fn relative_path(&self, from: &Path, to: &Path) -> Result<PathBuf> {
        let from = lexical_normalize(from);
        let to = lexical_normalize(to);
        let from_components = from.components().collect::<Vec<_>>();
        let to_components = to.components().collect::<Vec<_>>();
        let mut common = 0usize;
        while common < from_components.len()
            && common < to_components.len()
            && from_components[common] == to_components[common]
        {
            common += 1;
        }
        let incompatible_roots = from_components
            .first()
            .zip(to_components.first())
            .is_some_and(|(left, right)| {
                matches!(left, Component::Prefix(_) | Component::RootDir)
                    && matches!(right, Component::Prefix(_) | Component::RootDir)
                    && left != right
            });
        if incompatible_roots {
            anyhow::bail!(
        "ARKDOWN_ETS_CHECKER_CROSS_VOLUME: cannot create TypeScript path mapping from {} to {}",
        from.display(),
        to.display()
      );
        }
        let mut output = PathBuf::new();
        for component in &from_components[common..] {
            if matches!(component, Component::Normal(_)) {
                output.push("..");
            }
        }
        for component in &to_components[common..] {
            if matches!(component, Component::Normal(_)) {
                output.push(component.as_os_str());
            }
        }
        if output.as_os_str().is_empty() {
            output.push(".");
        }
        Ok(output)
    }

    fn path(&self, path: &Path) -> Result<String> {
        ArtifactPathSerializer::path_to_slash_string(path)
            .map_err(|error| anyhow::anyhow!("ARKDOWN_ETS_CHECKER_PATH: {error}"))
    }
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct CompileProjectConfig {
    module_type: String,
    perf: u8,
    target_name: String,
    package_manager_type: String,
    local_properties_path: String,
    is_preview: bool,
    check_entry: bool,
    is_ohos_test: bool,
    is_local_test: bool,
    build_mode: String,
    watch_mode: String,
    ace_profile_path: String,
    ets_loader_path: String,
    module_path: String,
    test_framework_par: Value,
    need_coverage_insert: bool,
    debug_line: bool,
    project_top_dir: String,
    compile_sdk_version: u32,
    compatible_sdk_version: u32,
    origin_compatible_sdk_version: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    compatible_sdk_version_stage: Option<String>,
    bundle_name: String,
    ets_loader_version: String,
    ets_loader_release_type: String,
    aot_compile_mode: String,
    ap_path: String,
    entry_module_name: String,
    entry_module_version: String,
    entry_package_name: String,
    all_module_name_hash: String,
    external_api_paths: Vec<String>,
    compiler_types: Vec<String>,
    is_crossplatform: bool,
    build_generated_profile_path: String,
    bundle_type: String,
    api_version: u32,
    need_complete_sources_map: bool,
    is_fa_mode: bool,
    build_dir: String,
    device_types: Vec<String>,
    #[serde(flatten)]
    extras: BTreeMap<String, Value>,
}
