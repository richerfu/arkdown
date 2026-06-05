use std::fs;
use std::path::PathBuf;

use arkdown_core::{
    AbilityEntry, ArkOptions, ArkdownConfig, BuildMode, BuildOption, ExtensionAbilityEntry,
    HapManifest, MetadataEntry, ModuleConfig, OhosModuleType, OutputTarget, ProductConfig,
    RuntimeOnlyConfig, SdkConfig, StrictModeConfig,
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
            compatible_sdk_version_stage: None,
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
                preload_system_so: false,
                preload_so: vec![],
                remove_permissions: vec![],
                aot_compile_mode: None,
                host_pgo: None,
                ap_path: None,
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

fn form_extension_ability(profile_name: &str) -> ExtensionAbilityEntry {
    ExtensionAbilityEntry {
        name: "EntryFormAbility".to_string(),
        src_entry: "./ets/form/EntryFormAbility.ets".to_string(),
        extension_type: "form".to_string(),
        label: None,
        description: None,
        metadata: vec![MetadataEntry {
            name: "ohos.extension.form".to_string(),
            value: None,
            resource: Some(format!("$profile:{profile_name}")),
        }],
    }
}

fn write_minimal_module_json5(module_root: &std::path::Path) {
    fs::create_dir_all(module_root.join("src/main")).expect("create module src/main");
    fs::write(
        module_root.join("src/main/module.json5"),
        r#"{ module: { name: "entry", type: "entry" } }"#,
    )
    .expect("write module.json5");
}

#[test]
fn test_loader_json_does_not_scan_unreferenced_form_profiles() {
    let tmp = tempfile::tempdir().expect("tempdir");
    let module_root = tmp.path().join("entry");
    let profile_dir = module_root.join("src/main/resources/base/profile");
    fs::create_dir_all(&profile_dir).expect("create profile dir");
    write_minimal_module_json5(&module_root);
    fs::write(
        profile_dir.join("unused_form.json"),
        r#"{ forms: [{ src: "./ets/forms/Unused.ets", uiSyntax: "arkts" }] }"#,
    )
    .expect("write unused form profile");

    let mut config = make_config(true);
    config.project_root = tmp.path().to_path_buf();
    let mut module_cfg = make_module_cfg();
    module_cfg.src_path = module_root;
    let loader = LoaderJson::try_generate(&config, &module_cfg, &make_manifest()).unwrap();

    assert!(
        !loader
            .compile_entry
            .iter()
            .any(|entry| entry.contains("Unused.ets")),
        "unreferenced form profile sources must not be added to compile_entry: {:?}",
        loader.compile_entry
    );
}

#[test]
fn test_loader_json_rejects_missing_form_profile_reference() {
    let tmp = tempfile::tempdir().expect("tempdir");
    let module_root = tmp.path().join("entry");
    write_minimal_module_json5(&module_root);
    let mut config = make_config(true);
    config.project_root = tmp.path().to_path_buf();
    let mut module_cfg = make_module_cfg();
    module_cfg.src_path = module_root;
    let mut manifest = make_manifest();
    manifest.extension_abilities = vec![form_extension_ability("missing_form")];

    let error = LoaderJson::try_generate(&config, &module_cfg, &manifest).unwrap_err();
    assert!(error.to_string().contains("CAN_NOT_RESOLVE_SYMBOL"));
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
    let loader = LoaderJson::generate(&config, &module_cfg, &make_manifest());

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

#[test]
fn test_loader_json_populates_bytecode_har_update_version_info() {
    let tmp = tempfile::tempdir().expect("tempdir");
    let project_root = tmp.path().to_path_buf();
    let module_root = project_root.join("entry");
    let oh_modules = module_root.join("oh_modules");
    let bytecode_har = oh_modules.join("bytecode-har");

    fs::create_dir_all(bytecode_har.join("ets")).expect("create bytecode har");
    fs::write(
        bytecode_har.join("ets").join("modules.abc"),
        b"PANDA\x00\x00\x00",
    )
    .expect("write abc");
    fs::write(
        bytecode_har.join("oh-package.json5"),
        r#"{
              name: "bytecode-har",
              metadata: {
                byteCodeHar: true,
                dependencyPkgVersion: { "dep-a": "1.0.0" }
              }
            }"#,
    )
    .expect("write bytecode har package");
    fs::create_dir_all(oh_modules.join("dep-a")).expect("create dep package");
    fs::write(
        oh_modules.join("dep-a").join("oh-package.json5"),
        r#"{ name: "dep-a", version: "1.1.0" }"#,
    )
    .expect("write dep package");

    let mut config = make_config(true);
    config.project_root = project_root;
    let mut module_cfg = make_module_cfg();
    module_cfg.src_path = module_root;
    let loader = LoaderJson::generate(&config, &module_cfg, &make_manifest());

    assert_eq!(
        loader
            .update_version_info
            .get("bytecode-har")
            .and_then(|v| v.get("dep-a")),
        Some(&serde_json::Value::String("1.1.0".to_string()))
    );
}

#[test]
fn test_loader_json_populates_bytecode_har_other_compile_files() {
    let tmp = tempfile::tempdir().expect("tempdir");
    let project_root = tmp.path().to_path_buf();
    let module_root = project_root.join("entry");
    let oh_modules = module_root.join("oh_modules");
    let bytecode_har = oh_modules.join("bytecode-har");
    let source_har = oh_modules.join("source-har");
    let extra_pkg = oh_modules.join("extra-pkg");

    fs::create_dir_all(&module_root).expect("create module root");
    fs::write(
        module_root.join("oh-package.json5"),
        r#"{ dependencies: { "bytecode-har": "1.0.0" } }"#,
    )
    .expect("write module oh-package");
    fs::create_dir_all(bytecode_har.join("ets")).expect("create bytecode har");
    fs::write(
        bytecode_har.join("ets").join("modules.abc"),
        b"PANDA\x00\x00\x00",
    )
    .expect("write abc");
    fs::write(
        bytecode_har.join("oh-package.json5"),
        r#"{
              name: "bytecode-har",
              dependencies: { "source-har": "1.0.0" },
              metadata: {
                byteCodeHar: true,
                extraImportees: ["extra-pkg/src/feature"]
              }
            }"#,
    )
    .expect("write bytecode har package");
    fs::create_dir_all(&source_har).expect("create source har package root");
    fs::write(
        source_har.join("oh-package.json5"),
        r#"{ name: "source-har", main: "src/main/ets/index.ets" }"#,
    )
    .expect("write source har package");
    fs::create_dir_all(source_har.join("src/main")).expect("create source har module dir");
    fs::write(
        source_har.join("src/main/module.json5"),
        r#"{ module: { name: "source", type: "har" } }"#,
    )
    .expect("write source har module json");
    fs::create_dir_all(source_har.join("src/main/ets/pages")).expect("create source har ets dir");
    fs::write(
        source_har.join("src/main/ets/index.ets"),
        "export default 1;\n",
    )
    .expect("write source har entry");
    fs::write(
        source_har.join("src/main/ets/pages/Page.ets"),
        "export default 2;\n",
    )
    .expect("write source har page");
    fs::create_dir_all(&extra_pkg).expect("create extra package root");
    fs::write(
        extra_pkg.join("oh-package.json5"),
        r#"{ name: "extra-pkg" }"#,
    )
    .expect("write extra package");
    fs::create_dir_all(extra_pkg.join("src")).expect("create extra package src");
    fs::write(extra_pkg.join("src/feature.ets"), "export default 3;\n")
        .expect("write extra feature");

    let mut config = make_config(true);
    config.project_root = project_root;
    let mut module_cfg = make_module_cfg();
    module_cfg.src_path = module_root;
    let loader = LoaderJson::generate(&config, &module_cfg, &make_manifest());

    assert!(loader
        .other_compile_files
        .iter()
        .any(|path| path.ends_with("source-har/src/main/ets/index.ets")));
    assert!(loader
        .other_compile_files
        .iter()
        .any(|path| path.ends_with("source-har/src/main/ets/pages/Page.ets")));
    assert!(loader
        .other_compile_files
        .iter()
        .any(|path| path.ends_with("extra-pkg/src/feature.ets")));
    for path in &loader.other_compile_files {
        assert!(
            loader.compile_entry.contains(path),
            "otherCompileFiles entry must also be compiled by arkdown: {path}"
        );
    }
}

/// Verify loader.json round-trips through serde correctly.
#[test]
fn test_loader_json_serialization() {
    let config = make_config(false);
    let module_cfg = make_module_cfg();
    let manifest = make_manifest();

    let loader = LoaderJson::generate(&config, &module_cfg, &manifest);

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
    assert_eq!(loader2.an_build_mode.as_deref(), Some("type"));
}

#[test]
fn test_loader_json_uses_normalized_ohm_url() {
    let config = make_config(true);
    let module_cfg = make_module_cfg();
    let manifest = make_manifest();

    let loader = LoaderJson::generate(&config, &module_cfg, &manifest);
    assert!(loader.use_normalized_ohm_url);
}

#[test]
fn test_loader_json_hvigor_keys() {
    let config = make_config(false);
    let module_cfg = make_module_cfg();
    let manifest = make_manifest();

    let loader = LoaderJson::generate(&config, &module_cfg, &manifest);
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

    let loader = LoaderJson::generate(&config, &module_cfg, &manifest);
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

    let loader = LoaderJson::generate(&config, &module_cfg, &manifest);
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

    let loader = LoaderJson::generate(&config, &module_cfg, &manifest);
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

    let loader = LoaderJson::generate(&config, &module_cfg, &manifest);
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

    let loader = LoaderJson::generate(&config, &module_cfg, &manifest);
    assert_eq!(loader.module_name, "entry");
}

#[test]
fn test_loader_json_api_version() {
    let config = make_config(false);
    let module_cfg = make_module_cfg();
    let manifest = make_manifest();

    let loader = LoaderJson::generate(&config, &module_cfg, &manifest);
    assert_eq!(loader.api_version, 12);
}

#[test]
fn test_loader_json_module_path_map() {
    let config = make_config(false);
    let module_cfg = make_module_cfg();
    let manifest = make_manifest();

    let loader = LoaderJson::generate(&config, &module_cfg, &manifest);
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
fn test_loader_json_module_path_map_and_pkg_brief_follow_declared_dependencies() {
    let tmp = tempfile::tempdir().expect("create temp dir");
    let project_root = tmp.path();
    let module_root = project_root.join("entry");
    let source_har = module_root.join("oh_modules/source-har");
    let transitive_pkg = source_har.join("oh_modules/@scope/transitive");
    let undeclared_pkg = project_root.join("oh_modules/undeclared");

    fs::create_dir_all(&module_root).expect("create module root");
    fs::write(
        module_root.join("oh-package.json5"),
        r#"{ dependencies: { "source-har": "1.0.0" } }"#,
    )
    .expect("write module oh-package");

    fs::create_dir_all(source_har.join("src/main/ets")).expect("create source har ets");
    fs::write(
        source_har.join("oh-package.json5"),
        r#"{
            name: "source-har",
            dependencies: { "@scope/transitive": "1.0.0" }
        }"#,
    )
    .expect("write source har oh-package");
    fs::write(
        source_har.join("src/main/ets/index.ets"),
        "export default 1;",
    )
    .expect("write source har entry");

    fs::create_dir_all(transitive_pkg.join("src/main/ets")).expect("create transitive ets");
    fs::write(
        transitive_pkg.join("oh-package.json5"),
        r#"{ name: "@scope/transitive" }"#,
    )
    .expect("write transitive oh-package");
    fs::write(
        transitive_pkg.join("src/main/ets/index.ets"),
        "export default 2;",
    )
    .expect("write transitive entry");

    fs::create_dir_all(undeclared_pkg.join("src/main/ets")).expect("create undeclared ets");
    fs::write(
        undeclared_pkg.join("oh-package.json5"),
        r#"{ name: "undeclared" }"#,
    )
    .expect("write undeclared oh-package");
    fs::write(
        undeclared_pkg.join("src/main/ets/index.ets"),
        "export default 3;",
    )
    .expect("write undeclared entry");

    let mut config = make_config(false);
    config.project_root = project_root.to_path_buf();
    let mut module_cfg = make_module_cfg();
    module_cfg.src_path = module_root;

    let loader = LoaderJson::generate(&config, &module_cfg, &make_manifest());

    assert!(loader.module_path_map.contains_key("source-har"));
    assert!(loader.module_path_map.contains_key("@scope/transitive"));
    assert!(!loader.module_path_map.contains_key("undeclared"));
    assert!(loader.pkg_name_to_pkg_brief_info.contains_key("source-har"));
    assert!(loader
        .pkg_name_to_pkg_brief_info
        .contains_key("@scope/transitive"));
    assert!(!loader.pkg_name_to_pkg_brief_info.contains_key("undeclared"));
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

    let loader = LoaderJson::generate(&config, &module_cfg, &manifest);
    assert_eq!(loader.project_root_path, "/project/app");
    assert_eq!(loader.module_path_map["entry"], "/project/app/entry");
    assert_eq!(
        loader.node_modules_path,
        "/project/app/entry/build/default/intermediates/loader_out/default/node_modules"
    );
    assert_eq!(
        loader.an_build_out_put.as_deref(),
        Some("/project/app/entry/build/default/intermediates/loader_out/default/an/arm64-v8a")
    );
}

#[test]
fn test_loader_json_fallback_an_build_is_false() {
    let config = make_config(false);
    let module_cfg = make_module_cfg();
    let manifest = make_manifest();

    let loader = LoaderJson::generate(&config, &module_cfg, &manifest);
    assert!(!loader.fallback_an_build);
}

#[test]
fn test_loader_json_writes_ap_path_and_copies_ap_when_present() {
    let tmp = tempfile::tempdir().expect("create temp dir");
    let project_root = tmp.path();
    let module_root = project_root.join("entry");
    std::fs::create_dir_all(&module_root).expect("create module root");
    std::fs::write(module_root.join("modules.ap"), b"profile guided data").expect("write ap");

    let mut config = make_config(false);
    config.project_root = project_root.to_path_buf();
    let mut module_cfg = make_module_cfg();
    module_cfg.src_path = module_root.clone();

    let loader = LoaderJson::generate(&config, &module_cfg, &make_manifest());

    let source_ap = module_root.join("modules.ap");
    let canonical_module_root = std::fs::canonicalize(&module_root).expect("canonical module root");
    let canonical_source_ap = std::fs::canonicalize(&source_ap).expect("canonical ap");
    assert_eq!(
        loader.an_build_out_put.as_deref(),
        Some(
            canonical_module_root
                .join("build/default/intermediates/loader_out/default/an/arm64-v8a")
                .to_string_lossy()
                .as_ref()
        )
    );
    assert_eq!(loader.an_build_mode.as_deref(), Some("type"));
    assert_eq!(
        loader.ap_path.as_deref(),
        Some(canonical_source_ap.to_string_lossy().as_ref())
    );
    let copied_ap = module_root.join("build/default/intermediates/ap/entry.ap");
    assert_eq!(
        std::fs::read(&copied_ap).expect("read copied ap"),
        b"profile guided data"
    );
    let value = serde_json::to_value(&loader).expect("serialize loader");
    assert_eq!(
        value["apPath"].as_str(),
        Some(canonical_source_ap.to_string_lossy().as_ref())
    );
}

#[test]
fn test_loader_json_api9_without_aot_omits_aot_fields() {
    let mut config = make_config(false);
    config.sdk_config.compile_api_version = 9;
    let loader = LoaderJson::generate(&config, &make_module_cfg(), &make_manifest());
    let value = serde_json::to_value(&loader).expect("serialize loader");

    assert!(loader.an_build_out_put.is_none());
    assert!(loader.an_build_mode.is_none());
    assert!(loader.ap_path.is_none());
    assert!(value.get("anBuildOutPut").is_none());
    assert!(value.get("anBuildMode").is_none());
    assert!(value.get("apPath").is_none());
}

#[test]
fn test_loader_json_api10_ignores_aot_compile_mode_without_host_pgo() {
    let mut config = make_config(false);
    config.product.build_option.aot_compile_mode = Some("partial".to_string());

    let loader = LoaderJson::generate(&config, &make_module_cfg(), &make_manifest());

    assert_eq!(loader.an_build_mode.as_deref(), Some("type"));
    assert!(loader.ap_path.is_none());
}

#[test]
fn test_loader_json_openharmony_old_aot_sdk_omits_aot_fields() {
    let mut config = make_config(false);
    config.product.runtime_os = "OpenHarmony".to_string();
    config.sdk_config.compile_sdk_version = "4.0.8.1".to_string();

    let loader = LoaderJson::generate(&config, &make_module_cfg(), &make_manifest());
    let value = serde_json::to_value(&loader).expect("serialize loader");

    assert!(loader.an_build_out_put.is_none());
    assert!(loader.an_build_mode.is_none());
    assert!(loader.ap_path.is_none());
    assert!(value.get("anBuildOutPut").is_none());
    assert!(value.get("anBuildMode").is_none());
    assert!(value.get("apPath").is_none());
}

#[test]
fn test_loader_json_openharmony_fixed_aot_sdk_writes_type_mode() {
    let mut config = make_config(false);
    config.product.runtime_os = "OpenHarmony".to_string();
    config.sdk_config.compile_sdk_version = "4.0.8.2".to_string();

    let loader = LoaderJson::generate(&config, &make_module_cfg(), &make_manifest());

    assert_eq!(loader.an_build_mode.as_deref(), Some("type"));
    assert!(loader.ap_path.is_none());
}

#[test]
fn test_loader_json_harmonyos_ignores_openharmony_aot_fixed_gate() {
    let mut config = make_config(false);
    config.sdk_config.compile_sdk_version = "4.0.8.1".to_string();

    let loader = LoaderJson::generate(&config, &make_module_cfg(), &make_manifest());

    assert_eq!(loader.an_build_mode.as_deref(), Some("type"));
    assert!(loader.ap_path.is_none());
}

#[test]
fn test_loader_json_har_fallback_omits_aot_fields() {
    let mut module_cfg = make_module_cfg();
    module_cfg.module_type = OhosModuleType::Har;
    let mut manifest = make_manifest();
    manifest.module_type = OhosModuleType::Har;

    let loader = LoaderJson::generate(&make_config(false), &module_cfg, &manifest);
    let value = serde_json::to_value(&loader).expect("serialize loader");

    assert!(loader.fallback_an_build);
    assert!(loader.an_build_out_put.is_none());
    assert!(loader.an_build_mode.is_none());
    assert!(loader.ap_path.is_none());
    assert!(value.get("anBuildOutPut").is_none());
    assert!(value.get("anBuildMode").is_none());
    assert!(value.get("apPath").is_none());
}

#[test]
fn test_loader_json_serializes_patch_config_from_build_config() {
    let tmp = tempfile::tempdir().expect("create temp dir");
    let project_root = tmp.path();
    let module_root = project_root.join("entry");
    let build_config = module_root.join("build/default/config/buildConfig.json");
    fs::create_dir_all(build_config.parent().unwrap()).expect("create build config dir");
    fs::write(
        &build_config,
        r#"{
            "compileConfig": { "watchMode": "false" },
            "patchConfig": {
                "mode": "hotReload",
                "changedFileList": "changed-files.json"
            }
        }"#,
    )
    .expect("write build config");

    let mut config = make_config(false);
    config.project_root = project_root.to_path_buf();
    let mut module_cfg = make_module_cfg();
    module_cfg.src_path = module_root;

    let loader = LoaderJson::generate(&config, &module_cfg, &make_manifest());
    let value = serde_json::to_value(&loader).expect("serialize loader");

    assert_eq!(
        value
            .get("patchConfig")
            .and_then(|patch| patch.get("mode"))
            .and_then(serde_json::Value::as_str),
        Some("hotReload")
    );
    assert!(value.get("buildConfigPath").is_none());
}

#[test]
fn test_loader_json_omits_patch_config_for_test_target() {
    let tmp = tempfile::tempdir().expect("create temp dir");
    let project_root = tmp.path();
    let module_root = project_root.join("entry");
    let build_config = module_root.join("build/default/config/buildConfig.json");
    fs::create_dir_all(build_config.parent().unwrap()).expect("create build config dir");
    fs::write(
        &build_config,
        r#"{ "patchConfig": { "mode": "hotReload" } }"#,
    )
    .expect("write build config");

    let mut config = make_config(false);
    config.project_root = project_root.to_path_buf();
    config.output_target = OutputTarget::Test;
    let mut module_cfg = make_module_cfg();
    module_cfg.src_path = module_root;

    let loader = LoaderJson::generate(&config, &module_cfg, &make_manifest());
    let value = serde_json::to_value(&loader).expect("serialize loader");

    assert!(value.get("patchConfig").is_none());
}

#[test]
fn test_loader_json_preview_serializes_build_config_path() {
    let tmp = tempfile::tempdir().expect("create temp dir");
    let project_root = tmp.path();
    let module_root = project_root.join("entry");
    let build_config = module_root.join("build/default/config/buildConfig.json");
    fs::create_dir_all(build_config.parent().unwrap()).expect("create build config dir");
    fs::write(
        &build_config,
        r#"{ "patchConfig": { "mode": "coldReload" } }"#,
    )
    .expect("write build config");

    let mut config = make_config(false);
    config.project_root = project_root.to_path_buf();
    config.output_target = OutputTarget::Preview;
    let mut module_cfg = make_module_cfg();
    module_cfg.src_path = module_root;

    let loader = LoaderJson::generate(&config, &module_cfg, &make_manifest());
    let value = serde_json::to_value(&loader).expect("serialize loader");

    assert_eq!(
        value
            .get("buildConfigPath")
            .and_then(serde_json::Value::as_str),
        Some("build/default/config/buildConfig.json")
    );
    assert_eq!(
        value
            .get("patchConfig")
            .and_then(|patch| patch.get("mode"))
            .and_then(serde_json::Value::as_str),
        Some("coldReload")
    );
}

#[test]
fn test_loader_json_preview_and_test_compile_entry_uses_mock_config_sources() {
    let tmp = tempfile::tempdir().expect("create temp dir");
    let project_root = tmp.path().to_path_buf();
    let module_root = project_root.join("entry");
    let insight_dir = module_root.join("src/main/resources/base/profile");
    let ets_dir = module_root.join("src/main/ets");
    let mock_dir = module_root.join("src/mock");
    fs::create_dir_all(&insight_dir).expect("create insight dir");
    fs::create_dir_all(&ets_dir).expect("create ets dir");
    fs::create_dir_all(&mock_dir).expect("create mock dir");

    let insight = ets_dir.join("Insight.ets");
    let mock_service = mock_dir.join("MockService.ets");
    let mock_page = mock_dir.join("PageMock.ts");
    fs::write(&insight, "export class Insight {}").expect("write insight source");
    fs::write(&mock_service, "export class MockService {}").expect("write mock service");
    fs::write(&mock_page, "export const page = true;").expect("write mock page");
    fs::write(mock_dir.join("types.d.ts"), "export interface Types {}").expect("write types");
    fs::write(
        insight_dir.join("insight_intent.json"),
        r#"{
            insightIntentsSrcEntry: [
                { srcEntry: "ets/Insight.ets" }
            ]
        }"#,
    )
    .expect("write insight profile");
    fs::write(
        mock_dir.join("mock-config.json5"),
        r#"{
            service: { source: "src/mock/MockService.ets" },
            page: { source: "./src/mock/PageMock.ts" },
            declaration: { source: "src/mock/types.d.ts" },
            missing: { source: "src/mock/Missing.ets" },
            empty: { source: "" }
        }"#,
    )
    .expect("write mock config");

    for output_target in [OutputTarget::Preview, OutputTarget::Test] {
        let mut config = make_config(false);
        config.project_root = project_root.clone();
        config.output_target = output_target;
        let mut module_cfg = make_module_cfg();
        module_cfg.src_path = module_root.clone();

        let loader = LoaderJson::generate(&config, &module_cfg, &make_manifest());

        assert_eq!(
            loader.hvigor_compile_entry,
            vec![
                to_absolute_path_string(&insight),
                to_absolute_path_string(&mock_service),
                to_absolute_path_string(&mock_page),
            ]
        );
        assert!(loader
            .compile_entry
            .contains(&"./ets/Insight.ets".to_string()));
        assert!(loader
            .compile_entry
            .contains(&to_absolute_path_string(&mock_service)));
        assert!(loader
            .compile_entry
            .contains(&to_absolute_path_string(&mock_page)));
        assert!(!loader
            .hvigor_compile_entry
            .iter()
            .any(|entry| entry.ends_with("types.d.ts")));
    }
}

#[test]
fn test_loader_json_hap_compile_entry_ignores_mock_config_sources() {
    let tmp = tempfile::tempdir().expect("create temp dir");
    let project_root = tmp.path().to_path_buf();
    let module_root = project_root.join("entry");
    let mock_dir = module_root.join("src/mock");
    fs::create_dir_all(&mock_dir).expect("create mock dir");
    let mock_service = mock_dir.join("MockService.ets");
    fs::write(&mock_service, "export class MockService {}").expect("write mock service");
    fs::write(
        mock_dir.join("mock-config.json5"),
        r#"{
            service: { source: "src/mock/MockService.ets" }
        }"#,
    )
    .expect("write mock config");

    let mut config = make_config(false);
    config.project_root = project_root;
    let mut module_cfg = make_module_cfg();
    module_cfg.src_path = module_root;

    let loader = LoaderJson::generate(&config, &module_cfg, &make_manifest());

    assert!(loader.hvigor_compile_entry.is_empty());
    assert!(!loader
        .compile_entry
        .contains(&to_absolute_path_string(&mock_service)));
}

#[test]
#[should_panic(expected = "PARTIAL_INVALID_PATH")]
fn test_loader_json_host_pgo_requires_existing_ap() {
    let mut config = make_config(false);
    config.product.build_option.host_pgo = Some(true);

    let _ = LoaderJson::generate(&config, &make_module_cfg(), &make_manifest());
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

    let loader = LoaderJson::generate(&config, &module_cfg, &manifest);

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
        std::path::Path::new(loader.an_build_out_put.as_deref().unwrap()).is_absolute(),
        "anBuildOutPut must be absolute: {}",
        loader.an_build_out_put.as_deref().unwrap()
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

    let loader = LoaderJson::generate(&config, &module_cfg, &manifest);
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

#[test]
fn test_loader_json_hsp_resources_map_collects_resource_table() {
    let tmp = tempfile::tempdir().expect("create temp dir");
    let project_root = tmp.path().to_path_buf();
    let module_root = project_root.join("entry");
    fs::create_dir_all(&module_root).expect("create module dir");
    fs::write(
        module_root.join("oh-package.json5"),
        r#"{ dependencies: { "shared-hsp": "1.0.0" } }"#,
    )
    .expect("write module oh-package");

    let hsp_root = project_root.join("oh_modules/shared-hsp");
    fs::create_dir_all(hsp_root.join("src/main")).expect("create hsp dir");
    fs::write(
        hsp_root.join("oh-package.json5"),
        r#"{ name: "shared-hsp" }"#,
    )
    .expect("write hsp oh-package");
    fs::write(
        hsp_root.join("src/main/module.json5"),
        r#"{
            "module": {
                "name": "shared",
                "type": "shared"
            }
        }"#,
    )
    .expect("write hsp module.json5");
    fs::write(hsp_root.join("ResourceTable.txt"), "resource table")
        .expect("write hsp ResourceTable");

    let mut config = make_config(false);
    config.project_root = project_root;
    let module_cfg = ModuleConfig {
        name: "entry".to_string(),
        src_path: module_root,
        module_type: OhosModuleType::Entry,
    };

    let loader = LoaderJson::generate(&config, &module_cfg, &make_manifest());

    let resource_table = loader
        .hsp_resources_map
        .get("shared-hsp")
        .expect("hspResourcesMap entry");
    assert!(
        resource_table.ends_with("oh_modules/shared-hsp/ResourceTable.txt"),
        "hspResourcesMap must point at the HSP ResourceTable.txt: {resource_table}"
    );
}

#[test]
fn test_loader_json_hsp_resources_map_follows_transitive_scoped_dependencies() {
    let tmp = tempfile::tempdir().expect("create temp dir");
    let project_root = tmp.path().to_path_buf();
    let module_root = project_root.join("entry");
    let source_har = module_root.join("oh_modules/source-har");
    let scoped_hsp = source_har.join("oh_modules/@scope/shared-hsp");
    let undeclared_hsp = project_root.join("oh_modules/undeclared-hsp");

    fs::create_dir_all(&module_root).expect("create module dir");
    fs::write(
        module_root.join("oh-package.json5"),
        r#"{ dependencies: { "source-har": "1.0.0" } }"#,
    )
    .expect("write module oh-package");

    fs::create_dir_all(source_har.join("src/main")).expect("create source har dir");
    fs::write(
        source_har.join("oh-package.json5"),
        r#"{
            name: "source-har",
            dependencies: { "@scope/shared-hsp": "1.0.0" }
        }"#,
    )
    .expect("write source har oh-package");
    fs::write(
        source_har.join("src/main/module.json5"),
        r#"{ module: { name: "source_har", type: "har" } }"#,
    )
    .expect("write source har module");

    fs::create_dir_all(scoped_hsp.join("src/main")).expect("create scoped hsp dir");
    fs::write(
        scoped_hsp.join("oh-package.json5"),
        r#"{ name: "@scope/shared-hsp" }"#,
    )
    .expect("write scoped hsp oh-package");
    fs::write(
        scoped_hsp.join("src/main/module.json5"),
        r#"{ module: { name: "shared_hsp", type: "shared" } }"#,
    )
    .expect("write scoped hsp module");
    fs::write(scoped_hsp.join("ResourceTable.txt"), "resource table")
        .expect("write scoped hsp ResourceTable");

    fs::create_dir_all(undeclared_hsp.join("src/main")).expect("create undeclared hsp dir");
    fs::write(
        undeclared_hsp.join("oh-package.json5"),
        r#"{ name: "undeclared-hsp" }"#,
    )
    .expect("write undeclared hsp oh-package");
    fs::write(
        undeclared_hsp.join("src/main/module.json5"),
        r#"{ module: { name: "undeclared_hsp", type: "shared" } }"#,
    )
    .expect("write undeclared hsp module");
    fs::write(undeclared_hsp.join("ResourceTable.txt"), "resource table")
        .expect("write undeclared hsp ResourceTable");

    let mut config = make_config(false);
    config.project_root = project_root;
    let module_cfg = ModuleConfig {
        name: "entry".to_string(),
        src_path: module_root,
        module_type: OhosModuleType::Entry,
    };

    let loader = LoaderJson::generate(&config, &module_cfg, &make_manifest());

    let resource_table = loader
        .hsp_resources_map
        .get("@scope/shared-hsp")
        .expect("transitive scoped hspResourcesMap entry");
    assert!(
        resource_table.ends_with("oh_modules/@scope/shared-hsp/ResourceTable.txt"),
        "hspResourcesMap must point at transitive scoped HSP ResourceTable.txt: {resource_table}"
    );
    assert!(
        !loader.hsp_resources_map.contains_key("undeclared-hsp"),
        "undeclared HSP must not be collected"
    );
}

#[test]
fn test_loader_json_hsp_har_ohm_maps_follow_declared_dependencies() {
    let tmp = tempfile::tempdir().expect("create temp dir");
    let project_root = tmp.path();
    let module_root = project_root.join("entry");
    let direct_hsp = module_root.join("oh_modules/direct-hsp");
    let direct_har = module_root.join("oh_modules/direct-har");
    let transitive_hsp = direct_har.join("oh_modules/@scope/transitive-hsp");
    let undeclared_har = project_root.join("oh_modules/undeclared-har");

    fs::create_dir_all(&module_root).expect("create module root");
    fs::write(
        module_root.join("oh-package.json5"),
        r#"{ dependencies: { "direct-hsp": "1.0.0", "direct-har": "1.0.0" } }"#,
    )
    .expect("write module oh-package");

    fs::create_dir_all(direct_hsp.join("src/main/ets")).expect("create hsp ets");
    fs::write(
        direct_hsp.join("oh-package.json5"),
        r#"{ name: "direct-hsp", main: "src/main/ets/HspEntry.ets" }"#,
    )
    .expect("write hsp oh-package");
    fs::write(
        direct_hsp.join("src/main/module.json5"),
        r#"{ module: { name: "direct_hsp_module", type: "shared" } }"#,
    )
    .expect("write hsp module");
    fs::write(direct_hsp.join("src/main/ets/HspEntry.ets"), "export {}").expect("write hsp entry");

    fs::create_dir_all(direct_har.join("src/main/ets")).expect("create har ets");
    fs::write(
        direct_har.join("oh-package.json5"),
        r#"{
            name: "direct-har",
            main: "src/main/ets/HarEntry.ts",
            dependencies: { "@scope/transitive-hsp": "1.0.0" }
        }"#,
    )
    .expect("write har oh-package");
    fs::write(
        direct_har.join("src/main/module.json5"),
        r#"{ module: { name: "direct_har_module", type: "har" } }"#,
    )
    .expect("write har module");
    fs::write(direct_har.join("src/main/ets/HarEntry.ts"), "export {}").expect("write har entry");

    fs::create_dir_all(transitive_hsp.join("src/main/ets")).expect("create transitive hsp ets");
    fs::write(
        transitive_hsp.join("oh-package.json5"),
        r#"{ name: "@scope/transitive-hsp" }"#,
    )
    .expect("write transitive hsp oh-package");
    fs::write(
        transitive_hsp.join("src/main/module.json5"),
        r#"{ module: { name: "transitive_hsp_module", type: "shared" } }"#,
    )
    .expect("write transitive hsp module");
    fs::write(transitive_hsp.join("src/main/ets/index.ets"), "export {}")
        .expect("write transitive hsp entry");

    fs::create_dir_all(undeclared_har.join("src/main/ets")).expect("create undeclared har ets");
    fs::write(
        undeclared_har.join("oh-package.json5"),
        r#"{ name: "undeclared-har", main: "src/main/ets/index.ets" }"#,
    )
    .expect("write undeclared har oh-package");
    fs::write(
        undeclared_har.join("src/main/module.json5"),
        r#"{ module: { name: "undeclared_har_module", type: "har" } }"#,
    )
    .expect("write undeclared har module");
    fs::write(undeclared_har.join("src/main/ets/index.ets"), "export {}")
        .expect("write undeclared har entry");

    let mut config = make_config(false);
    config.project_root = project_root.to_path_buf();
    let mut module_cfg = make_module_cfg();
    module_cfg.src_path = module_root;

    let loader = LoaderJson::generate(&config, &module_cfg, &make_manifest());

    assert_eq!(
        loader.hsp_name_ohm_map.get("direct-hsp"),
        Some(&"@bundle:com.example.app/direct_hsp_module/src/main/ets/HspEntry".to_string())
    );
    assert_eq!(
        loader.hsp_name_ohm_map.get("@scope/transitive-hsp"),
        Some(&"@bundle:com.example.app/transitive_hsp_module/src/main/ets/index".to_string())
    );
    assert_eq!(
        loader.har_name_ohm_map.get("direct-har"),
        Some(&"@bundle:com.example.app/direct_har_module/src/main/ets/HarEntry".to_string())
    );
    assert!(
        !loader.har_name_ohm_map.contains_key("undeclared-har"),
        "undeclared HAR must not be collected"
    );
}

/// Verify all path-valued fields in the serialized loader.json are absolute.
#[test]
fn test_loader_json_serialized_paths_are_absolute() {
    let config = make_config(false); // project_root = /project
    let module_cfg = make_module_cfg(); // src_path = /project/entry
    let manifest = make_manifest();

    let loader = LoaderJson::generate(&config, &module_cfg, &manifest);
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

    let loader = LoaderJson::generate(&config, &module_cfg, &manifest);
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

    let loader = LoaderJson::generate(&config, &module_cfg, &manifest);
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
fn test_loader_json_compile_entry_includes_startup_config_and_tasks() {
    let tmp = tempfile::tempdir().expect("create temp dir");
    let project_root = tmp.path();
    let module_root = project_root.join("entry");
    let profile_dir = module_root.join("src/main/resources/base/profile");
    let startup_dir = module_root.join("src/main/ets/startup");
    std::fs::create_dir_all(&profile_dir).expect("create profile dir");
    std::fs::create_dir_all(&startup_dir).expect("create startup dir");
    std::fs::write(
        module_root.join("src/main/module.json5"),
        r#"{ "module": { "name": "entry", "type": "entry", "appStartup": "$profile:startup_config" } }"#,
    )
    .expect("write module.json5");
    std::fs::write(
        profile_dir.join("startup_config.json"),
        r#"{
            "configEntry": "ets/startup/AppStartupConfig.ets",
            "startupTasks": [{ "name": "Task", "srcEntry": "ets/startup/StartupTask.ets" }]
        }"#,
    )
    .expect("write startup profile");
    let config_entry = startup_dir.join("AppStartupConfig.ets");
    let startup_task = startup_dir.join("StartupTask.ets");
    std::fs::write(&config_entry, "export default class AppStartupConfig {}")
        .expect("write config entry");
    std::fs::write(&startup_task, "export default class StartupTask {}")
        .expect("write startup task");

    let temp_startup = module_root
        .join("build/default/intermediates/startup_config/default/temp-startup-config.json");
    std::fs::create_dir_all(temp_startup.parent().unwrap()).expect("create temp startup dir");
    std::fs::write(
        &temp_startup,
        serde_json::json!({
            "startupTasks": [{
                "name": "Task",
                "srcEntry": "ets/startup/StartupTask.ets",
                "srcEntryAbsolutePath": startup_task
            }]
        })
        .to_string(),
    )
    .expect("write temp startup");

    let mut config = make_config(false);
    config.project_root = project_root.to_path_buf();
    let mut module_cfg = make_module_cfg();
    module_cfg.src_path = module_root;

    let loader = LoaderJson::generate(&config, &module_cfg, &make_manifest());
    let value = serde_json::to_value(&loader).expect("serialize loader");
    let compile_entry = value["compileEntry"].as_array().expect("compileEntry");
    let entries: Vec<_> = compile_entry
        .iter()
        .filter_map(|entry| entry.as_str())
        .collect();

    let expected_config_entry = std::fs::canonicalize(&config_entry)
        .expect("canonical config entry")
        .to_string_lossy()
        .into_owned();
    let expected_startup_task = std::fs::canonicalize(&startup_task)
        .expect("canonical startup task")
        .to_string_lossy()
        .into_owned();
    assert!(entries.contains(&expected_config_entry.as_str()));
    assert!(entries.contains(&expected_startup_task.as_str()));
    assert!(loader
        .compile_entry
        .contains(&"./ets/startup/AppStartupConfig.ets".to_string()));
    assert!(loader
        .compile_entry
        .contains(&"./ets/startup/StartupTask.ets".to_string()));
}

#[test]
fn test_loader_json_compile_entry_includes_insight_intent_entries() {
    let tmp = tempfile::tempdir().expect("create temp dir");
    let project_root = tmp.path();
    let module_root = project_root.join("entry");
    let profile_dir = module_root.join("src/main/resources/base/profile");
    let insight_dir = module_root.join("src/main/ets/insight");
    std::fs::create_dir_all(&profile_dir).expect("create profile dir");
    std::fs::create_dir_all(&insight_dir).expect("create insight dir");
    let intent = insight_dir.join("IntentExecutor.ets");
    let intent_entry = insight_dir.join("IntentEntry.ts");
    std::fs::write(&intent, "export default class IntentExecutor {}").expect("write intent");
    std::fs::write(&intent_entry, "export default class IntentEntry {}")
        .expect("write intent entry");
    std::fs::write(
        profile_dir.join("insight_intent.json"),
        r#"{
            "insightIntents": [{ "srcEntry": "ets/insight/IntentExecutor.ets" }],
            "insightIntentsSrcEntry": [{ "srcEntry": "ets/insight/IntentEntry.ts" }]
        }"#,
    )
    .expect("write insight intent");

    let mut config = make_config(false);
    config.project_root = project_root.to_path_buf();
    let mut module_cfg = make_module_cfg();
    module_cfg.src_path = module_root;

    let loader = LoaderJson::generate(&config, &module_cfg, &make_manifest());
    let value = serde_json::to_value(&loader).expect("serialize loader");
    let compile_entry = value["compileEntry"].as_array().expect("compileEntry");
    let entries: Vec<_> = compile_entry
        .iter()
        .filter_map(|entry| entry.as_str())
        .collect();

    let expected_intent = std::fs::canonicalize(&intent)
        .expect("canonical intent")
        .to_string_lossy()
        .into_owned();
    let expected_intent_entry = std::fs::canonicalize(&intent_entry)
        .expect("canonical intent entry")
        .to_string_lossy()
        .into_owned();
    assert!(entries.contains(&expected_intent.as_str()));
    assert!(entries.contains(&expected_intent_entry.as_str()));
    assert!(loader
        .compile_entry
        .contains(&"./ets/insight/IntentExecutor.ets".to_string()));
    assert!(loader
        .compile_entry
        .contains(&"./ets/insight/IntentEntry.ts".to_string()));
}

#[test]
fn test_loader_json_compile_entry_includes_source_har_ability_entries() {
    let tmp = tempfile::tempdir().expect("create temp dir");
    let project_root = tmp.path();
    let module_root = project_root.join("entry");
    let oh_modules = module_root.join("oh_modules");
    let source_har = oh_modules.join("dep-har");
    let shared_hsp = oh_modules.join("dep-hsp");
    let bytecode_har = oh_modules.join("bytecode-har");

    fs::create_dir_all(&module_root).expect("create module root");
    fs::write(
        module_root.join("oh-package.json5"),
        r#"{
            dependencies: {
                "dep-har": "1.0.0",
                "dep-hsp": "1.0.0",
                "bytecode-har": "1.0.0"
            }
        }"#,
    )
    .expect("write module oh-package");

    let ability_entry = source_har.join("src/main/ets/DepAbility.ets");
    let extension_entry = source_har.join("src/main/ets/DepExtension.ts");
    fs::create_dir_all(ability_entry.parent().unwrap()).expect("create source har ets dir");
    fs::write(&ability_entry, "export default class DepAbility {}").expect("write dep ability");
    fs::write(&extension_entry, "export default class DepExtension {}")
        .expect("write dep extension ability");
    fs::write(
        source_har.join("oh-package.json5"),
        r#"{ name: "dep-har" }"#,
    )
    .expect("write source har oh-package");
    fs::write(
        source_har.join("src/main/module.json5"),
        r#"{
            module: {
                name: "dep_har",
                type: "har",
                abilities: [{ name: "DepAbility", srcEntry: "./ets/DepAbility.ets" }],
                extensionAbilities: [{ name: "DepExtension", srcEntry: "ets/DepExtension.ts" }]
            }
        }"#,
    )
    .expect("write source har module");

    let hsp_entry = shared_hsp.join("src/main/ets/HspAbility.ets");
    fs::create_dir_all(hsp_entry.parent().unwrap()).expect("create shared hsp ets dir");
    fs::write(&hsp_entry, "export default class HspAbility {}").expect("write hsp ability");
    fs::write(
        shared_hsp.join("oh-package.json5"),
        r#"{ name: "dep-hsp" }"#,
    )
    .expect("write hsp oh-package");
    fs::write(
        shared_hsp.join("src/main/module.json5"),
        r#"{
            module: {
                name: "dep_hsp",
                type: "shared",
                abilities: [{ name: "HspAbility", srcEntry: "ets/HspAbility.ets" }]
            }
        }"#,
    )
    .expect("write hsp module");

    let bytecode_entry = bytecode_har.join("src/main/ets/BytecodeAbility.ets");
    fs::create_dir_all(bytecode_entry.parent().unwrap()).expect("create bytecode har ets dir");
    fs::create_dir_all(bytecode_har.join("ets")).expect("create bytecode har abc dir");
    fs::write(&bytecode_entry, "export default class BytecodeAbility {}")
        .expect("write bytecode ability");
    fs::write(bytecode_har.join("ets/modules.abc"), b"PANDA\x00\x00\x00")
        .expect("write bytecode abc");
    fs::write(
        bytecode_har.join("oh-package.json5"),
        r#"{ name: "bytecode-har", metadata: { byteCodeHar: true } }"#,
    )
    .expect("write bytecode oh-package");
    fs::write(
        bytecode_har.join("src/main/module.json5"),
        r#"{
            module: {
                name: "bytecode_har",
                type: "har",
                abilities: [{ name: "BytecodeAbility", srcEntry: "ets/BytecodeAbility.ets" }]
            }
        }"#,
    )
    .expect("write bytecode module");

    let mut config = make_config(false);
    config.project_root = project_root.to_path_buf();
    let mut module_cfg = make_module_cfg();
    module_cfg.src_path = module_root;

    let loader = LoaderJson::generate(&config, &module_cfg, &make_manifest());
    let value = serde_json::to_value(&loader).expect("serialize loader");
    let entries: Vec<_> = value["compileEntry"]
        .as_array()
        .expect("compileEntry")
        .iter()
        .filter_map(|entry| entry.as_str())
        .collect();

    let expected_ability = std::fs::canonicalize(&ability_entry)
        .expect("canonical ability")
        .to_string_lossy()
        .into_owned();
    let expected_extension = std::fs::canonicalize(&extension_entry)
        .expect("canonical extension")
        .to_string_lossy()
        .into_owned();
    let skipped_hsp = std::fs::canonicalize(&hsp_entry)
        .expect("canonical hsp")
        .to_string_lossy()
        .into_owned();
    let skipped_bytecode = std::fs::canonicalize(&bytecode_entry)
        .expect("canonical bytecode")
        .to_string_lossy()
        .into_owned();

    assert!(entries.contains(&expected_ability.as_str()));
    assert!(entries.contains(&expected_extension.as_str()));
    assert!(!entries.contains(&skipped_hsp.as_str()));
    assert!(!entries.contains(&skipped_bytecode.as_str()));
    assert!(loader.compile_entry.contains(&expected_ability));
    assert!(loader.compile_entry.contains(&expected_extension));
}

#[test]
fn test_loader_json_compile_entry_includes_source_har_insight_intent_entries() {
    let tmp = tempfile::tempdir().expect("create temp dir");
    let project_root = tmp.path();
    let module_root = project_root.join("entry");
    let oh_modules = module_root.join("oh_modules");
    let source_har = oh_modules.join("dep-har");
    let custom_resource_har = oh_modules.join("dep-har-custom");
    let shared_hsp = oh_modules.join("dep-hsp");
    let bytecode_har = oh_modules.join("bytecode-har");

    fs::create_dir_all(&module_root).expect("create module root");
    fs::write(
        module_root.join("oh-package.json5"),
        r#"{
            dependencies: {
                "dep-har": "1.0.0",
                "dep-har-custom": "1.0.0",
                "dep-hsp": "1.0.0",
                "bytecode-har": "1.0.0"
            }
        }"#,
    )
    .expect("write module oh-package");

    let default_entry = source_har.join("src/main/ets/insight/DefaultInsight.ts");
    let ignored_default_entry = source_har.join("src/main/ets/insight/IgnoredInsight.ets");
    fs::create_dir_all(default_entry.parent().unwrap()).expect("create source har insight dir");
    fs::create_dir_all(source_har.join("src/main/resources/base/profile"))
        .expect("create default profile dir");
    fs::write(&default_entry, "export default class DefaultInsight {}")
        .expect("write default insight");
    fs::write(
        &ignored_default_entry,
        "export default class IgnoredInsight {}",
    )
    .expect("write ignored insight");
    fs::write(
        source_har.join("oh-package.json5"),
        r#"{ name: "dep-har" }"#,
    )
    .expect("write source har oh-package");
    fs::write(
        source_har.join("src/main/module.json5"),
        r#"{ module: { name: "dep_har", type: "har" } }"#,
    )
    .expect("write source har module");
    fs::write(
        source_har.join("src/main/resources/base/profile/insight_intent.json"),
        r#"{
            insightIntents: [{ srcEntry: "ets/insight/IgnoredInsight.ets" }],
            insightIntentsSrcEntry: [{ srcEntry: "ets/insight/DefaultInsight.ts" }]
        }"#,
    )
    .expect("write source har insight intent");

    let custom_entry = custom_resource_har.join("src/main/ets/insight/CustomInsight.ets");
    fs::create_dir_all(custom_entry.parent().unwrap()).expect("create custom har insight dir");
    fs::create_dir_all(custom_resource_har.join("custom/resources/base/profile"))
        .expect("create custom profile dir");
    fs::write(&custom_entry, "export default class CustomInsight {}")
        .expect("write custom insight");
    fs::write(
        custom_resource_har.join("oh-package.json5"),
        r#"{
            name: "dep-har-custom",
            metadata: { resource: { directories: ["custom/resources"] } }
        }"#,
    )
    .expect("write custom har oh-package");
    fs::write(
        custom_resource_har.join("src/main/module.json5"),
        r#"{ module: { name: "dep_har_custom", type: "har" } }"#,
    )
    .expect("write custom har module");
    fs::write(
        custom_resource_har.join("custom/resources/base/profile/insight_intent.json"),
        r#"{ insightIntentsSrcEntry: [{ srcEntry: "./ets/insight/CustomInsight.ets" }] }"#,
    )
    .expect("write custom har insight intent");

    let hsp_entry = shared_hsp.join("src/main/ets/insight/HspInsight.ets");
    fs::create_dir_all(hsp_entry.parent().unwrap()).expect("create hsp insight dir");
    fs::create_dir_all(shared_hsp.join("src/main/resources/base/profile"))
        .expect("create hsp profile dir");
    fs::write(&hsp_entry, "export default class HspInsight {}").expect("write hsp insight");
    fs::write(
        shared_hsp.join("oh-package.json5"),
        r#"{ name: "dep-hsp" }"#,
    )
    .expect("write hsp oh-package");
    fs::write(
        shared_hsp.join("src/main/module.json5"),
        r#"{ module: { name: "dep_hsp", type: "shared" } }"#,
    )
    .expect("write hsp module");
    fs::write(
        shared_hsp.join("src/main/resources/base/profile/insight_intent.json"),
        r#"{ insightIntentsSrcEntry: [{ srcEntry: "ets/insight/HspInsight.ets" }] }"#,
    )
    .expect("write hsp insight intent");

    let bytecode_entry = bytecode_har.join("src/main/ets/insight/BytecodeInsight.ets");
    fs::create_dir_all(bytecode_entry.parent().unwrap()).expect("create bytecode insight dir");
    fs::create_dir_all(bytecode_har.join("ets")).expect("create bytecode abc dir");
    fs::create_dir_all(bytecode_har.join("src/main/resources/base/profile"))
        .expect("create bytecode profile dir");
    fs::write(&bytecode_entry, "export default class BytecodeInsight {}")
        .expect("write bytecode insight");
    fs::write(bytecode_har.join("ets/modules.abc"), b"PANDA\x00\x00\x00")
        .expect("write bytecode abc");
    fs::write(
        bytecode_har.join("oh-package.json5"),
        r#"{ name: "bytecode-har", metadata: { byteCodeHar: true } }"#,
    )
    .expect("write bytecode oh-package");
    fs::write(
        bytecode_har.join("src/main/module.json5"),
        r#"{ module: { name: "bytecode_har", type: "har" } }"#,
    )
    .expect("write bytecode module");
    fs::write(
        bytecode_har.join("src/main/resources/base/profile/insight_intent.json"),
        r#"{ insightIntentsSrcEntry: [{ srcEntry: "ets/insight/BytecodeInsight.ets" }] }"#,
    )
    .expect("write bytecode insight intent");

    let mut config = make_config(false);
    config.project_root = project_root.to_path_buf();
    let mut module_cfg = make_module_cfg();
    module_cfg.src_path = module_root;

    let loader = LoaderJson::generate(&config, &module_cfg, &make_manifest());
    let value = serde_json::to_value(&loader).expect("serialize loader");
    let entries: Vec<_> = value["compileEntry"]
        .as_array()
        .expect("compileEntry")
        .iter()
        .filter_map(|entry| entry.as_str())
        .collect();

    let expected_default = std::fs::canonicalize(&default_entry)
        .expect("canonical default insight")
        .to_string_lossy()
        .into_owned();
    let expected_custom = std::fs::canonicalize(&custom_entry)
        .expect("canonical custom insight")
        .to_string_lossy()
        .into_owned();
    let skipped_default_intent = std::fs::canonicalize(&ignored_default_entry)
        .expect("canonical ignored insight")
        .to_string_lossy()
        .into_owned();
    let skipped_hsp = std::fs::canonicalize(&hsp_entry)
        .expect("canonical hsp")
        .to_string_lossy()
        .into_owned();
    let skipped_bytecode = std::fs::canonicalize(&bytecode_entry)
        .expect("canonical bytecode")
        .to_string_lossy()
        .into_owned();

    assert!(entries.contains(&expected_default.as_str()));
    assert!(entries.contains(&expected_custom.as_str()));
    assert!(!entries.contains(&skipped_default_intent.as_str()));
    assert!(!entries.contains(&skipped_hsp.as_str()));
    assert!(!entries.contains(&skipped_bytecode.as_str()));
    assert!(loader.compile_entry.contains(&expected_default));
    assert!(loader.compile_entry.contains(&expected_custom));
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

    let loader = LoaderJson::generate(&config, &module_cfg, &manifest);
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

    let loader = LoaderJson::generate(&config, &module_cfg, &manifest);
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

    let loader = LoaderJson::generate(&config, &module_cfg, &manifest);

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
        support_chunks: false,
    });
    let module_cfg = ModuleConfig {
        name: "entry".to_string(),
        src_path: module_root,
        module_type: OhosModuleType::Entry,
    };

    let loader = LoaderJson::generate(&config, &module_cfg, &make_manifest());

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
        support_chunks: false,
    });
    let module_cfg = ModuleConfig {
        name: "entry".to_string(),
        src_path: module_root.clone(),
        module_type: OhosModuleType::Entry,
    };

    let loader = LoaderJson::generate(&config, &module_cfg, &make_manifest());

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

    let loader = LoaderJson::generate(&config, &module_cfg, &make_manifest());

    assert!(
        loader
            .dynamic_import_lib_info
            .contains_key("@runtime/indirect"),
        "dependency metadata.runtimeOnly.packages must feed dynamicImportLibInfo"
    );
}

#[test]
fn test_loader_json_runtime_only_metadata_follows_declared_transitive_scoped_dependencies() {
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

    let source_har = project_root.join("oh_modules").join("source-har");
    std::fs::create_dir_all(&source_har).expect("create source har");
    std::fs::write(
        source_har.join("oh-package.json5"),
        r#"{
              name: "source-har",
              dependencies: {
                "@scope/transitive-har": "1.0.0"
              }
            }"#,
    )
    .expect("write source har oh-package");

    let transitive_har = project_root
        .join("oh_modules")
        .join("@scope")
        .join("transitive-har");
    let transitive_runtime_dir = transitive_har.join("src/main/ets/runtime");
    std::fs::create_dir_all(&transitive_runtime_dir).expect("create transitive runtime dir");
    let transitive_source = transitive_runtime_dir.join("TransitiveRuntime.ets");
    std::fs::write(
        &transitive_source,
        "export default class TransitiveRuntime {}",
    )
    .expect("write transitive source");
    std::fs::write(
        transitive_har.join("oh-package.json5"),
        r#"{
              name: "@scope/transitive-har",
              metadata: {
                runtimeOnly: {
                  sources: ["src/main/ets/runtime"],
                  packages: ["@runtime/transitive"]
                }
              }
            }"#,
    )
    .expect("write transitive har oh-package");

    let runtime_pkg = project_root
        .join("oh_modules")
        .join("@runtime")
        .join("transitive");
    let runtime_index = runtime_pkg.join("src/main/ets/Index.ets");
    std::fs::create_dir_all(runtime_index.parent().unwrap()).expect("create runtime package");
    std::fs::write(
        runtime_pkg.join("oh-package.json5"),
        r#"{ name: "@runtime/transitive", main: "src/main/ets/Index.ets" }"#,
    )
    .expect("write runtime package oh-package");
    std::fs::write(&runtime_index, "export default class RuntimePackage {}")
        .expect("write runtime package entry");

    let undeclared_har = project_root.join("oh_modules").join("undeclared-har");
    std::fs::create_dir_all(&undeclared_har).expect("create undeclared har");
    std::fs::write(
        undeclared_har.join("oh-package.json5"),
        r#"{
              name: "undeclared-har",
              metadata: {
                runtimeOnly: {
                  packages: ["@runtime/undeclared"]
                }
              }
            }"#,
    )
    .expect("write undeclared har oh-package");
    let undeclared_pkg = project_root
        .join("oh_modules")
        .join("@runtime")
        .join("undeclared");
    std::fs::create_dir_all(undeclared_pkg.join("src/main/ets"))
        .expect("create undeclared runtime package");
    std::fs::write(
        undeclared_pkg.join("oh-package.json5"),
        r#"{ name: "@runtime/undeclared", main: "src/main/ets/Index.ets" }"#,
    )
    .expect("write undeclared runtime package oh-package");

    let mut config = make_config(false);
    config.project_root = project_root.clone();
    let module_cfg = ModuleConfig {
        name: "entry".to_string(),
        src_path: module_root,
        module_type: OhosModuleType::Entry,
    };

    let loader = LoaderJson::generate(&config, &module_cfg, &make_manifest());

    assert!(loader
        .hvigor_compile_entry
        .contains(&to_absolute_path_string(&transitive_source)));
    assert!(loader
        .hvigor_compile_entry
        .contains(&to_absolute_path_string(&runtime_index)));
    assert!(
        loader
            .dynamic_import_lib_info
            .contains_key("@runtime/transitive"),
        "transitive dependency metadata.runtimeOnly.packages must feed dynamicImportLibInfo"
    );
    assert!(
        !loader
            .dynamic_import_lib_info
            .contains_key("@runtime/undeclared"),
        "undeclared package metadata.runtimeOnly must not leak into dynamicImportLibInfo"
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

    let loader = LoaderJson::generate(&config, &module_cfg, &manifest);

    assert!(
        loader.byte_code_har_info.contains_key("@ohos/abc-lib"),
        "byteCodeHarInfo must contain @ohos/abc-lib"
    );
    let entry = &loader.byte_code_har_info["@ohos/abc-lib"];
    assert_eq!(entry.compatible_sdk_version, "5.0.0(12)");
    assert!(entry.abc_path.ends_with("index.abc"));
    assert!(entry.source_maps_path.is_none());
}
