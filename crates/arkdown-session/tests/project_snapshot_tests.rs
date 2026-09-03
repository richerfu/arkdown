use arkdown_core::{
    AppOptionOverride, ArkdownConfig, BuildMode, BuildOption, HarArtifactType, ModuleConfig,
    NativeLibOption, OhosModuleType, OutputTarget, ProductConfig, SdkConfig, StrictModeConfig,
};
use arkdown_session::ProjectSnapshot;

fn config(project_root: &std::path::Path, build_version: Option<&str>) -> ArkdownConfig {
    ArkdownConfig {
        project_root: project_root.to_path_buf(),
        build_mode: BuildMode::Debug,
        output_target: OutputTarget::Hap,
        product: ProductConfig {
            name: "default".to_string(),
            artifact_name: None,
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
                    no_external_import_by_path: None,
                    duplicate_dependency_check: false,
                    api_compatibility_check: None,
                },
                ark_options: None,
                res_options: None,
                source_option: None,
                preload_system_so: false,
                preload_so: Vec::new(),
                deduplicate_har: false,
                enable_source_code_check: true,
                app_with_signed_pkg: false,
                generate_shared_tgz: None,
                customized_har_base_package: None,
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
            sdk_component_api_version: 12,
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
                buildVersion: "app-build",
                minCompatibleVersionCode: 42
            }
        }"#,
    )
    .expect("write app.json5");

    let app_fallback =
        ProjectSnapshot::from_config(&config(temp.path(), None)).expect("snapshot from app value");
    assert_eq!(app_fallback.build_version.as_deref(), Some("app-build"));
    assert_eq!(app_fallback.min_compatible_version_code, Some(42));

    let product_override =
        ProjectSnapshot::from_config(&config(temp.path(), Some("product-build")))
            .expect("snapshot from product value");
    assert_eq!(
        product_override.build_version.as_deref(),
        Some("product-build")
    );
}

#[test]
fn project_parameter_file_matches_latest_hvigor_missing_file_and_cli_override_precedence() {
    let temp = tempfile::tempdir().expect("tempdir");
    std::fs::write(
        temp.path().join("oh-package.json5"),
        r#"{
            "modelVersion": "6.0.0",
            "name": "app",
            "parameterFile": "missing-params.json5"
        }"#,
    )
    .expect("write project package");

    let error = ProjectSnapshot::from_config(&config(temp.path(), None))
        .expect_err("latest hvigor rejects a configured missing parameter file");
    assert!(error.to_string().contains("missing-params.json5"));

    std::fs::write(
        temp.path().join("params.json5"),
        r#"{ "versions.lib": "1.0.0" }"#,
    )
    .expect("write CLI parameter file");
    ProjectSnapshot::from_config_with_parameter_file(
        &config(temp.path(), None),
        Some("params.json5".into()),
    )
    .expect("CLI parameterFile overrides the project declaration");
}

#[test]
fn api8_project_uses_package_json_and_node_modules_for_dependency_snapshots() {
    let temp = tempfile::tempdir().expect("tempdir");
    let module_root = temp.path().join("entry");
    let dependency_root = temp.path().join("node_modules").join("legacy-har");
    std::fs::create_dir_all(module_root.join("src/main/ets")).expect("module source");
    std::fs::create_dir_all(dependency_root.join("src/main")).expect("dependency profile root");
    std::fs::write(
        temp.path().join("package.json"),
        r#"{
            "name": "legacy-project",
            "parameterFile": "missing-npm-parameter-file.json5"
        }"#,
    )
    .expect("project package.json");
    std::fs::write(
        module_root.join("package.json"),
        r#"{
            "name": "entry",
            "dependencies": { "legacy-har": "1.0.0" }
        }"#,
    )
    .expect("module package.json");
    std::fs::write(
        dependency_root.join("package.json"),
        r#"{
            "name": "legacy-har",
            "version": "1.0.0",
            "ohos": {}
        }"#,
    )
    .expect("dependency package.json");
    std::fs::write(
        dependency_root.join("src/main/module.json5"),
        r#"{ module: { name: "legacy-har", type: "har" } }"#,
    )
    .expect("dependency module profile");

    let mut project_config = config(temp.path(), None);
    project_config.sdk_config.compile_api_version = 8;
    project_config.modules.push(ModuleConfig {
        name: "entry".to_string(),
        src_path: module_root.clone(),
        module_type: OhosModuleType::Entry,
    });

    let snapshot = ProjectSnapshot::from_config(&project_config)
        .expect("npm projects do not initialize OHPM parameter files");
    let module = snapshot.modules.get("entry").expect("entry snapshot");
    assert_eq!(
        module.package_path.as_deref(),
        Some(module_root.join("package.json").as_path())
    );
    let graph = snapshot
        .dependency_graph("entry")
        .expect("dependency graph");
    let dependency = graph.nodes.get("legacy-har").expect("npm HAR dependency");
    assert!(graph
        .har_nodes()
        .any(|node| node.package_name == "legacy-har"));
    assert_eq!(
        dependency.package_root.as_deref(),
        Some(
            dependency_root
                .canonicalize()
                .expect("canonical dependency")
                .as_path()
        )
    );
    assert!(graph.dependency_parameters.source_path.is_none());
}
