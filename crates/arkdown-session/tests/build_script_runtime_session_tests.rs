use arkdown_config::{
    BuildScriptRuntimeModulePlanRequest, BuildScriptRuntimePlanRequest,
    HvigorRuntimeParameterPlanRequest,
};
use arkdown_session::{
    BuildScriptRuntimeModelLoader, BuildScriptRuntimeModelRequest, BuildScriptRuntimePlanUpdate,
    BuildScriptRuntimeSdkConfig, BuildScriptRuntimeSession,
};
use serde_json::{json, Map};

#[test]
fn runtime_session_replans_live_module_overrides_without_returning_ownership_to_javascript() {
    let temp = tempfile::tempdir().expect("tempdir");
    let project_root = temp.path().join("project");
    let module_root = project_root.join("entry");
    std::fs::create_dir_all(module_root.join("src/main")).expect("create module");
    std::fs::write(
        project_root.join("build-profile.json5"),
        r#"{
            app: { products: [{ name: 'default' }] },
            modules: [{
                name: 'entry',
                srcPath: './entry',
                targets: [{ name: 'default', applyToProducts: ['default'] }]
            }]
        }"#,
    )
    .expect("write project profile");
    std::fs::write(
        project_root.join("oh-package.json5"),
        r#"{
            name: 'project',
            parameterFile: 'params/${flavor.name}-${enabled}.json5',
            properties: { flavor: { name: 'phone' }, enabled: true }
        }"#,
    )
    .expect("write project package");
    std::fs::write(
        module_root.join("build-profile.json5"),
        r#"{
            buildOption: { arkOptions: { supportChunks: false } },
            targets: [{ name: 'default' }]
        }"#,
    )
    .expect("write module profile");
    std::fs::write(
        module_root.join("src/main/module.json5"),
        r#"{ module: { type: 'entry' } }"#,
    )
    .expect("write module json");
    std::fs::write(
        module_root.join("oh-package.json5"),
        r#"{ name: 'entry-ohpm' }"#,
    )
    .expect("write module package");

    let model = BuildScriptRuntimeModelLoader::new(BuildScriptRuntimeModelRequest {
        project_root: project_root.clone(),
        hvigor_user_home: None,
    })
    .load()
    .expect("load runtime model");
    let mut session = BuildScriptRuntimeSession::new(
        model,
        BuildScriptRuntimePlanRequest {
            project_root: project_root.clone(),
            product_name: "default".to_string(),
            build_mode_name: "debug".to_string(),
            build_target_name: Some("default".to_string()),
            command_entry_tasks: vec!["assembleHap".to_string()],
            project_kind: Some("app".to_string()),
            ..Default::default()
        },
        HvigorRuntimeParameterPlanRequest {
            user_config: json!({}),
            project_config: json!({}),
            runtime_config: json!({ "parallel": false }),
            command_entry_tasks: Vec::new(),
            workspace_dir: project_root.clone(),
        },
        false,
    );

    assert_eq!(
        session.parameter_plan().core_start_params["parallelExecution"],
        false
    );

    let relative_cache = session.runtime_path_plan(
        json!({ "hvigor.cacheDir": "relative-cache", "ohos.buildDir": "build-output" })
            .as_object()
            .expect("properties"),
        json!({ "parameterFile": "", "previewMode": "true", "isLocalTest": "true" })
            .as_object()
            .expect("ext params"),
    );
    assert_eq!(
        relative_cache.parameter_file_override.as_deref(),
        Some("params/phone-true.json5")
    );
    assert_eq!(
        relative_cache.hvigor_cache_dir,
        project_root.join(".hvigor")
    );
    assert_eq!(
        relative_cache.hvigor_cache_warning.as_deref(),
        Some("Invalid custom hvigor data dir:relative-cache")
    );
    assert_eq!(
        relative_cache.configured_build_dir.as_deref(),
        Some("build-output")
    );
    assert!(relative_cache.preview_mode);
    assert!(relative_cache.local_test);
    assert!(!relative_cache.ohos_test);

    let custom_cache_root = temp.path().join("custom-cache");
    std::fs::create_dir_all(&custom_cache_root).expect("create custom cache");
    let absolute_cache = session.runtime_path_plan(
        &Map::new(),
        json!({ "build-cache-dir": custom_cache_root })
            .as_object()
            .expect("ext params"),
    );
    assert_eq!(
        absolute_cache.hvigor_cache_dir,
        std::fs::canonicalize(&custom_cache_root)
            .expect("canonical custom cache")
            .join("project")
            .join(".hvigor")
    );
    assert_eq!(
        absolute_cache.configured_cache_dir.as_deref(),
        custom_cache_root.to_str()
    );
    assert_eq!(
        absolute_cache.configured_build_dir.as_deref(),
        custom_cache_root.to_str()
    );

    let descriptor = session
        .module_descriptor(&module_root)
        .expect("prepared descriptor");
    assert_eq!(descriptor["modulePackage"]["name"], "entry-ohpm");
    let package_mutation = session
        .set_context_module_version(&module_root, json!("2.0.0"))
        .expect("update module version through the unified context");
    assert_eq!(package_mutation.value["version"], "2.0.0");
    assert_eq!(
        session
            .module_descriptor(&module_root)
            .expect("updated descriptor")["modulePackage"]["version"],
        "2.0.0"
    );
    let mut project_profile = session.project_model().project_profile.clone();
    project_profile["app"]["buildOption"] = json!({ "packOptions": { "fastBuildApp": true } });
    let profile_mutation = session
        .set_context_project_build_profile(project_profile)
        .expect("update project profile through the unified context");
    assert_eq!(
        profile_mutation.selected_product_name.as_deref(),
        Some("default")
    );
    assert_eq!(
        session.project_model().project_profile["app"]["buildOption"]["packOptions"]
            ["fastBuildApp"],
        true
    );
    assert!(session.runtime_context_overrides().is_some());
    let descriptor = session
        .module_descriptor(&module_root)
        .expect("descriptor after context mutations");
    session.upsert_module(BuildScriptRuntimeModulePlanRequest {
        name: "entry".to_string(),
        root: module_root.clone(),
        descriptor: descriptor.clone(),
        build_profile: descriptor["moduleProfile"].clone(),
        build_script_override: None,
        kind: Some("hap".to_string()),
        package: descriptor["modulePackage"].clone(),
    });
    session.set_module_override(
        &module_root,
        Some(json!({ "arkOptions": { "supportChunks": true } })),
    );

    let update = || BuildScriptRuntimePlanUpdate {
        product_name: "default".to_string(),
        build_mode_name: "debug".to_string(),
        module_selection: Some("entry@default".to_string()),
        properties: Map::new(),
        ext_params: Map::new(),
        start_params: Map::new(),
        sync_version_changed: false,
    };
    session.plan(update()).expect("plan overridden module");
    assert_eq!(
        session
            .module_plan(&module_root)
            .expect("module plan")
            .build_options["default"]["arkOptions"]["supportChunks"],
        true
    );
    let command = session
        .command_snapshot()
        .expect("command input is projected by the runtime session");
    assert_eq!(command.project_name, "project");
    assert_eq!(command.task_names, ["assembleHap"]);
    assert_eq!(command.module_selections.len(), 1);
    assert_eq!(command.module_selections[0].module_name, "entry");
    assert_eq!(
        command.module_selections[0].target_name.as_deref(),
        Some("default")
    );
    assert_eq!(command.build_target_name.as_deref(), Some("default"));

    let clean = session
        .clean_snapshot(&module_root, false)
        .expect("clean input is projected by the runtime session");
    assert_eq!(clean.project_root, project_root);
    assert_eq!(clean.node_root, module_root);
    assert!(!clean.project_task);
    assert_eq!(clean.target_names, ["default"]);
    assert_eq!(clean.build_mode, "debug");
    let sync = session
        .sync_snapshot()
        .expect("project sync state is projected by the runtime session");
    assert_eq!(sync.project_name, "project");
    assert_eq!(sync.selected_product_name, "default");
    assert_eq!(sync.modules.len(), 1);
    assert_eq!(sync.modules[0].name, "entry");
    assert_eq!(sync.modules[0].kind, "hap");
    assert_eq!(sync.modules[0].selected_targets[0].name, "default");
    assert_eq!(
        sync.modules[0].selected_targets[0].build_option["arkOptions"]["supportChunks"],
        true
    );

    session.set_module_override(&module_root, None);
    session.plan(update()).expect("replan module");
    assert_eq!(
        session
            .module_plan(&module_root)
            .expect("module plan")
            .build_options["default"]["arkOptions"]["supportChunks"],
        false
    );

    let sdk_details = session
        .sdk_details(&BuildScriptRuntimeSdkConfig {
            sdk_path: Some(temp.path().join("sdk")),
            runtime_os: Some("HarmonyOS".to_string()),
            ets_component_version: Some("6.1.1".to_string()),
            ..Default::default()
        })
        .expect("project SDK details");
    assert!(!sdk_details.is_ohos);
    assert_eq!(sdk_details.sdk_dir, temp.path().join("sdk"));
    assert_eq!(sdk_details.ets_component_version, "6.1.1");
    assert_eq!(sdk_details.ets_component_release_type, "Release");
}
