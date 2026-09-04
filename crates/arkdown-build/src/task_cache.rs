use std::io;
use std::path::{Path, PathBuf};

use arkdown_core::{ArkdownConfig, ArktsLanguageVersion, BuildMode, ObfuscationConfig};
use arkdown_ets_transform::ModuleAnalyzerKind;
use arkdown_path::{ArtifactPathSerializer, SourceFileName};
use arkdown_profile::LoaderJson;
use arkdown_session::BuildSession;

/// Session-derived, content-addressed inputs for one module compile task.
///
/// Hvigor's `ArkCompile.declareInputFiles()` tracks actual input file content.
/// The build service creates this snapshot from `BuildSession` so cache checks do not
/// perform filesystem discovery or content reads during stage execution.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TaskInputSnapshot {
    pub source_hash: String,
    pub resource_hash: String,
    pub dependency_content_hash: String,
    pub config_hash: String,
    pub profile_hash: String,
    pub sdk_hash: String,
}

impl TaskInputSnapshot {
    pub fn for_module(session: &BuildSession, module_name: &str) -> Option<Self> {
        let module = session.fingerprints.module(module_name)?;
        Some(Self {
            source_hash: module.sources.to_hex(),
            resource_hash: module.resources.to_hex(),
            dependency_content_hash: module.dependencies.to_hex(),
            config_hash: session.fingerprints.config.to_hex(),
            profile_hash: session.fingerprints.profiles.to_hex(),
            sdk_hash: session.fingerprints.sdk.to_hex(),
        })
    }
}

/// Snapshot of all task-level inputs that determine whether the compile step
/// can be skipped entirely.
///
/// Mirrors hvigor's `declareInputs()` on `ArkCompile`:
/// ```text
/// { sourceFiles, obfuscationHash, sdkVersion, compileEnv }
/// ```
/// If all fields match the previously persisted values AND all expected output
/// files exist, the entire compile + ABC step is skipped and the packager runs
/// directly against the cached `.abc`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TaskInputs {
    pub schema_version: u32,
    pub source_hash: String,
    pub obfuscation_hash: u64,
    pub sdk_version: String,
    pub compile_env: String,
    pub module_analyzer: String,
    pub arkts_language_version: ArktsLanguageVersion,
    pub plugin_hash: u64,
    pub resource_hash: String,
    pub dependency_content_hash: String,
    pub config_hash: String,
    pub profile_hash: String,
    pub sdk_hash: String,
    /// Content fingerprint of `buildOption.arkOptions.types` declaration inputs.
    pub custom_types_hash: u64,
    /// FNV-1a hash of the module dependency graph identity (hsp/har ohm maps).
    /// Mirrors hvigor's inclusion of dependency changes in ArkCompile cache inputs.
    pub dependency_hash: u64,
}

const TASK_INPUTS_FILE: &str = ".task_inputs";
const TASK_INPUTS_SCHEMA_VERSION: u32 = 6;
const FNV_OFFSET_BASIS: u64 = 0xcbf29ce484222325;
const FNV_PRIME: u64 = 0x100000001b3;

impl TaskInputs {
    /// Compute current task inputs from config and loader data.
    pub fn compute(
        config: &ArkdownConfig,
        loader: &LoaderJson,
        snapshot: &TaskInputSnapshot,
    ) -> Self {
        let obfuscation_hash = Self::compute_obfuscation_hash(config);
        let sdk_version = config.sdk_config.compile_sdk_version.clone();
        let obfuscation_enabled = config
            .product
            .build_option
            .ark_options
            .as_ref()
            .and_then(|options| options.obfuscation.as_ref())
            .map(|obfuscation| obfuscation.enable_obfuscation)
            .unwrap_or(false);
        let module_type = config
            .modules
            .first()
            .map(|module| format!("{:?}", module.module_type))
            .unwrap_or_default();
        let compile_env = arkdown_cache::CompileEnv::from_plan(
            config.sdk_config.compile_sdk_version.clone(),
            config.product.compatible_sdk_version.clone(),
            config.product.runtime_os.clone(),
            ArtifactPathSerializer::path_to_slash_string(&config.sdk_config.ets_loader_path)
                .unwrap_or_else(|_| config.sdk_config.ets_loader_path.display().to_string()),
            obfuscation_enabled,
            module_type,
            arkdown_cache::compiler_cache_path(&loader.cache_path),
        )
        .to_key_string();
        let dependency_hash = {
            let mut h: u64 = 0xcbf29ce484222325;
            // Hash HAR name→OHM map entries (sorted for determinism).
            let mut har_entries: Vec<_> = loader.har_name_ohm_map.iter().collect();
            har_entries.sort_by_key(|(k, _)| k.as_str());
            for (k, v) in &har_entries {
                for b in k.bytes().chain(b"|".iter().copied()).chain(v.bytes()) {
                    h ^= b as u64;
                    h = h.wrapping_mul(0x100000001b3);
                }
            }
            // Hash HSP name→OHM map entries.
            let mut hsp_entries: Vec<_> = loader.hsp_name_ohm_map.iter().collect();
            hsp_entries.sort_by_key(|(k, _)| k.as_str());
            for (k, v) in &hsp_entries {
                for b in k.bytes().chain(b"|".iter().copied()).chain(v.bytes()) {
                    h ^= b as u64;
                    h = h.wrapping_mul(0x100000001b3);
                }
            }
            h
        };
        TaskInputs {
            schema_version: TASK_INPUTS_SCHEMA_VERSION,
            source_hash: snapshot.source_hash.clone(),
            obfuscation_hash,
            sdk_version,
            compile_env,
            module_analyzer: ModuleAnalyzerKind::default().as_str().to_string(),
            arkts_language_version: ArktsLanguageVersion::default(),
            plugin_hash: 0,
            resource_hash: snapshot.resource_hash.clone(),
            dependency_content_hash: snapshot.dependency_content_hash.clone(),
            config_hash: snapshot.config_hash.clone(),
            profile_hash: snapshot.profile_hash.clone(),
            sdk_hash: snapshot.sdk_hash.clone(),
            custom_types_hash: 0,
            dependency_hash,
        }
    }

    /// Builder: set the plugin hash (FNV-1a of package plugin identities).
    pub fn with_plugin_hash(mut self, h: u64) -> Self {
        self.plugin_hash = h;
        self
    }

    pub fn with_module_analyzer(mut self, module_analyzer: ModuleAnalyzerKind) -> Self {
        self.module_analyzer = module_analyzer.as_str().to_string();
        self
    }

    pub fn with_arkts_language_version(mut self, version: ArktsLanguageVersion) -> Self {
        self.arkts_language_version = version;
        self
    }

    /// Replace the product-level obfuscation input with the module's effective
    /// build-option result. Hvigor resolves module build-option sets and target
    /// overrides before declaring `ArkCompile` inputs, so the cache key must use
    /// that same resolved configuration.
    pub fn with_effective_obfuscation(
        mut self,
        obfuscation: Option<&ObfuscationConfig>,
        build_mode: &BuildMode,
    ) -> Self {
        self.obfuscation_hash = Self::compute_obfuscation_config_hash(obfuscation, build_mode);
        self
    }

    /// Use the exact per-module compile environment passed to the bundler and
    /// bytecode cache bridge.
    pub fn with_compile_env(mut self, compile_env: String) -> Self {
        self.compile_env = compile_env;
        self
    }

    /// Track the exact compiler type identities and declaration contents.
    ///
    /// Hvigor's `ArkCompile.declareInputFiles()` includes the files resolved
    /// from `arkOptions.types`. These files can live outside the module's
    /// standard `src/main` roots, so the session source fingerprint alone is
    /// insufficient.
    pub fn with_custom_type_inputs(
        mut self,
        compiler_types: &[String],
        declaration_inputs: &[PathBuf],
    ) -> io::Result<Self> {
        if compiler_types.is_empty() {
            self.custom_types_hash = 0;
            return Ok(self);
        }

        let mut hash = FNV_OFFSET_BASIS;
        for configured in compiler_types {
            Self::hash_bytes_into(&mut hash, configured.as_bytes());
        }
        for (input_index, input) in declaration_inputs.iter().enumerate() {
            let mut files = Vec::new();
            if input.is_file() {
                if SourceFileName::new(input).is_declaration() {
                    files.push(input.clone());
                }
            } else if input.is_dir() {
                for entry in walkdir::WalkDir::new(input).follow_links(false) {
                    let entry = entry.map_err(|error| {
                        io::Error::other(format!(
                            "scan custom type input {} failed: {error}",
                            input.display()
                        ))
                    })?;
                    if entry.file_type().is_file()
                        && SourceFileName::new(entry.path()).is_declaration()
                    {
                        files.push(entry.into_path());
                    }
                }
            }
            files.sort();
            for path in files {
                Self::hash_bytes_into(&mut hash, input_index.to_string().as_bytes());
                let path_key = path
                    .strip_prefix(input)
                    .ok()
                    .filter(|relative| !relative.as_os_str().is_empty())
                    .map(ArtifactPathSerializer::rel_to_slash)
                    .or_else(|| {
                        path.file_name()
                            .map(|name| name.to_string_lossy().into_owned())
                    })
                    .unwrap_or_else(|| path.display().to_string());
                Self::hash_bytes_into(&mut hash, path_key.as_bytes());
                let content = std::fs::read(&path)?;
                Self::hash_bytes_into(&mut hash, &content);
            }
        }
        self.custom_types_hash = hash;
        Ok(self)
    }

    /// Builder: add dependency-exported obfuscation rule files to the existing
    /// obfuscation hash.
    pub fn with_additional_obfuscation_rule_files(mut self, paths: &[PathBuf]) -> Self {
        if paths.is_empty() {
            return self;
        }
        let mut h = if self.obfuscation_hash == 0 {
            FNV_OFFSET_BASIS
        } else {
            self.obfuscation_hash
        };
        Self::hash_file_contents_into(&mut h, paths.iter().map(PathBuf::as_path));
        self.obfuscation_hash = h;
        self
    }

    /// Return `true` if the persisted inputs in `cache_dir` match `self`.
    ///
    /// A mismatch on any field means the task must re-run.  Missing file is
    /// treated as a cache miss (first build).
    pub fn matches_cached(&self, cache_dir: &Path) -> bool {
        let path = cache_dir.join(TASK_INPUTS_FILE);
        let Ok(content) = std::fs::read_to_string(&path) else {
            return false;
        };
        let Ok(stored) = serde_json::from_str::<StoredTaskInputs>(&content) else {
            return false;
        };
        stored.schema_version == self.schema_version
            && stored.source_hash == self.source_hash
            && stored.obfuscation_hash == self.obfuscation_hash
            && stored.sdk_version == self.sdk_version
            && stored.compile_env == self.compile_env
            && stored.module_analyzer == self.module_analyzer
            && stored.arkts_language_version == self.arkts_language_version
            && stored.plugin_hash == self.plugin_hash
            && stored.resource_hash == self.resource_hash
            && stored.dependency_content_hash == self.dependency_content_hash
            && stored.config_hash == self.config_hash
            && stored.profile_hash == self.profile_hash
            && stored.sdk_hash == self.sdk_hash
            && stored.custom_types_hash == self.custom_types_hash
            && stored.dependency_hash == self.dependency_hash
    }

    /// Persist `self` to `cache_dir/.task_inputs` for the next build.
    pub fn save(&self, cache_dir: &Path) -> io::Result<()> {
        std::fs::create_dir_all(cache_dir)?;
        let stored = StoredTaskInputs {
            schema_version: self.schema_version,
            source_hash: self.source_hash.clone(),
            obfuscation_hash: self.obfuscation_hash,
            sdk_version: self.sdk_version.clone(),
            compile_env: self.compile_env.clone(),
            module_analyzer: self.module_analyzer.clone(),
            arkts_language_version: self.arkts_language_version,
            plugin_hash: self.plugin_hash,
            resource_hash: self.resource_hash.clone(),
            dependency_content_hash: self.dependency_content_hash.clone(),
            config_hash: self.config_hash.clone(),
            profile_hash: self.profile_hash.clone(),
            sdk_hash: self.sdk_hash.clone(),
            custom_types_hash: self.custom_types_hash,
            dependency_hash: self.dependency_hash,
        };
        let json = serde_json::to_string_pretty(&stored).map_err(io::Error::other)?;
        std::fs::write(cache_dir.join(TASK_INPUTS_FILE), json)
    }
}

#[derive(serde::Serialize, serde::Deserialize)]
struct StoredTaskInputs {
    #[serde(default)]
    schema_version: u32,
    #[serde(default)]
    source_hash: String,
    obfuscation_hash: u64,
    sdk_version: String,
    compile_env: String,
    #[serde(default = "StoredTaskInputs::default_module_analyzer")]
    module_analyzer: String,
    #[serde(default)]
    arkts_language_version: ArktsLanguageVersion,
    #[serde(default)]
    plugin_hash: u64,
    #[serde(default)]
    resource_hash: String,
    #[serde(default)]
    dependency_content_hash: String,
    #[serde(default)]
    config_hash: String,
    #[serde(default)]
    profile_hash: String,
    #[serde(default)]
    sdk_hash: String,
    #[serde(default)]
    custom_types_hash: u64,
    #[serde(default)]
    dependency_hash: u64,
}

impl StoredTaskInputs {
    fn default_module_analyzer() -> String {
        ModuleAnalyzerKind::Oxc.as_str().to_string()
    }
}

impl TaskInputs {
    /// Derive a stable obfuscation hash from the config's obfuscation settings.
    ///
    /// Hashes the content of ALL rule files (not just the first) so that a
    /// change to any rule file invalidates the ABC compile cache.
    ///
    /// Returns 0 when obfuscation is disabled or no rule files are configured.
    fn compute_obfuscation_hash(config: &ArkdownConfig) -> u64 {
        let obfuscation = config
            .product
            .build_option
            .ark_options
            .as_ref()
            .and_then(|options| options.obfuscation.as_ref());
        Self::compute_obfuscation_config_hash(obfuscation, &config.build_mode)
    }

    fn compute_obfuscation_config_hash(
        obfuscation: Option<&ObfuscationConfig>,
        build_mode: &BuildMode,
    ) -> u64 {
        let Some(obf) = obfuscation else {
            return 0;
        };
        if !obf.is_enabled_for_mode(build_mode)
            || (obf.rule_files.is_empty() && obf.consumer_rule_files.is_empty())
        {
            return 0;
        }
        let mut h: u64 = FNV_OFFSET_BASIS;
        // Hash all rule files in order — a change to any file invalidates the cache.
        Self::hash_file_contents_into(
            &mut h,
            obf.rule_files
                .iter()
                .chain(obf.consumer_rule_files.iter())
                .map(PathBuf::as_path),
        );
        h
    }

    fn hash_file_contents_into<'a>(h: &mut u64, paths: impl IntoIterator<Item = &'a Path>) {
        for rules_path in paths {
            let Ok(content) = std::fs::read(rules_path) else {
                continue;
            };
            for b in content {
                *h ^= b as u64;
                *h = (*h).wrapping_mul(FNV_PRIME);
            }
            // Separator between files so that [A, AB] != [AA, B].
            *h ^= b'\0' as u64;
            *h = (*h).wrapping_mul(FNV_PRIME);
        }
    }

    fn hash_bytes_into(h: &mut u64, bytes: &[u8]) {
        for byte in bytes {
            *h ^= u64::from(*byte);
            *h = (*h).wrapping_mul(FNV_PRIME);
        }
        *h ^= 0;
        *h = (*h).wrapping_mul(FNV_PRIME);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;

    fn make_loader(compile_entry: Vec<String>, cache_path: &str) -> LoaderJson {
        LoaderJson {
            module_path_map: Default::default(),
            compile_mode: "esmodule".to_string(),
            project_root_path: String::new(),
            node_modules_path: String::new(),
            byte_code_har_info: Default::default(),
            declaration_entry: vec![],
            module_name: "entry".to_string(),
            hsp_name_ohm_map: Default::default(),
            har_name_ohm_map: Default::default(),
            package_manager_type: "ohpm".to_string(),
            hvigor_compile_entry: vec![],
            other_compile_files: vec![],
            dynamic_import_lib_info: Default::default(),
            router_map: vec![],
            hsp_resources_map: Default::default(),
            update_version_info: Default::default(),
            customized_har: false,
            byte_code_har: None,
            pkg_name2_other_compile_files: None,
            an_build_out_put: None,
            an_build_mode: None,
            ap_path: None,
            build_config_path: None,
            patch_config: None,
            non_entry_importees: vec![],
            compiled_importees: vec![],
            compile_entry,
            form_entry_sources: vec![],
            insight_intent_form_bindings: vec![],
            runtime_entry_sources: vec![],
            page_entry_sources: vec![],
            module_root_path: String::new(),
            cache_path: cache_path.to_string(),
            ace_module_build: String::new(),
            build_mode: "debug".to_string(),
            support_chunks: false,
            use_normalized_ohm_url: false,
            api_version: 12,
            fallback_an_build: false,
            pkg_name_to_pkg_brief_info: Default::default(),
            workers: vec![],
            source_roots: vec![],
            bundle_dependencies: vec![],
            global_dependencies: vec![],
            compiler_types: vec![],
            compiler_type_declaration_inputs: vec![],
            dependency_pkg_version: vec![],
            extra_importees: vec![],
            compiled_package_names: vec![],
            oh_exports: vec![],
        }
    }

    fn make_config() -> ArkdownConfig {
        use arkdown_core::*;
        ArkdownConfig {
            project_root: PathBuf::from("/project"),
            build_mode: BuildMode::Debug,
            output_target: OutputTarget::Hap,
            product: ProductConfig {
                name: "default".to_string(),
                artifact_name: None,
                bundle_name: "com.example.test".to_string(),
                compatible_sdk_version: "12".to_string(),
                compatible_sdk_version_stage: None,
                target_sdk_version: None,
                runtime_os: "OpenHarmony".to_string(),
                signing_config: None,
                build_option: BuildOption {
                    preload_system_so: false,
                    preload_so: vec![],
                    deduplicate_har: false,
                    enable_source_code_check: true,
                    app_with_signed_pkg: false,
                    generate_shared_tgz: None,
                    customized_har_base_package: None,
                    remove_permissions: vec![],
                    aot_compile_mode: None,
                    host_pgo: None,
                    ap_path: None,
                    abi_filters: vec![],
                    external_native_options: None,
                    har_artifact_type: arkdown_core::HarArtifactType::Original,
                    native_lib: NativeLibOption::default(),
                    strict_mode: StrictModeConfig {
                        case_sensitive_check: false,
                        use_normalized_ohm_url: true,
                        no_external_import_by_path: None,
                        duplicate_dependency_check: false,
                        api_compatibility_check: None,
                    },
                    ark_options: None,
                    res_options: None,
                    source_option: None,
                },
                version_code: None,
                version_name: None,
                build_version: None,
                app_options: Default::default(),
            },
            modules: vec![ModuleConfig {
                name: "entry".to_string(),
                src_path: PathBuf::from("/project/entry"),
                module_type: OhosModuleType::Entry,
            }],
            sdk_config: SdkConfig {
                ets_loader_path: PathBuf::from("/sdk/ets-loader"),
                es2abc_path: PathBuf::from("/sdk/es2abc"),
                restool_path: PathBuf::from("/sdk/restool"),
                compile_api_version: 12,
                sdk_component_api_version: 12,
                has_rollup_plugin: true,
                compile_sdk_version: "6.0.0.47".to_string(),
            },
        }
    }

    fn make_snapshot(source: &str) -> TaskInputSnapshot {
        TaskInputSnapshot {
            source_hash: arkdown_cache::CacheKeyMaterial::compute_content_hash(source),
            resource_hash: arkdown_cache::CacheKeyMaterial::compute_content_hash("resources"),
            dependency_content_hash: arkdown_cache::CacheKeyMaterial::compute_content_hash(
                "dependencies",
            ),
            config_hash: arkdown_cache::CacheKeyMaterial::compute_content_hash("config"),
            profile_hash: arkdown_cache::CacheKeyMaterial::compute_content_hash("profiles"),
            sdk_hash: arkdown_cache::CacheKeyMaterial::compute_content_hash("sdk"),
        }
    }

    #[test]
    fn test_task_inputs_roundtrip() {
        let tmp = std::env::temp_dir().join("arkdown_task_cache_roundtrip");
        let _ = std::fs::remove_dir_all(&tmp);

        let config = make_config();
        let loader = make_loader(
            vec!["./ets/entryability/EntryAbility.ets".to_string()],
            "/tmp/cache",
        );
        let inputs = TaskInputs::compute(&config, &loader, &make_snapshot("entry source"));

        assert!(!inputs.matches_cached(&tmp), "no stored file → cache miss");

        inputs.save(&tmp).unwrap();
        assert!(inputs.matches_cached(&tmp), "after save → cache hit");

        let _ = std::fs::remove_dir_all(&tmp);
    }

    #[test]
    fn task_inputs_do_not_reuse_cache_across_arkts_languages() {
        let tmp = std::env::temp_dir().join("arkdown_task_cache_arkts_language_miss");
        let _ = std::fs::remove_dir_all(&tmp);

        let config = make_config();
        let loader = make_loader(vec!["./ets/Index.ets".to_string()], "/tmp/cache");
        let snapshot = make_snapshot("entry source");
        let arkts_1_1 = TaskInputs::compute(&config, &loader, &snapshot)
            .with_arkts_language_version(ArktsLanguageVersion::V1_1);
        arkts_1_1.save(&tmp).expect("save ArkTS 1.1 inputs");

        let arkts_1_2 = TaskInputs::compute(&config, &loader, &snapshot)
            .with_arkts_language_version(ArktsLanguageVersion::V1_2);
        assert!(
            !arkts_1_2.matches_cached(&tmp),
            "ArkTS 1.2 must not reuse an ArkTS 1.1 task cache"
        );

        let _ = std::fs::remove_dir_all(&tmp);
    }

    #[test]
    fn test_task_inputs_legacy_schema_is_miss() {
        let tmp = std::env::temp_dir().join("arkdown_task_cache_legacy_schema_miss");
        let _ = std::fs::remove_dir_all(&tmp);
        std::fs::create_dir_all(&tmp).unwrap();

        let config = make_config();
        let loader = make_loader(vec!["./ets/Index.ets".to_string()], "/tmp/cache");
        let inputs = TaskInputs::compute(&config, &loader, &make_snapshot("index source"));
        let legacy = serde_json::json!({
          "source_hash": inputs.source_hash,
          "obfuscation_hash": inputs.obfuscation_hash,
          "sdk_version": inputs.sdk_version,
          "compile_env": inputs.compile_env,
          "plugin_hash": inputs.plugin_hash,
          "resource_hash": inputs.resource_hash,
          "dependency_hash": inputs.dependency_hash,
        });
        std::fs::write(
            tmp.join(TASK_INPUTS_FILE),
            serde_json::to_string_pretty(&legacy).unwrap(),
        )
        .unwrap();

        assert!(
            !inputs.matches_cached(&tmp),
            "legacy task inputs without schema version must be a cache miss"
        );

        let _ = std::fs::remove_dir_all(&tmp);
    }

    #[test]
    fn test_task_inputs_different_compile_env_is_miss() {
        let tmp = std::env::temp_dir().join("arkdown_task_cache_env_miss");
        let _ = std::fs::remove_dir_all(&tmp);

        let mut config = make_config();
        let loader = make_loader(vec!["./ets/Index.ets".to_string()], "/tmp/cache");

        let snapshot = make_snapshot("index source");
        let inputs_v1 = TaskInputs::compute(&config, &loader, &snapshot);
        inputs_v1.save(&tmp).unwrap();

        // Bump SDK version → different compile_env
        config.sdk_config.compile_sdk_version = "7.0.0.1".to_string();
        let inputs_v2 = TaskInputs::compute(&config, &loader, &snapshot);

        assert!(
            !inputs_v2.matches_cached(&tmp),
            "changed compile_env must be a cache miss"
        );

        let _ = std::fs::remove_dir_all(&tmp);
    }

    #[test]
    fn test_task_inputs_different_source_hash_is_miss() {
        let tmp = std::env::temp_dir().join("arkdown_task_cache_src_miss");
        let _ = std::fs::remove_dir_all(&tmp);

        let config = make_config();
        let loader = make_loader(vec!["./ets/A.ets".to_string()], "/tmp/cache");
        let inputs_v1 = TaskInputs::compute(&config, &loader, &make_snapshot("const value = 1;"));
        inputs_v1.save(&tmp).unwrap();

        let inputs_v2 = TaskInputs::compute(&config, &loader, &make_snapshot("const value = 2;"));

        assert!(
            !inputs_v2.matches_cached(&tmp),
            "changed source content at the same path must be a cache miss"
        );

        let _ = std::fs::remove_dir_all(&tmp);
    }

    #[test]
    fn test_additional_obfuscation_rule_files_change_hash() {
        let tmp = tempfile::tempdir().expect("tempdir");
        let dep_rules = tmp.path().join("obfuscation.txt");
        std::fs::write(&dep_rules, "-keep-global-name\nDepApi\n").unwrap();
        let config = make_config();
        let loader = make_loader(vec!["./ets/Index.ets".to_string()], "/tmp/cache");

        let base = TaskInputs::compute(&config, &loader, &make_snapshot("index source"));
        let with_dep = base
            .clone()
            .with_additional_obfuscation_rule_files(std::slice::from_ref(&dep_rules));

        assert_ne!(base.obfuscation_hash, with_dep.obfuscation_hash);

        std::fs::write(&dep_rules, "-keep-global-name\nDepApiV2\n").unwrap();
        let changed = base.with_additional_obfuscation_rule_files(std::slice::from_ref(&dep_rules));

        assert_ne!(with_dep.obfuscation_hash, changed.obfuscation_hash);
    }

    #[test]
    fn custom_type_declaration_content_invalidates_compile_cache() {
        let temp = tempfile::tempdir().expect("tempdir");
        let declaration = temp.path().join("runtime.d.ts");
        std::fs::write(
            &declaration,
            "declare interface Runtime { value: string }\n",
        )
        .unwrap();
        let config = make_config();
        let loader = make_loader(vec!["./ets/Index.ets".to_string()], "/tmp/cache");
        let base = TaskInputs::compute(&config, &loader, &make_snapshot("index source"));
        let first = base
            .clone()
            .with_custom_type_inputs(
                &["./types/runtime".to_string()],
                std::slice::from_ref(&declaration),
            )
            .expect("fingerprint custom type");

        std::fs::write(
            &declaration,
            "declare interface Runtime { value: number }\n",
        )
        .unwrap();
        let changed = base
            .with_custom_type_inputs(
                &["./types/runtime".to_string()],
                std::slice::from_ref(&declaration),
            )
            .expect("fingerprint changed custom type");

        assert_ne!(first.custom_types_hash, 0);
        assert_ne!(first.custom_types_hash, changed.custom_types_hash);
    }

    #[test]
    fn effective_module_obfuscation_replaces_product_cache_inputs() {
        let tmp = tempfile::tempdir().expect("tempdir");
        let module_rules = tmp.path().join("module-rules.txt");
        std::fs::write(&module_rules, "-enable-property-obfuscation\n").unwrap();
        let mut config = make_config();
        config.build_mode = BuildMode::Release;
        let loader = make_loader(vec!["./ets/Index.ets".to_string()], "/tmp/cache");
        let module_obfuscation = ObfuscationConfig {
            enable_obfuscation: true,
            rule_files: vec![module_rules.clone()],
            consumer_rule_files: vec![],
        };

        let product_only = TaskInputs::compute(&config, &loader, &make_snapshot("index source"));
        let effective = product_only
            .clone()
            .with_effective_obfuscation(Some(&module_obfuscation), &config.build_mode)
            .with_compile_env("module-release-obfuscation".to_string());
        assert_eq!(product_only.obfuscation_hash, 0);
        assert_ne!(effective.obfuscation_hash, 0);
        assert_eq!(effective.compile_env, "module-release-obfuscation");

        std::fs::write(&module_rules, "-enable-toplevel-obfuscation\n").unwrap();
        let changed =
            product_only.with_effective_obfuscation(Some(&module_obfuscation), &config.build_mode);
        assert_ne!(effective.obfuscation_hash, changed.obfuscation_hash);
    }
}
