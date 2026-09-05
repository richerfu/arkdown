use std::fs;
use std::path::PathBuf;

use arkdown_core::{
    AbilityEntry, ArkOptions, ArkdownConfig, BuildMode, BuildOption, ExtensionAbilityEntry,
    HapManifest, MetadataEntry, ModuleConfig, NativeLibOption, OhosModuleType, OutputTarget,
    ProductConfig, RuntimeOnlyConfig, SdkConfig, StrictModeConfig,
};

use super::*;

#[test]
fn compile_lib_entry_property_matches_hvigor_full_compilation_truth_table() {
    // GenerateLoaderJson.isFullCompilationEnabled in Hvigor 6.24.2:
    // (!isFormWidget || property !== undefined) && !property.
    assert!(LoaderCompilationPolicy::default().is_full_compilation_enabled(false));
    assert!(!LoaderCompilationPolicy::default().is_full_compilation_enabled(true));
    assert!(LoaderCompilationPolicy::from_hvigor_property(Some(false))
        .is_full_compilation_enabled(false));
    assert!(LoaderCompilationPolicy::from_hvigor_property(Some(false))
        .is_full_compilation_enabled(true));
    assert!(!LoaderCompilationPolicy::from_hvigor_property(Some(true))
        .is_full_compilation_enabled(false));
    assert!(!LoaderCompilationPolicy::from_hvigor_property(Some(true))
        .is_full_compilation_enabled(true));
}

fn make_config(use_normalized_ohm_url: bool) -> ArkdownConfig {
    ArkdownConfig {
        project_root: PathBuf::from("/project"),
        build_mode: BuildMode::Debug,
        output_target: OutputTarget::Hap,
        product: ProductConfig {
            name: "default".to_string(),
            artifact_name: None,
            bundle_name: "com.example.app".to_string(),
            compatible_sdk_version: "5.0.0(12)".to_string(),
            compatible_sdk_version_stage: None,
            target_sdk_version: None,
            runtime_os: "HarmonyOS".to_string(),
            signing_config: None,
            build_option: BuildOption {
                strict_mode: StrictModeConfig {
                    case_sensitive_check: false,
                    use_normalized_ohm_url,
                    no_external_import_by_path: None,
                    duplicate_dependency_check: false,
                    api_compatibility_check: None,
                },
                ark_options: None,
                res_options: None,
                source_option: None,
                preload_system_so: false,
                preload_so: vec![],
                deduplicate_har: false,
                enable_source_code_check: true,
                app_with_signed_pkg: false,
                generate_shared_tgz: None,
                customized_har_base_package: None,
                remove_permissions: vec![],
                aot_compile_mode: None,
                host_pgo: None,
                ap_path: None,
                abi_filters: vec![],
                external_native_options: None,
                har_artifact_type: arkdown_core::HarArtifactType::Original,
                native_lib: NativeLibOption::default(),
            },
            version_code: None,
            version_name: None,
            build_version: None,
            app_options: Default::default(),
        },
        modules: vec![],
        sdk_config: SdkConfig {
            ets_loader_path: PathBuf::from("/sdk/ets-loader"),
            es2abc_path: PathBuf::from("/sdk/es2abc"),
            restool_path: PathBuf::from("/sdk/restool"),
            compile_api_version: 12,
            sdk_component_api_version: 12,
            has_rollup_plugin: true,
            compile_sdk_version: "6.0.0.47".to_string(),
        },
    }
}

#[test]
fn collect_compile_files_matches_hvigor_breadth_first_directory_order() {
    let temp = tempfile::tempdir().expect("tempdir");
    let root = temp.path().join("ets");
    fs::create_dir_all(root.join("components/tasks")).expect("nested source directory");
    fs::write(root.join("Index.ets"), "export {};\n").expect("root Index");
    fs::write(root.join("Root.ts"), "export {};\n").expect("root source");
    fs::write(root.join("components/Component.ets"), "export {};\n").expect("component source");
    fs::write(root.join("components/tasks/Task.ets"), "export {};\n").expect("task source");

    let files = LoaderJson::collect_code_files(&root);
    let relative = files
        .iter()
        .map(|path| {
            arkdown_path::ArtifactPathSerializer::rel_to_slash(
                path.strip_prefix(&root).expect("source below root"),
            )
        })
        .collect::<Vec<_>>();

    assert_eq!(
        relative,
        [
            "Index.ets",
            "Root.ts",
            "components/Component.ets",
            "components/tasks/Task.ets",
        ]
    );
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
        pages: None,
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
fn test_loader_json_main_pages_are_runtime_entries() {
    let tmp = tempfile::tempdir().expect("tempdir");
    let module_root = tmp.path().join("entry");
    let profile_dir = module_root.join("src/main/resources/base/profile");
    fs::create_dir_all(&profile_dir).expect("create profile dir");
    write_minimal_module_json5(&module_root);
    fs::write(
        profile_dir.join("main_pages.json"),
        r#"{"src":["pages/Index","./ets/pages/PrivacyPage"]}"#,
    )
    .expect("write main pages profile");

    let mut config = make_config(true);
    config.project_root = tmp.path().to_path_buf();
    let mut module_cfg = make_module_cfg();
    module_cfg.src_path = module_root;
    let mut manifest = make_manifest();
    manifest.pages = Some("$profile:main_pages".to_string());

    let loader = LoaderJson::try_generate(&config, &module_cfg, &manifest, "default").unwrap();

    assert!(loader
        .runtime_entry_sources
        .contains(&"./ets/pages/Index.ets".to_string()));
    assert!(loader
        .runtime_entry_sources
        .contains(&"./ets/pages/PrivacyPage.ets".to_string()));
    assert_eq!(
        loader.page_entry_sources,
        [
            "./ets/pages/Index.ets".to_string(),
            "./ets/pages/PrivacyPage.ets".to_string(),
        ]
    );
    assert!(!loader
        .page_entry_sources
        .contains(&"./ets/entryability/EntryAbility.ets".to_string()));
    assert!(loader
        .compile_entry
        .contains(&"./ets/pages/Index.ets".to_string()));
    assert!(loader
        .compile_entry
        .contains(&"./ets/pages/PrivacyPage.ets".to_string()));
}

#[test]
fn test_loader_json_missing_main_pages_profile_is_error() {
    let tmp = tempfile::tempdir().expect("tempdir");
    let module_root = tmp.path().join("entry");
    write_minimal_module_json5(&module_root);

    let mut config = make_config(true);
    config.project_root = tmp.path().to_path_buf();
    let mut module_cfg = make_module_cfg();
    module_cfg.src_path = module_root;
    let mut manifest = make_manifest();
    manifest.pages = Some("$profile:main_pages".to_string());

    let error = LoaderJson::try_generate(&config, &module_cfg, &manifest, "default").unwrap_err();
    assert!(
        matches!(error, ArkdownError::Io(_)),
        "missing main_pages profile must fail instead of silently dropping entries: {error}"
    );
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
    let loader =
        LoaderJson::try_generate(&config, &module_cfg, &make_manifest(), "default").unwrap();

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
fn test_loader_json_form_profile_sources_feed_internal_compile_entries() {
    let tmp = tempfile::tempdir().expect("tempdir");
    let module_root = tmp.path().join("entry");
    let profile_dir = module_root.join("src/main/resources/base/profile");
    let form_source = module_root.join("src/main/ets/forms/Card.ets");
    fs::create_dir_all(&profile_dir).expect("create profile dir");
    fs::create_dir_all(form_source.parent().expect("form source parent"))
        .expect("create form source dir");
    write_minimal_module_json5(&module_root);
    fs::write(&form_source, "export default struct Card {}").expect("write form source");
    fs::write(
        profile_dir.join("form_config.json"),
        r#"{ forms: [{ name: "Card", src: "./ets/forms/Card.ets", uiSyntax: "arkts" }] }"#,
    )
    .expect("write form profile");

    let mut config = make_config(true);
    config.project_root = tmp.path().to_path_buf();
    let mut module_cfg = make_module_cfg();
    module_cfg.src_path = module_root;
    let mut manifest = make_manifest();
    manifest.extension_abilities = vec![form_extension_ability("form_config")];
    let loader = LoaderJson::try_generate(&config, &module_cfg, &manifest, "default").unwrap();

    // 6.24.3: form card sources never enter the module compile entries —
    // hvigor compiles them in the dedicated widget pass (`widgets.abc`) and
    // the serialized `compileEntry` only carries ResourceTable.ts and the
    // configured entries (`isFullCompilationEnabled` is false for the form
    // widget module). The internal `form_entry_sources` still feed the widget
    // pass and the form widget module's main es2abc input.
    assert!(
        !loader
            .compile_entry
            .iter()
            .any(|entry| entry.contains("forms/Card.ets")),
        "form widget sources must not enter compile_entry: {:?}",
        loader.compile_entry
    );
    assert_eq!(
        loader.form_entry_sources,
        vec!["./ets/forms/Card.ets".to_string()],
        "SDK readCardForm preserves referenced form sources in cardObj for ETS entry lowering"
    );
    assert_eq!(loader.insight_intent_form_bindings.len(), 1);
    assert_eq!(
        loader.insight_intent_form_bindings[0].source,
        "./ets/form/EntryFormAbility.ets"
    );
    assert_eq!(
        loader.insight_intent_form_bindings[0].ability_name,
        "EntryFormAbility"
    );
    assert_eq!(
        loader.insight_intent_form_bindings[0].form_names,
        vec!["Card".to_string()]
    );
    assert!(
        !loader
            .hvigor_compile_entry
            .iter()
            .any(|entry| entry.contains("forms/Card.ets")),
        "hvigor loader.json compileEntry does not serialize form widget sources"
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

    let error = LoaderJson::try_generate(&config, &module_cfg, &manifest, "default").unwrap_err();
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
    fs::write(
        module_root.join("oh-package.json5"),
        r#"{ name: "entry", dependencies: { "bytecode-har": "1.0.0" } }"#,
    )
    .expect("write entry package");

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
    fs::create_dir_all(bytecode_har.join("src/main")).expect("create bytecode har profile dir");
    fs::write(
        bytecode_har.join("ets").join("modules.abc"),
        b"PANDA\x00\x00\x00",
    )
    .expect("write abc");
    fs::write(
        bytecode_har.join("oh-package.json5"),
        r#"{
              name: "bytecode-har",
              dependencies: { "dep-a": "1.1.0" },
              metadata: {
                byteCodeHar: true,
                dependencyPkgVersion: { "dep-a": "1.0.0" }
              }
            }"#,
    )
    .expect("write bytecode har package");
    fs::write(
        bytecode_har.join("src/main/module.json5"),
        r#"{ module: { name: "bytecode-har", type: "har" } }"#,
    )
    .expect("write bytecode har module profile");
    fs::create_dir_all(oh_modules.join("dep-a")).expect("create dep package");
    fs::write(
        oh_modules.join("dep-a").join("oh-package.json5"),
        r#"{ name: "dep-a", version: "1.1.0" }"#,
    )
    .expect("write dep package");
    fs::write(
        module_root.join("oh-package.json5"),
        r#"{ name: "entry", dependencies: { "bytecode-har": "1.0.0" } }"#,
    )
    .expect("write entry package");

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

#[test]
fn test_loader_json_refreshes_bytecode_har_other_compile_files_from_late_scan() {
    let tmp = tempfile::tempdir().expect("tempdir");
    let project_root = tmp.path().to_path_buf();
    let module_root = project_root.join("entry");
    let oh_modules = module_root.join("oh_modules");
    let bytecode_har = oh_modules.join("bytecode-har");
    let source_har = oh_modules.join("source-har");

    fs::create_dir_all(&module_root).expect("create module root");
    fs::write(
        module_root.join("oh-package.json5"),
        r#"{ dependencies: {} }"#,
    )
    .expect("write module package");
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
                declarationEntry: [
                  "@normalized:N&&&source-har/src/main/ets/index&1.0.0"
                ]
              }
            }"#,
    )
    .expect("write bytecode har package");
    fs::create_dir_all(source_har.join("src/main/ets/pages")).expect("create source har ets dir");
    fs::write(
        source_har.join("oh-package.json5"),
        r#"{ name: "source-har", main: "src/main/ets/index.ets" }"#,
    )
    .expect("write source har package");
    fs::write(
        source_har.join("src/main/module.json5"),
        r#"{ module: { name: "source", type: "har" } }"#,
    )
    .expect("write source har module json");
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

    let mut config = make_config(true);
    config.project_root = project_root.clone();
    let mut module_cfg = make_module_cfg();
    module_cfg.src_path = module_root.clone();
    let mut loader = LoaderJson::generate(&config, &module_cfg, &make_manifest());
    loader.byte_code_har_info.clear();
    loader.other_compile_files.clear();
    loader
        .compile_entry
        .retain(|entry| !entry.contains("source-har"));

    let deps =
        ByteCodeHarMetadataWriter::scan_bytecode_har_deps_from_graph(&module_root, &project_root);
    loader.apply_bytecode_har_scan_entries(&deps, &module_root, &project_root, OutputTarget::Hap);

    assert!(loader.byte_code_har_info.contains_key("bytecode-har"));
    assert_eq!(
        loader.declaration_entry,
        vec!["@normalized:N&&&source-har/src/main/ets/index&1.0.0".to_string()]
    );
    assert!(loader
        .other_compile_files
        .iter()
        .any(|path| path.ends_with("source-har/src/main/ets/index.ets")));
    assert!(loader
        .other_compile_files
        .iter()
        .any(|path| path.ends_with("source-har/src/main/ets/pages/Page.ets")));
    for path in &loader.other_compile_files {
        assert!(
            loader.compile_entry.contains(path),
            "late ByteCode HAR otherCompileFiles entry must also be compiled: {path}"
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

    let mut loader = LoaderJson::generate(&config, &module_cfg, &manifest);
    loader.compiler_types = vec!["./types/runtime".to_string()];
    loader.compiler_type_declaration_inputs = vec![PathBuf::from("/project/types/runtime.d.ts")];
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
        "customizedHar",
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
    assert!(
        value.get("compilerTypes").is_none(),
        "hvigor forwards arkOptions.types in compiler memory, not loader.json"
    );
    assert!(value.get("compilerTypeDeclarationInputs").is_none());
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
  "customizedHar": false,
  "anBuildOutPut": "/project/entry/build/default/intermediates/loader_out/default/an/arm64-v8a",
  "anBuildMode": "type"
}"#
    );
}

/// Installed hvigor 6.24.3 `GenerateLoaderJson.processByteCodeHar()` appends
/// both the marker and the generated ModuleInfo source only for ByteCode HAR.
#[test]
fn bytecode_har_loader_serializes_marker_and_module_info_entry() {
    let temp = tempfile::tempdir().expect("create project");
    let module_root = temp.path().join("entry");
    let source_root = module_root.join("src/main/ets");
    let module_info = module_root.join("build/default/generated/profile/default/ModuleInfo.ts");
    fs::create_dir_all(&source_root).expect("create source root");
    fs::create_dir_all(module_info.parent().unwrap()).expect("create generated profile root");
    fs::write(source_root.join("Index.ets"), "export class Index {}").expect("write HAR source");
    fs::write(&module_info, "export const __MODULE_NAME__ = ''\n").expect("write ModuleInfo.ts");

    let mut config = make_config(true);
    config.project_root = temp.path().to_path_buf();
    config.output_target = OutputTarget::Har;
    config.product.build_option.har_artifact_type = arkdown_core::HarArtifactType::Bytecode;
    let module_cfg = ModuleConfig {
        name: "entry".to_string(),
        src_path: module_root,
        module_type: OhosModuleType::Har,
    };
    let mut manifest = make_manifest();
    manifest.module_type = OhosModuleType::Har;

    let mut loader = LoaderJson::generate(&config, &module_cfg, &manifest);
    loader.include_byte_code_har_module_info(&module_info);

    let value = serde_json::to_value(&loader).expect("serialize loader");
    assert_eq!(value["byteCodeHar"], true);
    assert_eq!(
        loader.hvigor_compile_entry.last(),
        Some(&to_absolute_path_string(&module_info))
    );
    assert_eq!(
        loader.compile_entry.last(),
        Some(&to_absolute_path_string(&module_info))
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
    assert_eq!(
        loader.cache_path,
        "/project/entry/build/default/cache/default/default@CompileArkTS/esmodule/debug/"
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
fn har_loader_uses_har_compile_arkts_cache_owner() {
    let mut config = make_config(false);
    config.output_target = OutputTarget::Har;
    let mut module_cfg = make_module_cfg();
    module_cfg.module_type = OhosModuleType::Har;

    let loader = LoaderJson::generate(&config, &module_cfg, &make_manifest());

    assert_eq!(
        loader.cache_path,
        "/project/entry/build/default/cache/default/default@HarCompileArkTS/esmodule/debug/"
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
fn test_loader_json_module_path_map_keeps_hvigor_submodule_scope() {
    let tmp = tempfile::tempdir().expect("create temp dir");
    let project_root = tmp.path();
    let module_root = project_root.join("entry");
    let source_har = module_root.join("oh_modules/source-har");
    let transitive_pkg = source_har.join("oh_modules/@scope/transitive");
    let root_dev_pkg = project_root.join("oh_modules/@ohos/hypium");
    let undeclared_pkg = project_root.join("oh_modules/undeclared");

    fs::create_dir_all(&module_root).expect("create module root");
    fs::write(
        module_root.join("oh-package.json5"),
        r#"{ dependencies: { "source-har": "1.0.0" } }"#,
    )
    .expect("write module oh-package");
    fs::write(
        project_root.join("oh-package.json5"),
        r#"{ devDependencies: { "@ohos/hypium": "1.0.11" } }"#,
    )
    .expect("write project oh-package");

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

    fs::create_dir_all(&root_dev_pkg).expect("create root dev package");
    fs::write(
        root_dev_pkg.join("oh-package.json5"),
        r#"{ name: "@ohos/hypium", version: "1.0.11", main: "index.js" }"#,
    )
    .expect("write root dev package oh-package");
    fs::write(root_dev_pkg.join("index.js"), "export default 3;")
        .expect("write root dev package entry");

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

    assert_eq!(
        loader.module_path_map.len(),
        1,
        "hvigor GenerateLoaderJson#getModulePathMap only serializes project submodules"
    );
    assert_eq!(
        loader.module_path_map["entry"],
        to_absolute_path_string(&module_cfg.src_path)
    );
    assert!(!loader.module_path_map.contains_key("source-har"));
    assert!(!loader.module_path_map.contains_key("@scope/transitive"));
    assert!(!loader.module_path_map.contains_key("@ohos/hypium"));
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
    let canonical_module_root = arkdown_path::canonicalize_or_owned(&module_root);
    let canonical_source_ap = arkdown_path::canonicalize_or_owned(&source_ap);
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
    let build_config = module_root.join("build/config/buildConfig.json");
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
    let build_config = module_root.join(".test/config/buildConfig.json");
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
    let build_config = module_root.join(".preview/config/buildConfig.json");
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
        Some(".preview/config/buildConfig.json")
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
    let entry_ability = ets_dir.join("entryability").join("EntryAbility.ets");
    let mock_service = mock_dir.join("MockService.ets");
    let mock_page = mock_dir.join("PageMock.ts");
    fs::create_dir_all(entry_ability.parent().expect("entry ability directory"))
        .expect("create entry ability directory");
    fs::write(&insight, "export class Insight {}").expect("write insight source");
    fs::write(&entry_ability, "export class EntryAbility {}").expect("write entry ability");
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
        config.output_target = output_target.clone();
        let mut module_cfg = make_module_cfg();
        module_cfg.src_path = module_root.clone();

        let loader = LoaderJson::generate(&config, &module_cfg, &make_manifest());

        if output_target == OutputTarget::Preview {
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
        } else {
            // Local unit-test builds serialize a narrowed compileEntry
            // (hvigor `test` task: ResourceTable.ts plus mock sources);
            // the module srcEntry and pages stay internal.
            assert_eq!(
                loader.hvigor_compile_entry,
                vec![
                    to_absolute_path_string(&mock_service),
                    to_absolute_path_string(&mock_page),
                ]
            );
            assert!(
                loader
                    .compile_entry
                    .contains(&"./ets/entryability/EntryAbility.ets".to_string()),
                "UnitTestArkCompile.mainModulePath keeps the entry module ability in the internal compiler input"
            );
        }
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
        "customizedHar",
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
        loader.har_name_ohm_map.len(),
        0,
        "ordinary HAP targets must match hvigor and leave harNameOhmMap empty"
    );
    assert!(
        !loader.har_name_ohm_map.contains_key("undeclared-har"),
        "undeclared HAR must not be collected"
    );
}

/// Verify that hspNameOhmMap/harNameOhmMap use `@normalized:N&...` format
/// when `useNormalizedOHMUrl=true`.
///
/// Addresses: "normalized-off 模式仍未证明" — proves both modes work.
#[test]
fn test_loader_json_hsp_har_ohm_maps_normalized_mode() {
    let tmp = tempfile::tempdir().expect("create temp dir");
    let project_root = tmp.path();
    let module_root = project_root.join("entry");
    let direct_hsp = module_root.join("oh_modules/my-hsp");
    let direct_har = module_root.join("oh_modules/direct-har");

    fs::create_dir_all(&module_root).expect("create module root");
    fs::write(
        module_root.join("oh-package.json5"),
        r#"{ byteCodeHar: true, dependencies: { "my-hsp": "1.0.0", "direct-har": "2.0.0" } }"#,
    )
    .expect("write module oh-package");

    fs::create_dir_all(direct_hsp.join("src/main/ets")).expect("create hsp ets");
    fs::write(
        direct_hsp.join("oh-package.json5"),
        r#"{ name: "my-hsp", version: "1.2.3", main: "index.ets" }"#,
    )
    .expect("write hsp oh-package");
    fs::write(
        direct_hsp.join("src/main/module.json5"),
        r#"{ module: { name: "myHsp", type: "shared" } }"#,
    )
    .expect("write hsp module");
    fs::write(direct_hsp.join("index.ets"), "export {}").expect("write hsp entry");

    fs::create_dir_all(direct_har.join("src/main/ets")).expect("create har ets");
    fs::write(
        direct_har.join("oh-package.json5"),
        r#"{ name: "direct-har", version: "2.0.0", main: "src/main/ets/HarEntry.ts" }"#,
    )
    .expect("write har oh-package");
    fs::write(
        direct_har.join("src/main/module.json5"),
        r#"{ module: { name: "direct_har_module", type: "har" } }"#,
    )
    .expect("write har module");
    fs::write(direct_har.join("src/main/ets/HarEntry.ts"), "export {}").expect("write har entry");

    // Test normalized mode.
    let mut config = make_config(true); // use_normalized_ohm_url = true
    config.project_root = project_root.to_path_buf();
    config.output_target = OutputTarget::Har;
    config.product.build_option.har_artifact_type = arkdown_core::HarArtifactType::Bytecode;
    let mut module_cfg = make_module_cfg();
    module_cfg.src_path = module_root.clone();
    module_cfg.module_type = OhosModuleType::Har;
    let mut manifest = make_manifest();
    manifest.module_type = OhosModuleType::Har;

    let loader = LoaderJson::generate(&config, &module_cfg, &manifest);

    let ohm = loader
        .hsp_name_ohm_map
        .get("my-hsp")
        .expect("my-hsp must be in map");
    assert_eq!(
        ohm, "@normalized:N&myHsp&&my-hsp/index&",
        "normalized HSP map must be generated from pkgContextInfo"
    );
    let har_ohm = loader
        .har_name_ohm_map
        .get("direct-har")
        .expect("direct-har must be in ByteCode HAR map");
    assert_eq!(
        har_ohm, "@normalized:N&&&direct-har/src/main/ets/HarEntry&2.0.0",
        "normalized HAR map must keep pkgContextInfo moduleName empty"
    );

    // Test non-normalized mode.
    let mut config2 = make_config(false);
    config2.project_root = project_root.to_path_buf();
    config2.output_target = OutputTarget::Har;
    config2.product.build_option.har_artifact_type = arkdown_core::HarArtifactType::Bytecode;
    let mut module_cfg2 = make_module_cfg();
    module_cfg2.src_path = module_root;
    module_cfg2.module_type = OhosModuleType::Har;

    let loader2 = LoaderJson::generate(&config2, &module_cfg2, &manifest);

    let ohm2 = loader2
        .hsp_name_ohm_map
        .get("my-hsp")
        .expect("my-hsp must be in map");
    assert!(
        ohm2.starts_with("@bundle:"),
        "non-normalized mode must use @bundle: prefix; got: {ohm2}"
    );
    assert!(
        loader2.har_name_ohm_map.is_empty(),
        "non-normalized mode has no pkgContextInfo-backed harNameOhmMap"
    );
}

/// Verify all path-valued fields in the serialized loader.json are absolute.
#[test]
fn test_loader_json_serialized_paths_are_absolute() {
    let temp = tempfile::tempdir().expect("temporary project");
    let project_root = temp.path().join("project");
    let module_root = project_root.join("entry");
    fs::create_dir_all(&module_root).expect("module root");
    let config = ArkdownConfig {
        project_root,
        ..make_config(false)
    };
    let module_cfg = ModuleConfig {
        src_path: module_root,
        ..make_module_cfg()
    };
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
    assert_eq!(value["customizedHar"], false);
}

#[test]
fn test_loader_json_uses_sdk_component_api_for_resource_table_and_serializes_customized_har() {
    let mut config = make_config(false);
    config.sdk_config.compile_api_version = 17;
    config.sdk_config.sdk_component_api_version = 24;
    config.product.build_option.customized_har_base_package =
        Some(PathBuf::from("/project/entry/base.abc"));
    let module_cfg = make_module_cfg();
    let manifest = make_manifest();

    let loader = LoaderJson::generate(&config, &module_cfg, &manifest);
    assert!(loader
        .hvigor_compile_entry
        .iter()
        .any(|entry| entry.ends_with("/entry/build/default/generated/r/default/ResourceTable.ts")));
    assert!(loader
        .compile_entry
        .iter()
        .any(|entry| entry.ends_with("/entry/build/default/generated/r/default/ResourceTable.ts")));
    let value = serde_json::to_value(&loader).expect("serialize loader");
    assert_eq!(value["customizedHar"], true);

    config.sdk_config.sdk_component_api_version = 22;
    let legacy_sdk_loader = LoaderJson::generate(&config, &module_cfg, &manifest);
    assert!(!legacy_sdk_loader
        .hvigor_compile_entry
        .iter()
        .any(|entry| entry.ends_with("ResourceTable.ts")));

    config.sdk_config.sdk_component_api_version = 24;
    config.output_target = OutputTarget::Preview;
    let preview_loader = LoaderJson::generate(&config, &module_cfg, &manifest);
    assert!(!preview_loader
        .hvigor_compile_entry
        .iter()
        .any(|entry| entry.ends_with("ResourceTable.ts")));
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

    let temp_startup =
        module_root.join("build/default/intermediates/startup/default/startup_config.json");
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

    let expected_config_entry = to_absolute_path_string(&config_entry);
    let expected_startup_task = to_absolute_path_string(&startup_task);
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

    let expected_intent = to_absolute_path_string(&intent);
    let expected_intent_entry = to_absolute_path_string(&intent_entry);
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

    let expected_ability = to_absolute_path_string(&ability_entry);
    let expected_extension = to_absolute_path_string(&extension_entry);
    let skipped_hsp = to_absolute_path_string(&hsp_entry);
    let skipped_bytecode = to_absolute_path_string(&bytecode_entry);

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

    let expected_default = to_absolute_path_string(&default_entry);
    let expected_custom = to_absolute_path_string(&custom_entry);
    let skipped_default_intent = to_absolute_path_string(&ignored_default_entry);
    let skipped_hsp = to_absolute_path_string(&hsp_entry);
    let skipped_bytecode = to_absolute_path_string(&bytecode_entry);

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

    // GenerateLoaderJson serializes a source HAR's package main only when the
    // HAR itself is the build target. Consumer HAP builds keep this entry in
    // arkdown's internal compile plan without leaking it into loader.json.
    let mut har_config = make_config(false);
    har_config.project_root = tmp.path().to_path_buf();
    har_config.output_target = OutputTarget::Har;
    let har_loader = LoaderJson::generate(&har_config, &module_cfg, &manifest);
    let expected_main = to_absolute_path_string(&module_root.join("Index.ets"));
    assert_eq!(har_loader.hvigor_compile_entry, vec![expected_main]);
    assert!(har_loader.source_roots.is_empty());
}

#[test]
fn test_hsp_full_compilation_serializes_all_module_ets_sources() {
    let tmp = tempfile::tempdir().expect("create temp dir");
    let module_root = tmp.path().join("shared");
    let ets_root = module_root.join("src/main/ets");
    std::fs::create_dir_all(ets_root.join("model")).unwrap();
    std::fs::write(ets_root.join("Index.ets"), "export {};").unwrap();
    std::fs::write(ets_root.join("model/User.ts"), "export interface User {}").unwrap();
    std::fs::write(ets_root.join("model/User.d.ts"), "export interface User {}").unwrap();

    let mut config = make_config(false);
    config.project_root = tmp.path().to_path_buf();
    config.output_target = OutputTarget::Hsp;
    let module_cfg = ModuleConfig {
        name: "shared".to_string(),
        src_path: module_root,
        module_type: OhosModuleType::Shared,
    };
    let mut manifest = make_manifest();
    manifest.module_name = "shared".to_string();
    manifest.module_type = OhosModuleType::Shared;
    manifest.abilities.clear();
    manifest.main_element = None;
    manifest.pages = None;
    manifest.src_entry = Some("./ets/Index.ets".to_string());

    let loader = LoaderJson::generate(&config, &module_cfg, &manifest);
    let index = to_absolute_path_string(&ets_root.join("Index.ets"));
    let user = to_absolute_path_string(&ets_root.join("model/User.ts"));
    assert!(loader.hvigor_compile_entry.contains(&index));
    assert!(loader.hvigor_compile_entry.contains(&user));
    assert!(loader
        .hvigor_compile_entry
        .iter()
        .all(|entry| !entry.ends_with("User.d.ts")));
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

#[test]
fn test_loader_json_collects_router_map_from_ohpm_source_har_module_json() {
    let tmp = tempfile::tempdir().expect("create temp dir");
    let project_root = tmp.path().to_path_buf();
    let module_root = project_root.join("entry");
    let dep_root = project_root
        .join("oh_modules")
        .join("@cashier_alipay")
        .join("cashiersdk");
    let dep_profile_dir = dep_root.join("src/main/resources/base/profile");

    fs::create_dir_all(&module_root).expect("create module root");
    fs::create_dir_all(&dep_profile_dir).expect("create dep profile dir");
    fs::write(
        module_root.join("oh-package.json5"),
        r#"{
          name: "entry",
          dependencies: { "@cashier_alipay/cashiersdk": "15.8.26" }
        }"#,
    )
    .expect("write entry package");
    fs::write(
        dep_root.join("oh-package.json5"),
        r#"{
          name: "@cashier_alipay/cashiersdk",
          version: "15.8.26",
          main: "Index.ets"
        }"#,
    )
    .expect("write dep package");
    fs::write(
        dep_root.join("src/main/module.json"),
        r#"{
          "module": {
            "name": "cashiersdk",
            "type": "har",
            "routerMap": "$profile:route_map",
            "packageName": "@cashier_alipay/cashiersdk"
          }
        }"#,
    )
    .expect("write dep module");
    fs::write(
        dep_profile_dir.join("route_map.json"),
        r#"{"routerMap":[{"name":"alipay/cashier/H5Page","pageSourceFile":"src/main/ets/components/h5page/AlipayH5Page.js","buildFunction":"AlipayH5PageBuilder"}]}"#,
    )
    .expect("write dep router map");

    let mut config = make_config(true);
    config.project_root = project_root;
    let module_cfg = ModuleConfig {
        name: "entry".to_string(),
        src_path: module_root,
        module_type: OhosModuleType::Entry,
    };

    let loader = LoaderJson::generate(&config, &module_cfg, &make_manifest());

    assert_eq!(loader.router_map.len(), 1);
    assert_eq!(
        loader.router_map[0]["ohmurl"].as_str(),
        Some(
            "@normalized:N&&&@cashier_alipay/cashiersdk/src/main/ets/components/h5page/AlipayH5Page&15.8.26"
        )
    );
    assert_eq!(
        loader.router_map[0]["name"].as_str(),
        Some("alipay/cashier/H5Page")
    );
    assert!(
        loader
            .hvigor_compile_entry
            .iter()
            .any(|entry| entry.ends_with(
                "oh_modules/@cashier_alipay/cashiersdk/src/main/ets/components/h5page/AlipayH5Page.js"
            )),
        "dependency router page must be included in compileEntry: {:?}",
        loader.hvigor_compile_entry
    );
}

#[test]
fn test_loader_json_collects_workers_from_ohpm_source_har_metadata() {
    let tmp = tempfile::tempdir().expect("create temp dir");
    let project_root = tmp.path().to_path_buf();
    let module_root = project_root.join("entry");
    let dep_root = project_root.join("oh_modules").join("worker-har");

    fs::create_dir_all(&module_root).expect("create module root");
    fs::create_dir_all(dep_root.join("src/main/ets/workers")).expect("create worker dir");
    fs::write(
        module_root.join("oh-package.json5"),
        r#"{ name: "entry", dependencies: { "worker-har": "1.0.0" } }"#,
    )
    .expect("write entry package");
    fs::write(
        dep_root.join("oh-package.json5"),
        r#"{
          name: "worker-har",
          version: "1.0.0",
          metadata: { workers: ["./src/main/ets/workers/MetaWorker.ets"] }
        }"#,
    )
    .expect("write dep package");
    fs::write(
        dep_root.join("build-profile.json5"),
        r#"{
          buildOption: {
            sourceOption: { workers: ["./src/main/ets/workers/ProfileWorker.ets"] }
          }
        }"#,
    )
    .expect("write dep build profile");
    fs::write(
        dep_root.join("src/main/module.json"),
        r#"{
          "module": {
            "name": "workerhar",
            "type": "har",
            "extensionAbilities": [{
              "name": "SchedulerWorker",
              "srcEntry": "./src/main/ets/workers/SchedulerWorker.ets",
              "type": "workScheduler"
            }]
          }
        }"#,
    )
    .expect("write dep module");
    for file in ["MetaWorker.ets", "ProfileWorker.ets", "SchedulerWorker.ets"] {
        fs::write(
            dep_root.join("src/main/ets/workers").join(file),
            "export default 1;",
        )
        .expect("write worker");
    }

    let mut config = make_config(true);
    config.project_root = project_root;
    let module_cfg = ModuleConfig {
        name: "entry".to_string(),
        src_path: module_root,
        module_type: OhosModuleType::Entry,
    };

    let loader = LoaderJson::generate(&config, &module_cfg, &make_manifest());

    for worker in ["MetaWorker.ets", "ProfileWorker.ets", "SchedulerWorker.ets"] {
        assert!(
            loader
                .workers
                .iter()
                .any(|entry| entry.ends_with(&format!("src/main/ets/workers/{worker}"))),
            "missing worker {worker}: {:?}",
            loader.workers
        );
    }
    for metadata_only_worker in ["MetaWorker.ets", "ProfileWorker.ets", "SchedulerWorker.ets"] {
        assert!(
            !loader
                .hvigor_compile_entry
                .iter()
                .any(|entry| entry.ends_with(metadata_only_worker)),
            "HAP compileEntry must not include dependency metadata worker {metadata_only_worker}: {:?}",
            loader.hvigor_compile_entry
        );
    }
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
            exclude_packages: vec![],
        }),
        build_profile_fields: serde_json::Map::new(),
        support_chunks: false,
        byte_code_har: Some(false),
        bundled_dependencies: false,
        bundled_declare: false,
        integrated_hsp: None,
        widget: None,
        branch_elimination: None,
        transform_lib: None,
        skip_oh_modules_lint: None,
        re_export_check_mode: None,
        auto_lazy_import: None,
        auto_lazy_filter: None,
        expand_import_path: None,
        tsc_config: Default::default(),
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
            exclude_packages: vec![],
        }),
        build_profile_fields: serde_json::Map::new(),
        support_chunks: false,
        byte_code_har: Some(false),
        bundled_dependencies: false,
        bundled_declare: false,
        integrated_hsp: None,
        widget: None,
        branch_elimination: None,
        transform_lib: None,
        skip_oh_modules_lint: None,
        re_export_check_mode: None,
        auto_lazy_import: None,
        auto_lazy_filter: None,
        expand_import_path: None,
        tsc_config: Default::default(),
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
fn runtime_only_exclude_packages_skips_only_remote_source_har_code() {
    let tmp = tempfile::tempdir().expect("create temp dir");
    let project_root = tmp.path().to_path_buf();
    let module_root = project_root.join("entry");
    std::fs::create_dir_all(&module_root).expect("create module dir");
    std::fs::write(
        module_root.join("oh-package.json5"),
        r#"{
          name: "entry",
          dependencies: {
            "remote-har": "1.0.0",
            "local-har": "file:../local-har"
          }
        }"#,
    )
    .expect("write host package");

    let create_har = |name: &str| {
        let root = project_root.join("oh_modules").join(name);
        let source = root.join("src/main/ets/Index.ets");
        std::fs::create_dir_all(source.parent().unwrap()).expect("create HAR source");
        std::fs::write(
            root.join("oh-package.json5"),
            format!(r#"{{ name: "{name}", main: "src/main/ets/Index.ets" }}"#),
        )
        .expect("write HAR package");
        std::fs::write(
            root.join("src/main/module.json5"),
            r#"{ module: { name: "library", type: "har" } }"#,
        )
        .expect("write HAR profile");
        std::fs::write(
            &source,
            format!("export class {} {{}}", name.replace('-', "_")),
        )
        .expect("write HAR entry");
        source
    };
    let remote_source = create_har("remote-har");
    let local_source = create_har("local-har");

    let mut config = make_config(false);
    config.project_root = project_root;
    config.product.build_option.ark_options = Some(ArkOptions {
        obfuscation: None,
        types: vec![],
        runtime_only: Some(RuntimeOnlyConfig {
            packages: vec!["remote-har".into(), "local-har".into()],
            sources: vec![],
            exclude_packages: vec!["remote-har".into(), "local-har".into()],
        }),
        build_profile_fields: serde_json::Map::new(),
        support_chunks: false,
        byte_code_har: Some(false),
        bundled_dependencies: false,
        bundled_declare: false,
        integrated_hsp: None,
        widget: None,
        branch_elimination: None,
        transform_lib: None,
        skip_oh_modules_lint: None,
        re_export_check_mode: None,
        auto_lazy_import: None,
        auto_lazy_filter: None,
        expand_import_path: None,
        tsc_config: Default::default(),
    });
    let module_cfg = ModuleConfig {
        name: "entry".to_string(),
        src_path: module_root,
        module_type: OhosModuleType::Entry,
    };

    let loader = LoaderJson::generate(&config, &module_cfg, &make_manifest());

    assert!(loader.dynamic_import_lib_info.contains_key("remote-har"));
    assert!(loader.dynamic_import_lib_info.contains_key("local-har"));
    assert!(
        !loader
            .hvigor_compile_entry
            .contains(&to_absolute_path_string(&remote_source)),
        "remote source HAR code must be excluded"
    );
    assert!(
        loader
            .hvigor_compile_entry
            .contains(&to_absolute_path_string(&local_source)),
        "local file: HAR code must remain a compile input"
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

#[test]
fn runtime_only_sources_ignore_dynamic_and_dev_dependency_metadata() {
    let tmp = tempfile::tempdir().expect("create temp dir");
    let project_root = tmp.path().to_path_buf();
    let module_root = project_root.join("entry");
    std::fs::create_dir_all(&module_root).expect("create module dir");
    std::fs::write(
        module_root.join("oh-package.json5"),
        r#"{
          name: "entry",
          dynamicDependencies: { "dynamic-source": "1.0.0" },
          devDependencies: { "dev-source": "1.0.0" }
        }"#,
    )
    .expect("write entry oh-package");

    let make_runtime_source = |package_name: &str| {
        let package_root = project_root.join("oh_modules").join(package_name);
        let source = package_root.join("runtime/ShouldNotCompile.ets");
        std::fs::create_dir_all(source.parent().expect("runtime source parent"))
            .expect("create runtime source directory");
        std::fs::write(
            package_root.join("oh-package.json5"),
            format!(
                r#"{{
                  name: "{package_name}",
                  metadata: {{ runtimeOnly: {{ sources: ["runtime"] }} }}
                }}"#
            ),
        )
        .expect("write dependency package");
        std::fs::write(&source, "export const ignored = true;").expect("write runtime source");
        source
    };
    let dynamic_source = make_runtime_source("dynamic-source");
    let dev_source = make_runtime_source("dev-source");

    let mut config = make_config(false);
    config.project_root = project_root;
    let module_cfg = ModuleConfig {
        name: "entry".to_string(),
        src_path: module_root,
        module_type: OhosModuleType::Entry,
    };

    let loader = LoaderJson::generate(&config, &module_cfg, &make_manifest());

    assert!(!loader
        .hvigor_compile_entry
        .contains(&to_absolute_path_string(&dynamic_source)));
    assert!(!loader
        .hvigor_compile_entry
        .contains(&to_absolute_path_string(&dev_source)));
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
        r#"{
          name: "@ohos/abc-lib",
          compatibleSdkVersion: "5.0.0(12)",
          metadata: { byteCodeHar: true }
        }"#,
    )
    .expect("write oh-package.json5");
    std::fs::write(
        project_root.join("oh-package.json5"),
        r#"{ name: "app", dependencies: { "@ohos/abc-lib": "1.0.0" } }"#,
    )
    .expect("write project package");

    // `GenerateLoaderJson.setByteCodeHarInfoAndOtherCompileFiles()` gates the
    // whole ByteCode HAR collection on `useNormalizedOHMUrl` (and HAP/HSP
    // targets); `byte-code-har-utils.checkByteCodeHar` hard-errors when it is
    // off. This test therefore uses `make_config(true)`.
    let mut config = make_config(true);
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
    assert_eq!(
        entry.compatible_sdk_version,
        Some(crate::bytecode_har::CompatibleSdkVersionValue::Str(
            "5.0.0(12)".to_string()
        ))
    );
    assert!(entry.abc_path.ends_with("index.abc"));
    assert!(entry.source_maps_path.is_none());
}
/// Verify that a non-default effective target's `src/<target>/ets/` directory
/// appears in `pkgNameToPkgBriefInfo.sourceRoots` when the directory exists.
///
/// Addresses section 3.1 gap:
///   "sourceRoots/originalSourceRoots 严格语义和字段顺序仍不完整"
#[test]
fn test_loader_json_target_ets_src_added_to_source_roots() {
    let tmp = tempfile::tempdir().expect("create temp dir");
    let project_root = tmp.path().to_path_buf();
    let module_root = project_root.join("entry");

    // Create the default source directory.
    std::fs::create_dir_all(module_root.join("src/main/ets")).expect("create src/main/ets");

    // Create a target-specific ETS directory.
    let target_ets = module_root.join("src/phone/ets");
    std::fs::create_dir_all(&target_ets).expect("create src/phone/ets");

    // Create a build-profile.json5 with a non-default "phone" target.
    std::fs::write(
        project_root.join("build-profile.json5"),
        r#"{
          app: { products: [{ name: "default" }] },
          modules: [{
            name: "entry",
            srcPath: "./entry",
            targets: [{
              name: "phone",
              applyToProducts: ["default"],
              buildModeSet: [{ name: "debug" }]
            }]
          }]
        }"#,
    )
    .expect("write build-profile");

    let mut config = make_config(false);
    config.project_root = project_root.clone();
    let module_cfg = ModuleConfig {
        name: "entry".to_string(),
        src_path: module_root.clone(),
        module_type: OhosModuleType::Entry,
    };
    let manifest = make_manifest();

    let loader = LoaderJson::generate(&config, &module_cfg, &manifest);

    let pkg_info = loader
        .pkg_name_to_pkg_brief_info
        .get("entry")
        .expect("entry must be in pkgBriefInfo");

    // sourceRoots should include the target-specific path BEFORE src/main/ets.
    assert!(
        pkg_info.source_roots.len() >= 2,
        "sourceRoots must include target-specific dir; got: {:?}",
        pkg_info.source_roots
    );
    let has_target_ets = pkg_info
        .source_roots
        .iter()
        .any(|r| r.contains("src/phone/ets"));
    assert!(
        has_target_ets,
        "sourceRoots must contain src/phone/ets; got: {:?}",
        pkg_info.source_roots
    );

    // originalSourceRoots should still be src/main/ets only.
    assert_eq!(
        pkg_info.original_source_roots.len(),
        1,
        "originalSourceRoots must be [src/main/ets]; got: {:?}",
        pkg_info.original_source_roots
    );
    assert!(
        pkg_info.original_source_roots[0].contains("src/main/ets"),
        "originalSourceRoots[0] must be src/main/ets; got: {}",
        pkg_info.original_source_roots[0]
    );
}

/// Verify that `src/ohosTest/ets/` sources are NOT added to the local
/// unit-test compileEntry (`OutputTarget::Test` = `hvigorw test`). The
/// `test` task compiles the generated `.test/testability` templates plus
/// `src/test`; `src/ohosTest` participates in the separate `ohosTest`
/// target build (`-p module=entry@ohosTest`).
#[test]
fn test_loader_json_routes_ohos_test_sources_only_to_ohos_test_target() {
    let tmp = tempfile::tempdir().expect("create temp dir");
    let project_root = tmp.path().to_path_buf();
    let module_root = project_root.join("entry");

    // Create src/main/ets/Index.ets (main source)
    fs::create_dir_all(module_root.join("src/main/ets")).expect("create main ets");
    fs::write(module_root.join("src/main/ets/Index.ets"), "export {}").expect("write main");

    // Create src/ohosTest/ets/ with test sources
    let test_ets = module_root.join("src/ohosTest/ets");
    fs::create_dir_all(&test_ets).expect("create test ets");
    fs::write(
        test_ets.join("TestRunner.ets"),
        "export class TestRunner {}",
    )
    .expect("write test runner");
    fs::write(test_ets.join("index.ets"), "export * from './TestRunner'")
        .expect("write test index");
    // A .d.ts file should NOT be included
    fs::write(test_ets.join("TestTypes.d.ts"), "export type T = {}").expect("write types");

    // Local unit-test mode must NOT include src/ohosTest sources (hvigor
    // `test` task compiler inputs; the generated `.test/testability`
    // templates and `src/test` suite replace them).
    let mut test_config = make_config(false);
    test_config.project_root = project_root.clone();
    test_config.output_target = arkdown_core::OutputTarget::Test;
    let mut module_cfg = make_module_cfg();
    module_cfg.src_path = module_root.clone();

    let loader = LoaderJson::generate(&test_config, &module_cfg, &make_manifest());

    let has_test_runner = loader
        .hvigor_compile_entry
        .iter()
        .any(|e| e.contains("TestRunner"));
    assert!(
        !has_test_runner,
        "src/ohosTest sources must NOT be in the local-test compileEntry; entries: {:?}",
        loader.hvigor_compile_entry
    );

    let mut ohos_test_config = make_config(false);
    ohos_test_config.project_root = project_root.clone();
    let ohos_test_loader =
        LoaderJson::try_generate(&ohos_test_config, &module_cfg, &make_manifest(), "ohosTest")
            .expect("generate ohosTest loader");
    assert!(ohos_test_loader
        .hvigor_compile_entry
        .contains(&to_absolute_path_string(&test_ets.join("TestRunner.ets"))));
    assert!(!ohos_test_loader
        .hvigor_compile_entry
        .contains(&to_absolute_path_string(&test_ets.join("TestTypes.d.ts"))));

    // Debug mode should NOT include ohosTest sources
    let debug_config = make_config(false);
    let mut debug_module_cfg = make_module_cfg();
    debug_module_cfg.src_path = module_root;

    let debug_loader = LoaderJson::generate(&debug_config, &debug_module_cfg, &make_manifest());
    let has_ohos_test = debug_loader
        .hvigor_compile_entry
        .iter()
        .any(|e| e.contains("ohosTest"));
    assert!(
        !has_ohos_test,
        "ohosTest sources must not be included in debug compileEntry; entries: {:?}",
        debug_loader.hvigor_compile_entry
    );
}

#[test]
fn ohos_test_loader_disables_chunks_like_ohos_test_ark_compile() {
    let tmp = tempfile::tempdir().expect("create module root");
    let module_root = tmp.path().join("entry");
    fs::create_dir_all(module_root.join("src/main/ets")).expect("create main source root");

    let mut config = make_config(false);
    config.project_root = tmp.path().to_path_buf();
    config.product.build_option.ark_options = Some(
        serde_json::from_value(serde_json::json!({
            "obfuscation": null,
            "types": [],
            "runtime_only": null,
            "support_chunks": true
        }))
        .expect("decode ArkOptions"),
    );
    let module = ModuleConfig {
        name: "entry".to_string(),
        src_path: module_root,
        module_type: OhosModuleType::Entry,
    };

    let default_loader = LoaderJson::try_generate(&config, &module, &make_manifest(), "default")
        .expect("generate default loader");
    let ohos_test_loader = LoaderJson::try_generate(&config, &module, &make_manifest(), "ohosTest")
        .expect("generate ohosTest loader");

    assert!(default_loader.support_chunks);
    assert!(!ohos_test_loader.support_chunks);
}

/// 6.24.3 `pkgName2OtherCompileFiles` is a widget-loader-only field: the
/// common `loader.json` never carries it, the widget loader always does
/// (`Some(empty)` object when `widget.transitiveDeps` is off).
#[test]
fn test_loader_json_pkg_name2_other_compile_files_widget_only() {
    let tmp = tempfile::tempdir().expect("create temp dir");
    let project_root = tmp.path().to_path_buf();
    let module_root = project_root.join("entry");
    std::fs::create_dir_all(module_root.join("src/main/ets")).expect("create src/main/ets");

    let mut config = make_config(true);
    config.project_root = project_root.clone();
    let module_cfg = ModuleConfig {
        name: "entry".to_string(),
        src_path: module_root,
        module_type: OhosModuleType::Entry,
    };

    let loader = LoaderJson::generate(&config, &module_cfg, &make_manifest());
    let value = serde_json::to_value(&loader).expect("serialize common loader");
    assert!(
        value.get("pkgName2OtherCompileFiles").is_none(),
        "the common loader.json must not carry pkgName2OtherCompileFiles"
    );

    // Widget loader with transitiveDeps off: field present and empty.
    let mut widget_loader = loader.clone();
    widget_loader.pkg_name2_other_compile_files = Some(IndexMap::new());
    let value = serde_json::to_value(&widget_loader).expect("serialize widget loader");
    assert_eq!(
        value.get("pkgName2OtherCompileFiles"),
        Some(&serde_json::json!({})),
        "widget loader without transitiveDeps carries an empty object"
    );

    // Widget loader with grouped blocked files.
    let mut widget_loader = loader.clone();
    widget_loader.pkg_name2_other_compile_files = Some(IndexMap::from([(
        "bc-har-a".to_string(),
        vec!["/abs/path/blocked.ets".to_string()],
    )]));
    let value = serde_json::to_value(&widget_loader).expect("serialize widget loader");
    assert_eq!(
        value.get("pkgName2OtherCompileFiles"),
        Some(&serde_json::json!({ "bc-har-a": ["/abs/path/blocked.ets"] })),
        "widget loader groups blocked files per topmost BC HAR"
    );
}
