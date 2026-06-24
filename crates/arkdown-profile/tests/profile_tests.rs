//! Integration tests for `arkdown-profile` public API.

use std::path::PathBuf;
use std::sync::Arc;

use arkdown_core::{
    AbilityConfig, ArkdownConfig, BuildMode, BuildOption, ModuleConfig, ModuleJson5,
    ModuleJson5Module, OhosModuleType, OutputTarget, ProductConfig, SdkConfig, StrictModeConfig,
};
use arkdown_profile::{LoaderJson, ProcessProfileProcessor, ProfileProcessor};

// ---------------------------------------------------------------------------
// Shared helpers
// ---------------------------------------------------------------------------

fn default_config() -> Arc<ArkdownConfig> {
    Arc::new(ArkdownConfig {
        project_root: PathBuf::from("/project"),
        build_mode: BuildMode::Debug,
        output_target: OutputTarget::Hap,
        product: ProductConfig {
            name: "default".to_string(),
            bundle_name: "com.example.profiletest".to_string(),
            compatible_sdk_version: "5.0.0(12)".to_string(),
            compatible_sdk_version_stage: None,
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
                preload_so: vec![],
                remove_permissions: vec![],
                aot_compile_mode: None,
                host_pgo: None,
                ap_path: None,
                abi_filters: vec![],
            },
            version_code: None,
            version_name: None,
        },
        modules: vec![],
        sdk_config: SdkConfig {
            ets_loader_path: PathBuf::from("/sdk/ets-loader"),
            es2abc_path: PathBuf::from("/sdk/es2abc"),
            restool_path: PathBuf::from("/sdk/restool"),
            compile_api_version: 12,
            has_rollup_plugin: true,
            compile_sdk_version: "6.0.0.47".to_string(),
        },
    })
}

fn single_ability_json5(module_name: &str) -> ModuleJson5 {
    ModuleJson5 {
        module: ModuleJson5Module {
            name: module_name.to_string(),
            type_: "entry".to_string(),
            main_element: Some("EntryAbility".to_string()),
            device_types: vec!["phone".to_string()],
            pages: None,
            router_map: None,
            description: None,
            abilities: vec![AbilityConfig {
                name: "EntryAbility".to_string(),
                src_entry: "./ets/entryability/EntryAbility.ets".to_string(),
                exported: true,
                skills: vec![],
                icon: None,
                label: None,
                description: None,
                start_window_icon: None,
                start_window_background: None,
            }],
            extension_abilities: vec![],
            src_entry: None,
            metadata: vec![],
            app_startup: None,
            cross_app_shared_config: Default::default(),
            ..Default::default()
        },
    }
}

fn make_module_cfg(name: &str) -> ModuleConfig {
    ModuleConfig {
        name: name.to_string(),
        src_path: PathBuf::from(format!("/project/{}", name)),
        module_type: OhosModuleType::Entry,
    }
}

// ---------------------------------------------------------------------------
// process_profile function
// ---------------------------------------------------------------------------

#[test]
fn test_process_profile_virtual_machine_api12() {
    use arkdown_core::CompileMode;
    let json5 = single_ability_json5("entry");
    let manifest = ProcessProfileProcessor::new().process_profile(
        &json5,
        12,
        "com.example.test",
        CompileMode::EsModule,
    );
    assert_eq!(
        manifest.virtual_machine, "ark12.0.1.0",
        "API 12 must produce virtual_machine = 'ark12.0.1.0'"
    );
}

#[test]
fn test_process_profile_virtual_machine_api9() {
    use arkdown_core::CompileMode;
    let json5 = single_ability_json5("entry");
    let manifest = ProcessProfileProcessor::new().process_profile(
        &json5,
        9,
        "com.example.test",
        CompileMode::EsModule,
    );
    assert_eq!(manifest.virtual_machine, "ark9.0.1.0");
}

#[test]
fn test_process_profile_module_name_preserved() {
    use arkdown_core::CompileMode;
    let json5 = single_ability_json5("my_module");
    let manifest = ProcessProfileProcessor::new().process_profile(
        &json5,
        12,
        "com.example",
        CompileMode::EsModule,
    );
    assert_eq!(manifest.module_name, "my_module");
}

#[test]
fn test_process_profile_abilities_mapped() {
    use arkdown_core::CompileMode;
    let json5 = single_ability_json5("entry");
    let manifest = ProcessProfileProcessor::new().process_profile(
        &json5,
        12,
        "com.example",
        CompileMode::EsModule,
    );
    assert_eq!(manifest.abilities.len(), 1);
    assert_eq!(manifest.abilities[0].name, "EntryAbility");
    assert_eq!(
        manifest.abilities[0].src_entry,
        "./ets/entryability/EntryAbility.ets"
    );
}

// ---------------------------------------------------------------------------
// ProfileProcessor::process_module
// ---------------------------------------------------------------------------

#[test]
fn test_process_module_returns_ok() {
    let processor = ProfileProcessor::new(default_config());
    let json5 = single_ability_json5("entry");
    let module_cfg = make_module_cfg("entry");
    let result = processor.process_module(&module_cfg, &json5);
    assert!(
        result.is_ok(),
        "process_module must return Ok: {:?}",
        result.err()
    );
}

#[test]
fn test_process_module_manifest_contains_ability_compile_entry() {
    let processor = ProfileProcessor::new(default_config());
    let json5 = single_ability_json5("entry");
    let module_cfg = make_module_cfg("entry");
    let (_, loader) = processor.process_module(&module_cfg, &json5).unwrap();

    assert!(
        !loader.compile_entry.is_empty(),
        "compile_entry must not be empty for a module with abilities"
    );
    assert!(
        loader
            .compile_entry
            .iter()
            .any(|e| e.contains("EntryAbility")),
        "compile_entry must include the ability src_entry; got: {:?}",
        loader.compile_entry
    );
}

#[test]
fn test_process_module_build_mode_debug() {
    let processor = ProfileProcessor::new(default_config());
    let json5 = single_ability_json5("entry");
    let module_cfg = make_module_cfg("entry");
    let (_, loader) = processor.process_module(&module_cfg, &json5).unwrap();
    assert_eq!(loader.build_mode, "debug");
}

#[test]
fn test_process_module_package_manager_is_ohpm() {
    let processor = ProfileProcessor::new(default_config());
    let json5 = single_ability_json5("entry");
    let module_cfg = make_module_cfg("entry");
    let (_, loader) = processor.process_module(&module_cfg, &json5).unwrap();
    assert_eq!(loader.package_manager_type, "ohpm");
}

// ---------------------------------------------------------------------------
// Multiple abilities → multiple compile entries
// ---------------------------------------------------------------------------

#[test]
fn test_multiple_abilities_produce_multiple_compile_entries() {
    let processor = ProfileProcessor::new(default_config());
    let json5 = ModuleJson5 {
        module: ModuleJson5Module {
            name: "entry".to_string(),
            type_: "entry".to_string(),
            main_element: None,
            device_types: vec!["phone".to_string()],
            pages: None,
            router_map: None,
            description: None,
            abilities: vec![
                AbilityConfig {
                    name: "MainAbility".to_string(),
                    src_entry: "./ets/main/MainAbility.ets".to_string(),
                    exported: true,
                    skills: vec![],
                    icon: None,
                    label: None,
                    description: None,
                    start_window_icon: None,
                    start_window_background: None,
                },
                AbilityConfig {
                    name: "ShareAbility".to_string(),
                    src_entry: "./ets/share/ShareAbility.ets".to_string(),
                    exported: false,
                    skills: vec![],
                    icon: None,
                    label: None,
                    description: None,
                    start_window_icon: None,
                    start_window_background: None,
                },
            ],
            extension_abilities: vec![],
            src_entry: None,
            metadata: vec![],
            app_startup: None,
            cross_app_shared_config: Default::default(),
            ..Default::default()
        },
    };

    let module_cfg = make_module_cfg("entry");
    let (_, loader) = processor.process_module(&module_cfg, &json5).unwrap();
    assert!(
        loader.compile_entry.len() >= 2,
        "two abilities must produce at least 2 compile entries; got: {:?}",
        loader.compile_entry
    );
}

// ---------------------------------------------------------------------------
// generate_loader_json standalone
// ---------------------------------------------------------------------------

#[test]
fn test_generate_loader_json_has_compile_entry() {
    use arkdown_core::CompileMode;
    let config = default_config();
    let json5 = single_ability_json5("feature");
    let manifest = ProcessProfileProcessor::new().process_profile(
        &json5,
        12,
        "com.example",
        CompileMode::EsModule,
    );
    let module_cfg = make_module_cfg("feature");
    let loader = LoaderJson::generate(&config, &module_cfg, &manifest);

    assert!(
        !loader.compile_entry.is_empty(),
        "compile_entry must not be empty"
    );
}

#[test]
fn test_generate_loader_json_use_normalized_ohm_url_propagated() {
    use arkdown_core::CompileMode;
    let config = default_config(); // use_normalized_ohm_url = true
    let json5 = single_ability_json5("entry");
    let manifest = ProcessProfileProcessor::new().process_profile(
        &json5,
        12,
        "com.example",
        CompileMode::EsModule,
    );
    let module_cfg = make_module_cfg("entry");
    let loader = LoaderJson::generate(&config, &module_cfg, &manifest);

    assert!(
        loader.use_normalized_ohm_url,
        "use_normalized_ohm_url must be propagated from config"
    );
}

// ---------------------------------------------------------------------------
// write_loader_json round-trip
// ---------------------------------------------------------------------------

#[test]
fn test_write_and_read_loader_json_roundtrip() {
    let processor = ProfileProcessor::new(default_config());
    let json5 = single_ability_json5("entry");
    let module_cfg = make_module_cfg("entry");
    let (_, loader) = processor.process_module(&module_cfg, &json5).unwrap();

    let tmp = tempfile::tempdir().expect("tempdir");
    let out_path = tmp.path().join("loader.json");

    processor
        .write_loader_json(&loader, &out_path)
        .expect("write_loader_json must succeed");

    assert!(out_path.exists(), "loader.json must be written to disk");

    // Round-trip: parse the written JSON back.
    let content = std::fs::read_to_string(&out_path).expect("read loader.json");
    let parsed: arkdown_profile::LoaderJson =
        serde_json::from_str(&content).expect("loader.json must be valid JSON");
    assert_eq!(parsed.compile_mode, "esmodule");
    assert_eq!(parsed.package_manager_type, "ohpm");
    assert!(
        !loader.compile_entry.is_empty(),
        "internal bundler entries must remain available"
    );
    assert!(
        parsed.hvigor_compile_entry.is_empty(),
        "minimal hvigor loader.json compileEntry must be empty"
    );
    assert!(
        parsed.compile_entry.is_empty(),
        "serialized loader.json must not repopulate internal bundler entries"
    );
}

// ---------------------------------------------------------------------------
// write_module_json
// ---------------------------------------------------------------------------

#[test]
fn test_write_module_json_emits_virtual_machine_and_compile_mode() {
    use arkdown_core::{AbilityEntry, CompileMode, HapManifest, OhosModuleType};

    let tmp = tempfile::tempdir().expect("tempdir");
    let mut config = (*default_config()).clone();
    config.project_root = tmp.path().to_path_buf();
    let processor = ProfileProcessor::new(Arc::new(config));

    let manifest = HapManifest {
        module_name: "entry".to_string(),
        module_type: OhosModuleType::Entry,
        device_types: vec!["phone".to_string()],
        virtual_machine: "ark12.0.6.0".to_string(),
        compile_mode: CompileMode::EsModule,
        src_entry: None,
        abilities: vec![AbilityEntry {
            name: "EntryAbility".to_string(),
            src_entry: "./ets/entryability/EntryAbility.ets".to_string(),
            exported: true,
            skills: vec![],
            icon: None,
            label: None,
            description: None,
            start_window_icon: None,
            start_window_background: None,
        }],
        extension_abilities: vec![],
        dependencies: vec![],
        router_map: None,
        pages: Some("$profile:main_pages".to_string()),
        main_element: Some("EntryAbility".to_string()),
        description: None,
    };

    let out_path = tmp.path().join("module.json");
    processor
        .write_module_json(&manifest, &out_path, None)
        .expect("write_module_json must succeed");

    assert!(out_path.exists(), "module.json must be written to disk");

    let content = std::fs::read_to_string(&out_path).expect("read module.json");
    let parsed: serde_json::Value =
        serde_json::from_str(&content).expect("module.json must be valid JSON");

    let module = &parsed["module"];
    assert_eq!(
        module["virtualMachine"].as_str(),
        Some("ark12.0.6.0"),
        "module.virtualMachine must be 'ark12.0.6.0'; got: {}",
        module["virtualMachine"]
    );
    assert_eq!(
        module["compileMode"].as_str(),
        Some("esmodule"),
        "module.compileMode must be 'esmodule'; got: {}",
        module["compileMode"]
    );
}

#[test]
fn test_write_module_json_preserves_raw_skill_fields() {
    use arkdown_core::{AbilityEntry, CompileMode, HapManifest, ModuleConfig, OhosModuleType};

    let tmp = tempfile::tempdir().expect("tempdir");
    let module_src = tmp.path().join("entry");
    std::fs::create_dir_all(module_src.join("src/main")).expect("create module dirs");
    std::fs::write(
        module_src.join("src/main/module.json5"),
        r#"{
          "module": {
            "name": "entry",
            "type": "entry",
            "abilities": [{
              "name": "EntryAbility",
              "srcEntry": "./ets/entryability/EntryAbility.ets",
              "skills": [{
                "domainVerify": true,
                "actions": ["ohos.want.action.viewData"],
                "uris": [{ "scheme": "https", "host": "i.example.com", "pathRegex": "applinking/.*" }]
              }]
            }]
          }
        }"#,
    )
    .expect("write module.json5");

    let mut config = (*default_config()).clone();
    config.project_root = tmp.path().to_path_buf();
    config.modules = vec![ModuleConfig {
        name: "entry".to_string(),
        src_path: module_src,
        module_type: OhosModuleType::Entry,
    }];
    let processor = ProfileProcessor::new(Arc::new(config));

    let manifest = HapManifest {
        module_name: "entry".to_string(),
        module_type: OhosModuleType::Entry,
        device_types: vec!["phone".to_string()],
        virtual_machine: "ark12.0.6.0".to_string(),
        compile_mode: CompileMode::EsModule,
        src_entry: None,
        abilities: vec![AbilityEntry {
            name: "EntryAbility".to_string(),
            src_entry: "./ets/entryability/EntryAbility.ets".to_string(),
            exported: true,
            skills: vec![],
            icon: None,
            label: None,
            description: None,
            start_window_icon: None,
            start_window_background: None,
        }],
        extension_abilities: vec![],
        dependencies: vec![],
        router_map: None,
        pages: None,
        main_element: None,
        description: None,
    };

    let out_path = tmp.path().join("module.json");
    processor
        .write_module_json(&manifest, &out_path, None)
        .expect("write module.json");

    let parsed: serde_json::Value =
        serde_json::from_str(&std::fs::read_to_string(out_path).expect("read module.json"))
            .expect("valid module.json");
    let skill = &parsed["module"]["abilities"][0]["skills"][0];
    assert_eq!(skill["domainVerify"].as_bool(), Some(true));
    assert_eq!(
        skill["uris"][0]["pathRegex"].as_str(),
        Some("applinking/.*")
    );
}

#[test]
fn test_write_module_json_sets_has_insight_intent_from_profile() {
    use arkdown_core::{CompileMode, HapManifest, ModuleConfig, OhosModuleType};

    let tmp = tempfile::tempdir().expect("tempdir");
    let module_src = tmp.path().join("entry");
    let profile_dir = module_src.join("src/main/resources/base/profile");
    std::fs::create_dir_all(&profile_dir).expect("create profile dir");
    std::fs::write(profile_dir.join("insight_intent.json"), "{}").expect("write profile");

    let mut config = (*default_config()).clone();
    config.project_root = tmp.path().to_path_buf();
    config.modules = vec![ModuleConfig {
        name: "entry".to_string(),
        src_path: module_src,
        module_type: OhosModuleType::Entry,
    }];
    let processor = ProfileProcessor::new(Arc::new(config));

    let manifest = HapManifest {
        module_name: "entry".to_string(),
        module_type: OhosModuleType::Entry,
        device_types: vec!["phone".to_string()],
        virtual_machine: "ark12.0.6.0".to_string(),
        compile_mode: CompileMode::EsModule,
        src_entry: None,
        abilities: vec![],
        extension_abilities: vec![],
        dependencies: vec![],
        router_map: None,
        pages: None,
        main_element: None,
        description: None,
    };

    let out_path = tmp.path().join("module.json");
    processor
        .write_module_json(&manifest, &out_path, None)
        .expect("write module.json");

    let parsed: serde_json::Value =
        serde_json::from_str(&std::fs::read_to_string(out_path).expect("read module.json"))
            .expect("valid module.json");
    assert_eq!(parsed["module"]["hasInsightIntent"].as_bool(), Some(true));
}

#[test]
fn test_write_module_json_merges_dependency_extension_abilities() {
    use arkdown_core::{CompileMode, HapManifest, ModuleConfig, OhosModuleType};

    let tmp = tempfile::tempdir().expect("tempdir");
    let module_src = tmp.path().join("entry");
    std::fs::create_dir_all(module_src.join("src/main")).expect("create module dirs");
    std::fs::write(
        module_src.join("oh-package.json5"),
        r#"{ "name": "entry", "dependencies": { "ad-core": "1.0.0" } }"#,
    )
    .expect("write package");
    std::fs::write(
        module_src.join("src/main/module.json5"),
        r#"{ "module": { "name": "entry", "type": "entry", "deviceTypes": ["phone"] } }"#,
    )
    .expect("write module.json5");

    let dep_main = module_src.join("oh_modules/ad-core/src/main");
    std::fs::create_dir_all(&dep_main).expect("create dependency dirs");
    std::fs::write(
        module_src.join("oh_modules/ad-core/oh-package.json5"),
        r#"{ "name": "ad-core", "version": "1.0.0" }"#,
    )
    .expect("write dependency package");
    std::fs::write(
        dep_main.join("module.json5"),
        r#"{
          "app": { "minAPIVersion": 12 },
          "module": {
            "name": "ad_core",
            "type": "har",
            "deviceTypes": ["phone"],
            "extensionAbilities": [{
              "name": "CacheRetryWorker",
              "srcEntry": "./ets/CacheRetryWorker.ets",
              "type": "workScheduler",
              "permissions": ["ohos.permission.KEEP_BACKGROUND_RUNNING"]
            }]
          }
        }"#,
    )
    .expect("write dependency module");

    let mut config = (*default_config()).clone();
    config.project_root = tmp.path().to_path_buf();
    config.modules = vec![ModuleConfig {
        name: "entry".to_string(),
        src_path: module_src,
        module_type: OhosModuleType::Entry,
    }];
    let processor = ProfileProcessor::new(Arc::new(config));

    let manifest = HapManifest {
        module_name: "entry".to_string(),
        module_type: OhosModuleType::Entry,
        device_types: vec!["phone".to_string()],
        virtual_machine: "ark12.0.6.0".to_string(),
        compile_mode: CompileMode::EsModule,
        src_entry: None,
        abilities: vec![],
        extension_abilities: vec![],
        dependencies: vec![],
        router_map: None,
        pages: None,
        main_element: None,
        description: None,
    };

    let out_path = tmp.path().join("module.json");
    processor
        .write_module_json(&manifest, &out_path, None)
        .expect("write module.json");

    let parsed: serde_json::Value =
        serde_json::from_str(&std::fs::read_to_string(out_path).expect("read module.json"))
            .expect("valid module.json");
    let extension_abilities = parsed["module"]["extensionAbilities"]
        .as_array()
        .expect("extensionAbilities array");
    let worker = extension_abilities
        .iter()
        .find(|item| item["name"].as_str() == Some("CacheRetryWorker"))
        .expect("CacheRetryWorker merged");
    assert_eq!(
        worker["srcEntry"].as_str(),
        Some("@normalized:N&&&ad-core/src/main/ets/CacheRetryWorker&1.0.0")
    );
    assert_eq!(worker["type"].as_str(), Some("workScheduler"));
    assert_eq!(
        worker["permissions"][0].as_str(),
        Some("ohos.permission.KEEP_BACKGROUND_RUNNING")
    );
}

#[test]
fn test_write_module_json_resolves_resource_ids_into_abilities() {
    use arkdown_core::{AbilityEntry, CompileMode, HapManifest, OhosModuleType};
    use rustc_hash::FxHashMap;

    let tmp = tempfile::tempdir().expect("tempdir");
    let mut config = (*default_config()).clone();
    config.project_root = tmp.path().to_path_buf();
    let processor = ProfileProcessor::new(Arc::new(config));

    let manifest = HapManifest {
        module_name: "entry".to_string(),
        module_type: OhosModuleType::Entry,
        device_types: vec!["phone".to_string()],
        virtual_machine: "ark12.0.6.0".to_string(),
        compile_mode: CompileMode::EsModule,
        src_entry: None,
        abilities: vec![AbilityEntry {
            name: "EntryAbility".to_string(),
            src_entry: "./ets/entryability/EntryAbility.ets".to_string(),
            exported: true,
            skills: vec![],
            icon: Some("$media:icon".to_string()),
            label: Some("$string:EntryAbility_label".to_string()),
            description: Some("$string:EntryAbility_desc".to_string()),
            start_window_icon: Some("$media:startIcon".to_string()),
            start_window_background: Some("$color:start_window_background".to_string()),
        }],
        extension_abilities: vec![],
        dependencies: vec![],
        router_map: None,
        pages: None,
        main_element: None,
        description: None,
    };

    let mut resource_ids: FxHashMap<String, u32> = FxHashMap::default();
    resource_ids.insert("icon".to_string(), 16777219);
    resource_ids.insert("EntryAbility_label".to_string(), 16777216);
    resource_ids.insert("EntryAbility_desc".to_string(), 16777217);
    resource_ids.insert("startIcon".to_string(), 16777220);
    resource_ids.insert("start_window_background".to_string(), 16777218);

    let out_path = tmp.path().join("module.json");
    processor
        .write_module_json(&manifest, &out_path, Some(&resource_ids))
        .expect("write_module_json must succeed");

    let content = std::fs::read_to_string(&out_path).expect("read module.json");
    let parsed: serde_json::Value =
        serde_json::from_str(&content).expect("module.json must be valid JSON");

    let ability = &parsed["module"]["abilities"][0];
    assert_eq!(
        ability["iconId"].as_u64(),
        Some(16777219),
        "iconId must be resolved; got: {}",
        ability["iconId"]
    );
    assert_eq!(
        ability["labelId"].as_u64(),
        Some(16777216),
        "labelId must be resolved; got: {}",
        ability["labelId"]
    );
    assert_eq!(
        ability["descriptionId"].as_u64(),
        Some(16777217),
        "descriptionId must be resolved; got: {}",
        ability["descriptionId"]
    );
    assert_eq!(
        ability["startWindowIconId"].as_u64(),
        Some(16777220),
        "startWindowIconId must be resolved; got: {}",
        ability["startWindowIconId"]
    );
    assert_eq!(
        ability["startWindowBackgroundId"].as_u64(),
        Some(16777218),
        "startWindowBackgroundId must be resolved; got: {}",
        ability["startWindowBackgroundId"]
    );
}

#[test]
fn test_write_module_json_adds_permission_reason_ids_and_router_map() {
    use arkdown_core::{CompileMode, HapManifest, ModuleConfig, OhosModuleType};
    use rustc_hash::FxHashMap;

    let tmp = tempfile::tempdir().expect("tempdir");
    let module_src = tmp.path().join("entry");
    std::fs::create_dir_all(module_src.join("src/main")).expect("create module dirs");
    std::fs::write(
        module_src.join("src/main/module.json5"),
        r#"{
          "module": {
            "name": "entry",
            "type": "entry",
            "deviceTypes": ["phone"],
            "requestPermissions": [
              {
                "name": "ohos.permission.INTERNET",
                "reason": "$string:internet_reason",
                "usedScene": { "abilities": ["EntryAbility"], "when": "inuse" }
              }
            ]
          }
        }"#,
    )
    .expect("write module.json5");

    let mut config = (*default_config()).clone();
    config.project_root = tmp.path().to_path_buf();
    config.modules = vec![ModuleConfig {
        name: "entry".to_string(),
        src_path: module_src,
        module_type: OhosModuleType::Entry,
    }];
    let processor = ProfileProcessor::new(Arc::new(config));

    let manifest = HapManifest {
        module_name: "entry".to_string(),
        module_type: OhosModuleType::Entry,
        device_types: vec!["phone".to_string()],
        virtual_machine: "ark12.0.6.0".to_string(),
        compile_mode: CompileMode::EsModule,
        src_entry: None,
        abilities: vec![],
        extension_abilities: vec![],
        dependencies: vec![],
        router_map: Some("$profile:default-router-map".to_string()),
        pages: None,
        main_element: None,
        description: None,
    };

    let mut resource_ids: FxHashMap<String, u32> = FxHashMap::default();
    resource_ids.insert("internet_reason".to_string(), 201326602);

    let out_path = tmp.path().join("module.json");
    processor
        .write_module_json(&manifest, &out_path, Some(&resource_ids))
        .expect("write module.json");

    let content = std::fs::read_to_string(&out_path).expect("read module.json");
    let parsed: serde_json::Value = serde_json::from_str(&content).expect("valid module.json");

    assert_eq!(
        parsed["module"]["requestPermissions"][0]["reasonId"].as_u64(),
        Some(201326602)
    );
    assert_eq!(
        parsed["module"]["routerMap"].as_str(),
        Some("$profile:default-router-map")
    );
}

#[test]
fn test_write_module_json_applies_target_source_ability_overrides() {
    use arkdown_core::{AbilityEntry, CompileMode, HapManifest, ModuleConfig, OhosModuleType};
    use rustc_hash::FxHashMap;

    let tmp = tempfile::tempdir().expect("tempdir");
    let module_src = tmp.path().join("entry");
    std::fs::create_dir_all(module_src.join("src/main")).expect("create dirs");
    std::fs::write(
        module_src.join("oh-package.json5"),
        r#"{ "name": "entry" }"#,
    )
    .expect("write oh-package.json5");
    std::fs::write(
        module_src.join("src/main/module.json5"),
        r#"{
          "module": {
            "name": "entry",
            "type": "entry",
            "deviceTypes": ["phone"],
            "abilities": [{
              "name": "EntryAbility",
              "srcEntry": "./ets/entryability/EntryAbility.ets",
              "icon": "$media:base_icon",
              "label": "$string:base_label",
              "launchType": "standard"
            }]
          }
        }"#,
    )
    .expect("write module.json5");
    std::fs::write(
        tmp.path().join("build-profile.json5"),
        r#"{
          app: {
            products: [{
              name: "default",
              compatibleSdkVersion: "5.0.0(12)",
              runtimeOS: "HarmonyOS"
            }]
          },
          modules: [{
            name: "entry",
            srcPath: "./entry",
            targets: [{
              name: "default",
              applyToProducts: ["default"],
              source: {
                abilities: [{
                  name: "EntryAbility",
                  icon: "$media:target_icon",
                  label: "$string:target_label",
                  launchType: "singleton"
                }]
              }
            }]
          }]
        }"#,
    )
    .expect("write build-profile.json5");

    let mut config = (*default_config()).clone();
    config.project_root = tmp.path().to_path_buf();
    config.modules = vec![ModuleConfig {
        name: "entry".to_string(),
        src_path: module_src,
        module_type: OhosModuleType::Entry,
    }];
    let processor = ProfileProcessor::new(Arc::new(config));

    let manifest = HapManifest {
        module_name: "entry".to_string(),
        module_type: OhosModuleType::Entry,
        device_types: vec!["phone".to_string()],
        virtual_machine: "ark12.0.6.0".to_string(),
        compile_mode: CompileMode::EsModule,
        src_entry: None,
        abilities: vec![AbilityEntry {
            name: "EntryAbility".to_string(),
            src_entry: "./ets/entryability/EntryAbility.ets".to_string(),
            exported: true,
            skills: vec![],
            icon: Some("$media:base_icon".to_string()),
            label: Some("$string:base_label".to_string()),
            description: None,
            start_window_icon: None,
            start_window_background: None,
        }],
        extension_abilities: vec![],
        dependencies: vec![],
        router_map: None,
        pages: None,
        main_element: Some("EntryAbility".to_string()),
        description: None,
    };

    let mut resource_ids: FxHashMap<String, u32> = FxHashMap::default();
    resource_ids.insert("base_icon".to_string(), 1);
    resource_ids.insert("base_label".to_string(), 2);
    resource_ids.insert("target_icon".to_string(), 3);
    resource_ids.insert("target_label".to_string(), 4);

    let out_path = tmp.path().join("module.json");
    processor
        .write_module_json(&manifest, &out_path, Some(&resource_ids))
        .expect("write module.json");

    let content = std::fs::read_to_string(&out_path).expect("read module.json");
    let parsed: serde_json::Value = serde_json::from_str(&content).expect("valid JSON");
    let ability = &parsed["module"]["abilities"][0];
    assert_eq!(ability["icon"].as_str(), Some("$media:target_icon"));
    assert_eq!(ability["label"].as_str(), Some("$string:target_label"));
    assert_eq!(ability["launchType"].as_str(), Some("singleton"));
    assert_eq!(ability["iconId"].as_u64(), Some(3));
    assert_eq!(ability["labelId"].as_u64(), Some(4));
}

#[test]
fn test_write_module_json_does_not_append_target_distro_filter_metadata() {
    use arkdown_core::{CompileMode, HapManifest, ModuleConfig, OhosModuleType};

    let tmp = tempfile::tempdir().expect("tempdir");
    let module_src = tmp.path().join("entry");
    std::fs::create_dir_all(module_src.join("src/main")).expect("create dirs");
    std::fs::write(
        module_src.join("oh-package.json5"),
        r#"{ "name": "entry" }"#,
    )
    .expect("write oh-package.json5");
    std::fs::write(
        module_src.join("src/main/module.json5"),
        r#"{ "module": { "name": "entry", "type": "entry", "deviceTypes": ["phone"] } }"#,
    )
    .expect("write module.json5");
    std::fs::write(
        tmp.path().join("build-profile.json5"),
        r#"{
          app: { products: [{ name: "default" }] },
          modules: [{
            name: "entry",
            srcPath: "./entry",
            targets: [{
              name: "default",
              applyToProducts: ["default"],
              config: {
                distributionFilter: {
                  apiVersion: { policy: "include", value: [12] }
                }
              }
            }]
          }]
        }"#,
    )
    .expect("write build-profile.json5");

    let mut config = (*default_config()).clone();
    config.project_root = tmp.path().to_path_buf();
    config.modules = vec![ModuleConfig {
        name: "entry".to_string(),
        src_path: module_src,
        module_type: OhosModuleType::Entry,
    }];
    let processor = ProfileProcessor::new(Arc::new(config));

    let manifest = HapManifest {
        module_name: "entry".to_string(),
        module_type: OhosModuleType::Entry,
        device_types: vec!["phone".to_string()],
        virtual_machine: "ark12.0.6.0".to_string(),
        compile_mode: CompileMode::EsModule,
        src_entry: None,
        abilities: vec![],
        extension_abilities: vec![],
        dependencies: vec![],
        router_map: None,
        pages: None,
        main_element: None,
        description: None,
    };

    let out_path = tmp.path().join("module.json");
    processor
        .write_module_json(&manifest, &out_path, None)
        .expect("write module.json");

    let content = std::fs::read_to_string(&out_path).expect("read module.json");
    let parsed: serde_json::Value = serde_json::from_str(&content).expect("valid JSON");
    assert!(parsed["module"].get("metadata").is_none());
}

#[test]
fn test_write_loader_json_creates_parent_dirs() {
    let processor = ProfileProcessor::new(default_config());
    let json5 = single_ability_json5("entry");
    let module_cfg = make_module_cfg("entry");
    let (_, loader) = processor.process_module(&module_cfg, &json5).unwrap();

    let tmp = tempfile::tempdir().expect("tempdir");
    let out_path = tmp
        .path()
        .join("entry/build/default/intermediates/loader/default/loader.json");

    processor
        .write_loader_json(&loader, &out_path)
        .expect("write_loader_json must create parent directories");

    assert!(
        out_path.exists(),
        "loader.json must be written at the hvigor intermediate path"
    );
}

// ---------------------------------------------------------------------------
// Phase 1.6 – field-preservation regression tests
// ---------------------------------------------------------------------------

/// `packageName` is read from `oh-package.json5#name` rather than defaulting
/// to the module directory name.
#[test]
fn test_write_module_json_package_name_from_oh_package_json5() {
    use arkdown_core::{CompileMode, HapManifest, ModuleConfig, OhosModuleType};

    let tmp = tempfile::tempdir().expect("tempdir");
    let module_src = tmp.path().join("mymodule");
    std::fs::create_dir_all(module_src.join("src/main")).expect("create dirs");

    // Write oh-package.json5 with a distinct name to verify it is read.
    std::fs::write(
        module_src.join("oh-package.json5"),
        r#"{ "name": "my-scoped-package", "version": "1.0.0" }"#,
    )
    .expect("write oh-package.json5");

    // Write a minimal module.json5 so raw passthrough does not fail.
    std::fs::write(
        module_src.join("src/main/module.json5"),
        r#"{ "module": { "name": "mymodule", "type": "entry", "deviceTypes": ["phone"] } }"#,
    )
    .expect("write module.json5");

    let mut config = (*default_config()).clone();
    config.project_root = tmp.path().to_path_buf();
    config.modules = vec![ModuleConfig {
        name: "mymodule".to_string(),
        src_path: module_src.clone(),
        module_type: OhosModuleType::Entry,
    }];
    let processor = ProfileProcessor::new(Arc::new(config));

    let manifest = HapManifest {
        module_name: "mymodule".to_string(),
        module_type: OhosModuleType::Entry,
        device_types: vec!["phone".to_string()],
        virtual_machine: "ark12.0.6.0".to_string(),
        compile_mode: CompileMode::EsModule,
        src_entry: None,
        abilities: vec![],
        extension_abilities: vec![],
        dependencies: vec![],
        router_map: None,
        pages: None,
        main_element: None,
        description: None,
    };

    let out_path = tmp.path().join("module.json");
    processor
        .write_module_json(&manifest, &out_path, None)
        .expect("write_module_json must succeed");

    let content = std::fs::read_to_string(&out_path).expect("read module.json");
    let parsed: serde_json::Value = serde_json::from_str(&content).expect("valid JSON");
    assert_eq!(
        parsed["module"]["packageName"].as_str(),
        Some("my-scoped-package"),
        "packageName must be read from oh-package.json5#name"
    );
}

/// Unknown fields in `module.json5` (e.g. `requestPermissions`) are preserved
/// verbatim in the written `module.json` (preserve-and-mutate semantics).
#[test]
fn test_write_module_json_preserves_unknown_fields_from_module_json5() {
    use arkdown_core::{CompileMode, HapManifest, ModuleConfig, OhosModuleType};

    let tmp = tempfile::tempdir().expect("tempdir");
    let module_src = tmp.path().join("entry");
    std::fs::create_dir_all(module_src.join("src/main")).expect("create dirs");

    std::fs::write(
        module_src.join("oh-package.json5"),
        r#"{ "name": "entry" }"#,
    )
    .expect("write oh-package.json5");

    // module.json5 with preserved hvigor fields and a custom unknown field.
    std::fs::write(
        module_src.join("src/main/module.json5"),
        r#"{
          "module": {
            "name": "entry",
            "type": "entry",
            "deviceTypes": ["phone"],
            "requestPermissions": [{ "name": "ohos.permission.CAMERA" }],
            "definePermissions": [{ "name": "com.example.permission.CUSTOM" }],
            "proxyData": [{ "uri": "datashareproxy://com.example/item" }],
            "appStartup": "$profile:startup_config",
            "crossAppSharedConfig": "$profile:cross_app_shared_config",
            "customFutureField": "preserved"
          }
        }"#,
    )
    .expect("write module.json5");

    let mut config = (*default_config()).clone();
    config.project_root = tmp.path().to_path_buf();
    config.modules = vec![ModuleConfig {
        name: "entry".to_string(),
        src_path: module_src.clone(),
        module_type: OhosModuleType::Entry,
    }];
    let processor = ProfileProcessor::new(Arc::new(config));

    let manifest = HapManifest {
        module_name: "entry".to_string(),
        module_type: OhosModuleType::Entry,
        device_types: vec!["phone".to_string()],
        virtual_machine: "ark12.0.6.0".to_string(),
        compile_mode: CompileMode::EsModule,
        src_entry: None,
        abilities: vec![],
        extension_abilities: vec![],
        dependencies: vec![],
        router_map: None,
        pages: None,
        main_element: None,
        description: None,
    };

    let out_path = tmp.path().join("module.json");
    processor
        .write_module_json(&manifest, &out_path, None)
        .expect("write_module_json must succeed");

    let content = std::fs::read_to_string(&out_path).expect("read module.json");
    let parsed: serde_json::Value = serde_json::from_str(&content).expect("valid JSON");
    let module = &parsed["module"];

    let perms = module["requestPermissions"]
        .as_array()
        .expect("requestPermissions must be an array");
    assert_eq!(perms.len(), 1, "requestPermissions must have 1 entry");
    assert_eq!(
        perms[0]["name"].as_str(),
        Some("ohos.permission.CAMERA"),
        "requestPermissions[0].name must be preserved"
    );
    assert_eq!(
        module["customFutureField"].as_str(),
        Some("preserved"),
        "unknown future fields must be passed through verbatim"
    );
    assert_eq!(
        module["definePermissions"][0]["name"].as_str(),
        Some("com.example.permission.CUSTOM"),
        "definePermissions must be preserved"
    );
    assert_eq!(
        module["proxyData"][0]["uri"].as_str(),
        Some("datashareproxy://com.example/item"),
        "proxyData must be preserved"
    );
    assert_eq!(
        module["appStartup"].as_str(),
        Some("$profile:startup_config"),
        "appStartup must be preserved until resource rewrite"
    );
    assert_eq!(
        module["crossAppSharedConfig"].as_str(),
        Some("$profile:cross_app_shared_config"),
        "crossAppSharedConfig profile references must be preserved"
    );
}

#[test]
fn test_write_module_json_multi_projects_uses_build_profile_app_field() {
    use arkdown_core::{CompileMode, HapManifest, ModuleConfig, OhosModuleType};

    let tmp = tempfile::tempdir().expect("tempdir");
    let app_scope = tmp.path().join("AppScope");
    let module_src = tmp.path().join("entry");
    std::fs::create_dir_all(&app_scope).expect("create AppScope");
    std::fs::create_dir_all(module_src.join("src/main")).expect("create module dirs");
    std::fs::write(
        app_scope.join("app.json5"),
        r#"{
          "app": {
            "bundleName": "com.example.profiletest",
            "versionCode": 1000000,
            "versionName": "1.0.0",
            "multiProjects": true,
            "sanitizer": { "address": true }
          }
        }"#,
    )
    .expect("write app.json5");
    std::fs::write(
        module_src.join("oh-package.json5"),
        r#"{ "name": "entry" }"#,
    )
    .expect("write oh-package.json5");
    std::fs::write(
        module_src.join("src/main/module.json5"),
        r#"{ "module": { "name": "entry", "type": "entry", "deviceTypes": ["phone"] } }"#,
    )
    .expect("write module.json5");

    let mut config = (*default_config()).clone();
    config.project_root = tmp.path().to_path_buf();
    config.modules = vec![ModuleConfig {
        name: "entry".to_string(),
        src_path: module_src,
        module_type: OhosModuleType::Entry,
    }];
    let processor = ProfileProcessor::new(Arc::new(config));
    let manifest = HapManifest {
        module_name: "entry".to_string(),
        module_type: OhosModuleType::Entry,
        device_types: vec!["phone".to_string()],
        virtual_machine: "ark12.0.6.0".to_string(),
        compile_mode: CompileMode::EsModule,
        src_entry: None,
        abilities: vec![],
        extension_abilities: vec![],
        dependencies: vec![],
        router_map: None,
        pages: None,
        main_element: None,
        description: None,
    };

    let appscope_only_path = tmp.path().join("appscope-only-module.json");
    processor
        .write_module_json(&manifest, &appscope_only_path, None)
        .expect("write module.json without build-profile");
    let parsed: serde_json::Value =
        serde_json::from_str(&std::fs::read_to_string(&appscope_only_path).expect("read output"))
            .expect("valid JSON");
    assert!(
        parsed["app"].get("multiProjects").is_none(),
        "hvigor ignores AppScope/app.json5 app.multiProjects"
    );
    assert_eq!(
        parsed["app"]["sanitizer"]["address"].as_bool(),
        Some(true),
        "other AppScope app fields must still be preserved"
    );

    std::fs::write(
        tmp.path().join("build-profile.json5"),
        r#"{
          app: {
            multiProjects: true,
            products: [{
              name: "default",
              bundleName: "com.example.profiletest",
              compatibleSdkVersion: "5.0.0(12)",
              runtimeOS: "HarmonyOS"
            }]
          },
          modules: [{ name: "entry", srcPath: "./entry" }]
        }"#,
    )
    .expect("write build-profile.json5");

    let build_profile_path = tmp.path().join("build-profile-module.json");
    processor
        .write_module_json(&manifest, &build_profile_path, None)
        .expect("write module.json with build-profile");
    let parsed: serde_json::Value =
        serde_json::from_str(&std::fs::read_to_string(&build_profile_path).expect("read output"))
            .expect("valid JSON");
    assert_eq!(
        parsed["app"]["multiProjects"].as_bool(),
        Some(true),
        "build-profile app.multiProjects must be written"
    );
}

#[test]
fn test_write_module_json_does_not_remove_current_module_permissions() {
    use arkdown_core::{CompileMode, HapManifest, ModuleConfig, OhosModuleType};

    let tmp = tempfile::tempdir().expect("tempdir");
    let module_src = tmp.path().join("entry");
    std::fs::create_dir_all(module_src.join("src/main")).expect("create dirs");
    std::fs::write(
        module_src.join("oh-package.json5"),
        r#"{ "name": "entry" }"#,
    )
    .expect("write oh-package.json5");
    std::fs::write(
        module_src.join("src/main/module.json5"),
        r#"{
          "module": {
            "name": "entry",
            "type": "entry",
            "deviceTypes": ["phone"],
            "requestPermissions": [{ "name": "ohos.permission.CAMERA" }]
          }
        }"#,
    )
    .expect("write module.json5");

    let mut config = (*default_config()).clone();
    config.project_root = tmp.path().to_path_buf();
    config.product.build_option.remove_permissions = vec!["ohos.permission.CAMERA".to_string()];
    config.modules = vec![ModuleConfig {
        name: "entry".to_string(),
        src_path: module_src,
        module_type: OhosModuleType::Entry,
    }];
    let processor = ProfileProcessor::new(Arc::new(config));

    let manifest = HapManifest {
        module_name: "entry".to_string(),
        module_type: OhosModuleType::Entry,
        device_types: vec!["phone".to_string()],
        virtual_machine: "ark12.0.6.0".to_string(),
        compile_mode: CompileMode::EsModule,
        src_entry: None,
        abilities: vec![],
        extension_abilities: vec![],
        dependencies: vec![],
        router_map: None,
        pages: None,
        main_element: None,
        description: None,
    };

    let out_path = tmp.path().join("module.json");
    processor
        .write_module_json(&manifest, &out_path, None)
        .expect("write module.json");

    let content = std::fs::read_to_string(&out_path).expect("read module.json");
    let parsed: serde_json::Value = serde_json::from_str(&content).expect("valid JSON");
    assert_eq!(
        parsed["module"]["requestPermissions"][0]["name"].as_str(),
        Some("ohos.permission.CAMERA")
    );
}

#[test]
fn test_write_module_json_merges_dependency_profile_arrays_and_filters_permissions() {
    use arkdown_core::{CompileMode, HapManifest, ModuleConfig, OhosModuleType};

    let tmp = tempfile::tempdir().expect("tempdir");
    let module_src = tmp.path().join("entry");
    std::fs::create_dir_all(module_src.join("src/main")).expect("create module dirs");
    std::fs::write(
        module_src.join("oh-package.json5"),
        r#"{ "name": "entry", "dependencies": { "dep-har": "1.0.0", "shared-hsp": "1.0.0" } }"#,
    )
    .expect("write module oh-package");
    std::fs::write(
        module_src.join("src/main/module.json5"),
        r#"{
          "module": {
            "name": "entry",
            "type": "entry",
            "deviceTypes": ["phone"],
            "requestPermissions": [{ "name": "ohos.permission.CAMERA" }],
            "metadata": [{ "name": "current_meta", "value": "self" }]
          }
        }"#,
    )
    .expect("write module.json5");

    let dep_har = module_src.join("oh_modules/dep-har/src/main");
    std::fs::create_dir_all(&dep_har).expect("create dep har dirs");
    std::fs::write(
        dep_har.join("module.json5"),
        r#"{
          "app": {
            "minAPIVersion": 12
          },
          "module": {
            "name": "dep_har",
            "type": "har",
            "deviceTypes": ["phone"],
            "requestPermissions": [
              { "name": "ohos.permission.LOCATION" },
              { "name": "ohos.permission.MICROPHONE" },
              { "name": "ohos.permission.CAMERA" }
            ],
            "metadata": [
              { "name": "current_meta", "value": "dep_duplicate" },
              { "name": "dep_meta", "resource": "$profile:dep_profile" }
            ]
          }
        }"#,
    )
    .expect("write dep har module.json5");

    let dep_hsp = module_src.join("oh_modules/shared-hsp/src/main");
    std::fs::create_dir_all(&dep_hsp).expect("create dep hsp dirs");
    std::fs::write(
        dep_hsp.join("module.json5"),
        r#"{
          "app": {
            "minAPIVersion": 12
          },
          "module": {
            "name": "shared_hsp",
            "type": "shared",
            "deviceTypes": ["phone"],
            "requestPermissions": [{ "name": "ohos.permission.STORAGE" }]
          }
        }"#,
    )
    .expect("write dep hsp module.json5");

    let mut config = (*default_config()).clone();
    config.project_root = tmp.path().to_path_buf();
    config.product.build_option.remove_permissions = vec![
        "ohos.permission.MICROPHONE".to_string(),
        "ohos.permission.CAMERA".to_string(),
    ];
    config.modules = vec![ModuleConfig {
        name: "entry".to_string(),
        src_path: module_src,
        module_type: OhosModuleType::Entry,
    }];
    let processor = ProfileProcessor::new(Arc::new(config));

    let manifest = HapManifest {
        module_name: "entry".to_string(),
        module_type: OhosModuleType::Entry,
        device_types: vec!["phone".to_string()],
        virtual_machine: "ark12.0.6.0".to_string(),
        compile_mode: CompileMode::EsModule,
        src_entry: None,
        abilities: vec![],
        extension_abilities: vec![],
        dependencies: vec![],
        router_map: None,
        pages: None,
        main_element: None,
        description: None,
    };

    let out_path = tmp.path().join("module.json");
    processor
        .write_module_json(&manifest, &out_path, None)
        .expect("write module.json");

    let content = std::fs::read_to_string(&out_path).expect("read module.json");
    let parsed: serde_json::Value = serde_json::from_str(&content).expect("valid JSON");
    let permission_names: Vec<_> = parsed["module"]["requestPermissions"]
        .as_array()
        .expect("requestPermissions array")
        .iter()
        .filter_map(|permission| permission["name"].as_str())
        .collect();
    assert_eq!(
        permission_names,
        vec![
            "ohos.permission.CAMERA",
            "ohos.permission.LOCATION",
            "ohos.permission.STORAGE"
        ],
        "current module permission must be kept, duplicate dep permission skipped, and removePermissions must filter dependency-only permissions"
    );

    let metadata = parsed["module"]["metadata"]
        .as_array()
        .expect("metadata array");
    assert_eq!(metadata.len(), 2);
    assert_eq!(metadata[0]["name"].as_str(), Some("current_meta"));
    assert_eq!(metadata[0]["value"].as_str(), Some("self"));
    assert_eq!(metadata[1]["name"].as_str(), Some("dep_meta"));
}

#[test]
fn test_write_module_json_dependency_profile_arrays_follow_declared_transitive_scoped_deps() {
    use arkdown_core::{CompileMode, HapManifest, ModuleConfig, OhosModuleType};

    let tmp = tempfile::tempdir().expect("tempdir");
    let module_src = tmp.path().join("entry");
    std::fs::create_dir_all(module_src.join("src/main")).expect("create module dirs");
    std::fs::write(
        module_src.join("oh-package.json5"),
        r#"{ "name": "entry", "dependencies": { "source-har": "1.0.0" } }"#,
    )
    .expect("write module oh-package");
    std::fs::write(
        module_src.join("src/main/module.json5"),
        r#"{
          "module": {
            "name": "entry",
            "type": "entry",
            "deviceTypes": ["phone"]
          }
        }"#,
    )
    .expect("write module.json5");

    let source_har = module_src.join("oh_modules/source-har");
    std::fs::create_dir_all(source_har.join("src/main")).expect("create source har dirs");
    std::fs::write(
        source_har.join("oh-package.json5"),
        r#"{ "name": "source-har", "dependencies": { "@scope/shared-hsp": "1.0.0" } }"#,
    )
    .expect("write source har oh-package");
    std::fs::write(
        source_har.join("src/main/module.json5"),
        r#"{
          "app": { "minAPIVersion": 12 },
          "module": {
            "name": "source_har",
            "type": "har",
            "deviceTypes": ["phone"],
            "metadata": [{ "name": "source_meta" }]
          }
        }"#,
    )
    .expect("write source har module.json5");

    let scoped_hsp = source_har.join("oh_modules/@scope/shared-hsp/src/main");
    std::fs::create_dir_all(&scoped_hsp).expect("create scoped hsp dirs");
    std::fs::write(
        scoped_hsp.join("module.json5"),
        r#"{
          "app": { "minAPIVersion": 12 },
          "module": {
            "name": "shared_hsp",
            "type": "shared",
            "deviceTypes": ["phone"],
            "requestPermissions": [{ "name": "ohos.permission.LOCATION" }],
            "metadata": [{ "name": "shared_meta" }]
          }
        }"#,
    )
    .expect("write scoped hsp module.json5");

    let undeclared_hsp = module_src.join("oh_modules/undeclared-hsp/src/main");
    std::fs::create_dir_all(&undeclared_hsp).expect("create undeclared hsp dirs");
    std::fs::write(
        undeclared_hsp.join("module.json5"),
        r#"{
          "app": { "minAPIVersion": 13 },
          "module": {
            "name": "undeclared_hsp",
            "type": "shared",
            "deviceTypes": ["phone"],
            "requestPermissions": [{ "name": "ohos.permission.MICROPHONE" }],
            "metadata": [{ "name": "undeclared_meta" }]
          }
        }"#,
    )
    .expect("write undeclared hsp module.json5");

    let mut config = (*default_config()).clone();
    config.project_root = tmp.path().to_path_buf();
    config.product.compatible_sdk_version = "5.0.0(12)".to_string();
    config.modules = vec![ModuleConfig {
        name: "entry".to_string(),
        src_path: module_src,
        module_type: OhosModuleType::Entry,
    }];
    let processor = ProfileProcessor::new(Arc::new(config));

    let manifest = HapManifest {
        module_name: "entry".to_string(),
        module_type: OhosModuleType::Entry,
        device_types: vec!["phone".to_string()],
        virtual_machine: "ark12.0.6.0".to_string(),
        compile_mode: CompileMode::EsModule,
        src_entry: None,
        abilities: vec![],
        extension_abilities: vec![],
        dependencies: vec![],
        router_map: None,
        pages: None,
        main_element: None,
        description: None,
    };

    let out_path = tmp.path().join("module.json");
    processor
        .write_module_json(&manifest, &out_path, None)
        .expect("write module.json");

    let content = std::fs::read_to_string(&out_path).expect("read module.json");
    let parsed: serde_json::Value = serde_json::from_str(&content).expect("valid JSON");
    let metadata_names: Vec<_> = parsed["module"]["metadata"]
        .as_array()
        .expect("metadata array")
        .iter()
        .filter_map(|metadata| metadata["name"].as_str())
        .collect();
    assert_eq!(metadata_names, vec!["source_meta", "shared_meta"]);

    let permission_names: Vec<_> = parsed["module"]["requestPermissions"]
        .as_array()
        .expect("requestPermissions array")
        .iter()
        .filter_map(|permission| permission["name"].as_str())
        .collect();
    assert_eq!(permission_names, vec!["ohos.permission.LOCATION"]);
}

#[test]
fn test_write_module_json_rejects_dependency_with_higher_min_api() {
    use arkdown_core::{CompileMode, HapManifest, ModuleConfig, OhosModuleType};

    let tmp = tempfile::tempdir().expect("tempdir");
    let module_src = tmp.path().join("entry");
    std::fs::create_dir_all(module_src.join("src/main")).expect("create module dirs");
    std::fs::write(
        module_src.join("oh-package.json5"),
        r#"{ "name": "entry", "dependencies": { "dep-har": "1.0.0" } }"#,
    )
    .expect("write module oh-package");
    std::fs::write(
        module_src.join("src/main/module.json5"),
        r#"{
          "module": {
            "name": "entry",
            "type": "entry",
            "deviceTypes": ["phone"]
          }
        }"#,
    )
    .expect("write module.json5");

    let dep_har = module_src.join("oh_modules/dep-har/src/main");
    std::fs::create_dir_all(&dep_har).expect("create dep dirs");
    std::fs::write(
        dep_har.join("module.json5"),
        r#"{
          "app": {
            "minAPIVersion": 13
          },
          "module": {
            "name": "dep_har",
            "type": "har",
            "deviceTypes": ["phone"],
            "requestPermissions": [{ "name": "ohos.permission.LOCATION" }]
          }
        }"#,
    )
    .expect("write dep module.json5");

    let mut config = (*default_config()).clone();
    config.project_root = tmp.path().to_path_buf();
    config.product.compatible_sdk_version = "5.0.0(12)".to_string();
    config.modules = vec![ModuleConfig {
        name: "entry".to_string(),
        src_path: module_src,
        module_type: OhosModuleType::Entry,
    }];
    let processor = ProfileProcessor::new(Arc::new(config));

    let manifest = HapManifest {
        module_name: "entry".to_string(),
        module_type: OhosModuleType::Entry,
        device_types: vec!["phone".to_string()],
        virtual_machine: "ark12.0.6.0".to_string(),
        compile_mode: CompileMode::EsModule,
        src_entry: None,
        abilities: vec![],
        extension_abilities: vec![],
        dependencies: vec![],
        router_map: None,
        pages: None,
        main_element: None,
        description: None,
    };

    let err = processor
        .write_module_json(&manifest, &tmp.path().join("module.json"), None)
        .expect_err("dependency min API above compatible API must fail");
    assert!(
        err.to_string().contains("HAR_UNSUPPORTED_API_VERSION"),
        "unexpected error: {err}"
    );
}

#[test]
fn test_write_module_json_rejects_hsp_dependency_device_type_mismatch() {
    use arkdown_core::{CompileMode, HapManifest, ModuleConfig, OhosModuleType};

    let tmp = tempfile::tempdir().expect("tempdir");
    let module_src = tmp.path().join("entry");
    std::fs::create_dir_all(module_src.join("src/main")).expect("create module dirs");
    std::fs::write(
        module_src.join("oh-package.json5"),
        r#"{ "name": "entry", "dependencies": { "shared-hsp": "1.0.0" } }"#,
    )
    .expect("write module oh-package");
    std::fs::write(
        module_src.join("src/main/module.json5"),
        r#"{
          "module": {
            "name": "entry",
            "type": "entry",
            "deviceTypes": ["tablet"]
          }
        }"#,
    )
    .expect("write module.json5");

    let dep_hsp = module_src.join("oh_modules/shared-hsp/src/main");
    std::fs::create_dir_all(&dep_hsp).expect("create dep dirs");
    std::fs::write(
        dep_hsp.join("module.json5"),
        r#"{
          "app": {
            "minAPIVersion": 12
          },
          "module": {
            "name": "shared_hsp",
            "type": "shared",
            "deviceTypes": ["phone"],
            "requestPermissions": [{ "name": "ohos.permission.LOCATION" }]
          }
        }"#,
    )
    .expect("write dep module.json5");

    let mut config = (*default_config()).clone();
    config.project_root = tmp.path().to_path_buf();
    config.modules = vec![ModuleConfig {
        name: "entry".to_string(),
        src_path: module_src,
        module_type: OhosModuleType::Entry,
    }];
    let processor = ProfileProcessor::new(Arc::new(config));

    let manifest = HapManifest {
        module_name: "entry".to_string(),
        module_type: OhosModuleType::Entry,
        device_types: vec!["tablet".to_string()],
        virtual_machine: "ark12.0.6.0".to_string(),
        compile_mode: CompileMode::EsModule,
        src_entry: None,
        abilities: vec![],
        extension_abilities: vec![],
        dependencies: vec![],
        router_map: None,
        pages: None,
        main_element: None,
        description: None,
    };

    let err = processor
        .write_module_json(&manifest, &tmp.path().join("module.json"), None)
        .expect_err("HSP dependency deviceTypes mismatch must fail");
    assert!(
        err.to_string()
            .contains("CURRENT_MODULE_NOT_SUPPORT_THE_DEVICE_TYPE"),
        "unexpected error: {err}"
    );
}

/// `installationFree` is `true` when `bundleType` is `atomicService`.
#[test]
fn test_write_module_json_installation_free_for_atomic_service() {
    use arkdown_core::{CompileMode, HapManifest, OhosModuleType};

    let tmp = tempfile::tempdir().expect("tempdir");
    // Write an app.json5 with bundleType = atomicService.
    let app_scope = tmp.path().join("AppScope");
    std::fs::create_dir_all(&app_scope).expect("create AppScope");
    std::fs::write(
        app_scope.join("app.json5"),
        r#"{ "app": { "bundleName": "com.example.atomic", "versionCode": 1000000,
                      "versionName": "1.0.0", "bundleType": "atomicService" } }"#,
    )
    .expect("write app.json5");

    let mut config = (*default_config()).clone();
    config.project_root = tmp.path().to_path_buf();
    config.product.bundle_name = "com.example.atomic".to_string();
    let processor = ProfileProcessor::new(Arc::new(config));

    let manifest = HapManifest {
        module_name: "entry".to_string(),
        module_type: OhosModuleType::Entry,
        device_types: vec!["phone".to_string()],
        virtual_machine: "ark12.0.6.0".to_string(),
        compile_mode: CompileMode::EsModule,
        src_entry: None,
        abilities: vec![],
        extension_abilities: vec![],
        dependencies: vec![],
        router_map: None,
        pages: None,
        main_element: None,
        description: None,
    };

    let out_path = tmp.path().join("module.json");
    processor
        .write_module_json(&manifest, &out_path, None)
        .expect("write_module_json must succeed");

    let content = std::fs::read_to_string(&out_path).expect("read module.json");
    let parsed: serde_json::Value = serde_json::from_str(&content).expect("valid JSON");
    assert_eq!(
        parsed["module"]["installationFree"].as_bool(),
        Some(true),
        "installationFree must be true for atomicService bundles"
    );
}
