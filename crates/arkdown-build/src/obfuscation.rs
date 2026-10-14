//! Hvigor-aligned module obfuscation planning for the build application.

use std::collections::{BTreeMap, BTreeSet};
use std::path::{Path, PathBuf};
use std::sync::Arc;

use anyhow::{Context, Result};
use arkdown_config::build_profile::parse_api_version;
use arkdown_core::{
    ArkdownConfig, BuildMode, HarArtifactType, ModuleConfig, ObfuscationConfig, OutputTarget,
};
use arkdown_obfuscate::{
    ArkguardAdapterOptions, BytecodeObfuscationInput, BytecodeObfuscationModuleRecord,
    ConsumerObfuscationFileWriter, ObfuscationModuleFile, ObfuscationModuleInput,
    ObfuscationSource, ObfuscationSourceKind, Obfuscator,
};
use arkdown_path::{canonicalize_or_owned, ArtifactPathSerializer};
use arkdown_profile::LoaderJson;
use arkdown_session::dependency_graph::snapshot::{DependencyGraphSnapshot, DependencyNodeType};
use arkdown_session::{BuildSession, ModuleCompileContext};
use rustc_hash::{FxHashMap, FxHashSet};

use crate::bundle_plan::BundleStageOutput;
use crate::layout::{ModuleBuildLayout, ModuleSourceLayout};
use crate::static_arkts::StaticArktsObfuscationPlan;

pub struct ModuleObfuscationContext {
    pub config: Option<ObfuscationConfig>,
    pub dependency_rule_files: Vec<PathBuf>,
    pub output_target: OutputTarget,
    pub har_artifact_type: HarArtifactType,
    pub target_name: Option<String>,
}

pub struct ModuleArkguardPlanBuilder<'a> {
    config: &'a ArkdownConfig,
    session: &'a BuildSession,
    module: &'a ModuleConfig,
    loader_json: &'a LoaderJson,
    bundle_out: &'a BundleStageOutput,
}

impl<'a> ModuleArkguardPlanBuilder<'a> {
    pub fn new(
        config: &'a ArkdownConfig,
        session: &'a BuildSession,
        module: &'a ModuleConfig,
        loader_json: &'a LoaderJson,
        bundle_out: &'a BundleStageOutput,
    ) -> Self {
        Self {
            config,
            session,
            module,
            loader_json,
            bundle_out,
        }
    }

    pub fn build(&self) -> Result<(ArkguardAdapterOptions, ObfuscationModuleInput)> {
        let source_maps_path = self
            .bundle_out
            .bundle_output
            .source_maps_path
            .clone()
            .ok_or_else(|| {
                anyhow::anyhow!(
                    "ArkGuard requires the pre-obfuscation sourceMaps.map for module '{}'",
                    self.module.name
                )
            })?;
        let mut source_maps: serde_json::Map<String, serde_json::Value> =
            serde_json::from_slice(&std::fs::read(&source_maps_path)?)?;
        let insight_intent_keep = self
            .bundle_out
            .bundle_output
            .insight_intent_sources
            .iter()
            .map(|source| (source.module_id.as_str(), &source.metadata))
            .collect::<FxHashMap<_, _>>();
        let mut files = self
            .bundle_out
            .bundle_output
            .module_source_files
            .iter()
            .map(|source| {
                let previous_source_map =
                    source_maps.remove(&source.source_file).ok_or_else(|| {
                        anyhow::anyhow!(
                            "sourceMaps.map has no entry '{}' for ArkGuard input {}",
                            source.source_file,
                            source.path.display()
                        )
                    })?;
                let relative_path = previous_source_map
                    .get("sources")
                    .and_then(serde_json::Value::as_array)
                    .and_then(|sources| sources.first())
                    .and_then(serde_json::Value::as_str)
                    .filter(|source| !source.is_empty())
                    .ok_or_else(|| {
                        anyhow::anyhow!(
                            "source map '{}' has no original source path",
                            source.source_file
                        )
                    })?
                    .to_string();
                let original_path = PathBuf::from(&source.module_id);
                if !original_path.is_file() {
                    anyhow::bail!(
                        "ArkGuard original source does not exist for '{}': {}",
                        source.source_file,
                        original_path.display()
                    );
                }
                let insight_intent = insight_intent_keep.get(source.module_id.as_str());
                Ok(ObfuscationModuleFile {
                    build_path: source.path.clone(),
                    original_path,
                    relative_path,
                    record_name: (!source.record_name_is_explicit)
                        .then(|| source.record_name.clone()),
                    rename_file: !source.is_package_module,
                    source_name_cache_key: None,
                    source_map_key: source.source_file.clone(),
                    previous_source_map: Some(previous_source_map),
                    source: Arc::from(fs_err::read_to_string(&source.path)?),
                    insight_intent_global_names: insight_intent
                        .map(|metadata| metadata.obfuscation_global_names.clone())
                        .unwrap_or_default(),
                    insight_intent_property_names: insight_intent
                        .map(|metadata| metadata.obfuscation_property_names.clone())
                        .unwrap_or_default(),
                })
            })
            .collect::<Result<Vec<_>>>()?;
        files.extend(self.declaration_files()?);
        if files.is_empty() {
            anyhow::bail!(
                "ArkGuard source obfuscation has no CompileArkTS module sources for '{}'",
                self.module.name
            );
        }
        let source_module_paths = self
            .bundle_out
            .bundle_output
            .module_source_files
            .iter()
            .map(|source| PathBuf::from(&source.module_id))
            .collect::<FxHashSet<_>>();
        let module_dependencies = self
            .bundle_out
            .bundle_output
            .module_imports
            .iter()
            .filter_map(|(importer, imports)| {
                let importer = PathBuf::from(importer);
                if !source_module_paths.contains(&importer) {
                    return None;
                }
                let mut imports = imports
                    .iter()
                    .map(PathBuf::from)
                    .filter(|path| source_module_paths.contains(path))
                    .collect::<Vec<_>>();
                imports.sort();
                imports.dedup();
                Some((importer, imports))
            })
            .collect::<BTreeMap<_, _>>();
        let resource_table_path = self.generated_resource_table_path();
        let canonical_resource_table_path =
            resource_table_path.as_deref().map(canonicalize_or_owned);
        let mut inventory_sources = Vec::with_capacity(files.len().saturating_mul(2));
        for file in &files {
            let original_source = if file.relative_path.ends_with(".d.ts")
                || file.relative_path.ends_with(".d.ets")
                || canonical_resource_table_path
                    .as_ref()
                    .is_some_and(|path| *path == canonicalize_or_owned(&file.original_path))
            {
                Arc::clone(&file.source)
            } else {
                self.session.source_store.read_text(&file.original_path)?
            };
            inventory_sources.push(ObfuscationSource {
                path: file.original_path.clone(),
                source: original_source,
                kind: ObfuscationSourceKind::Project,
            });
            inventory_sources.push(ObfuscationSource {
                path: file.build_path.clone(),
                source: Arc::clone(&file.source),
                kind: ObfuscationSourceKind::Project,
            });
        }
        for root in self.session.sdk.arkguard_source_roots() {
            let kind = self.arkguard_sdk_source_kind(&root);
            inventory_sources.extend(
                self.session
                    .source_store
                    .text_sources_under(&root)?
                    .into_iter()
                    .filter(|(path, _)| Self::is_arkguard_source(path))
                    .map(|(path, source)| ObfuscationSource {
                        path: path.to_path_buf(),
                        source,
                        kind,
                    }),
            );
        }
        inventory_sources.sort_by(|left, right| left.path.cmp(&right.path));
        inventory_sources.dedup_by(|left, right| left.path == right.path);
        let module_context = self
            .session
            .module_context(&self.module.name)
            .ok_or_else(|| {
                anyhow::anyhow!(
                    "missing build session module context for {}",
                    self.module.name
                )
            })?;
        let package = module_context.module.package.as_ref().ok_or_else(|| {
            anyhow::anyhow!(
                "ArkGuard requires oh-package.json5 metadata for module '{}'",
                self.module.name
            )
        })?;
        let (arkui_reserved_property_names, arkui_optimized_reserved_property_names) =
            self.arkui_reserved_property_names()?;
        let adapter = ArkguardAdapterOptions {
            panda_guard_path: self.config.sdk_config.es2abc_path.with_file_name(
                if cfg!(target_os = "windows") {
                    "panda_guard.exe"
                } else {
                    "panda_guard"
                },
            ),
            arkguard_package: self
                .config
                .sdk_config
                .ets_loader_path
                .join("node_modules")
                .join("arkguard"),
            module_root: self.module.src_path.clone(),
            project_root: self.config.project_root.clone(),
            project_path: ModuleBuildLayout::new(&self.module.src_path).source().ets(),
            cache_path: PathBuf::from(&self.loader_json.cache_path),
            ace_module_build: PathBuf::from(&self.loader_json.ace_module_build),
            compile_sdk_version: self.config.sdk_config.compile_sdk_version.clone(),
            compatible_sdk_version: parse_api_version(&self.config.product.compatible_sdk_version)
                .unwrap_or(self.config.sdk_config.compile_api_version),
            compatible_sdk_version_stage: self.config.product.compatible_sdk_version_stage.clone(),
            entry_package_info: format!("{}|{}", package.name, package.version),
            compile_har: matches!(self.module.module_type, arkdown_core::OhosModuleType::Har),
            resource_table_path,
            use_normalized: self.loader_json.use_normalized_ohm_url,
            byte_code_har_info: serde_json::to_value(&self.loader_json.byte_code_har_info)?,
            reserved_file_paths: self.reserved_file_paths(&module_context),
            arkui_reserved_property_names,
            arkui_optimized_reserved_property_names,
        };
        Ok((
            adapter,
            ObfuscationModuleInput {
                files,
                inventory_sources,
                module_dependencies,
                preserved_source_maps: source_maps.into_iter().collect(),
                source_maps_path: Some(source_maps_path),
                source_map_mirror_paths: vec![
                    PathBuf::from(&self.loader_json.cache_path).join("sourceMaps.json"),
                    PathBuf::from(&self.loader_json.cache_path).join("sourceMaps.map"),
                ],
            },
        ))
    }

    fn is_arkguard_source(path: &Path) -> bool {
        let name = path
            .file_name()
            .and_then(|name| name.to_str())
            .unwrap_or_default();
        name.ends_with(".d.ts")
            || name.ends_with(".d.ets")
            || name.ends_with(".ts")
            || name.ends_with(".ets")
            || name.ends_with(".js")
    }

    fn arkguard_sdk_source_kind(&self, root: &Path) -> ObfuscationSourceKind {
        match root.file_name().and_then(|name| name.to_str()) {
            Some("pre_define.js") => ObfuscationSourceKind::PreDefine,
            Some("component") => ObfuscationSourceKind::SdkComponent,
            _ => ObfuscationSourceKind::SdkApi,
        }
    }

    /// Reconstruct the exact path families consumed by
    /// `ConfigResolver.collectResevedFileNameInIDEConfig` from immutable
    /// loader, module, and package snapshots.
    fn reserved_file_paths(&self, module_context: &ModuleCompileContext) -> Vec<String> {
        let mut paths = BTreeSet::new();
        paths.extend(self.loader_json.compile_entry.iter().cloned());
        paths.extend(self.loader_json.form_entry_sources.iter().cloned());
        paths.extend(self.loader_json.runtime_entry_sources.iter().cloned());
        paths.extend(self.loader_json.page_entry_sources.iter().cloned());
        paths.extend(self.loader_json.workers.iter().cloned());
        if let Some(path) = self.generated_resource_table_path() {
            paths.insert(path.to_string_lossy().into_owned());
        }
        if !self.loader_json.module_path_map.is_empty() {
            for (name, path) in &self.loader_json.module_path_map {
                paths.insert(name.clone());
                paths.insert(path.clone());
            }
        }
        if let Some(package) = module_context.module.package.as_ref() {
            paths.extend(package.package_json.main.iter().cloned());
            paths.extend(package.package_json.types.iter().cloned());
        }
        if let Some(src_entry) = module_context
            .module
            .module_json_value
            .as_deref()
            .and_then(|value| value.pointer("/module/srcEntry"))
            .and_then(serde_json::Value::as_str)
        {
            paths.insert(src_entry.to_string());
        }
        paths.insert(self.loader_json.module_root_path.clone());
        paths.insert(self.loader_json.cache_path.clone());
        if matches!(
            self.module.module_type,
            arkdown_core::OhosModuleType::Shared
        ) || self.loader_json.byte_code_har == Some(true)
        {
            paths.insert(self.loader_json.ace_module_build.clone());
            paths.insert("etsFortgz".to_string());
        }
        paths.into_iter().collect()
    }

    /// Load the two ArkUI property groups from the startup source snapshot.
    /// `ConfigResolver.collectUIApiWhitelist` always uses the base group and
    /// adds the optimized group only for `strip-system-api-args`.
    fn arkui_reserved_property_names(&self) -> Result<(Vec<String>, Vec<String>)> {
        let mut reserved = BTreeSet::new();
        let mut optimized = BTreeSet::new();
        for path in self.session.sdk.arkguard_source_roots() {
            if path.file_name().and_then(|name| name.to_str()) != Some("obfuscateWhiteList.json5") {
                continue;
            }
            let Some(source) = self.session.source_store.source_by_path(&path) else {
                continue;
            };
            let arkdown_session::SourceContent::Text(content) = source.cached_content()? else {
                continue;
            };
            let value: serde_json::Value = json5::from_str(&content)?;
            Self::extend_string_array(&mut reserved, &value, "ReservedPropertyNames");
            Self::extend_string_array(&mut optimized, &value, "OptimizedReservedPropertyNames");
        }
        Ok((
            reserved.into_iter().collect(),
            optimized.into_iter().collect(),
        ))
    }

    fn extend_string_array(output: &mut BTreeSet<String>, value: &serde_json::Value, field: &str) {
        if let Some(values) = value.get(field).and_then(serde_json::Value::as_array) {
            output.extend(
                values
                    .iter()
                    .filter_map(serde_json::Value::as_str)
                    .map(str::to_owned),
            );
        }
    }

    pub fn bytecode_input(
        &self,
        module: ObfuscationModuleInput,
    ) -> Result<BytecodeObfuscationInput> {
        let module_records = self
            .bundle_out
            .bundle_output
            .module_source_files
            .iter()
            .map(|source| BytecodeObfuscationModuleRecord {
                source_path: PathBuf::from(&source.module_id),
                record_name: source.record_name.clone(),
            })
            .collect();
        // Latest SDK `module_mode.js::generateCompileContextInfo` registers only
        // explicit module-meta `ohmurl` values. Arkdown's matching meta collector
        // assigns that shape to generated ResourceTable entries (`&...&`).
        let resource_table_path = self.generated_resource_table_path();
        let mut record_name_white_list = self
            .bundle_out
            .bundle_output
            .module_source_files
            .iter()
            .filter(|source| {
                resource_table_path
                    .as_ref()
                    .is_some_and(|path| Path::new(&source.module_id) == path)
            })
            .map(|source| source.record_name.clone())
            .collect::<Vec<_>>();
        record_name_white_list.sort();
        record_name_white_list.dedup();
        let reserved_name_sources = module.inventory_sources.clone();
        Ok(BytecodeObfuscationInput {
            module,
            modules_abc_path: self.bundle_out.bundle_output.modules_abc_path.clone(),
            module_records,
            record_name_white_list,
            reserved_name_sources,
        })
    }

    fn generated_resource_table_path(&self) -> Option<PathBuf> {
        self.loader_json
            .hvigor_compile_entry
            .iter()
            .map(PathBuf::from)
            .find(|path| {
                path.file_name().and_then(|name| name.to_str()) == Some("ResourceTable.ts")
                    && path
                        .strip_prefix(&self.module.src_path)
                        .is_ok_and(|relative| {
                            relative.starts_with("build")
                                && relative
                                    .components()
                                    .any(|part| part.as_os_str() == "generated")
                        })
            })
    }

    fn declaration_files(&self) -> Result<Vec<ObfuscationModuleFile>> {
        let declarations_root = ModuleBuildLayout::new(&self.module.src_path).declarations_dir();
        if !declarations_root.is_dir() {
            return Ok(Vec::new());
        }
        let mut entries = walkdir::WalkDir::new(&declarations_root)
            .min_depth(1)
            .follow_links(false)
            .into_iter()
            .collect::<std::result::Result<Vec<_>, _>>()?;
        entries.retain(|entry| {
            entry.file_type().is_file()
                && entry
                    .file_name()
                    .to_str()
                    .is_some_and(|name| name.ends_with(".d.ts") || name.ends_with(".d.ets"))
        });
        entries.sort_by(|left, right| left.path().cmp(right.path()));
        entries
            .into_iter()
            .map(|entry| {
                let build_path = entry.into_path();
                let relative = build_path.strip_prefix(&declarations_root)?;
                let relative_path = ArtifactPathSerializer::rel_to_slash(relative);
                let source_relative = relative_path
                    .strip_suffix(".d.ets")
                    .map(|path| format!("{path}.ets"))
                    .or_else(|| {
                        relative_path
                            .strip_suffix(".d.ts")
                            .map(|path| format!("{path}.ts"))
                    })
                    .unwrap_or_else(|| relative_path.clone());
                let source_path = self.config.project_root.join(&source_relative);
                let declaration_path = self.config.project_root.join(relative);
                let original_path = if source_path.is_file() {
                    source_path
                } else if declaration_path.is_file() {
                    declaration_path
                } else {
                    build_path.clone()
                };
                Ok(ObfuscationModuleFile {
                    source: Arc::from(fs_err::read_to_string(&build_path)?),
                    build_path,
                    original_path,
                    relative_path: relative_path.clone(),
                    record_name: None,
                    rename_file: true,
                    source_name_cache_key: Some(source_relative),
                    source_map_key: relative_path,
                    previous_source_map: None,
                    insight_intent_global_names: Vec::new(),
                    insight_intent_property_names: Vec::new(),
                })
            })
            .collect()
    }
}

pub struct PendingBytecodeObfuscation {
    dynamic: Option<PendingDynamicBytecodeObfuscation>,
    static_driver: Option<StaticArktsObfuscationPlan>,
}

struct PendingDynamicBytecodeObfuscation {
    obfuscator: Box<Obfuscator>,
    input: BytecodeObfuscationInput,
}

impl PendingBytecodeObfuscation {
    pub fn dynamic(obfuscator: Obfuscator, input: BytecodeObfuscationInput) -> Self {
        Self {
            dynamic: Some(PendingDynamicBytecodeObfuscation {
                obfuscator: Box::new(obfuscator),
                input,
            }),
            static_driver: None,
        }
    }

    pub fn static_driver(plan: StaticArktsObfuscationPlan) -> Self {
        Self {
            dynamic: None,
            static_driver: Some(plan),
        }
    }

    pub fn with_dynamic(mut self, obfuscator: Obfuscator, input: BytecodeObfuscationInput) -> Self {
        self.dynamic = Some(PendingDynamicBytecodeObfuscation {
            obfuscator: Box::new(obfuscator),
            input,
        });
        self
    }

    pub fn static_driver_plan(&self) -> Option<&StaticArktsObfuscationPlan> {
        self.static_driver.as_ref()
    }

    pub fn run_static(&mut self, module_name: &str, modules_abc: &Path) -> Result<()> {
        if let Some(plan) = self.static_driver.take() {
            plan.execute(modules_abc).with_context(|| {
                format!(
                    "ArkTS 1.2 bytecode obfuscation failed for module '{}'",
                    module_name
                )
            })?;
        }
        Ok(())
    }

    pub fn run_dynamic(mut self, module_name: &str) -> Result<()> {
        if let Some(pending) = self.dynamic.take() {
            pending
                .obfuscator
                .obfuscate_bytecode(pending.input)
                .map_err(|error| {
                    anyhow::anyhow!(
                        "Bytecode obfuscation failed for module '{}': {}",
                        module_name,
                        error
                    )
                })?;
        }
        Ok(())
    }
}

impl ModuleObfuscationContext {
    pub fn resolve(
        config: &ArkdownConfig,
        session: &BuildSession,
        module: &ModuleConfig,
        build_option: &arkdown_core::BuildOption,
    ) -> Result<Self> {
        let effective = build_option
            .ark_options
            .as_ref()
            .and_then(|options| options.obfuscation.clone());
        let dependency_rule_files = if effective.as_ref().is_some_and(|obfuscation| {
            obfuscation.is_enabled_for_mode(&config.build_mode)
                || !obfuscation.consumer_rule_files.is_empty()
        }) {
            let module_context = session.module_context(&module.name).ok_or_else(|| {
                anyhow::anyhow!("missing module compile context for {}", module.name)
            })?;
            ObfuscationDependencyRuleCollector::new(module_context.deps.as_ref()).collect()
        } else {
            Vec::new()
        };
        Ok(Self {
            config: effective,
            dependency_rule_files,
            output_target: config.output_target.clone(),
            har_artifact_type: build_option.har_artifact_type,
            target_name: None,
        })
    }

    pub fn is_enabled(&self, build_mode: &BuildMode) -> bool {
        // hvigor UnitTestArkCompile/OhosTestArkCompile delete obfuscationOptions;
        // local unit-test and ohosTest builds never obfuscate.
        if matches!(self.output_target, OutputTarget::Test)
            || self.target_name.as_deref() == Some("ohosTest")
        {
            return false;
        }
        self.config
            .as_ref()
            .is_some_and(|config| config.is_enabled_for_mode(build_mode))
    }

    /// `ProcessObfuscationFiles.taskShouldDo()` from hvigor 6.24.3.
    pub fn should_process_consumer_rules(
        &self,
        module_context: &ModuleCompileContext,
        build_mode: &BuildMode,
    ) -> bool {
        if !matches!(self.output_target, OutputTarget::Har) {
            return false;
        }
        let target_name = self.target_name.as_deref().unwrap_or("default");
        let source_layout = ModuleSourceLayout::new(&module_context.module.root);
        let default_ets = source_layout.ets();
        let target_ets = source_layout.target_ets(target_name);
        let has_ets_source = default_ets.is_dir()
            || (target_name != "default" && target_ets.is_dir())
            || module_context
                .module
                .source_roots
                .iter()
                .any(|root| root == &default_ets || root == &target_ets);
        let is_obfuscated_har = !build_mode.is_debuggable()
            && (self
                .config
                .as_ref()
                .is_some_and(|config| config.enable_obfuscation)
                || matches!(self.har_artifact_type, HarArtifactType::Obfuscation));
        has_ets_source
            && !is_obfuscated_har
            && self
                .config
                .as_ref()
                .is_some_and(|config| !config.consumer_rule_files.is_empty())
    }

    /// `ProcessObfuscationFiles.doTaskAction()`.
    pub fn process_consumer_rules(
        &self,
        module_context: &ModuleCompileContext,
        build_mode: &BuildMode,
        compile_api_version: u32,
        output_path: PathBuf,
    ) -> Result<()> {
        if !self.should_process_consumer_rules(module_context, build_mode)
            || compile_api_version < 10
            || build_mode.is_debuggable()
        {
            return Ok(());
        }
        let config = self.config.as_ref().ok_or_else(|| {
            anyhow::anyhow!("ProcessObfuscationFiles lost its resolved obfuscation configuration")
        })?;
        ConsumerObfuscationFileWriter::new(config.consumer_rule_files.clone(), output_path)
            .with_dependency_rule_files(self.dependency_rule_files.clone())
            .write()
            .map_err(|error| anyhow::anyhow!("Generate obfuscation.txt failed: {error}"))
    }
}

struct ObfuscationDependencyRuleCollector<'a> {
    deps: &'a DependencyGraphSnapshot,
}

impl<'a> ObfuscationDependencyRuleCollector<'a> {
    fn new(deps: &'a DependencyGraphSnapshot) -> Self {
        Self { deps }
    }

    fn collect(&self) -> Vec<PathBuf> {
        let mut paths = Vec::new();
        let mut seen = FxHashSet::default();
        for node in self.deps.nodes.values() {
            if !matches!(
                node.node_type,
                DependencyNodeType::SourceHar
                    | DependencyNodeType::BytecodeHar
                    | DependencyNodeType::IntegratedHsp
                    | DependencyNodeType::SharedHsp
            ) {
                continue;
            }
            let Some(package_root) = node.package_root.as_deref() else {
                continue;
            };
            let path = package_root.join("obfuscation.txt");
            if !path.is_file() {
                continue;
            }
            let key = canonicalize_or_owned(&path);
            if seen.insert(key.clone()) {
                paths.push(key);
            }
        }
        paths
    }
}

#[cfg(test)]
mod tests {
    use arkdown_session::dependency_graph::snapshot::{
        DependencyGraphSnapshot, DependencyNodeSnapshot, DependencyNodeType,
    };

    use super::*;

    fn node(alias: &str, root: PathBuf, node_type: DependencyNodeType) -> DependencyNodeSnapshot {
        DependencyNodeSnapshot {
            alias: alias.to_string(),
            package_name: alias.to_string(),
            version_spec: None,
            package_root: Some(root),
            package: None,
            module_profile: None,
            compile_inputs: Default::default(),
            node_type,
            is_runtime_only: false,
            is_dynamic: false,
            is_transitive: false,
            direct_deps: vec![],
        }
    }

    #[test]
    fn collects_hvigor_dependency_obfuscation_txt_only() {
        let temp = tempfile::tempdir().expect("tempdir");
        let har_root = temp.path().join("har");
        let hsp_root = temp.path().join("hsp");
        let source_root = temp.path().join("source");
        fs_err::create_dir_all(&har_root).expect("har dir");
        fs_err::create_dir_all(&hsp_root).expect("hsp dir");
        fs_err::create_dir_all(&source_root).expect("source dir");
        fs_err::write(
            har_root.join("obfuscation.txt"),
            "-keep-global-name\nHarApi\n",
        )
        .expect("har obfuscation");
        fs_err::write(
            hsp_root.join("obfuscation.txt"),
            "-keep-property-name\nhspProp\n",
        )
        .expect("hsp obfuscation");
        fs_err::write(
            source_root.join("obfuscation.txt"),
            "-keep-global-name\nIgnored\n",
        )
        .expect("source obfuscation");

        let mut deps = DependencyGraphSnapshot::default();
        deps.nodes.insert(
            "har".to_string(),
            node("har", har_root.clone(), DependencyNodeType::SourceHar),
        );
        deps.nodes.insert(
            "hsp".to_string(),
            node("hsp", hsp_root.clone(), DependencyNodeType::SharedHsp),
        );
        deps.nodes.insert(
            "source".to_string(),
            node("source", source_root, DependencyNodeType::OhpmSource),
        );
        deps.nodes.insert(
            "same-har".to_string(),
            node(
                "same-har",
                har_root.clone(),
                DependencyNodeType::BytecodeHar,
            ),
        );

        let paths = ObfuscationDependencyRuleCollector::new(&deps).collect();

        assert_eq!(paths.len(), 2, "paths: {paths:?}");
        assert!(paths.contains(&canonicalize_or_owned(&har_root.join("obfuscation.txt"))));
        assert!(paths.contains(&canonicalize_or_owned(&hsp_root.join("obfuscation.txt"))));
    }
}
