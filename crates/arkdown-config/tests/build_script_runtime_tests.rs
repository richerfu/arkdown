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
        }],
        product_name: "default".to_string(),
        build_mode_name: "debug".to_string(),
        module_selection: Some("entry@phone".to_string()),
        build_target_name: None,
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
        }],
        product_name: "default".to_string(),
        build_mode_name: "debug".to_string(),
        module_selection: Some("entry@missing".to_string()),
        build_target_name: None,
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
        }],
        product_name: "default".to_string(),
        build_mode_name: "debug".to_string(),
        module_selection: None,
        build_target_name: None,
    };

    let plan = BuildScriptRuntimePlanner::new(request)
        .plan()
        .expect("resolve compile plugin");
    assert_eq!(plan.compile_plugin_records.len(), 1);
    assert_eq!(plan.compile_plugin_records[0].file, plugin_file);
    assert_eq!(plan.compile_plugin_records[0].module_root, module_root);
    assert_eq!(plan.compile_plugin_records[0].target_name, "default");
}
