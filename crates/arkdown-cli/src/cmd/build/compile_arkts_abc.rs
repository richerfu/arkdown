use std::path::PathBuf;

use arkdown_core::{ArkdownConfig, BuildMode};
use arkdown_ts2abc::{Ts2abcInput, Ts2abcOptions};

const ANNOTATION_SUPPORTED_API_VERSION: u32 = 20;

#[derive(Debug, Clone, PartialEq, Eq)]
pub(super) struct AbcCompilePlan {
    target_api_version: u32,
    target_api_sub_version: Option<String>,
    enable_annotations: bool,
    debug_info: bool,
    opt_level: u8,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(super) struct AbcCompileRequest {
    pub inputs: Vec<Ts2abcInput>,
    pub output: PathBuf,
    pub cache_file: Option<PathBuf>,
    pub npm_module_entry_list: Option<PathBuf>,
}

impl AbcCompilePlan {
    pub(super) fn from_config(config: &ArkdownConfig) -> Self {
        let target_api_version = config.sdk_config.compile_api_version;

        Self {
            target_api_version,
            target_api_sub_version: config.product.compatible_sdk_version_stage.clone(),
            enable_annotations: target_api_version >= ANNOTATION_SUPPORTED_API_VERSION,
            debug_info: matches!(config.build_mode, BuildMode::Debug),
            opt_level: if matches!(config.build_mode, BuildMode::Release) {
                2
            } else {
                0
            },
        }
    }

    pub(super) fn ts2abc_options(&self, request: AbcCompileRequest) -> Ts2abcOptions {
        Ts2abcOptions {
            inputs: request.inputs,
            output: request.output,
            target_api_version: self.target_api_version,
            target_api_sub_version: self.target_api_sub_version.clone(),
            enable_annotations: self.enable_annotations,
            debug_info: self.debug_info,
            opt_level: self.opt_level,
            optimize_try_catch_func: true,
            merge_abc: true,
            module: true,
            commonjs: false,
            input_extension: "js".to_string(),
            module_record_field_name: None,
            cache_file: request.cache_file,
            output_proto: None,
            compile_context_info: Default::default(),
            enable_abc_input: false,
            dump_deps_info: false,
            remove_redundant_file: false,
            npm_module_entry_list: request.npm_module_entry_list,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use arkdown_core::{
        ArkOptions, BuildOption, OutputTarget, ProductConfig, SdkConfig, StrictModeConfig,
    };

    fn config_for(api: u32, build_mode: BuildMode) -> ArkdownConfig {
        ArkdownConfig {
            project_root: PathBuf::from("/project"),
            build_mode,
            output_target: OutputTarget::Hap,
            product: ProductConfig {
                name: "default".to_string(),
                bundle_name: "com.example.app".to_string(),
                compatible_sdk_version: format!("6.0.0({api})"),
                compatible_sdk_version_stage: Some("beta".to_string()),
                runtime_os: "HarmonyOS".to_string(),
                signing_config: None,
                build_option: BuildOption {
                    strict_mode: StrictModeConfig {
                        case_sensitive_check: false,
                        use_normalized_ohm_url: false,
                    },
                    ark_options: Some(ArkOptions {
                        obfuscation: None,
                        types: Vec::new(),
                        runtime_only: None,
                        support_chunks: false,
                    }),
                    res_options: None,
                    source_option: None,
                    preload_system_so: false,
                    preload_so: Vec::new(),
                    remove_permissions: Vec::new(),
                    aot_compile_mode: None,
                    host_pgo: None,
                    ap_path: None,
                    abi_filters: vec!["arm64-v8a".to_string()],
                },
                version_code: None,
                version_name: None,
            },
            modules: Vec::new(),
            sdk_config: SdkConfig {
                ets_loader_path: PathBuf::from("/sdk/ets-loader"),
                es2abc_path: PathBuf::from("/sdk/es2abc"),
                restool_path: PathBuf::from("/sdk/restool"),
                compile_api_version: api,
                has_rollup_plugin: true,
                compile_sdk_version: "6.0.0.0".to_string(),
            },
        }
    }

    fn empty_request() -> AbcCompileRequest {
        AbcCompileRequest {
            inputs: Vec::new(),
            output: PathBuf::from("/out/modules.abc"),
            cache_file: Some(PathBuf::from("/cache/modules.cache")),
            npm_module_entry_list: Some(PathBuf::from("/cache/npm_entries.txt")),
        }
    }

    #[test]
    fn api_below_20_keeps_annotations_disabled() {
        let plan = AbcCompilePlan::from_config(&config_for(19, BuildMode::Debug));
        let options = plan.ts2abc_options(empty_request());

        assert_eq!(options.target_api_version, 19);
        assert!(!options.enable_annotations);
        assert!(options.debug_info);
        assert_eq!(options.opt_level, 0);
    }

    #[test]
    fn api_20_enables_annotations_and_preserves_stage() {
        let plan = AbcCompilePlan::from_config(&config_for(20, BuildMode::Release));
        let options = plan.ts2abc_options(empty_request());

        assert_eq!(options.target_api_version, 20);
        assert_eq!(options.target_api_sub_version.as_deref(), Some("beta"));
        assert!(options.enable_annotations);
        assert!(!options.debug_info);
        assert_eq!(options.opt_level, 2);
        assert_eq!(
            options.cache_file.as_deref(),
            Some(std::path::Path::new("/cache/modules.cache"))
        );
        assert_eq!(
            options.npm_module_entry_list.as_deref(),
            Some(std::path::Path::new("/cache/npm_entries.txt"))
        );
    }
}
