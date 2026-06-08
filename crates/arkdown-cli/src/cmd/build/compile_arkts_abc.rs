use std::path::PathBuf;

use super::AbcBackendChoice;
use anyhow::Result;
use arkdown_core::{ArkdownConfig, BuildMode};
use arkdown_ts2abc::{
    compile_to_abc, compile_to_abc_with_backend, SdkEs2abcBackend, Ts2abcInput, Ts2abcOptions,
    Ts2abcOutput,
};

#[derive(Debug, Clone, PartialEq, Eq)]
pub(super) struct AbcCompilePlan {
    backend: AbcBackendChoice,
    sdk_es2abc_path: PathBuf,
    target_api_version: u32,
    target_api_sub_version: Option<String>,
    enable_annotations: bool,
    use_define_semantic: bool,
    debug_info: bool,
    opt_level: u8,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(super) struct AbcCompileRequest {
    pub inputs: Vec<Ts2abcInput>,
    pub output: PathBuf,
    pub input_file_list: Option<PathBuf>,
    pub cache_file: Option<PathBuf>,
    pub npm_module_entry_list: Option<PathBuf>,
}

impl AbcCompilePlan {
    pub(super) fn from_config(config: &ArkdownConfig, backend: AbcBackendChoice) -> Self {
        Self {
            backend,
            sdk_es2abc_path: config.sdk_config.es2abc_path.clone(),
            target_api_version: config.sdk_config.compile_api_version,
            target_api_sub_version: config.product.compatible_sdk_version_stage.clone(),
            // Upstream ets-loader initializes `projectConfig.allowEtsAnnotations`
            // to true and passes `--enable-annotations` for esmodule builds. The
            // ABC backend still gates annotation records by target API.
            enable_annotations: true,
            use_define_semantic: false,
            debug_info: matches!(config.build_mode, BuildMode::Debug),
            opt_level: if matches!(config.build_mode, BuildMode::Release) {
                2
            } else {
                0
            },
        }
    }

    pub(super) fn compile(&self, request: AbcCompileRequest) -> Result<Ts2abcOutput> {
        let input_file_list = request.input_file_list.clone();
        let options = self.ts2abc_options(request);
        match self.backend {
            AbcBackendChoice::Rust => compile_to_abc(options).map_err(Into::into),
            AbcBackendChoice::Sdk => {
                let backend = SdkEs2abcBackend::new(self.sdk_es2abc_path.clone())
                    .with_input_file_list(input_file_list);
                compile_to_abc_with_backend(options, &backend).map_err(Into::into)
            }
            AbcBackendChoice::Auto => {
                if self.sdk_es2abc_path.exists() {
                    let backend = SdkEs2abcBackend::new(self.sdk_es2abc_path.clone())
                        .with_input_file_list(input_file_list);
                    compile_to_abc_with_backend(options, &backend).map_err(Into::into)
                } else {
                    compile_to_abc(options).map_err(Into::into)
                }
            }
        }
    }

    pub(super) fn ts2abc_options(&self, request: AbcCompileRequest) -> Ts2abcOptions {
        Ts2abcOptions {
            inputs: request.inputs,
            output: request.output,
            target_api_version: self.target_api_version,
            target_api_sub_version: self.target_api_sub_version.clone(),
            enable_annotations: self.enable_annotations,
            use_define_semantic: false,
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

    fn config_for(
        compatible_sdk_version: impl Into<String>,
        compile_api_version: u32,
        build_mode: BuildMode,
    ) -> ArkdownConfig {
        ArkdownConfig {
            project_root: PathBuf::from("/project"),
            build_mode,
            output_target: OutputTarget::Hap,
            product: ProductConfig {
                name: "default".to_string(),
                bundle_name: "com.example.app".to_string(),
                compatible_sdk_version: compatible_sdk_version.into(),
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
                compile_api_version,
                has_rollup_plugin: true,
                compile_sdk_version: "6.0.0.0".to_string(),
            },
        }
    }

    fn empty_request() -> AbcCompileRequest {
        AbcCompileRequest {
            inputs: Vec::new(),
            output: PathBuf::from("/out/modules.abc"),
            input_file_list: None,
            cache_file: Some(PathBuf::from("/cache/modules.cache")),
            npm_module_entry_list: Some(PathBuf::from("/cache/npm_entries.txt")),
        }
    }

    fn compile_request(tmp: &tempfile::TempDir) -> AbcCompileRequest {
        let source = tmp.path().join("Index.js");
        fs_err::write(&source, "export const value = 1;").unwrap();
        AbcCompileRequest {
            inputs: vec![Ts2abcInput {
                path: source,
                output: None,
                cache_file: None,
                record_name: "entry|entry|1.0.0|src/main/ets/pages/Index".to_string(),
                source_file: Some("entry/src/main/ets/pages/Index.ts".to_string()),
                package_name: Some("entry".to_string()),
                is_commonjs: false,
                is_shared_module: false,
                origin_source_lang: "ets".to_string(),
                source_map: None,
            }],
            output: tmp.path().join("modules.abc"),
            input_file_list: None,
            cache_file: None,
            npm_module_entry_list: None,
        }
    }

    #[test]
    fn target_api_uses_discovered_sdk_api() {
        let plan = AbcCompilePlan::from_config(
            &config_for("5.0.0(12)", 20, BuildMode::Debug),
            AbcBackendChoice::Rust,
        );
        let options = plan.ts2abc_options(empty_request());

        assert_eq!(options.target_api_version, 20);
        assert!(options.enable_annotations);
        assert!(options.debug_info);
        assert_eq!(options.opt_level, 0);
    }

    #[test]
    fn target_api_falls_back_to_discovered_sdk_api() {
        let plan = AbcCompilePlan::from_config(
            &config_for("6.0.0", 19, BuildMode::Debug),
            AbcBackendChoice::Rust,
        );
        let options = plan.ts2abc_options(empty_request());

        assert_eq!(options.target_api_version, 19);
        assert!(options.enable_annotations);
    }

    #[test]
    fn release_build_preserves_stage_and_optimization() {
        let plan = AbcCompilePlan::from_config(
            &config_for("6.0.0(20)", 20, BuildMode::Release),
            AbcBackendChoice::Rust,
        );
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

    #[test]
    fn auto_backend_falls_back_to_rust_when_sdk_missing() {
        let tmp = tempfile::tempdir().unwrap();
        let plan = AbcCompilePlan::from_config(
            &config_for("6.0.0(20)", 20, BuildMode::Debug),
            AbcBackendChoice::Auto,
        );

        let output = plan.compile(compile_request(&tmp)).unwrap();
        assert!(output.abc_path.exists());
    }

    #[test]
    fn sdk_backend_requires_existing_es2abc() {
        let tmp = tempfile::tempdir().unwrap();
        let plan = AbcCompilePlan::from_config(
            &config_for("6.0.0(20)", 20, BuildMode::Debug),
            AbcBackendChoice::Sdk,
        );

        let err = plan.compile(compile_request(&tmp)).unwrap_err();
        assert!(
            err.to_string()
                .contains("SDK es2abc compiler does not exist"),
            "unexpected error: {err:#}"
        );
    }
}
