use arkdown_config::{
    BuildScriptRuntimeModulePlanRequest, BuildScriptRuntimePlanRequest, BuildScriptRuntimePlanner,
};
use serde_json::json;

#[test]
fn runtime_planner_selects_targets_and_merges_build_options_in_rust() {
    let temp = tempfile::tempdir().expect("tempdir");
    let module_root = temp.path().join("entry");
    std::fs::create_dir_all(&module_root).expect("create module");
    let request = BuildScriptRuntimePlanRequest {
        project_root: temp.path().to_path_buf(),
        project_profile: json!({
            "app": {
                "products": [{
                    "name": "default",
                    "buildOption": { "arkOptions": { "runtimeOnly": { "packages": ["product"] } } }
                }],
                "buildModeSet": [{
                    "name": "debug",
                    "buildOption": { "strictMode": { "caseSensitiveCheck": false } }
                }]
            },
            "modules": [{
                "name": "entry",
                "srcPath": "./entry",
                "targets": [
                    { "name": "default", "applyToProducts": ["default"] },
                    {
                        "name": "phone",
                        "applyToProducts": ["default"],
                        "buildModeSet": [{ "name": "debug" }],
                        "output": { "artifactName": "phone-artifact" }
                    }
                ]
            }]
        }),
        modules: vec![BuildScriptRuntimeModulePlanRequest {
            name: "entry".to_string(),
            root: module_root.clone(),
            descriptor: json!({
                "targets": [
                    { "name": "default", "applyToProducts": ["default"] },
                    {
                        "name": "phone",
                        "applyToProducts": ["default"],
                        "buildModeSet": [{ "name": "debug" }],
                        "output": { "artifactName": "phone-artifact" }
                    }
                ]
            }),
            build_profile: json!({
                "buildOption": { "resOptions": { "directories": ["resources/base"] } },
                "buildOptionSet": [
                    { "name": "base", "arkOptions": { "supportChunks": true } },
                    {
                        "name": "debug-phone",
                        "copyFrom": "base",
                        "strictMode": { "caseSensitiveCheck": true }
                    }
                ],
                "buildModeBinder": [{
                    "buildModeName": "debug",
                    "mappings": [{ "targetName": "phone", "buildOptionName": "debug-phone" }]
                }],
                "targets": [{
                    "name": "phone",
                    "config": { "buildOption": { "preloadSystemSo": true } }
                }]
            }),
            build_script_override: Some(json!({
                "strictMode": { "useNormalizedOHMUrl": true }
            })),
            kind: Some("hap".to_string()),
            package: json!({ "name": "entry" }),
            ..Default::default()
        }],
        product_name: "default".to_string(),
        build_mode_name: "debug".to_string(),
        module_selection: Some("entry@phone".to_string()),
        build_target_name: None,
        device_test: false,
        project_oh_package_exists: true,
        project_kind: Some("app".to_string()),
        command_entry_tasks: vec!["entry:assembleHap".to_string()],
        ext_params: json!({ "ohos-enable-binxo": "true" })
            .as_object()
            .cloned()
            .expect("ext params"),
        ..Default::default()
    };

    let plan = BuildScriptRuntimePlanner::new(request)
        .plan()
        .expect("plan runtime model");

    assert_eq!(plan.module_selections.len(), 1);
    let module = &plan.modules[0];
    assert_eq!(module.selected_targets.len(), 1);
    assert_eq!(module.selected_targets[0]["name"], "phone");
    assert_eq!(
        module.effective_target.as_ref().expect("effective target")["name"],
        "phone"
    );
    assert_eq!(
        module.selected_targets[0]["output"]["artifactName"],
        "phone-artifact"
    );
    let option = &module.build_options["phone"];
    assert_eq!(
        option["arkOptions"]["runtimeOnly"]["packages"][0],
        "product"
    );
    assert_eq!(option["arkOptions"]["supportChunks"], true);
    assert_eq!(option["strictMode"]["caseSensitiveCheck"], true);
    assert_eq!(option["strictMode"]["useNormalizedOHMUrl"], true);
    assert_eq!(option["preloadSystemSo"], true);
    assert_eq!(
        option["resOptions"]["directories"][0],
        module_root
            .join("resources/base")
            .to_string_lossy()
            .as_ref()
    );
    assert!(option.get("name").is_none());
    assert!(option.get("copyFrom").is_none());
    let project_catalog = plan
        .task_catalogs
        .iter()
        .find(|catalog| catalog.project_task)
        .expect("project task catalog");
    assert_eq!(project_catalog.kind.as_deref(), Some("app"));
    assert_eq!(
        project_catalog.child_modules[0].kind.as_deref(),
        Some("hap")
    );
    let module_catalog = plan
        .task_catalogs
        .iter()
        .find(|catalog| !catalog.project_task)
        .expect("module task catalog");
    assert!(module_catalog.target_options[0].binxo_enabled);
    assert_eq!(module_catalog.target_options[0].compile_api_version, 10);
}

#[test]
fn runtime_planner_owns_binxo_override_and_cmake_argument_fallback() {
    fn request(root: &std::path::Path, configured: Option<&str>) -> BuildScriptRuntimePlanRequest {
        let module_root = root.join("entry");
        let mut ext_params = serde_json::Map::new();
        if let Some(value) = configured {
            ext_params.insert("ohos-enable-binxo".to_string(), json!(value));
        }
        BuildScriptRuntimePlanRequest {
            project_root: root.to_path_buf(),
            project_profile: json!({
                "app": { "products": [{ "name": "default" }] },
                "modules": [{
                    "name": "entry",
                    "srcPath": "./entry",
                    "targets": [{ "name": "default", "applyToProducts": ["default"] }]
                }]
            }),
            modules: vec![BuildScriptRuntimeModulePlanRequest {
                name: "entry".to_string(),
                root: module_root,
                descriptor: json!({
                    "targets": [{ "name": "default", "applyToProducts": ["default"] }]
                }),
                build_profile: json!({
                    "buildOption": {
                        "externalNativeOptions": {
                            "arguments": ["-DOTHER=ON", "-DOHOS_ENABLE_BINXO=ON -DVALUE=1"]
                        }
                    },
                    "targets": [{ "name": "default" }]
                }),
                kind: Some("hap".to_string()),
                ..Default::default()
            }],
            product_name: "default".to_string(),
            build_mode_name: "debug".to_string(),
            ext_params,
            ..Default::default()
        }
    }

    let temp = tempfile::tempdir().expect("tempdir");
    std::fs::create_dir_all(temp.path().join("entry")).expect("create module");
    let fallback = BuildScriptRuntimePlanner::new(request(temp.path(), None))
        .plan()
        .expect("fallback plan");
    assert!(fallback.task_catalogs[1].target_options[0].binxo_enabled);

    let disabled = BuildScriptRuntimePlanner::new(request(temp.path(), Some("false")))
        .plan()
        .expect("disabled plan");
    assert!(!disabled.task_catalogs[1].target_options[0].binxo_enabled);
}

#[test]
fn runtime_planner_excludes_ohos_test_from_hvigor_implicit_all() {
    let temp = tempfile::tempdir().expect("tempdir");
    let module_root = temp.path().join("entry");
    std::fs::create_dir_all(&module_root).expect("create module");
    let plan = BuildScriptRuntimePlanner::new(BuildScriptRuntimePlanRequest {
        project_root: temp.path().to_path_buf(),
        project_profile: json!({
            "app": { "products": [{ "name": "default" }] },
            "modules": [{
                "name": "entry",
                "srcPath": "./entry",
                "targets": [{ "name": "default", "applyToProducts": ["default"] }]
            }]
        }),
        modules: vec![BuildScriptRuntimeModulePlanRequest {
            name: "entry".to_string(),
            root: module_root,
            descriptor: json!({
                "targets": [{ "name": "default", "applyToProducts": ["default"] }]
            }),
            build_profile: json!({
                "targets": [{ "name": "default" }, { "name": "ohosTest" }]
            }),
            build_script_override: None,
            ..Default::default()
        }],
        product_name: "default".to_string(),
        build_mode_name: "debug".to_string(),
        module_selection: None,
        build_target_name: None,
        device_test: false,
        ..Default::default()
    })
    .plan()
    .expect("plan implicit targets");

    assert_eq!(plan.modules[0].selected_targets.len(), 1);
    assert_eq!(plan.modules[0].selected_targets[0]["name"], "default");
    assert!(plan.modules[0].build_options.contains_key("ohosTest"));
}

#[test]
fn runtime_planner_selects_module_ohos_test_missing_from_project_target_mapping() {
    let temp = tempfile::tempdir().expect("tempdir");
    let module_root = temp.path().join("entry");
    std::fs::create_dir_all(&module_root).expect("create module");
    let plan = BuildScriptRuntimePlanner::new(BuildScriptRuntimePlanRequest {
        project_root: temp.path().to_path_buf(),
        project_profile: json!({
            "app": { "products": [{ "name": "default" }] },
            "modules": [{
                "name": "entry",
                "srcPath": "./entry",
                "targets": [{ "name": "default", "applyToProducts": ["default"] }]
            }]
        }),
        modules: vec![BuildScriptRuntimeModulePlanRequest {
            name: "entry".to_string(),
            root: module_root,
            descriptor: json!({
                "targets": [{ "name": "default", "applyToProducts": ["default"] }]
            }),
            build_profile: json!({
                "targets": [{ "name": "default" }, { "name": "ohosTest" }]
            }),
            build_script_override: None,
            ..Default::default()
        }],
        product_name: "default".to_string(),
        build_mode_name: "debug".to_string(),
        module_selection: None,
        build_target_name: Some("ohosTest".to_string()),
        device_test: false,
        ..Default::default()
    })
    .plan()
    .expect("plan explicit ohosTest target");

    assert_eq!(plan.modules[0].selected_targets.len(), 1);
    assert_eq!(plan.modules[0].selected_targets[0]["name"], "ohosTest");
}

#[test]
fn runtime_planner_returns_typed_unknown_target_diagnostic() {
    let temp = tempfile::tempdir().expect("tempdir");
    let module_root = temp.path().join("entry");
    std::fs::create_dir_all(&module_root).expect("create module");
    let request = BuildScriptRuntimePlanRequest {
        project_root: temp.path().to_path_buf(),
        project_profile: json!({
            "app": { "products": [{ "name": "default" }] },
            "modules": [{
                "name": "entry",
                "srcPath": "./entry",
                "targets": [{ "name": "default", "applyToProducts": ["default"] }]
            }]
        }),
        modules: vec![BuildScriptRuntimeModulePlanRequest {
            name: "entry".to_string(),
            root: module_root,
            descriptor: json!({
                "targets": [{ "name": "default", "applyToProducts": ["default"] }]
            }),
            build_profile: json!({}),
            build_script_override: None,
            ..Default::default()
        }],
        product_name: "default".to_string(),
        build_mode_name: "debug".to_string(),
        module_selection: Some("entry@missing".to_string()),
        build_target_name: None,
        device_test: false,
        ..Default::default()
    };

    let diagnostic = BuildScriptRuntimePlanner::new(request)
        .plan()
        .expect_err("unknown target must be rejected");
    assert_eq!(diagnostic.code, "UNKNOWN_MODULE_TARGET_IN_COMMAND_LINE");
    assert_eq!(diagnostic.module_name.as_deref(), Some("entry"));
    assert_eq!(diagnostic.target_name.as_deref(), Some("missing"));
}

#[test]
fn runtime_planner_resolves_compile_plugin_paths_inside_module_boundary() {
    let temp = tempfile::tempdir().expect("tempdir");
    let module_root = temp.path().join("entry");
    let plugin_root = module_root.join("build/hvigor_compile");
    std::fs::create_dir_all(&plugin_root).expect("create compile plugin dir");
    let plugin_file = plugin_root.join("fixture.ts");
    std::fs::write(&plugin_file, "export default { name: 'fixture' }")
        .expect("write compile plugin");
    let request = BuildScriptRuntimePlanRequest {
        project_root: temp.path().to_path_buf(),
        project_profile: json!({
            "app": { "products": [{ "name": "default" }] },
            "modules": [{
                "name": "entry",
                "srcPath": "./entry",
                "targets": [{ "name": "default", "applyToProducts": ["default"] }]
            }]
        }),
        modules: vec![BuildScriptRuntimeModulePlanRequest {
            name: "entry".to_string(),
            root: module_root.clone(),
            descriptor: json!({
                "targets": [{ "name": "default", "applyToProducts": ["default"] }]
            }),
            build_profile: json!({}),
            build_script_override: Some(json!({
                "arkOptions": { "compilePluginFile": "./build/hvigor_compile/fixture" }
            })),
            ..Default::default()
        }],
        product_name: "default".to_string(),
        build_mode_name: "debug".to_string(),
        module_selection: None,
        build_target_name: None,
        device_test: false,
        ..Default::default()
    };

    let plan = BuildScriptRuntimePlanner::new(request)
        .plan()
        .expect("resolve compile plugin");
    assert_eq!(plan.compile_plugin_records.len(), 1);
    assert_eq!(plan.compile_plugin_records[0].file, plugin_file);
    assert_eq!(plan.compile_plugin_records[0].module_root, module_root);
    assert_eq!(plan.compile_plugin_records[0].target_name, "default");
}

#[test]
fn runtime_planner_device_test_selects_default_and_ohos_test_targets() {
    let temp = tempfile::tempdir().expect("tempdir");
    let module_root = temp.path().join("entry");
    std::fs::create_dir_all(&module_root).expect("create module");
    let plan = BuildScriptRuntimePlanner::new(BuildScriptRuntimePlanRequest {
        project_root: temp.path().to_path_buf(),
        project_profile: json!({
            "app": { "products": [{ "name": "default" }] },
            "modules": [{
                "name": "entry",
                "srcPath": "./entry",
                "targets": [{ "name": "default", "applyToProducts": ["default"] }]
            }]
        }),
        modules: vec![BuildScriptRuntimeModulePlanRequest {
            name: "entry".to_string(),
            root: module_root,
            descriptor: json!({
                "targets": [{ "name": "default", "applyToProducts": ["default"] }]
            }),
            build_profile: json!({
                "targets": [
                    { "name": "default" },
                    { "name": "ohosTest" },
                    { "name": "phone" }
                ]
            }),
            build_script_override: None,
            ..Default::default()
        }],
        product_name: "default".to_string(),
        build_mode_name: "test".to_string(),
        module_selection: Some("entry@phone".to_string()),
        build_target_name: Some("phone".to_string()),
        device_test: true,
        ..Default::default()
    })
    .plan()
    .expect("plan device-test targets");

    let names = plan.modules[0]
        .selected_targets
        .iter()
        .map(|target| target["name"].as_str().expect("target name"))
        .collect::<Vec<_>>();
    assert_eq!(names, ["default", "ohosTest"]);
}

#[test]
fn module_specific_selection_keeps_other_module_targets_registered_like_hvigor() {
    let temp = tempfile::tempdir().expect("temp project");
    let entry_root = temp.path().join("entry");
    let library_root = temp.path().join("library");
    std::fs::create_dir_all(&entry_root).expect("create entry");
    std::fs::create_dir_all(&library_root).expect("create library");
    let modules = [(&entry_root, "entry"), (&library_root, "library")]
        .into_iter()
        .map(|(root, name)| BuildScriptRuntimeModulePlanRequest {
            name: name.to_string(),
            root: root.to_path_buf(),
            descriptor: json!({
                "targets": [{ "name": "default", "applyToProducts": ["default"] }]
            }),
            build_profile: json!({ "targets": [{ "name": "default" }] }),
            build_script_override: None,
            ..Default::default()
        })
        .collect();
    let plan = BuildScriptRuntimePlanner::new(BuildScriptRuntimePlanRequest {
        project_root: temp.path().to_path_buf(),
        project_profile: json!({
            "app": { "products": [{ "name": "default" }] },
            "modules": [
                { "name": "entry", "srcPath": "./entry" },
                { "name": "library", "srcPath": "./library" }
            ]
        }),
        modules,
        product_name: "default".to_string(),
        build_mode_name: "debug".to_string(),
        module_selection: Some("entry@default".to_string()),
        build_target_name: None,
        device_test: false,
        ..Default::default()
    })
    .plan()
    .expect("plan module-specific selection");

    assert_eq!(plan.modules[0].selected_targets[0]["name"], "default");
    assert_eq!(plan.modules[1].selected_targets[0]["name"], "default");
}

#[test]
fn runtime_planner_rejects_selected_cangjie_build_before_native_execution() {
    let temp = tempfile::tempdir().expect("temp project");
    let module_root = temp.path().join("entry");
    std::fs::create_dir_all(&module_root).expect("create module");
    let plan = BuildScriptRuntimePlanner::new(BuildScriptRuntimePlanRequest {
        project_root: temp.path().to_path_buf(),
        project_profile: json!({
            "app": { "products": [{ "name": "default" }] },
            "modules": [{ "name": "entry", "srcPath": "./entry" }]
        }),
        modules: vec![BuildScriptRuntimeModulePlanRequest {
            name: "entry".to_string(),
            root: module_root,
            descriptor: json!({
                "targets": [{ "name": "default", "applyToProducts": ["default"] }]
            }),
            build_profile: json!({
                "buildOption": {
                    "cangjieOptions": {
                        "path": "./src/main/cangjie/cjpm.toml",
                        "abiFilters": ["arm64-v8a"]
                    }
                },
                "targets": [{ "name": "default" }]
            }),
            kind: Some("hap".to_string()),
            ..Default::default()
        }],
        product_name: "default".to_string(),
        build_mode_name: "debug".to_string(),
        module_selection: Some("entry@default".to_string()),
        build_target_name: Some("default".to_string()),
        ..Default::default()
    })
    .plan()
    .expect("plan Cangjie target before the native execution boundary");
    let diagnostic = plan
        .unsupported_build_diagnostic
        .expect("Cangjie requires an unsupported external task pipeline");

    assert_eq!(diagnostic.code, "ARKDOWN_UNSUPPORTED_CANGJIE_BUILD");
    assert_eq!(diagnostic.module.as_deref(), Some("entry"));
    assert_eq!(diagnostic.target.as_deref(), Some("default"));
    assert_eq!(diagnostic.feature.as_deref(), Some("cangjieOptions"));
}
