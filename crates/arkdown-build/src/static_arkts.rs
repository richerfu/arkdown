use std::collections::BTreeMap;
use std::path::{Path, PathBuf};
use std::process::Command;

use anyhow::{Context, Result};
use indexmap::IndexMap;
use rustc_hash::FxHashSet;
use serde::Serialize;

use arkdown_config::build_profile::parse_api_version;
use arkdown_core::{
    ArkdownConfig, BuildMode, HapManifest, HvigorModuleSourceFile, ModuleConfig,
    StaticArktsSdkLayout,
};
use arkdown_path::{lexical_normalize, ArtifactPathSerializer};
use arkdown_profile::LoaderJson;
use arkdown_session::dependency_graph::snapshot::{DependencyNodeSnapshot, DependencyNodeType};
use arkdown_session::{ModuleCompileContext, SourceStore};

use crate::layout::ModuleBuildLayout;

pub struct StaticArktsCompileInput<'a> {
    pub config: &'a ArkdownConfig,
    pub module: &'a ModuleConfig,
    pub manifest: &'a HapManifest,
    pub loader_json: &'a LoaderJson,
    pub module_context: &'a ModuleCompileContext,
    pub module_layout: &'a ModuleBuildLayout<'a>,
    pub loader_json_path: &'a Path,
    pub module_source_files: &'a [HvigorModuleSourceFile],
    pub node_executable: PathBuf,
    pub output_abc: &'a Path,
    pub preview_mode: bool,
    pub check_entry: bool,
    pub is_crossplatform: bool,
    pub ui_transform_optimization: bool,
    pub bundle_type: &'a str,
}

/// ArkTS 1.2 SDK frontend adapter.
///
/// OXC and Rolldown own parsing and graph construction. The SDK driver is
/// the source-defined host for the parsed/checked ArkUI plugins and the native
/// es2panda state machine.
pub struct StaticArktsCompiler<'a> {
    input: StaticArktsCompileInput<'a>,
    sdk: StaticArktsSdkLayout,
}

impl<'a> StaticArktsCompiler<'a> {
    pub fn new(input: StaticArktsCompileInput<'a>) -> Result<Self> {
        let sdk = input
            .config
            .sdk_config
            .static_arkts_layout()
            .ok_or_else(|| {
                anyhow::anyhow!(
                    "ARKDOWN_ARKTS12_SDK_LAYOUT: cannot derive ets/static from {}",
                    input.config.sdk_config.ets_loader_path.display()
                )
            })?;
        let compiler = Self { input, sdk };
        compiler.validate_sdk()?;
        Ok(compiler)
    }

    pub fn compile(&self) -> Result<PathBuf> {
        let build_config = StaticArktsDriverConfigBuilder::new(&self.input, &self.sdk).build()?;
        let config_path = PathBuf::from(&self.input.loader_json.cache_path)
            .join("arkts12")
            .join("build_config.json");
        if let Some(parent) = config_path.parent() {
            fs_err::create_dir_all(parent).with_context(|| {
                format!("create ArkTS 1.2 config directory {}", parent.display())
            })?;
        }
        fs_err::write(&config_path, serde_json::to_vec_pretty(&build_config)?)
            .with_context(|| format!("write ArkTS 1.2 config {}", config_path.display()))?;
        let output_abc = self.output_abc()?;
        let output_dir = output_abc
            .parent()
            .expect("the static ABC path is always below loaderOutPath");
        fs_err::create_dir_all(output_dir)
            .with_context(|| format!("create ArkTS 1.2 output {}", output_dir.display()))?;

        let output = self.command(&config_path).output().with_context(|| {
            format!(
                "launch ArkTS 1.2 SDK driver {}",
                self.sdk.driver_entry.display()
            )
        })?;
        if !output.status.success() {
            anyhow::bail!(
                "ARKDOWN_ARKTS12_COMPILE_FAILED: driver {} exited with {}; stdout: {}; stderr: {}",
                self.sdk.driver_entry.display(),
                output.status,
                String::from_utf8_lossy(&output.stdout).trim(),
                String::from_utf8_lossy(&output.stderr).trim()
            );
        }
        if !output_abc.is_file() {
            anyhow::bail!(
                "ARKDOWN_ARKTS12_OUTPUT_MISSING: static compiler completed without writing {}",
                output_abc.display()
            );
        }
        Ok(output_abc)
    }

    /// `driver/build_system/src/pre_define.ts::MERGED_ABC_FILE` and
    /// `BaseMode::mergedAbcFile` fix this name below `loaderOutPath`.
    fn output_abc(&self) -> Result<PathBuf> {
        let output_dir = self
            .input
            .output_abc
            .parent()
            .ok_or_else(|| anyhow::anyhow!("ArkTS 1.2 ABC output has no parent directory"))?;
        Ok(output_dir.join("modules_static.abc"))
    }

    fn command(&self, config_path: &Path) -> Command {
        let mut command = Command::new(&self.input.node_executable);
        command.arg(&self.sdk.driver_entry).arg(config_path);
        self.prepend_native_library_path(&mut command);
        command
    }

    fn prepend_native_library_path(&self, command: &mut Command) {
        let variable = if cfg!(target_os = "macos") {
            "DYLD_LIBRARY_PATH"
        } else if cfg!(windows) {
            "PATH"
        } else {
            "LD_LIBRARY_PATH"
        };
        let mut paths = vec![self.sdk.panda_sdk.join("lib")];
        if let Some(existing) = std::env::var_os(variable) {
            paths.extend(std::env::split_paths(&existing));
        }
        if let Ok(joined) = std::env::join_paths(paths) {
            command.env(variable, joined);
        }
    }

    fn validate_sdk(&self) -> Result<()> {
        for (name, path) in [
            ("driver", &self.sdk.driver_entry),
            ("es2panda", &self.sdk.es2panda),
            ("ark_link", &self.sdk.abc_linker),
            ("dependency_analyzer", &self.sdk.dependency_analyzer),
        ] {
            if !path.is_file() {
                anyhow::bail!(
                    "ARKDOWN_ARKTS12_SDK_TOOL_MISSING: {name} is not available at {}",
                    path.display()
                );
            }
        }
        for (name, path) in [
            ("api", &self.sdk.api),
            ("kits", &self.sdk.kits),
            ("arkts", &self.sdk.arkts),
        ] {
            if !path.is_dir() {
                anyhow::bail!(
          "ARKDOWN_ARKTS12_SDK_DECLARATIONS_MISSING: {name} declarations are not available at {}",
          path.display()
        );
            }
        }
        for (name, path) in [
            ("ui_syntax_plugin", &self.sdk.ui_syntax_plugin),
            ("ui_plugin", &self.sdk.ui_plugin),
            ("memo_plugin", &self.sdk.memo_plugin),
        ] {
            if !path.is_file() && !path.with_extension("js").is_file() {
                anyhow::bail!(
                    "ARKDOWN_ARKTS12_PLUGIN_MISSING: {name} is not available at {}[.js]",
                    path.display()
                );
            }
        }
        Ok(())
    }
}

struct StaticArktsDriverConfigBuilder<'a> {
    input: &'a StaticArktsCompileInput<'a>,
    sdk: &'a StaticArktsSdkLayout,
}

impl<'a> StaticArktsDriverConfigBuilder<'a> {
    fn new(input: &'a StaticArktsCompileInput<'a>, sdk: &'a StaticArktsSdkLayout) -> Self {
        Self { input, sdk }
    }

    fn build(&self) -> Result<StaticArktsDriverConfig> {
        let mut dependent_module_list = self.dependent_modules()?;
        if let Some(main_module) = self.main_module()? {
            dependent_module_list.insert(0, main_module);
        }
        let compile_files = self.compile_files(&dependent_module_list)?;
        let module_files = self.module_files(&dependent_module_list)?;
        let source_roots = if self.input.module_context.module.source_roots.is_empty() {
            vec![self.path(&self.input.module_context.module.src_main)?]
        } else {
            self.input
                .module_context
                .module
                .source_roots
                .iter()
                .map(|path| self.path(path))
                .collect::<Result<Vec<_>>>()?
        };
        // `bindings/src/lsp/generateBuildConfig.ts::DEFAULT_PLUGIN_LIST`
        // defines the normal-build set/order, and
        // `PluginDriver::initPlugins` preserves object insertion order.
        // Interop is owned by the separate declgen configuration.
        let plugins = IndexMap::from([
            (
                "ui_syntax_plugin".to_string(),
                self.path(&self.sdk.ui_syntax_plugin)?,
            ),
            ("ui_plugin".to_string(), self.path(&self.sdk.ui_plugin)?),
            ("memo_plugin".to_string(), self.path(&self.sdk.memo_plugin)?),
        ]);
        let external_api_paths = [&self.sdk.api, &self.sdk.kits, &self.sdk.arkts]
            .into_iter()
            .map(|path| self.path(path))
            .collect::<Result<Vec<_>>>()?;
        let module_root = self.path(&self.input.module.src_path)?;
        let project_path = self.input.module_layout.source().ets();
        let output_dir = self
            .input
            .output_abc
            .parent()
            .ok_or_else(|| anyhow::anyhow!("ArkTS 1.2 ABC output has no parent directory"))?;
        let dependent_module_list_copy = dependent_module_list.clone();

        Ok(StaticArktsDriverConfig {
            plugins,
            paths: BTreeMap::new(),
            entry_file: self.main_entry_file()?,
            compile_files,
            module_files,
            build_mode: StaticArktsBuildMode::new(&self.input.config.build_mode).to_string(),
            project_root_path: self.path(&self.input.config.project_root)?,
            module_root_path: module_root,
            ace_profile_path: self.path(&self.input.module_layout.res_profile_dir())?,
            cache_path: self.path(Path::new(&self.input.loader_json.cache_path))?,
            loader_out_path: self.path(output_dir)?,
            compile_sdk_version: self.input.config.sdk_config.compile_api_version,
            compatible_sdk_version: parse_api_version(
                &self.input.config.product.compatible_sdk_version,
            )
            .unwrap_or(self.input.config.sdk_config.compile_api_version),
            ace_module_json_path: self.path(&self.input.module_layout.res_ark_module_json())?,
            bundle_name: self.input.config.product.bundle_name.clone(),
            use_normalized_ohm_url: self.input.loader_json.use_normalized_ohm_url,
            build_type: "build".to_string(),
            package_name: self.input.loader_json.module_name.clone(),
            build_sdk_path: self.path(&self.sdk.root)?,
            source_roots,
            module_type: self.input.module.module_type.to_string(),
            module_name: self.input.loader_json.module_name.clone(),
            dependent_module_list,
            dependency_module_list: dependent_module_list_copy,
            has_main_module: self
                .input
                .config
                .modules
                .iter()
                .any(|module| matches!(module.module_type, arkdown_core::OhosModuleType::Entry)),
            build_loader_json: self.path(self.input.loader_json_path)?,
            integrated_hsp: self
                .input
                .config
                .product
                .build_option
                .ark_options
                .as_ref()
                .and_then(|options| options.integrated_hsp)
                .unwrap_or(false),
            allow_empty_bundle_name: false,
            enable_declgen_ets2_ts: false,
            declgen_v2_out_path: String::new(),
            external_api_paths,
            interop_api_paths: Vec::new(),
            device_types: self.input.manifest.device_types.clone(),
            permissions: BTreeMap::new(),
            byte_code_har: self.input.loader_json.byte_code_har.unwrap_or(false),
            ui_transform_optimization: self.input.ui_transform_optimization,
            runtime_os: self.input.config.product.runtime_os.clone(),
            sdk_config_paths: Vec::new(),
            is_preview: self.input.preview_mode,
            check_entry: self.input.check_entry,
            code_root_path: self.path(&project_path)?,
            is_crossplatform: self.input.is_crossplatform,
            bundle_type: self.input.bundle_type.to_string(),
            level: StaticArktsLogLevel::default(),
            is_build_config_modified: false,
            panda_sdk_path: self.path(&self.sdk.panda_sdk)?,
            abc_linker_path: self.path(&self.sdk.abc_linker)?,
            dependency_analyzer_path: self.path(&self.sdk.dependency_analyzer)?,
            api_path: self.path(&self.sdk.api)?,
            kits_path: self.path(&self.sdk.kits)?,
            dep_analyzer_path: self.path(&self.sdk.dependency_analyzer)?,
            alias_config: BTreeMap::new(),
            sdk_alias_map: BTreeMap::new(),
            interop_sdk_paths: BTreeMap::new(),
            es2panda_mode: "simultaneous".to_string(),
            app_resource: self.path(&self.input.module_layout.resource_table_txt())?,
            raw_file_resource: self
                .path(&self.input.module_layout.res_resources_dir().join("rawfile"))?,
            project_path: self.path(&project_path)?,
            hsp_resources_map: !self.input.loader_json.hsp_resources_map.is_empty(),
            compile_har: matches!(
                self.input.module.module_type,
                arkdown_core::OhosModuleType::Har
            ),
            reset_bundle_name: self
                .input
                .config
                .product
                .build_option
                .ark_options
                .as_ref()
                .and_then(|options| options.integrated_hsp)
                .unwrap_or(false),
            ignore_error: false,
        })
    }

    fn compile_files(
        &self,
        dependent_modules: &[StaticArktsDependentModule],
    ) -> Result<Vec<String>> {
        // `bindings/src/lsp/generateBuildConfig.ts::getEtsFiles` contributes
        // every ArkTS file in the active module source inventory. Rolldown's
        // reachable graph is an additional source of generated/build-owned
        // entries, not a replacement for this complete input set.
        let inventory_roots = if self.input.module_context.module.source_roots.is_empty() {
            std::slice::from_ref(&self.input.module_context.module.src_main)
        } else {
            self.input.module_context.module.source_roots.as_slice()
        };
        let mut module_inventory = inventory_roots
            .iter()
            .flat_map(|root| {
                self.input
                    .module_context
                    .session
                    .source_store
                    .text_sources_under(root)
            })
            .filter(|(path, _)| {
                path.extension().and_then(|extension| extension.to_str()) == Some("ets")
            })
            .map(|(path, _)| path.to_path_buf())
            .collect::<Vec<_>>();
        module_inventory.sort();
        module_inventory.dedup();
        StaticArktsCompileEntryPlan::new(
            Path::new(&self.input.loader_json.module_root_path),
            &self.input.loader_json.hvigor_compile_entry,
            &self.input.loader_json.compile_entry,
            &module_inventory,
            self.input.module_source_files,
            dependent_modules,
        )
        .resolve()?
        .iter()
        .map(|path| self.path(path))
        .collect()
    }

    fn dependent_modules(&self) -> Result<Vec<StaticArktsDependentModule>> {
        let mut modules = Vec::new();
        let language_detector = StaticArktsLanguageDetector::new(
            self.input.module_context.session.source_store.as_ref(),
        );
        for node in self.input.module_context.deps.nodes.values() {
            if !matches!(
                node.node_type,
                DependencyNodeType::SourceHar
                    | DependencyNodeType::BytecodeHar
                    | DependencyNodeType::IntegratedHsp
                    | DependencyNodeType::SharedHsp
                    | DependencyNodeType::OhpmSource
            ) {
                continue;
            }
            let root = node.package_root.as_ref().ok_or_else(|| {
                anyhow::anyhow!(
                    "ARKDOWN_ARKTS12_DEPENDENCY_ROOT_MISSING: {}",
                    node.package_name
                )
            })?;
            let package = node.package.as_ref().ok_or_else(|| {
                anyhow::anyhow!(
                    "ARKDOWN_ARKTS12_DEPENDENCY_METADATA_MISSING: {}",
                    node.package_name
                )
            })?;
            let entry_file = package
                .package_json
                .main
                .as_deref()
                .map(PathBuf::from)
                .or_else(|| node.compile_inputs.dynamic_import_entry_file.clone())
                .or_else(|| node.compile_inputs.other_compile_entry_file.clone())
                .ok_or_else(|| {
                    anyhow::anyhow!(
                        "ARKDOWN_ARKTS12_DEPENDENCY_ENTRY_MISSING: package '{}' has no source-defined entry",
                        node.package_name
                    )
                })?;
            let absolute_entry_file = if entry_file.has_root() {
                lexical_normalize(&entry_file)
            } else {
                lexical_normalize(&root.join(&entry_file))
            };
            let source_root = absolute_entry_file.parent().unwrap_or(root);
            let bytecode = self
                .input
                .loader_json
                .byte_code_har_info
                .get(&node.package_name)
                .or_else(|| self.input.loader_json.byte_code_har_info.get(&node.alias));
            let language = language_detector.detect(node, bytecode.map(|entry| &entry.abc_path))?;
            let (dependencies, original_package_name_map) = self.dependency_edges(root);
            modules.push(StaticArktsDependentModule {
                package_name: node.package_name.clone(),
                module_name: node
                    .module_profile
                    .as_ref()
                    .and_then(|profile| profile.module_name.clone())
                    .unwrap_or_else(|| node.package_name.clone()),
                module_type: node
                    .module_profile
                    .as_ref()
                    .and_then(|profile| profile.module_type.as_ref())
                    .map(ToString::to_string)
                    .unwrap_or_else(|| match node.node_type {
                        DependencyNodeType::IntegratedHsp | DependencyNodeType::SharedHsp => {
                            "shared".to_string()
                        }
                        _ => "har".to_string(),
                    }),
                module_path: self.path(root)?,
                source_roots: vec![self.path(source_root)?],
                entry_file: if entry_file.has_root() {
                    self.path(&entry_file)?
                } else {
                    ArtifactPathSerializer::rel_to_slash(&entry_file)
                },
                language: language.clone(),
                // The current driver accepts this only from the separate
                // dynamic declgen output contract. Package source inventory
                // does not own that artifact, so it must not be guessed by
                // scanning for a matching filename.
                decl_files_path: None,
                dependencies,
                original_package_name_map,
                abc_path: bytecode
                    .map(|entry| self.path(Path::new(&entry.abc_path)))
                    .transpose()?,
                byte_code_har: matches!(node.node_type, DependencyNodeType::BytecodeHar),
                bundle_type: node
                    .module_profile
                    .as_ref()
                    .and_then(|profile| profile.bundle_type.clone()),
                bundle_name: node
                    .module_profile
                    .as_ref()
                    .and_then(|profile| profile.bundle_name.clone()),
                package_version: (!package.version.is_empty()).then(|| package.version.clone()),
                compile_sources: matches!(
                    node.node_type,
                    DependencyNodeType::SourceHar | DependencyNodeType::OhpmSource
                ) && matches!(language.as_str(), "1.2" | "hybrid"),
            });
        }
        modules.sort_by(|left, right| {
            left.package_name
                .cmp(&right.package_name)
                .then_with(|| left.module_name.cmp(&right.module_name))
                .then_with(|| left.module_path.cmp(&right.module_path))
        });
        Ok(modules)
    }

    /// `BaseMode::getMainModuleInfo` reads the current package's dependency
    /// aliases and `originalPackageNameMap` from the main package record when
    /// it is present in `dependencyModuleList`.
    fn main_module(&self) -> Result<Option<StaticArktsDependentModule>> {
        let (dependencies, original_package_name_map) =
            self.dependency_edges(&self.input.module_context.deps.host_root);
        if original_package_name_map.is_none() {
            return Ok(None);
        }
        let entry_file = self.main_entry_file()?;
        if entry_file.is_empty() {
            anyhow::bail!(
                "ARKDOWN_ARKTS12_MAIN_ENTRY_MISSING: aliased dependencies require the main package entryFile"
            );
        }
        let source_roots = if self.input.module_context.module.source_roots.is_empty() {
            vec![self.path(&self.input.module_context.module.src_main)?]
        } else {
            self.input
                .module_context
                .module
                .source_roots
                .iter()
                .map(|root| self.path(root))
                .collect::<Result<Vec<_>>>()?
        };
        let package = self.input.module_context.module.package.as_ref();
        Ok(Some(StaticArktsDependentModule {
            package_name: self.input.loader_json.module_name.clone(),
            module_name: self.input.loader_json.module_name.clone(),
            module_type: self.input.module.module_type.to_string(),
            module_path: self.path(&self.input.module.src_path)?,
            source_roots,
            entry_file,
            language: "1.2".to_string(),
            decl_files_path: None,
            dependencies,
            original_package_name_map,
            abc_path: None,
            byte_code_har: self.input.loader_json.byte_code_har.unwrap_or(false),
            bundle_type: Some(self.input.bundle_type.to_string()),
            bundle_name: Some(self.input.config.product.bundle_name.clone()),
            package_version: package
                .filter(|package| !package.version.is_empty())
                .map(|package| package.version.clone()),
            compile_sources: false,
        }))
    }

    fn main_entry_file(&self) -> Result<String> {
        self.input
            .module_context
            .module
            .package
            .as_ref()
            .and_then(|package| package.package_json.main.as_deref())
            .map(Path::new)
            .map(|path| {
                if path.has_root() {
                    self.path(path)
                } else {
                    Ok(ArtifactPathSerializer::rel_to_slash(path))
                }
            })
            .unwrap_or_else(|| Ok(String::new()))
    }

    /// `BaseMode::collectModuleInfos` resolves each declared dependency alias
    /// through the module-local `originalPackageNameMap` before looking it up
    /// in `dependencyModuleList`.
    fn dependency_edges(&self, root: &Path) -> (Vec<String>, Option<BTreeMap<String, String>>) {
        let mut dependencies = Vec::new();
        let mut seen = FxHashSet::default();
        let mut original_package_name_map = BTreeMap::new();
        for edge in &self.input.module_context.deps.edges {
            if edge.issuer_root != root {
                continue;
            }
            if seen.insert(edge.alias.clone()) {
                dependencies.push(edge.alias.clone());
            }
            if edge.alias != edge.package_name {
                original_package_name_map.insert(edge.alias.clone(), edge.package_name.clone());
            }
        }
        (
            dependencies,
            (!original_package_name_map.is_empty()).then_some(original_package_name_map),
        )
    }

    /// `BaseMode::collectModuleFiles` defines this inventory only for HSPs and
    /// packages reachable from an HSP. Each record contains that package's
    /// static ArkTS source paths for `getAllModuleFilesToPathSection`.
    fn module_files(
        &self,
        dependent_modules: &[StaticArktsDependentModule],
    ) -> Result<Vec<StaticArktsModuleFile>> {
        let mut pending = self
            .input
            .module_context
            .deps
            .nodes
            .values()
            .filter(|node| {
                matches!(
                    node.node_type,
                    DependencyNodeType::IntegratedHsp | DependencyNodeType::SharedHsp
                )
            })
            .map(|node| node.package_name.clone())
            .collect::<Vec<_>>();
        pending.sort();
        let mut seen = FxHashSet::default();
        let mut files = Vec::new();
        while let Some(package_name) = pending.pop() {
            let Some(node) = self
                .input
                .module_context
                .deps
                .nodes
                .values()
                .find(|node| node.package_name == package_name || node.alias == package_name)
            else {
                continue;
            };
            if !seen.insert(node.package_name.clone()) {
                continue;
            }
            pending.extend(node.direct_deps.iter().cloned());

            let Some(module) = dependent_modules
                .iter()
                .find(|module| module.package_name == node.package_name)
            else {
                continue;
            };
            if module.language == "1.1" {
                continue;
            }

            let mut static_files = Vec::new();
            if let Some(entry) = node.compile_inputs.other_compile_entry_file.as_ref() {
                self.push_static_module_file(entry, &module.language, &mut static_files)?;
            }
            for path in node.compile_inputs.other_compile_source_files.iter() {
                self.push_static_module_file(path, &module.language, &mut static_files)?;
            }
            static_files.sort();
            static_files.dedup();
            if !static_files.is_empty() {
                files.push(StaticArktsModuleFile {
                    package_name: node.package_name.clone(),
                    static_files,
                });
            }
        }
        files.sort_by(|left, right| left.package_name.cmp(&right.package_name));
        Ok(files)
    }

    fn push_static_module_file(
        &self,
        path: &Path,
        language: &str,
        files: &mut Vec<String>,
    ) -> Result<()> {
        if path.extension().and_then(|extension| extension.to_str()) != Some("ets")
            || arkdown_path::SourceFileName::new(path).is_declaration()
        {
            return Ok(());
        }
        if language == "hybrid" {
            let source = self
                .input
                .module_context
                .session
                .source_store
                .read_text(path)
                .with_context(|| {
                    format!(
                        "ARKDOWN_ARKTS12_DEPENDENCY_SOURCE_NOT_SNAPSHOTTED: {}",
                        path.display()
                    )
                })?;
            if !StaticArktsLanguageDetector::file_is_static(&source) {
                return Ok(());
            }
        }
        files.push(self.path(path)?);
        Ok(())
    }

    fn path(&self, path: &Path) -> Result<String> {
        ArtifactPathSerializer::path_to_slash_string(path).map_err(|error| {
            anyhow::anyhow!("serialize ArkTS 1.2 path {}: {error}", path.display())
        })
    }
}

/// Ports language selection from
/// `bindings/src/common/utils.ts::getFileLanguageVersion` and
/// `bindings/src/lsp/generateBuildConfig.ts::getModuleLanguageVersion`.
struct StaticArktsLanguageDetector<'a> {
    source_store: &'a SourceStore,
}

impl<'a> StaticArktsLanguageDetector<'a> {
    fn new(source_store: &'a SourceStore) -> Self {
        Self { source_store }
    }

    fn detect(
        &self,
        node: &DependencyNodeSnapshot,
        bytecode_abc_path: Option<&String>,
    ) -> Result<String> {
        if matches!(node.node_type, DependencyNodeType::BytecodeHar) {
            let path = bytecode_abc_path.ok_or_else(|| {
                anyhow::anyhow!(
                    "ARKDOWN_ARKTS12_BYTECODE_DEPENDENCY_METADATA_MISSING: {}",
                    node.package_name
                )
            })?;
            return match Path::new(path).file_name().and_then(|name| name.to_str()) {
                Some("modules_static.abc") => Ok("1.2".to_string()),
                Some("modules.abc") => Ok("1.1".to_string()),
                _ => anyhow::bail!(
                    "ARKDOWN_ARKTS12_BYTECODE_LANGUAGE_UNKNOWN: package '{}' uses {}",
                    node.package_name,
                    path
                ),
            };
        }

        let mut found_dynamic = false;
        let mut found_static = false;
        for path in node
            .compile_inputs
            .source_inventory_files
            .iter()
            .filter(|path| path.extension().and_then(|extension| extension.to_str()) == Some("ets"))
        {
            let source = self.source_store.read_text(path).with_context(|| {
                format!(
                    "ARKDOWN_ARKTS12_DEPENDENCY_SOURCE_NOT_SNAPSHOTTED: {}",
                    path.display()
                )
            })?;
            if Self::file_is_static(&source) {
                found_static = true;
            } else {
                found_dynamic = true;
            }
            if found_dynamic && found_static {
                return Ok("hybrid".to_string());
            }
        }
        Ok(if found_static { "1.2" } else { "1.1" }.to_string())
    }

    /// Exact comment skipping and first-effective-line test from
    /// `getFileLanguageVersion`; it deliberately checks `contains` rather than
    /// accepting only a normalized directive token.
    fn file_is_static(source: &str) -> bool {
        let mut in_multi_line_comment = false;
        for original_line in source.split('\n') {
            let mut line = original_line;
            let owned_line;
            if in_multi_line_comment {
                let Some(end_index) = line.find("*/") else {
                    continue;
                };
                line = &line[end_index + 2..];
                in_multi_line_comment = false;
            }
            if let Some(single_line_index) = line.find("//") {
                line = &line[..single_line_index];
            }
            if let Some(multi_line_start) = line.find("/*") {
                if let Some(relative_end) = line[multi_line_start + 2..].find("*/") {
                    let multi_line_end = multi_line_start + 2 + relative_end;
                    let mut value = String::with_capacity(line.len());
                    value.push_str(&line[..multi_line_start]);
                    value.push_str(&line[multi_line_end + 2..]);
                    owned_line = value;
                    line = &owned_line;
                } else {
                    line = &line[..multi_line_start];
                    in_multi_line_comment = true;
                }
            }
            let effective_line = line.trim();
            if !effective_line.is_empty() {
                return effective_line.contains("use static");
            }
        }
        false
    }
}

struct StaticArktsCompileEntryPlan<'a> {
    root: &'a Path,
    serialized_entries: &'a [String],
    internal_entries: &'a [String],
    module_inventory: &'a [PathBuf],
    module_sources: &'a [HvigorModuleSourceFile],
    dependent_modules: &'a [StaticArktsDependentModule],
}

impl<'a> StaticArktsCompileEntryPlan<'a> {
    fn new(
        root: &'a Path,
        serialized_entries: &'a [String],
        internal_entries: &'a [String],
        module_inventory: &'a [PathBuf],
        module_sources: &'a [HvigorModuleSourceFile],
        dependent_modules: &'a [StaticArktsDependentModule],
    ) -> Self {
        Self {
            root,
            serialized_entries,
            internal_entries,
            module_inventory,
            module_sources,
            dependent_modules,
        }
    }

    fn resolve(&self) -> Result<Vec<PathBuf>> {
        let mut files = Vec::new();
        let mut seen = FxHashSet::default();
        for entry in self
            .serialized_entries
            .iter()
            .chain(self.internal_entries.iter())
        {
            let entry = entry.strip_suffix("?entry").unwrap_or(entry);
            let path = Path::new(entry);
            let path = if path.has_root() {
                lexical_normalize(path)
            } else {
                lexical_normalize(&self.root.join(entry.strip_prefix("./").unwrap_or(entry)))
            };
            if !path.is_file() {
                anyhow::bail!("ARKDOWN_ARKTS12_COMPILE_ENTRY_MISSING: {}", path.display());
            }
            if seen.insert(path.clone()) {
                files.push(path);
            }
        }
        for path in self.module_inventory {
            let path = lexical_normalize(path);
            if seen.insert(path.clone()) {
                files.push(path);
            }
        }
        for source in self.module_sources {
            let path = lexical_normalize(Path::new(&source.module_id));
            if !path.has_root()
                || path.extension().and_then(|extension| extension.to_str()) != Some("ets")
                || !self.belongs_to_static_source_module(&path)
            {
                continue;
            }
            if !path.is_file() {
                anyhow::bail!("ARKDOWN_ARKTS12_COMPILE_ENTRY_MISSING: {}", path.display());
            }
            if seen.insert(path.clone()) {
                files.push(path);
            }
        }
        if files.is_empty() {
            anyhow::bail!("ARKDOWN_ARKTS12_COMPILE_ENTRY_EMPTY: no static compiler roots");
        }
        Ok(files)
    }

    /// `BaseMode::processEntryFiles` accepts only files below the main module
    /// or a `dependencyModuleList` entry. Dynamic 1.1 and separately built HSP
    /// dependencies remain dependency metadata; their sources are not merged
    /// into the current static ABC.
    fn belongs_to_static_source_module(&self, path: &Path) -> bool {
        if path.starts_with(self.root) {
            return true;
        }
        self.dependent_modules.iter().any(|module| {
            matches!(module.language.as_str(), "1.2" | "hybrid")
                && module.compile_sources
                && path.starts_with(Path::new(&module.module_path))
        })
    }
}

struct StaticArktsBuildMode<'a> {
    mode: &'a BuildMode,
}

impl<'a> StaticArktsBuildMode<'a> {
    fn new(mode: &'a BuildMode) -> Self {
        Self { mode }
    }
}

impl std::fmt::Display for StaticArktsBuildMode<'_> {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter.write_str(match self.mode {
            BuildMode::Debug => "Debug",
            BuildMode::Release => "Release",
            BuildMode::Test => "Test",
        })
    }
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
struct StaticArktsDependentModule {
    package_name: String,
    module_name: String,
    module_type: String,
    module_path: String,
    source_roots: Vec<String>,
    entry_file: String,
    language: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    decl_files_path: Option<String>,
    dependencies: Vec<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    original_package_name_map: Option<BTreeMap<String, String>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    abc_path: Option<String>,
    byte_code_har: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    bundle_type: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    bundle_name: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    package_version: Option<String>,
    #[serde(skip)]
    compile_sources: bool,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct StaticArktsDriverConfig {
    plugins: IndexMap<String, String>,
    paths: BTreeMap<String, Vec<String>>,
    entry_file: String,
    compile_files: Vec<String>,
    module_files: Vec<StaticArktsModuleFile>,
    build_mode: String,
    project_root_path: String,
    module_root_path: String,
    ace_profile_path: String,
    cache_path: String,
    loader_out_path: String,
    compile_sdk_version: u32,
    compatible_sdk_version: u32,
    ace_module_json_path: String,
    bundle_name: String,
    use_normalized_ohm_url: bool,
    build_type: String,
    package_name: String,
    build_sdk_path: String,
    source_roots: Vec<String>,
    module_type: String,
    module_name: String,
    dependent_module_list: Vec<StaticArktsDependentModule>,
    dependency_module_list: Vec<StaticArktsDependentModule>,
    has_main_module: bool,
    build_loader_json: String,
    integrated_hsp: bool,
    allow_empty_bundle_name: bool,
    enable_declgen_ets2_ts: bool,
    declgen_v2_out_path: String,
    external_api_paths: Vec<String>,
    interop_api_paths: Vec<String>,
    device_types: Vec<String>,
    permissions: BTreeMap<String, serde_json::Value>,
    byte_code_har: bool,
    ui_transform_optimization: bool,
    #[serde(rename = "runtimeOS")]
    runtime_os: String,
    sdk_config_paths: Vec<String>,
    is_preview: bool,
    check_entry: bool,
    code_root_path: String,
    is_crossplatform: bool,
    bundle_type: String,
    level: StaticArktsLogLevel,
    is_build_config_modified: bool,
    panda_sdk_path: String,
    abc_linker_path: String,
    dependency_analyzer_path: String,
    api_path: String,
    kits_path: String,
    dep_analyzer_path: String,
    alias_config: BTreeMap<String, serde_json::Value>,
    sdk_alias_map: BTreeMap<String, serde_json::Value>,
    interop_sdk_paths: BTreeMap<String, serde_json::Value>,
    es2panda_mode: String,
    app_resource: String,
    raw_file_resource: String,
    project_path: String,
    hsp_resources_map: bool,
    compile_har: bool,
    reset_bundle_name: bool,
    ignore_error: bool,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct StaticArktsModuleFile {
    package_name: String,
    static_files: Vec<String>,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct StaticArktsLogLevel {
    level: u32,
    level_str: String,
    colour: String,
}

impl Default for StaticArktsLogLevel {
    fn default() -> Self {
        Self {
            level: 20_000,
            level_str: "INFO".to_string(),
            colour: "green".to_string(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::{StaticArktsCompileEntryPlan, StaticArktsLanguageDetector};

    #[test]
    fn language_directive_matches_the_frontend_first_effective_line_algorithm() {
        assert!(StaticArktsLanguageDetector::file_is_static(
            "// heading\n/* block\n * comment\n */ 'use static'\nexport class A {}"
        ));
        assert!(StaticArktsLanguageDetector::file_is_static(
            "/* before */ 'use static' // after"
        ));
        assert!(!StaticArktsLanguageDetector::file_is_static(
            "export const value = 1;\n'use static'"
        ));
    }

    #[test]
    fn compile_entries_preserve_source_order_strip_query_and_deduplicate() {
        let temp = tempfile::tempdir().expect("tempdir");
        let root = temp.path().join("src/main");
        std::fs::create_dir_all(root.join("ets/pages")).expect("source directory");
        let page = root.join("ets/pages/Index.ets");
        let ability = root.join("ets/EntryAbility.ets");
        std::fs::write(&page, "").expect("page source");
        std::fs::write(&ability, "").expect("ability source");
        let serialized = vec![format!("{}?entry", ability.display())];
        let internal = vec![
            "./ets/pages/Index.ets?entry".to_string(),
            ability.display().to_string(),
        ];

        let resolved =
            StaticArktsCompileEntryPlan::new(&root, &serialized, &internal, &[], &[], &[])
                .resolve()
                .expect("static compile entry plan");

        assert_eq!(resolved, [ability, page]);
    }

    #[test]
    fn missing_static_compile_entry_is_a_typed_boundary_error() {
        let temp = tempfile::tempdir().expect("tempdir");
        let internal = vec!["./ets/pages/Missing.ets".to_string()];
        let error = StaticArktsCompileEntryPlan::new(temp.path(), &[], &internal, &[], &[], &[])
            .resolve()
            .expect_err("missing static root must be rejected before launching Node");

        assert!(
            error
                .to_string()
                .contains("ARKDOWN_ARKTS12_COMPILE_ENTRY_MISSING"),
            "{error}"
        );
    }
}
