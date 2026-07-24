use arkdown_core::{
    AppOptionOverride, ArkdownConfig, BuildMode, BuildOption, HarArtifactType, NativeLibOption,
    OutputTarget, ProductConfig, SdkConfig, StrictModeConfig,
};
use arkdown_session::ProjectSnapshot;

fn config(project_root: &std::path::Path, build_version: Option<&str>) -> ArkdownConfig {
    ArkdownConfig {
        project_root: project_root.to_path_buf(),
        build_mode: BuildMode::Debug,
        output_target: OutputTarget::Hap,
        product: ProductConfig {
            name: "default".to_string(),
            bundle_name: String::new(),
            compatible_sdk_version: "12".to_string(),
            compatible_sdk_version_stage: None,
            target_sdk_version: None,
            runtime_os: "HarmonyOS".to_string(),
            signing_config: None,
            build_option: BuildOption {
                strict_mode: StrictModeConfig {
                    case_sensitive_check: false,
                    use_normalized_ohm_url: true,
                },
                ark_options: None,
                res_options: None,
                source_option: None,
                preload_system_so: false,
                preload_so: Vec::new(),
                deduplicate_har: false,
                remove_permissions: Vec::new(),
                aot_compile_mode: None,
                host_pgo: None,
                ap_path: None,
                abi_filters: Vec::new(),
                external_native_options: None,
                har_artifact_type: HarArtifactType::Original,
                native_lib: NativeLibOption::default(),
            },
            version_code: None,
            version_name: None,
            build_version: build_version.map(str::to_string),
            app_options: AppOptionOverride::default(),
        },
        modules: Vec::new(),
        sdk_config: SdkConfig {
            ets_loader_path: project_root.join("sdk/ets-loader"),
            es2abc_path: project_root.join("sdk/es2abc"),
            restool_path: project_root.join("sdk/restool"),
            compile_api_version: 12,
            has_rollup_plugin: true,
            compile_sdk_version: "6.0.0.47".to_string(),
        },
    }
}

#[test]
fn project_snapshot_matches_hvigor_build_version_product_then_app_precedence() {
    let temp = tempfile::tempdir().expect("tempdir");
    let app_scope = temp.path().join("AppScope");
    std::fs::create_dir_all(&app_scope).expect("create AppScope");
    std::fs::write(
        app_scope.join("app.json5"),
        r#"{
            app: {
                bundleName: "com.example.app",
                versionCode: 1000000,
                versionName: "1.0.0",
                buildVersion: "app-build"
            }
        }"#,
    )
    .expect("write app.json5");

    let app_fallback =
        ProjectSnapshot::from_config(&config(temp.path(), None)).expect("snapshot from app value");
    assert_eq!(app_fallback.build_version.as_deref(), Some("app-build"));

    let product_override =
        ProjectSnapshot::from_config(&config(temp.path(), Some("product-build")))
            .expect("snapshot from product value");
    assert_eq!(
        product_override.build_version.as_deref(),
        Some("product-build")
    );
}
