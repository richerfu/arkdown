//! Hvigor-aligned module obfuscation planning for the build application.

use std::path::{Path, PathBuf};

use anyhow::Result;
use arkdown_config::build_profile::parse_api_version;
use arkdown_core::{
    ArkdownConfig, BuildMode, HarArtifactType, ModuleConfig, ObfuscationConfig, OhosModuleType,
    OutputTarget,
};
use arkdown_obfuscate::{
    ArkguardAdapterOptions, BytecodeObfuscationInput, BytecodeObfuscationModuleRecord,
    ConsumerObfuscationFileWriter, ObfuscationModuleFile, ObfuscationModuleInput, Obfuscator,
};
use arkdown_path::canonicalize_or_owned;
use arkdown_profile::LoaderJson;
use arkdown_session::dependency_graph::snapshot::{DependencyGraphSnapshot, DependencyNodeType};
use arkdown_session::{BuildSession, ModuleCompileContext};
use rustc_hash::{FxHashMap, FxHashSet};

use crate::bundle_plan::BundleStageOutput;
use crate::layout::{ModuleBuildLayout, ModuleSourceLayout};

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
    node_executable: PathBuf,
}

impl<'a> ModuleArkguardPlanBuilder<'a> {
    pub fn new(
        config: &'a ArkdownConfig,
        session: &'a BuildSession,
        module: &'a ModuleConfig,
        loader_json: &'a LoaderJson,
        bundle_out: &'a BundleStageOutput,
        node_executable: PathBuf,
    ) -> Self {
        Self {
            config,
            session,
            module,
            loader_json,
            bundle_out,
            node_executable,
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
        let source_maps: serde_json::Map<String, serde_json::Value> =
            serde_json::from_slice(&std::fs::read(&source_maps_path)?)?;
        let insight_intent_keep = self
            .bundle_out
            .bundle_output
            .insight_intent_sources
            .iter()
            .map(|source| (source.module_id.as_str(), &source.metadata))
            .collect::<FxHashMap<_, _>>();
        let files = self
            .bundle_out
            .bundle_output
            .module_source_files
            .iter()
            .map(|source| {
                let previous_source_map = source_maps
                    .get(&source.source_file)
                    .cloned()
                    .ok_or_else(|| {
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
                    source_map_key: source.source_file.clone(),
                    previous_source_map: Some(previous_source_map),
                    insight_intent_global_names: insight_intent
                        .map(|metadata| metadata.obfuscation_global_names.clone())
                        .unwrap_or_default(),
                    insight_intent_property_names: insight_intent
                        .map(|metadata| metadata.obfuscation_property_names.clone())
                        .unwrap_or_default(),
                })
            })
            .collect::<Result<Vec<_>>>()?;
        if files.is_empty() {
            anyhow::bail!(
                "ArkGuard source obfuscation has no CompileArkTS module sources for '{}'",
                self.module.name
            );
        }
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
        let module_path_map = self
            .loader_json
            .module_path_map
            .iter()
            .map(|(name, path)| (name.clone(), PathBuf::from(path)))
            .collect();
        let adapter = ArkguardAdapterOptions {
            node_executable: self.node_executable.clone(),
            ets_loader_path: self.config.sdk_config.ets_loader_path.clone(),
            arkguard_package: self
                .config
                .sdk_config
                .ets_loader_path
                .join("node_modules")
                .join("arkguard"),
            sdk_api_paths: self.session.sdk.global_module_paths.to_vec(),
            module_root: self.module.src_path.clone(),
            project_root: self.config.project_root.clone(),
            project_path: ModuleBuildLayout::new(&self.module.src_path).source().ets(),
            cache_path: PathBuf::from(&self.loader_json.cache_path),
            ace_module_build: PathBuf::from(&self.loader_json.ace_module_build),
            ace_module_json_path: ModuleBuildLayout::for_product(
                &self.module.src_path,
                &self.config.product.name,
            )
            .res_ark_module_json(),
            compile_sdk_version: self.config.sdk_config.compile_sdk_version.clone(),
            compatible_sdk_version: parse_api_version(&self.config.product.compatible_sdk_version)
                .unwrap_or(self.config.sdk_config.compile_api_version),
            compatible_sdk_version_stage: self.config.product.compatible_sdk_version_stage.clone(),
            entry_package_info: format!("{}|{}", package.name, package.version),
            package_dir: if self.loader_json.package_manager_type == "ohpm" {
                "oh_modules".to_string()
            } else {
                "node_modules".to_string()
            },
            module_path_map,
            entry_array: self.arkguard_entry_array(),
            resource_table_path: self.generated_resource_table_path(),
            local_packages: Vec::new(),
            compile_har: matches!(self.module.module_type, OhosModuleType::Har),
            compile_shared: matches!(self.module.module_type, OhosModuleType::Shared),
            use_normalized: self.loader_json.use_normalized_ohm_url,
            use_ts_har: false,
            byte_code_har_info: serde_json::to_value(&self.loader_json.byte_code_har_info)?,
        };
        Ok((
            adapter,
            ObfuscationModuleInput {
                files,
                source_maps_path: Some(source_maps_path),
            },
        ))
    }

    pub fn bytecode_input(&self, module: ObfuscationModuleInput) -> BytecodeObfuscationInput {
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
        BytecodeObfuscationInput {
            module,
            modules_abc_path: self.bundle_out.bundle_output.modules_abc_path.clone(),
            module_records,
            record_name_white_list,
        }
    }

    fn arkguard_entry_array(&self) -> Vec<String> {
        let mut entries = self.loader_json.runtime_entry_sources.clone();
        entries.extend(self.loader_json.hvigor_compile_entry.iter().cloned());
        let mut seen = FxHashSet::default();
        entries.retain(|entry| seen.insert(entry.clone()));
        entries
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
}

pub struct PendingBytecodeObfuscation {
    obfuscator: Obfuscator,
    input: BytecodeObfuscationInput,
}

impl PendingBytecodeObfuscation {
    pub fn new(obfuscator: Obfuscator, input: BytecodeObfuscationInput) -> Self {
        Self { obfuscator, input }
    }

    pub fn run(self, module_name: &str) -> Result<()> {
        self.obfuscator
            .obfuscate_bytecode(self.input)
            .map_err(|error| {
                anyhow::anyhow!(
                    "Bytecode obfuscation failed for module '{}': {}",
                    module_name,
                    error
                )
            })?;
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
