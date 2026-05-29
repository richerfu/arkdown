use std::io;
use std::path::Path;

use arkdown_cache::compute_compile_env;
use arkdown_core::ArkdownConfig;
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
    pub source_hash: u64,
    pub obfuscation_hash: u64,
    pub sdk_version: String,
    pub compile_env: String,
}

const TASK_INPUTS_FILE: &str = ".task_inputs";

impl TaskInputs {
    /// Compute current task inputs from config and loader data.
    pub fn compute(config: &ArkdownConfig, loader: &LoaderJson) -> Self {
        let source_hash = hash_source_files(&loader.compile_entry);
        let obfuscation_hash = compute_obfuscation_hash(config);
        let sdk_version = config.sdk_config.compile_sdk_version.clone();
        let compile_env = compute_compile_env(config, &loader.cache_path);
        TaskInputs {
            source_hash,
            obfuscation_hash,
            sdk_version,
            compile_env,
        }
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
        stored.source_hash == self.source_hash
            && stored.obfuscation_hash == self.obfuscation_hash
            && stored.sdk_version == self.sdk_version
            && stored.compile_env == self.compile_env
    }

    /// Persist `self` to `cache_dir/.task_inputs` for the next build.
    pub fn save(&self, cache_dir: &Path) -> io::Result<()> {
        std::fs::create_dir_all(cache_dir)?;
        let stored = StoredTaskInputs {
            source_hash: self.source_hash,
            obfuscation_hash: self.obfuscation_hash,
            sdk_version: self.sdk_version.clone(),
            compile_env: self.compile_env.clone(),
        };
        let json = serde_json::to_string_pretty(&stored)
            .map_err(|e| io::Error::new(io::ErrorKind::Other, e))?;
        std::fs::write(cache_dir.join(TASK_INPUTS_FILE), json)
    }
}

#[derive(serde::Serialize, serde::Deserialize)]
struct StoredTaskInputs {
    source_hash: u64,
    obfuscation_hash: u64,
    sdk_version: String,
    compile_env: String,
}

/// FNV-1a 64-bit hash of all source file paths in declaration order.
fn hash_source_files(compile_entry: &[String]) -> u64 {
    let mut h: u64 = 0xcbf29ce484222325;
    for path in compile_entry {
        // Hash the path string itself; a content-hash would require disk I/O
        // here but the source_hash in CacheKey already covers content.
        for b in path.bytes() {
            h ^= b as u64;
            h = h.wrapping_mul(0x100000001b3);
        }
        // Separator between paths
        h ^= b'|' as u64;
        h = h.wrapping_mul(0x100000001b3);
    }
    h
}

/// Derive a stable obfuscation hash from the config's obfuscation settings.
///
/// Returns 0 when obfuscation is disabled (no rules file to hash).
fn compute_obfuscation_hash(config: &ArkdownConfig) -> u64 {
    let Some(ark) = config.product.build_option.ark_options.as_ref() else {
        return 0;
    };
    let Some(obf) = ark.obfuscation.as_ref() else {
        return 0;
    };
    if !obf.enable_obfuscation {
        return 0;
    }
    let Some(rules_path) = obf.rules_file.as_ref() else {
        return 0;
    };
    let Ok(content) = std::fs::read(rules_path) else {
        return 0;
    };
    let mut h: u64 = 0xcbf29ce484222325;
    for b in content {
        h ^= b as u64;
        h = h.wrapping_mul(0x100000001b3);
    }
    h
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
            an_build_out_put: String::new(),
            an_build_mode: String::new(),
            non_entry_importees: vec![],
            compiled_importees: vec![],
            compile_entry,
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
                runtime_os: "OpenHarmony".to_string(),
                signing_config: None,
                build_option: BuildOption {
                    strict_mode: StrictModeConfig {
                        case_sensitive_check: false,
                        use_normalized_ohm_url: true,
                    },
                    ark_options: None,
                    res_options: None,
                    source_option: None,
                },
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
