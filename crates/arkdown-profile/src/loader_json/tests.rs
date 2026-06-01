use std::fs;
use std::path::PathBuf;

use arkdown_core::{
    AbilityEntry, ArkOptions, ArkdownConfig, BuildMode, BuildOption, HapManifest, ModuleConfig,
    OhosModuleType, OutputTarget, ProductConfig, RuntimeOnlyConfig, SdkConfig, StrictModeConfig,
};

use super::*;

fn make_config(use_normalized_ohm_url: bool) -> ArkdownConfig {
    ArkdownConfig {
        project_root: PathBuf::from("/project"),
        build_mode: BuildMode::Debug,
        output_target: OutputTarget::Hap,
        product: ProductConfig {
            name: "default".to_string(),
            bundle_name: "com.example.app".to_string(),
            compatible_sdk_version: "5.0.0(12)".to_string(),
            runtime_os: "HarmonyOS".to_string(),
            signing_config: None,
            build_option: BuildOption {
                strict_mode: StrictModeConfig {
                    case_sensitive_check: false,
                    use_normalized_ohm_url,
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
    }
}

fn make_module_cfg() -> ModuleConfig {
    ModuleConfig {
        name: "entry".to_string(),
        src_path: PathBuf::from("/project/entry"),
        module_type: OhosModuleType::Entry,
    }
}

fn make_manifest() -> HapManifest {
    HapManifest {
        module_name: "entry".to_string(),
        module_type: OhosModuleType::Entry,
        device_types: vec!["phone".to_string()],
        virtual_machine: "ark1.2.0.0".to_string(),
        compile_mode: arkdown_core::CompileMode::EsModule,
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
        src_entry: None,
    }
}

#[test]
fn test_loader_json_collects_bytecode_har_declaration_entry() {
    let tmp = tempfile::tempdir().expect("tempdir");
    let project_root = tmp.path().to_path_buf();
    let module_root = project_root.join("entry");
    let pkg_dir = module_root.join("oh_modules").join("bytecode-har");
    fs::create_dir_all(pkg_dir.join("ets")).expect("create bytecode har");
    fs::write(
        pkg_dir.join("ets").join("modules.abc"),
        b"PANDA\x00\x00\x00",
    )
    .expect("write abc");
    fs::write(
        pkg_dir.join("oh-package.json5"),
        r#"{
              name: "bytecode-har",
              compatibleSdkVersion: 12,
              metadata: {
                byteCodeHar: true,
                declarationEntry: [
                  "@normalized:N&entry&com.example&bytecode-har/src/main/ets/Worker&1.0.0",
                  "@normalized:N&entry&com.example&bytecode-har/src/main/ets/Worker&1.0.0",
                  "@normalized:N&entry&com.example&bytecode-har/src/main/ets/runtime&1.0.0"
                ]
              }
            }"#,
    )
    .expect("write oh-package");

    let mut config = make_config(true);
    config.project_root = project_root;
    let mut module_cfg = make_module_cfg();
    module_cfg.src_path = module_root;
    let loader = generate_loader_json(&config, &module_cfg, &make_manifest());

    assert_eq!(
        loader.declaration_entry,
        vec![
            "@normalized:N&entry&com.example&bytecode-har/src/main/ets/Worker&1.0.0",
            "@normalized:N&entry&com.example&bytecode-har/src/main/ets/runtime&1.0.0",
        ]
    );
    let entry = loader
        .byte_code_har_info
        .get("bytecode-har")
        .expect("bytecode har info");
    assert!(entry.abc_path.ends_with("ets/modules.abc"));
}

/// Verify loader.json round-trips through serde correctly.
#[test]
fn test_loader_json_serialization() {
    let config = make_config(false);
    let module_cfg = make_module_cfg();
    let manifest = make_manifest();

    let loader = generate_loader_json(&config, &module_cfg, &manifest);

    // Serialize to JSON string
    let json_str = serde_json::to_string_pretty(&loader).expect("serialization failed");

    // Deserialize back
    let loader2: LoaderJson = serde_json::from_str(&json_str).expect("deserialization failed");

    // Verify serialized hvigor fields survived the round-trip. Internal
    // bundler-only fields are intentionally not part of loader.json.
    assert_eq!(loader2.compile_mode, "esmodule");
    assert_eq!(loader2.package_manager_type, "ohpm");
    assert_eq!(loader2.project_root_path, "/project");
    assert_eq!(
        loader2.node_modules_path,
        "/project/entry/build/default/intermediates/loader_out/default/node_modules"
    );
    assert!(loader2.hvigor_compile_entry.is_empty());
    assert!(loader2.other_compile_files.is_empty());
    assert_eq!(loader2.module_name, "entry");
    assert!(loader2.module_path_map.contains_key("entry"));
    assert_eq!(loader2.module_path_map["entry"], "/project/entry");
    assert_eq!(loader2.an_build_mode, "type");
}

#[test]
fn test_loader_json_uses_normalized_ohm_url() {
    let config = make_config(true);
    let module_cfg = make_module_cfg();
    let manifest = make_manifest();

    let loader = generate_loader_json(&config, &module_cfg, &manifest);
    assert!(loader.use_normalized_ohm_url);
}

#[test]
fn test_loader_json_hvigor_keys() {
    let config = make_config(false);
    let module_cfg = make_module_cfg();
    let manifest = make_manifest();

    let loader = generate_loader_json(&config, &module_cfg, &manifest);
    let value = serde_json::to_value(&loader).expect("serialization failed");
    // serde_json::to_value produces a BTreeMap (alphabetical) without the
    // preserve_order feature, so compare key *sets* rather than ordered vecs.
    let keys: std::collections::BTreeSet<String> = value
        .as_object()
        .expect("loader.json must serialize as object")
        .keys()
        .cloned()
        .collect();

    let expected_keys: std::collections::BTreeSet<String> = vec![
        "modulePathMap",
        "compileMode",
        "projectRootPath",
        "nodeModulesPath",
        "byteCodeHarInfo",
        "declarationEntry",
        "moduleName",
        "hspNameOhmMap",
        "harNameOhmMap",
        "packageManagerType",
        "compileEntry",
        "otherCompileFiles",
        "dynamicImportLibInfo",
        "routerMap",
        "hspResourcesMap",
        "updateVersionInfo",
        "anBuildOutPut",
        "anBuildMode",
    ]
    .into_iter()
    .map(String::from)
    .collect();

    assert_eq!(keys, expected_keys);
    assert!(value.get("apiVersion").is_none());
    assert!(value.get("buildMode").is_none());
    assert!(value.get("supportChunks").is_none());
    assert!(value.get("useNormalizedOHMUrl").is_none());
    assert!(value.get("pkgNameToPkgBriefInfo").is_none());
    assert!(value.get("workers").is_none());
}

#[test]
fn test_loader_json_serializes_minimal_hvigor_shape() {
    let config = make_config(false);
    let module_cfg = make_module_cfg();
    let manifest = make_manifest();

    let loader = generate_loader_json(&config, &module_cfg, &manifest);
    let json = serde_json::to_string_pretty(&loader).expect("serialization failed");

    assert_eq!(
        json,
        r#"{
  "modulePathMap": {
    "entry": "/project/entry"
  },
  "compileMode": "esmodule",
  "projectRootPath": "/project",
  "nodeModulesPath": "/project/entry/build/default/intermediates/loader_out/default/node_modules",
  "byteCodeHarInfo": {},
  "declarationEntry": [],
  "moduleName": "entry",
  "hspNameOhmMap": {},
  "harNameOhmMap": {},
  "packageManagerType": "ohpm",
  "compileEntry": [],
  "otherCompileFiles": [],
  "dynamicImportLibInfo": {},
  "routerMap": [],
  "hspResourcesMap": {},
  "updateVersionInfo": {},
  "anBuildOutPut": "/project/entry/build/default/intermediates/loader_out/default/an/arm64-v8a",
  "anBuildMode": "type"
}"#
    );
}

#[test]
fn test_loader_json_pkg_brief_info_populated() {
    let config = make_config(false);
    let module_cfg = make_module_cfg();
    let manifest = make_manifest();

    let loader = generate_loader_json(&config, &module_cfg, &manifest);
    assert!(
        loader.pkg_name_to_pkg_brief_info.contains_key("entry"),
        "expected 'entry' key in pkgNameToPkgBriefInfo"
    );
    let info = &loader.pkg_name_to_pkg_brief_info["entry"];
    assert_eq!(info.pkg_name, "entry");
    assert_eq!(info.pkg_root, "/project/entry");
}

#[test]
fn test_loader_json_cache_and_build_paths() {
    let config = make_config(false);
    let module_cfg = make_module_cfg();
    let manifest = make_manifest();

    let loader = generate_loader_json(&config, &module_cfg, &manifest);
    assert!(
        loader.cache_path.starts_with("/project/entry"),
        "cache_path should be under module root"
    );
    assert!(
        loader.ace_module_build.starts_with("/project/entry"),
        "ace_module_build should be under module root"
    );
    assert_eq!(
        loader.ace_module_build,
        "/project/entry/build/default/intermediates/loader_out/default/ets/"
    );
}

#[test]
fn test_loader_json_module_root_path_is_src_main() {
    let config = make_config(false);
    let module_cfg = make_module_cfg();
    let manifest = make_manifest();

    let loader = generate_loader_json(&config, &module_cfg, &manifest);
    assert_eq!(
        loader.module_root_path, "/project/entry/src/main",
        "module_root_path must point to src/main, not the module root"
    );
}

#[test]
fn test_loader_json_module_name_populated() {
    let config = make_config(false);
    let module_cfg = make_module_cfg();
    let manifest = make_manifest();

    let loader = generate_loader_json(&config, &module_cfg, &manifest);
    assert_eq!(loader.module_name, "entry");
}

#[test]
fn test_loader_json_api_version() {
    let config = make_config(false);
    let module_cfg = make_module_cfg();
    let manifest = make_manifest();

    let loader = generate_loader_json(&config, &module_cfg, &manifest);
    assert_eq!(loader.api_version, 12);
}

#[test]
fn test_loader_json_module_path_map() {
    let config = make_config(false);
    let module_cfg = make_module_cfg();
    let manifest = make_manifest();

    let loader = generate_loader_json(&config, &module_cfg, &manifest);
    assert!(
        loader.module_path_map.contains_key("entry"),
        "modulePathMap must contain the module name key"
    );
    assert_eq!(
        loader.module_path_map["entry"], "/project/entry",
        "modulePathMap value must be the module root (not src/main)"
    );
}

#[test]
fn test_loader_json_normalizes_dot_segments_in_hvigor_paths() {
    let config = ArkdownConfig {
        project_root: PathBuf::from("/project/./app"),
        ..make_config(false)
    };
    let module_cfg = ModuleConfig {
        src_path: PathBuf::from("/project/./app/./entry"),
        ..make_module_cfg()
    };
    let manifest = make_manifest();

    let loader = generate_loader_json(&config, &module_cfg, &manifest);
    assert_eq!(loader.project_root_path, "/project/app");
    assert_eq!(loader.module_path_map["entry"], "/project/app/entry");
    assert_eq!(
        loader.node_modules_path,
        "/project/app/entry/build/default/intermediates/loader_out/default/node_modules"
    );
    assert_eq!(
        loader.an_build_out_put,
        "/project/app/entry/build/default/intermediates/loader_out/default/an/arm64-v8a"
    );
}

#[test]
fn test_loader_json_fallback_an_build_is_false() {
    let config = make_config(false);
    let module_cfg = make_module_cfg();
    let manifest = make_manifest();

    let loader = generate_loader_json(&config, &module_cfg, &manifest);
    assert!(!loader.fallback_an_build);
}

/// Verify that relative src_path / project_root inputs produce absolute paths in the
/// serialized loader.json fields.  hvigor always writes absolute paths; arkdown must
/// match that behaviour regardless of how the CLI was invoked.
#[test]
fn test_loader_json_relative_paths_become_absolute() {
    let cwd = std::env::current_dir().expect("cwd");
    let config = ArkdownConfig {
        project_root: PathBuf::from("relative_proj_root_test"),
        ..make_config(false)
    };
    let module_cfg = ModuleConfig {
        src_path: PathBuf::from("relative_proj_root_test/entry"),
        ..make_module_cfg()
    };
    let manifest = make_manifest();

    let loader = generate_loader_json(&config, &module_cfg, &manifest);

    let expected_project = cwd.join("relative_proj_root_test");
    let expected_module = cwd.join("relative_proj_root_test/entry");

    assert!(
        std::path::Path::new(&loader.project_root_path).is_absolute(),
        "projectRootPath must be absolute: {}",
        loader.project_root_path
    );
    assert_eq!(
        loader.project_root_path,
        expected_project.to_string_lossy().as_ref()
    );

    assert!(
        std::path::Path::new(&loader.node_modules_path).is_absolute(),
        "nodeModulesPath must be absolute: {}",
        loader.node_modules_path
    );
    assert!(
        loader
            .node_modules_path
            .starts_with(expected_module.to_string_lossy().as_ref()),
        "nodeModulesPath must be under module root"
    );

    assert!(
        std::path::Path::new(&loader.an_build_out_put).is_absolute(),
        "anBuildOutPut must be absolute: {}",
        loader.an_build_out_put
    );

    for (_, path) in &loader.module_path_map {
        assert!(
            std::path::Path::new(path).is_absolute(),
            "modulePathMap value must be absolute: {path}"
        );
    }
}

/// Verify the serialized key set exactly matches the golden hvigor output (18 fields).
#[test]
fn test_loader_json_golden_hvigor_key_set() {
    let expected_keys: std::collections::BTreeSet<&str> = [
        "modulePathMap",
        "compileMode",
        "projectRootPath",
        "nodeModulesPath",
        "byteCodeHarInfo",
        "declarationEntry",
        "moduleName",
        "hspNameOhmMap",
        "harNameOhmMap",
        "packageManagerType",
        "compileEntry",
        "otherCompileFiles",
        "dynamicImportLibInfo",
        "routerMap",
        "hspResourcesMap",
        "updateVersionInfo",
        "anBuildOutPut",
        "anBuildMode",
    ]
    .iter()
    .copied()
    .collect();

    let config = make_config(false);
    let module_cfg = make_module_cfg();
    let manifest = make_manifest();

    let loader = generate_loader_json(&config, &module_cfg, &manifest);
    let value = serde_json::to_value(&loader).expect("serialization failed");
    let actual_keys: std::collections::BTreeSet<&str> = value
        .as_object()
        .unwrap()
        .keys()
        .map(|k| k.as_str())
        .collect();

    assert_eq!(
        actual_keys, expected_keys,
        "serialized key set must match golden hvigor output"
    );
}

/// Verify all path-valued fields in the serialized loader.json are absolute.
#[test]
fn test_loader_json_serialized_paths_are_absolute() {
    let config = make_config(false); // project_root = /project
    let module_cfg = make_module_cfg(); // src_path = /project/entry
    let manifest = make_manifest();

    let loader = generate_loader_json(&config, &module_cfg, &manifest);
    let value = serde_json::to_value(&loader).expect("serialization failed");

    let project_root = value["projectRootPath"].as_str().unwrap();
    assert!(
        std::path::Path::new(project_root).is_absolute(),
        "projectRootPath must be absolute: {project_root}"
    );

    for (_, path) in value["modulePathMap"].as_object().unwrap() {
        let p = path.as_str().unwrap();
        assert!(
            std::path::Path::new(p).is_absolute(),
            "modulePathMap value must be absolute: {p}"
        );
    }

    let npm = value["nodeModulesPath"].as_str().unwrap();
    assert!(
        std::path::Path::new(npm).is_absolute(),
        "nodeModulesPath must be absolute: {npm}"
    );

    let an = value["anBuildOutPut"].as_str().unwrap();
    assert!(
        std::path::Path::new(an).is_absolute(),
        "anBuildOutPut must be absolute: {an}"
    );
}

#[test]
fn test_loader_json_serializes_recursive_workers_when_present() {
    let tmp = tempfile::tempdir().expect("create temp dir");
    let project_root = tmp.path();
    let module_root = project_root.join("entry");
    let workers_dir = module_root.join("src/main/ets/workers/nested");
    std::fs::create_dir_all(&workers_dir).expect("create workers dir");
    std::fs::write(module_root.join("src/main/ets/workers/MainWorker.ets"), "").unwrap();
    std::fs::write(workers_dir.join("ChildWorker.ts"), "").unwrap();
    std::fs::write(workers_dir.join("ignored.txt"), "").unwrap();

    let mut config = make_config(false);
    config.project_root = project_root.to_path_buf();
    let mut module_cfg = make_module_cfg();
    module_cfg.src_path = module_root.clone();
    let manifest = make_manifest();

    let loader = generate_loader_json(&config, &module_cfg, &manifest);
    assert_eq!(
        loader.workers,
        vec![
            to_absolute_path_string(&module_root.join("src/main/ets/workers/MainWorker.ets")),
            to_absolute_path_string(
                &module_root.join("src/main/ets/workers/nested/ChildWorker.ts")
            ),
        ]
    );
    assert!(loader
        .compile_entry
        .contains(&"./ets/workers/MainWorker.ets".to_string()));
    assert!(loader
        .compile_entry
        .contains(&"./ets/workers/nested/ChildWorker.ts".to_string()));

    let value = serde_json::to_value(&loader).expect("serialize loader");
    assert_eq!(
        value
            .get("workers")
            .and_then(serde_json::Value::as_array)
            .map(Vec::len),
        Some(2)
    );
}

#[test]
fn test_loader_json_serialized_compile_entry_is_separate_from_internal_entries() {
    let config = make_config(false);
    let module_cfg = make_module_cfg();
    let manifest = make_manifest();

    let loader = generate_loader_json(&config, &module_cfg, &manifest);
    assert_eq!(
        loader.compile_entry,
        vec!["./ets/entryability/EntryAbility.ets".to_string()]
    );
    assert!(loader.hvigor_compile_entry.is_empty());

    let value = serde_json::to_value(&loader).expect("serialization failed");
    assert_eq!(
        value
            .get("compileEntry")
            .and_then(|v| v.as_array())
            .unwrap()
            .len(),
        0
    );
}

#[test]
fn test_har_uses_oh_package_main_as_internal_entry() {
    let tmp = tempfile::tempdir().expect("create temp dir");
    let module_root = tmp.path().join("feature");
    std::fs::create_dir_all(&module_root).unwrap();
    std::fs::write(
        module_root.join("oh-package.json5"),
        r#"{ name: "feature", main: "Index.ets" }"#,
    )
    .unwrap();
    std::fs::write(module_root.join("Index.ets"), "export {}").unwrap();

    let config = make_config(false);
    let module_cfg = ModuleConfig {
        name: "feature".to_string(),
        src_path: module_root.clone(),
        module_type: OhosModuleType::Har,
    };
    let mut manifest = make_manifest();
    manifest.module_name = "feature".to_string();
    manifest.module_type = OhosModuleType::Har;
    manifest.abilities.clear();
    manifest.pages = None;

    let loader = generate_loader_json(&config, &module_cfg, &manifest);
    assert_eq!(
        loader.compile_entry,
        vec![to_absolute_path_string(&module_root.join("Index.ets"))]
    );
    assert!(loader.hvigor_compile_entry.is_empty());
}

#[test]
fn test_router_map_metadata_does_not_become_compile_entry() {
    let tmp = tempfile::tempdir().expect("create temp dir");
    let module_root = tmp.path().join("entry");
    let profile_dir = module_root.join("src/main/resources/base/profile");
    std::fs::create_dir_all(&profile_dir).unwrap();
    std::fs::write(
            profile_dir.join("router_map.json"),
            r#"{"routerMap":[{"name":"detail","pageSourceFile":"src/main/ets/pages/Detail.ets","buildFunction":"DetailBuilder"}]}"#,
        )
        .unwrap();

    let config = make_config(false);
    let module_cfg = ModuleConfig {
        name: "entry".to_string(),
        src_path: module_root.clone(),
        module_type: OhosModuleType::Entry,
    };
    let mut manifest = make_manifest();
    manifest.pages = None;
    manifest.router_map = Some("$profile:router_map".to_string());

    let loader = generate_loader_json(&config, &module_cfg, &manifest);
    assert_eq!(
        loader.compile_entry,
        vec!["./ets/entryability/EntryAbility.ets".to_string()]
    );
    assert_eq!(loader.router_map.len(), 1);
    assert!(loader.router_map[0]["pageSourceFile"]
        .as_str()
        .is_some_and(|path| path.ends_with("src/main/ets/pages/Detail.ets")));
}

/// Verify `dynamicImportLibInfo` is populated when an oh_modules dep exists.
#[test]
fn test_loader_json_dynamic_import_lib_info_populated_from_oh_modules() {
    let tmp = tempfile::tempdir().expect("create temp dir");
    let project_root = tmp.path().to_path_buf();
    let module_root = project_root.join("entry");

    // Write oh-package.json5 with a dynamicDependency.
    let oh_pkg_content = r#"{
            name: "entry",
            version: "1.0.0",
            dynamicDependencies: {
                "@remote/shared": "1.0.0"
            }
        }"#;
    std::fs::create_dir_all(&module_root).expect("create module dir");
    std::fs::write(module_root.join("oh-package.json5"), oh_pkg_content)
        .expect("write oh-package.json5");

    // Create the package under oh_modules with an entry file.
    let pkg_dir = project_root
        .join("oh_modules")
        .join("@remote")
        .join("shared");
    std::fs::create_dir_all(&pkg_dir).expect("create pkg dir");
    std::fs::write(
        pkg_dir.join("oh-package.json5"),
        r#"{ name: "@remote/shared" }"#,
    )
    .expect("write pkg oh-package.json5");
    let ets_dir = pkg_dir.join("src").join("main").join("ets");
    std::fs::create_dir_all(&ets_dir).expect("create ets dir");
    std::fs::write(ets_dir.join("index.ets"), "export default class Shared {}")
        .expect("write index.ets");

    let mut config = make_config(false);
    config.project_root = project_root.clone();
    let module_cfg = ModuleConfig {
        name: "entry".to_string(),
        src_path: module_root,
        module_type: OhosModuleType::Entry,
    };
    let manifest = make_manifest();

    let loader = generate_loader_json(&config, &module_cfg, &manifest);

    assert!(
        loader
            .dynamic_import_lib_info
            .contains_key("@remote/shared"),
        "dynamicImportLibInfo must contain @remote/shared"
    );
    let entry = &loader.dynamic_import_lib_info["@remote/shared"];
    assert!(
        entry.get("entryFilePath").is_some(),
        "entry must have entryFilePath"
    );
    assert!(
        entry.get("hostModulesInfo").is_some(),
        "entry must have hostModulesInfo"
    );
}

#[test]
fn test_loader_json_dynamic_import_lib_info_uses_runtime_only_packages() {
    let tmp = tempfile::tempdir().expect("create temp dir");
    let project_root = tmp.path().to_path_buf();
    let module_root = project_root.join("entry");
    std::fs::create_dir_all(&module_root).expect("create module dir");

    let pkg_dir = project_root
        .join("oh_modules")
        .join("@runtime")
        .join("feature");
    std::fs::create_dir_all(&pkg_dir).expect("create pkg dir");
    std::fs::write(
        pkg_dir.join("oh-package.json5"),
        r#"{ name: "@runtime/feature", main: "src/main/ets/Index.ets" }"#,
    )
    .expect("write pkg oh-package.json5");
    let ets_dir = pkg_dir.join("src").join("main").join("ets");
    std::fs::create_dir_all(&ets_dir).expect("create ets dir");
    std::fs::write(ets_dir.join("Index.ets"), "export default class Feature {}")
        .expect("write index.ets");

    let mut config = make_config(false);
    config.project_root = project_root.clone();
    config.product.build_option.ark_options = Some(ArkOptions {
        obfuscation: None,
        types: vec![],
        runtime_only: Some(RuntimeOnlyConfig {
            packages: vec!["@runtime/feature".to_string()],
            sources: vec![],
        }),
    });
    let module_cfg = ModuleConfig {
        name: "entry".to_string(),
        src_path: module_root,
        module_type: OhosModuleType::Entry,
    };

    let loader = generate_loader_json(&config, &module_cfg, &make_manifest());

    assert!(
        loader
            .dynamic_import_lib_info
            .contains_key("@runtime/feature"),
        "runtimeOnly.packages must feed dynamicImportLibInfo"
    );
    let entry = &loader.dynamic_import_lib_info["@runtime/feature"];
    assert_eq!(
        entry
            .get("hostModulesInfo")
            .and_then(|value| value.as_array())
            .and_then(|values| values.first())
            .and_then(|value| value.get("hostModuleName"))
            .and_then(|value| value.as_str()),
        Some("entry")
    );
}

#[test]
fn test_loader_json_compile_entry_uses_runtime_only_sources() {
    let tmp = tempfile::tempdir().expect("create temp dir");
    let project_root = tmp.path().to_path_buf();
    let module_root = project_root.join("entry");
    let runtime_dir = module_root.join("src/main/ets/runtime");
    let nested_dir = runtime_dir.join("nested");
    std::fs::create_dir_all(&nested_dir).expect("create runtime dirs");
    let keep = runtime_dir.join("Keep.ets");
    let side = nested_dir.join("Side.ts");
    std::fs::write(&keep, "export default class Keep {}").expect("write keep");
    std::fs::write(&side, "export const side = true;").expect("write side");
    std::fs::write(runtime_dir.join("ignored.txt"), "ignored").expect("write ignored");

    let mut config = make_config(false);
    config.project_root = project_root;
    config.product.build_option.ark_options = Some(ArkOptions {
        obfuscation: None,
        types: vec![],
        runtime_only: Some(RuntimeOnlyConfig {
            packages: vec![],
            sources: vec!["src/main/ets/runtime".to_string()],
        }),
    });
    let module_cfg = ModuleConfig {
        name: "entry".to_string(),
        src_path: module_root.clone(),
        module_type: OhosModuleType::Entry,
    };

    let loader = generate_loader_json(&config, &module_cfg, &make_manifest());

    assert_eq!(
        loader.hvigor_compile_entry,
        vec![
            to_absolute_path_string(&keep),
            to_absolute_path_string(&side)
        ]
    );
    assert!(loader
        .compile_entry
        .contains(&"./ets/runtime/Keep.ets".to_string()));
    assert!(loader
        .compile_entry
        .contains(&"./ets/runtime/nested/Side.ts".to_string()));
    assert!(!loader
        .compile_entry
        .contains(&"./ets/runtime/ignored.txt".to_string()));

    let value = serde_json::to_value(&loader).expect("serialize loader");
    assert_eq!(
        value
            .get("compileEntry")
            .and_then(|value| value.as_array())
            .map(Vec::len),
        Some(2)
    );
}

#[test]
fn test_loader_json_dynamic_import_lib_info_uses_dependency_runtime_only_packages() {
    let tmp = tempfile::tempdir().expect("create temp dir");
    let project_root = tmp.path().to_path_buf();
    let module_root = project_root.join("entry");
    std::fs::create_dir_all(&module_root).expect("create module dir");
    std::fs::write(
        module_root.join("oh-package.json5"),
        r#"{
              name: "entry",
              dependencies: {
                "source-har": "1.0.0"
              }
            }"#,
    )
    .expect("write entry oh-package");

    let dep_dir = project_root.join("oh_modules").join("source-har");
    std::fs::create_dir_all(&dep_dir).expect("create dep dir");
    std::fs::write(
        dep_dir.join("oh-package.json5"),
        r#"{
              name: "source-har",
              metadata: {
                runtimeOnly: {
                  packages: ["@runtime/indirect"]
                }
              }
            }"#,
    )
    .expect("write dep oh-package");

    let runtime_pkg = project_root
        .join("oh_modules")
        .join("@runtime")
        .join("indirect");
    std::fs::create_dir_all(runtime_pkg.join("src/main/ets")).expect("create runtime dep");
    std::fs::write(
        runtime_pkg.join("oh-package.json5"),
        r#"{ name: "@runtime/indirect", main: "src/main/ets/Index.ets" }"#,
    )
    .expect("write runtime oh-package");
    std::fs::write(
        runtime_pkg.join("src/main/ets/Index.ets"),
        "export default class RuntimeIndirect {}",
    )
    .expect("write runtime index");

    let mut config = make_config(false);
    config.project_root = project_root.clone();
    let module_cfg = ModuleConfig {
        name: "entry".to_string(),
        src_path: module_root,
        module_type: OhosModuleType::Entry,
    };

    let loader = generate_loader_json(&config, &module_cfg, &make_manifest());

    assert!(
        loader
            .dynamic_import_lib_info
            .contains_key("@runtime/indirect"),
        "dependency metadata.runtimeOnly.packages must feed dynamicImportLibInfo"
    );
}

/// Verify `byteCodeHarInfo` is populated when an oh_modules package has index.abc.
#[test]
fn test_loader_json_byte_code_har_info_populated_from_oh_modules() {
    let tmp = tempfile::tempdir().expect("create temp dir");
    let project_root = tmp.path().to_path_buf();
    let module_root = project_root.join("entry");

    // Create a ByteCode HAR package under oh_modules.
    let pkg_dir = project_root
        .join("oh_modules")
        .join("@ohos")
        .join("abc-lib");
    std::fs::create_dir_all(&pkg_dir).expect("create pkg dir");
    std::fs::write(pkg_dir.join("index.abc"), b"PANDA\x00\x00\x00").expect("write index.abc");
    std::fs::write(
        pkg_dir.join("oh-package.json5"),
        r#"{ name: "@ohos/abc-lib", compatibleSdkVersion: "5.0.0(12)" }"#,
    )
    .expect("write oh-package.json5");

    let mut config = make_config(false);
    config.project_root = project_root.clone();
    let module_cfg = ModuleConfig {
        name: "entry".to_string(),
        src_path: module_root,
        module_type: OhosModuleType::Entry,
    };
    let manifest = make_manifest();

    let loader = generate_loader_json(&config, &module_cfg, &manifest);

    assert!(
        loader.byte_code_har_info.contains_key("@ohos/abc-lib"),
        "byteCodeHarInfo must contain @ohos/abc-lib"
    );
    let entry = &loader.byte_code_har_info["@ohos/abc-lib"];
    assert_eq!(entry.compatible_sdk_version, "5.0.0(12)");
    assert!(entry.abc_path.ends_with("index.abc"));
    assert!(entry.source_maps_path.is_none());
}
