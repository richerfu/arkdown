use std::io;
use std::path::Path;

use arkdown_core::ArkdownConfig;
use arkdown_path::ArtifactPathSerializer;
use arkdown_profile::LoaderJson;

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
  pub source_hash: u64,
  pub obfuscation_hash: u64,
  pub sdk_version: String,
  pub compile_env: String,
  pub plugin_hash: u64,
  pub resource_hash: u64,
  /// FNV-1a hash of the module dependency graph identity (hsp/har ohm maps).
  /// Mirrors hvigor's inclusion of dependency changes in ArkCompile cache inputs.
  pub dependency_hash: u64,
}

const TASK_INPUTS_FILE: &str = ".task_inputs";
const TASK_INPUTS_SCHEMA_VERSION: u32 = 2;

impl TaskInputs {
  /// Compute current task inputs from config and loader data.
  pub fn compute(config: &ArkdownConfig, loader: &LoaderJson) -> Self {
    let source_hash = Self::hash_source_files(&loader.compile_entry);
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
      source_hash,
      obfuscation_hash,
      sdk_version,
      compile_env,
      plugin_hash: 0,
      resource_hash: 0,
      dependency_hash,
    }
  }

  /// Builder: set the plugin hash (FNV-1a of package plugin identities).
  pub fn with_plugin_hash(mut self, h: u64) -> Self {
    self.plugin_hash = h;
    self
  }

  /// Builder: set the resource hash (FNV-1a of resource dir state).
  pub fn with_resource_hash(mut self, h: u64) -> Self {
    self.resource_hash = h;
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
      && stored.plugin_hash == self.plugin_hash
      && stored.resource_hash == self.resource_hash
      && stored.dependency_hash == self.dependency_hash
  }

  /// Persist `self` to `cache_dir/.task_inputs` for the next build.
  pub fn save(&self, cache_dir: &Path) -> io::Result<()> {
    std::fs::create_dir_all(cache_dir)?;
    let stored = StoredTaskInputs {
      schema_version: self.schema_version,
      source_hash: self.source_hash,
      obfuscation_hash: self.obfuscation_hash,
      sdk_version: self.sdk_version.clone(),
      compile_env: self.compile_env.clone(),
      plugin_hash: self.plugin_hash,
      resource_hash: self.resource_hash,
      dependency_hash: self.dependency_hash,
    };
    let json =
      serde_json::to_string_pretty(&stored).map_err(|e| io::Error::new(io::ErrorKind::Other, e))?;
    std::fs::write(cache_dir.join(TASK_INPUTS_FILE), json)
  }
}

#[derive(serde::Serialize, serde::Deserialize)]
struct StoredTaskInputs {
  #[serde(default)]
  schema_version: u32,
  source_hash: u64,
  obfuscation_hash: u64,
  sdk_version: String,
  compile_env: String,
  #[serde(default)]
  plugin_hash: u64,
  #[serde(default)]
  resource_hash: u64,
  #[serde(default)]
  dependency_hash: u64,
}

impl TaskInputs {
  /// FNV-1a 64-bit hash of all source file paths in declaration order.
  fn hash_source_files(compile_entry: &[String]) -> u64 {
    let mut h: u64 = 0xcbf29ce484222325;
    for path in compile_entry {
      for b in path.bytes() {
        h ^= b as u64;
        h = h.wrapping_mul(0x100000001b3);
      }
      h ^= b'|' as u64;
      h = h.wrapping_mul(0x100000001b3);
    }
    h
  }

  /// Derive a stable obfuscation hash from the config's obfuscation settings.
  ///
  /// Hashes the content of ALL rule files (not just the first) so that a
  /// change to any rule file invalidates the ABC compile cache.
  ///
  /// Returns 0 when obfuscation is disabled or no rule files are configured.
  fn compute_obfuscation_hash(config: &ArkdownConfig) -> u64 {
    let Some(ark) = config.product.build_option.ark_options.as_ref() else {
      return 0;
    };
    let Some(obf) = ark.obfuscation.as_ref() else {
      return 0;
    };
    if !obf.enable_obfuscation || obf.rule_files.is_empty() {
      return 0;
    }
    let mut h: u64 = 0xcbf29ce484222325;
    // Hash all rule files in order — a change to any file invalidates the cache.
    for rules_path in &obf.rule_files {
      let Ok(content) = std::fs::read(rules_path) else {
        continue;
      };
      for b in content {
        h ^= b as u64;
        h = h.wrapping_mul(0x100000001b3);
      }
      // Separator between files so that [A, AB] ≠ [AA, B].
      h ^= b'\0' as u64;
      h = h.wrapping_mul(0x100000001b3);
    }
    h
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
      an_build_out_put: None,
      an_build_mode: None,
      ap_path: None,
      build_config_path: None,
      patch_config: None,
      non_entry_importees: vec![],
      compiled_importees: vec![],
      compile_entry,
      runtime_entry_sources: vec![],
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
      custom_types: vec![],
      dependency_pkg_version: vec![],
      extra_importees: vec![],
      compiled_package_names: vec![],
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
        bundle_name: "com.example.test".to_string(),
        compatible_sdk_version: "12".to_string(),
        compatible_sdk_version_stage: None,
        runtime_os: "OpenHarmony".to_string(),
        signing_config: None,
        build_option: BuildOption {
          preload_system_so: false,
          preload_so: vec![],
          remove_permissions: vec![],
          aot_compile_mode: None,
          host_pgo: None,
          ap_path: None,
          abi_filters: vec![],
          native_lib: NativeLibOption::default(),
          strict_mode: StrictModeConfig {
            case_sensitive_check: false,
            use_normalized_ohm_url: true,
          },
          ark_options: None,
          res_options: None,
          source_option: None,
        },
        version_code: None,
        version_name: None,
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
        has_rollup_plugin: true,
        compile_sdk_version: "6.0.0.47".to_string(),
      },
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
    let inputs = TaskInputs::compute(&config, &loader);

    assert!(!inputs.matches_cached(&tmp), "no stored file → cache miss");

    inputs.save(&tmp).unwrap();
    assert!(inputs.matches_cached(&tmp), "after save → cache hit");

    let _ = std::fs::remove_dir_all(&tmp);
  }

  #[test]
  fn test_task_inputs_legacy_schema_is_miss() {
    let tmp = std::env::temp_dir().join("arkdown_task_cache_legacy_schema_miss");
    let _ = std::fs::remove_dir_all(&tmp);
    std::fs::create_dir_all(&tmp).unwrap();

    let config = make_config();
    let loader = make_loader(vec!["./ets/Index.ets".to_string()], "/tmp/cache");
    let inputs = TaskInputs::compute(&config, &loader);
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

    let inputs_v1 = TaskInputs::compute(&config, &loader);
    inputs_v1.save(&tmp).unwrap();

    // Bump SDK version → different compile_env
    config.sdk_config.compile_sdk_version = "7.0.0.1".to_string();
    let inputs_v2 = TaskInputs::compute(&config, &loader);

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
    let loader_v1 = make_loader(vec!["./ets/A.ets".to_string()], "/tmp/cache");
    let inputs_v1 = TaskInputs::compute(&config, &loader_v1);
    inputs_v1.save(&tmp).unwrap();

    let loader_v2 = make_loader(
      vec!["./ets/A.ets".to_string(), "./ets/B.ets".to_string()],
      "/tmp/cache",
    );
    let inputs_v2 = TaskInputs::compute(&config, &loader_v2);

    assert!(
      !inputs_v2.matches_cached(&tmp),
      "added source file must be a cache miss"
    );

    let _ = std::fs::remove_dir_all(&tmp);
  }
}
