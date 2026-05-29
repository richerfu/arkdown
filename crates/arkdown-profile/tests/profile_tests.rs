//! Integration tests for `arkdown-profile` public API.

use std::path::PathBuf;
use std::sync::Arc;

use arkdown_core::{
    AbilityConfig, ArkdownConfig, BuildMode, BuildOption, ModuleConfig, ModuleJson5,
    ModuleJson5Module, OhosModuleType, OutputTarget, ProductConfig, SdkConfig, StrictModeConfig,
};
use arkdown_profile::{generate_loader_json, process_profile, ProfileProcessor};

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
                preload_so: vec![],
            },
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
            pages: Some("$profile:main_pages".to_string()),
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
            cross_app_shared_config: vec![],
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
    let manifest = process_profile(&json5, 12, "com.example.test", CompileMode::EsModule);
    assert_eq!(
        manifest.virtual_machine, "ark12.0.1.0",
        "API 12 must produce virtual_machine = 'ark12.0.1.0'"
    );
}

#[test]
fn test_process_profile_virtual_machine_api9() {
    use arkdown_core::CompileMode;
    let json5 = single_ability_json5("entry");
    let manifest = process_profile(&json5, 9, "com.example.test", CompileMode::EsModule);
    assert_eq!(manifest.virtual_machine, "ark9.0.1.0");
}

#[test]
fn test_process_profile_module_name_preserved() {
    use arkdown_core::CompileMode;
    let json5 = single_ability_json5("my_module");
    let manifest = process_profile(&json5, 12, "com.example", CompileMode::EsModule);
    assert_eq!(manifest.module_name, "my_module");
}

#[test]
fn test_process_profile_abilities_mapped() {
    use arkdown_core::CompileMode;
    let json5 = single_ability_json5("entry");
    let manifest = process_profile(&json5, 12, "com.example", CompileMode::EsModule);
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
            cross_app_shared_config: vec![],
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
    let manifest = process_profile(&json5, 12, "com.example", CompileMode::EsModule);
    let module_cfg = make_module_cfg("feature");
    let loader = generate_loader_json(&config, &module_cfg, &manifest);

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
    let manifest = process_profile(&json5, 12, "com.example", CompileMode::EsModule);
    let module_cfg = make_module_cfg("entry");
    let loader = generate_loader_json(&config, &module_cfg, &manifest);

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
fn test_write_module_json_resolves_resource_ids_into_abilities() {
    use arkdown_core::{AbilityEntry, CompileMode, HapManifest, OhosModuleType};
    use std::collections::HashMap;

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

    let mut resource_ids: HashMap<String, u32> = HashMap::new();
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
