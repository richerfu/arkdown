use super::module_source_state::{HvigorCollectedModuleSource, HvigorResolvedImportRecord};
use super::*;
use crate::chunk_module_request::HvigorChunkImportMap;
use crate::plan::{BundlePlan, BundleRuntimePackageContext, BundleRuntimePackageInfo};
use crate::{
    BundleCachePolicy, BundleInput, BundleOutputOptions, BundlePluginSet, LoaderResourceJoin,
    SourceMapPolicy,
};
use arkdown_core::BuildMode;
use rolldown::{BundlerOptions, InputItem, OutputFormat};
use rolldown_common::{BundlerTransformOptions, TreeshakeOptions, TypeScriptOptions};
use rustc_hash::FxHashMap;
use std::path::PathBuf;
use std::sync::Arc;

fn runtime_packages(contexts: &[(&str, &str, &str, &str, &str, bool)]) -> BundleRuntimePackageInfo {
    let mut packages = FxHashMap::default();
    for (key, package_name, bundle_name, module_name, version, is_so) in contexts {
        packages.insert(
            (*key).to_string(),
            BundleRuntimePackageContext {
                package_name: (*package_name).to_string(),
                bundle_name: (*bundle_name).to_string(),
                module_name: (*module_name).to_string(),
                version: (*version).to_string(),
                entry_path: if *key == "phone" || *key == "audioplayer" {
                    "src/main/".to_string()
                } else {
                    "index".to_string()
                },
                is_so: *is_so,
            },
        );
    }
    BundleRuntimePackageInfo { packages }
}

#[test]
fn insight_intent_facts_receive_normalized_decorator_and_page_urls() {
    let temp = tempfile::tempdir().expect("tempdir");
    let module_root = temp.path().join("entry");
    let source = module_root.join("src/main/ets/pages/Index.ets");
    std::fs::create_dir_all(source.parent().unwrap()).expect("source parent");
    std::fs::write(&source, "struct Index {}").expect("source");
    std::fs::write(
        module_root.join("oh-package.json5"),
        r#"{"name":"entry","version":"1.0.0"}"#,
    )
    .expect("package");
    let module_path_map = HvigorModulePathMap::new(
        "entry",
        &module_root,
        &[("entry".to_string(), module_root.clone())],
    );
    let mut packages = FxHashMap::default();
    packages.insert(
        "entry".to_string(),
        BundleRuntimePackageContext {
            package_name: "entry".to_string(),
            bundle_name: "com.example.intent".to_string(),
            module_name: "entry".to_string(),
            version: "1.0.0".to_string(),
            entry_path: "src/main/ets/pages/Index.ets".to_string(),
            is_so: false,
        },
    );
    let mut data = serde_json::Map::new();
    data.insert(
        "decoratorType".to_string(),
        serde_json::Value::String("@InsightIntentPage".to_string()),
    );
    data.insert(
        "pagePath".to_string(),
        serde_json::Value::String("ets/pages/Index".to_string()),
    );
    let mut facts = FxHashMap::default();
    facts.insert(
        source.to_string_lossy().into_owned(),
        arkdown_ets_transform::InsightIntentSourceMetadata {
            intents: vec![arkdown_ets_transform::InsightIntentDeclaration {
                data,
                entity_type_names: vec![],
            }],
            entities: vec![],
            obfuscation_global_names: vec![],
            obfuscation_property_names: vec![],
        },
    );

    let output = Bundler::insight_intent_sources(
        facts,
        &module_path_map,
        &BundleRuntimePackageInfo { packages },
        "entry",
        true,
    )
    .expect("normalize InsightIntent facts");

    assert_eq!(output.len(), 1);
    let expected = "@normalized:N&entry&com.example.intent&entry/src/main/ets/pages/Index&1.0.0";
    assert_eq!(output[0].decorator_file, expected);
    assert_eq!(
        output[0].metadata.intents[0].data["decoratorFile"],
        expected
    );
    assert_eq!(output[0].metadata.intents[0].data["pagePath"], expected);
}

/// Test-only replacement for the removed
/// `HvigorSourceSpan::for_module_request_literal` helper: locate a module
/// request literal's span through the shared ArkTS inventory.
fn source_span_for_module_request_literal(
    source: &str,
    module_id: &str,
    specifier: &str,
) -> Option<HvigorSourceSpan> {
    HvigorModuleRequestSpecifiers::new(source, module_id)
        .collect_literals()
        .into_iter()
        .find(|literal| literal.specifier == specifier)
        .map(|literal| literal.source_span)
}

fn record_resolved_import(
    state: &mut HvigorModuleSourceState,
    importer: &str,
    specifier: &str,
    resolved_id: &str,
) {
    state
        .resolved_imports
        .entry(importer.to_string())
        .or_default()
        .insert(specifier.to_string(), resolved_id.to_string());
    if let Some(source) = state.sources.get(importer) {
        if source_span_for_module_request_literal(source.code.as_ref(), importer, specifier)
            .is_some()
        {
            state
                .resolved_import_records
                .entry(importer.to_string())
                .or_default()
                .push(HvigorResolvedImportRecord {
                    specifier: specifier.to_string(),
                });
        }
    }
}

#[test]
fn non_normalized_source_record_matches_hvigor_get_ohm_url_by_filepath() {
    let project = tempfile::tempdir().unwrap();
    let module_root = project.path().join("entry");
    let source_path = module_root.join("src/main/ets/pages/Index.ets");
    std::fs::create_dir_all(source_path.parent().unwrap()).unwrap();
    std::fs::write(&source_path, "").unwrap();

    let module_root = arkdown_path::canonicalize_or_owned(&module_root);
    let source_path = arkdown_path::canonicalize_or_owned(&source_path);
    let module_path_map = HvigorModulePathMap::new("entry", &module_root, &[]);
    let source = module_path_map
        .source_file_for_entry(source_path.to_str().unwrap())
        .unwrap();
    let record = module_path_map
        .non_normalized_record_name(
            project.path(),
            source_path.to_str().unwrap(),
            "com.example.arkdown.test",
            "entry",
            &source,
        )
        .unwrap();

    assert_eq!(record, "com.example.arkdown.test/entry/ets/pages/Index");
}

#[test]
fn module_meta_info_ohm_url_survives_compile_entry_root_enrichment() {
    let project = tempfile::tempdir().unwrap();
    let module_root = project.path().join("entry");
    let resource_table = module_root.join("build/default/generated/r/default/ResourceTable.ts");
    std::fs::create_dir_all(resource_table.parent().unwrap()).unwrap();
    std::fs::write(&resource_table, "//@ts-noCheck\n").unwrap();

    let mut module_path_map = HvigorModulePathMap::new(
        "entry",
        &module_root,
        &[("entry".to_string(), module_root.clone())],
    );
    module_path_map.add_module_meta_info_files(&[crate::plan::BundleModuleMetaInfoFile {
        file_path: resource_table.clone(),
        ohm_url: "com.example/entry/build/generated/r/ResourceTable".to_string(),
    }]);
    module_path_map
        .add_compile_entry_package_roots(&[resource_table.to_string_lossy().into_owned()]);

    assert_eq!(
        module_path_map.module_meta_info_ohm_url(resource_table.to_str().unwrap()),
        Some("com.example/entry/build/generated/r/ResourceTable")
    );
}

#[test]
fn module_path_map_keeps_package_version_for_src_main_sources() {
    let project = tempfile::tempdir().unwrap();
    let module_root = project.path().join("entry");
    let src_main = module_root.join("src/main");
    let source_path = src_main.join("ets/pages/Index.ets");
    std::fs::create_dir_all(source_path.parent().unwrap()).unwrap();
    std::fs::write(
        module_root.join("oh-package.json5"),
        r#"{"name":"@example/entry","version":"12.61.200"}"#,
    )
    .unwrap();
    std::fs::write(&source_path, "").unwrap();

    let module_root = arkdown_path::canonicalize_or_owned(&module_root);
    let src_main = arkdown_path::canonicalize_or_owned(&src_main);
    let source_path = arkdown_path::canonicalize_or_owned(&source_path);
    let module_path_map =
        HvigorModulePathMap::new("entry", &src_main, &[("entry".to_string(), module_root)]);
    let source = module_path_map
        .source_file_for_entry(source_path.to_str().unwrap())
        .unwrap();

    assert_eq!(source.module_name, "@example/entry");
    assert_eq!(source.package_version, "12.61.200");
    assert_eq!(source.source_file, "ets/pages/Index.ets");
}

#[test]
fn normalized_source_record_and_dependency_attribution_match_latest_module_meta_plugin() {
    let project = tempfile::tempdir().unwrap();
    let module_root = project.path().join("entry");
    let entry_source = module_root.join("src/main/ets/Index.ets");
    let bundled_har_root = module_root.join("oh_modules/share-config-har");
    let bundled_har_source = bundled_har_root.join("Index.ets");
    let external_root = project.path().join("oh_modules/external-source-har");
    let external_source = external_root.join("Index.ets");
    std::fs::create_dir_all(entry_source.parent().unwrap()).unwrap();
    std::fs::create_dir_all(&bundled_har_root).unwrap();
    std::fs::create_dir_all(&external_root).unwrap();
    std::fs::write(
        module_root.join("oh-package.json5"),
        r#"{"name":"entry","version":"2.0.0"}"#,
    )
    .unwrap();
    std::fs::write(
        bundled_har_root.join("oh-package.json5"),
        r#"{"name":"share-config-har","version":"9.0.0","main":"Index.ets"}"#,
    )
    .unwrap();
    std::fs::write(
        external_root.join("oh-package.json5"),
        r#"{"name":"external-source-har","version":"8.0.0","main":"Index.ets"}"#,
    )
    .unwrap();
    std::fs::write(&entry_source, "export const entry = true;").unwrap();
    std::fs::write(&bundled_har_source, "export const bundled = true;").unwrap();
    std::fs::write(&external_source, "export const external = true;").unwrap();

    let module_path_map = HvigorModulePathMap::new(
        "entry",
        &module_root,
        &[
            ("entry".to_string(), module_root.clone()),
            ("share-config-har".to_string(), bundled_har_root),
            ("external-source-har".to_string(), external_root),
        ],
    );
    let entry = module_path_map
        .source_file_for_entry(entry_source.to_str().unwrap())
        .unwrap();
    let mut packages = FxHashMap::default();
    packages.insert(
        "entry".to_string(),
        BundleRuntimePackageContext {
            package_name: "entry".to_string(),
            bundle_name: String::new(),
            module_name: "entry".to_string(),
            version: "2.0.0".to_string(),
            entry_path: "src/main/ets/Index.ets".to_string(),
            is_so: false,
        },
    );
    assert_eq!(
        entry
            .normalized_record_name(&BundleRuntimePackageInfo { packages })
            .unwrap(),
        "&entry/src/main/ets/Index&2.0.0"
    );

    let bundled = module_path_map
        .source_file_for_entry(bundled_har_source.to_str().unwrap())
        .unwrap();
    assert!(!module_path_map.has_dependency_package_info(bundled_har_source.to_str().unwrap()));
    assert_eq!(
        bundled.file_info_source_file("entry", "2.0.0", false, false),
        "entry|entry|2.0.0|Index.ts"
    );
    assert_eq!(bundled.module_name, "share-config-har");

    let external = module_path_map
        .source_file_for_entry(external_source.to_str().unwrap())
        .unwrap();
    assert!(module_path_map.has_dependency_package_info(external_source.to_str().unwrap()));
    assert_eq!(
        external.file_info_source_file("entry", "2.0.0", true, false),
        "entry|external-source-har|8.0.0|Index.ts"
    );
}

#[test]
fn preload_system_so_tasks_follow_reachable_module_graph_like_sdk() {
    let temp = tempfile::tempdir().unwrap();
    let entry = temp.path().join("EntryAbility.ets");
    let dependency = temp.path().join("Dependency.ets");
    std::fs::write(&entry, "").unwrap();
    std::fs::write(&dependency, "").unwrap();
    let entry_id = arkdown_path::canonicalize_or_owned(&entry)
        .to_string_lossy()
        .into_owned();
    let dependency_id = arkdown_path::canonicalize_or_owned(&dependency)
        .to_string_lossy()
        .into_owned();
    let mut state = HvigorModuleSourceState::default();
    state.module_ids = vec![entry_id.clone(), dependency_id.clone()];
    state.sources.insert(
        entry_id.clone(),
        HvigorCollectedModuleSource {
            code: format!(
                "import './Dependency'; import hilog from '@ohos.hilog'; export {{ hilog }};"
            )
            .into(),
        },
    );
    state.sources.insert(
        dependency_id.clone(),
        HvigorCollectedModuleSource {
            code: "import crypto from 'libcrypto.so'; export { crypto };".into(),
        },
    );
    record_resolved_import(&mut state, &entry_id, "./Dependency", &dependency_id);
    record_resolved_import(&mut state, &entry_id, "@ohos.hilog", "@ohos.hilog");
    record_resolved_import(&mut state, &dependency_id, "libcrypto.so", "libcrypto.so");

    let tasks = state.preload_system_so_tasks(
        &[entry],
        &HvigorChunkModuleRequestProcessor::new("com.example.app", "entry"),
    );
    assert_eq!(tasks.len(), 2, "{tasks:?}");
    assert!(tasks
        .iter()
        .any(|task| { task.name == "@ohos.hilog" && task.ohm_url == "@ohos:hilog" }));
    assert!(tasks.iter().any(|task| {
        task.name == "libcrypto.so" && task.ohm_url == "@app:com.example.app/entry/crypto"
    }));
}

#[test]
fn bytecode_har_imports_follow_hvigor_normalized_alias_rules() {
    let mut packages = FxHashMap::default();
    packages.insert(
        "@tencent/qq-open-sdk".to_string(),
        BundleRuntimePackageContext {
            package_name: "@tencent/qq-open-sdk".to_string(),
            bundle_name: String::new(),
            module_name: String::new(),
            version: "1.0.4".to_string(),
            entry_path: "Index.d.ets".to_string(),
            is_so: false,
        },
    );
    let runtime_packages = BundleRuntimePackageInfo { packages };
    let package_names = vec!["@tencent/qq-open-sdk".to_string()];
    let imports = HvigorBytecodeHarImports::new(&runtime_packages, &package_names, true);

    assert_eq!(
        imports
            .ohm_url_for_request("@tencent/qq-open-sdk")
            .unwrap()
            .as_deref(),
        Some("@normalized:N&&&@tencent/qq-open-sdk/Index&1.0.4")
    );
    assert_eq!(
        imports
            .ohm_url_for_request("@tencent/qq-open-sdk/src/main/ets/QQOpenApi")
            .unwrap()
            .as_deref(),
        Some("@normalized:N&&&@tencent/qq-open-sdk/src/main/ets/QQOpenApi&1.0.4")
    );
}

#[test]
fn normalized_native_so_imports_follow_hvigor_pkg_context_info() {
    let mut packages = FxHashMap::default();
    packages.insert(
        "liblx.so".to_string(),
        BundleRuntimePackageContext {
            package_name: "liblx.so".to_string(),
            is_so: true,
            ..BundleRuntimePackageContext::default()
        },
    );
    let runtime_packages = BundleRuntimePackageInfo { packages };
    let imports = HvigorBytecodeHarImports::new(&runtime_packages, &[], true);

    assert_eq!(
        imports.ohm_url_for_request("liblx.so").unwrap().as_deref(),
        Some("@normalized:Y&&&liblx.so&")
    );
}

#[test]
fn module_source_ids_use_stable_graph_order_and_sorted_collector_fallbacks() {
    let entry_id = "/project/entry/src/main/ets/EntryAbility.ets".to_string();
    let profile_id = "/project/entry/build/generated/profile/ModuleInfo.ets".to_string();
    let dependency_id = "/project/oh_modules/@scope/pkg/Index.ets".to_string();
    let collected_id = "/project/oh_modules/@scope/z-extra/Index.ets".to_string();
    let earlier_collected_id = "/project/oh_modules/@scope/a-extra/Index.ets".to_string();
    let facts = ModuleGraphFacts {
        full_module_ids: vec![dependency_id.clone(), profile_id.clone(), entry_id.clone()],
        full_module_is_commonjs: FxHashMap::default(),
    };
    let mut state = HvigorModuleSourceState::default();
    state.sources.insert(
        collected_id.clone(),
        HvigorCollectedModuleSource {
            code: "export const value = 1;".to_string().into(),
        },
    );
    state.sources.insert(
        earlier_collected_id.clone(),
        HvigorCollectedModuleSource {
            code: "export const value = 2;".to_string().into(),
        },
    );

    assert_eq!(
        Bundler::module_source_ids(&[entry_id.clone(), profile_id.clone()], &facts, &state),
        vec![
            entry_id,
            profile_id,
            dependency_id,
            earlier_collected_id,
            collected_id
        ]
    );
}

#[test]
fn chunk_source_files_follow_hvigor_module_path_map() {
    let app_root = PathBuf::from("/workspace/app");
    let module_root = app_root.join("products/phone/src/main");
    let plan = BundlePlan {
        input: BundleInput {
            loader_json_path: app_root.join("loader.json"),
            project_root: app_root.clone(),
            project_path: module_root.join("ets"),
            module_root: module_root.clone(),
            module_name: "phone".to_string(),
            module_path_map: vec![
                ("phone".to_string(), app_root.join("products/phone")),
                (
                    "audioplayer".to_string(),
                    app_root.join("commons/audioplayer"),
                ),
            ],
            module_record_roots: Vec::new(),
            module_meta_info_files: Vec::new(),
            entries: vec![
                "./ets/entryability/EntryAbility.ets".to_string(),
                app_root
                    .join("commons/audioplayer/src/main/ets/service/SpeechPlayerService.ets")
                    .to_string_lossy()
                    .into_owned(),
                app_root
                    .join("commons/audioplayer/Index.ets")
                    .to_string_lossy()
                    .into_owned(),
            ],
            form_entry_sources: Vec::new(),
            insight_intent_form_bindings: Vec::new(),
            runtime_entry_sources: vec![],
            page_entry_sources: vec![],
            support_chunks: false,
            source_map_packages: crate::plan::BundleSourceMapPackageInfo {
                entry_package_name: "phone".to_string(),
                entry_package_version: "1.0.0".to_string(),
                dependency_package_versions: FxHashMap::default(),
            },
            runtime_packages: runtime_packages(&[("phone", "phone", "", "", "", false)]),
            byte_code_har_packages: Vec::new(),
            preload_system_so_entries: Vec::new(),
            mock_config: None,
        },
        output: BundleOutputOptions::default(),
        source_map: SourceMapPolicy::External,
        cache: BundleCachePolicy::default(),
        loader_resource: LoaderResourceJoin {
            loader_json_path: app_root.join("loader.json"),
            resources_index_path: None,
            resource_table_path: None,
        },
        plugins: BundlePluginSet::default(),
        build_mode: BuildMode::Debug,
        arkts_language_version: arkdown_core::ArktsLanguageVersion::default(),
        target_api_version: 12,
        compatible_sdk_version: "12".to_string(),
        runtime_os: "OpenHarmony".to_string(),
        bundle_name: "com.example.hmosworld".to_string(),
        use_normalized_ohm_url: true,
        should_emit_js: true,
        module_analyzer: arkdown_ets_transform::ModuleAnalyzerKind::default(),
        output_target: arkdown_core::OutputTarget::Hap,
        preview_mode: false,
        check_entry: false,
        shim_missing_exports_override: None,
        treeshake_override: None,
        context_override: None,
    };

    let input_plan = RolldownInputPlan::from_bundle_plan(&plan);
    let source_files = input_plan.chunk_source_files().unwrap();

    assert_eq!(
        source_files
            .get("entryability/EntryAbility")
            .unwrap()
            .source_file,
        "ets/entryability/EntryAbility.ets"
    );
    assert_eq!(
        source_files
            .get("entryability/EntryAbility")
            .unwrap()
            .module_name,
        "phone"
    );
    assert_eq!(
        source_files.get("SpeechPlayerService").unwrap().source_file,
        "ets/service/SpeechPlayerService.ets"
    );
    assert_eq!(
        source_files.get("SpeechPlayerService").unwrap().module_name,
        "audioplayer"
    );
    assert_eq!(source_files.get("Index").unwrap().source_file, "Index.ets");
    assert_eq!(
        source_files.get("Index").unwrap().module_name,
        "audioplayer"
    );
}

#[test]
fn compile_entry_package_roots_do_not_require_loader_module_path_map_entries() {
    let app_root = tempfile::tempdir().unwrap();
    let project_root = app_root.path();
    let module_root = project_root.join("phone/src/main");
    let phone_root = project_root.join("phone");
    let package_root = project_root.join("oh_modules/.ohpm/mtlauncher@hash=/oh_modules/mtlauncher");
    fs_err::create_dir_all(&package_root).unwrap();
    fs_err::write(
        package_root.join("oh-package.json5"),
        r#"{
  "name": "mtlauncher",
  "version": "12.59.403",
  "main": "Index.ets"
}"#,
    )
    .unwrap();
    let package_entry = package_root.join("Index.ets");
    fs_err::write(&package_entry, "export const launcher = 1;").unwrap();
    let package_entry_id = package_entry.to_string_lossy().into_owned();
    let mut runtime_package_info = runtime_packages(&[("phone", "phone", "", "", "", false)]);
    runtime_package_info.packages.insert(
        "mtlauncher".to_string(),
        BundleRuntimePackageContext {
            package_name: "mtlauncher".to_string(),
            version: "12.59.403".to_string(),
            entry_path: "Index.ets".to_string(),
            ..BundleRuntimePackageContext::default()
        },
    );
    let plan = BundlePlan {
        input: BundleInput {
            loader_json_path: project_root.join("loader.json"),
            project_root: project_root.to_path_buf(),
            project_path: module_root.join("ets"),
            module_root: module_root.clone(),
            module_name: "phone".to_string(),
            module_path_map: vec![("phone".to_string(), phone_root)],
            module_record_roots: Vec::new(),
            module_meta_info_files: Vec::new(),
            entries: vec![package_entry_id.clone()],
            form_entry_sources: Vec::new(),
            insight_intent_form_bindings: Vec::new(),
            runtime_entry_sources: vec![],
            page_entry_sources: vec![],
            support_chunks: false,
            source_map_packages: crate::plan::BundleSourceMapPackageInfo {
                entry_package_name: "phone".to_string(),
                entry_package_version: "1.0.0".to_string(),
                dependency_package_versions: FxHashMap::default(),
            },
            runtime_packages: runtime_package_info,
            byte_code_har_packages: Vec::new(),
            preload_system_so_entries: Vec::new(),
            mock_config: None,
        },
        output: BundleOutputOptions::default(),
        source_map: SourceMapPolicy::External,
        cache: BundleCachePolicy::default(),
        loader_resource: LoaderResourceJoin {
            loader_json_path: project_root.join("loader.json"),
            resources_index_path: None,
            resource_table_path: None,
        },
        plugins: BundlePluginSet::default(),
        build_mode: BuildMode::Debug,
        arkts_language_version: arkdown_core::ArktsLanguageVersion::default(),
        target_api_version: 12,
        compatible_sdk_version: "12".to_string(),
        runtime_os: "OpenHarmony".to_string(),
        bundle_name: "com.example.hmosworld".to_string(),
        use_normalized_ohm_url: true,
        should_emit_js: true,
        module_analyzer: arkdown_ets_transform::ModuleAnalyzerKind::default(),
        output_target: arkdown_core::OutputTarget::Hap,
        preview_mode: false,
        check_entry: false,
        shim_missing_exports_override: None,
        treeshake_override: None,
        context_override: None,
    };

    let input_plan = RolldownInputPlan::from_bundle_plan(&plan);
    let source_files = input_plan.chunk_source_files().unwrap();
    assert_eq!(source_files.get("Index").unwrap().module_name, "mtlauncher");
    assert_eq!(source_files.get("Index").unwrap().source_file, "Index.ets");

    let mut state = HvigorModuleSourceState::default();
    state.sources.insert(
        package_entry_id.clone(),
        HvigorCollectedModuleSource {
            code: "export const launcher = 1;".to_string().into(),
        },
    );
    let facts = ModuleGraphFacts {
        full_module_ids: vec![package_entry_id.clone()],
        full_module_is_commonjs: FxHashMap::default(),
    };
    let files = HvigorModuleSourceWriter::new(
        "com.example.hmosworld",
        "phone",
        project_root,
        &input_plan.module_path_map,
        &plan.input.runtime_packages,
        &[],
        true,
        &facts,
        facts.full_module_ids.clone(),
        state,
        project_root.join("cache"),
    )
    .write()
    .unwrap();

    assert_eq!(files.len(), 1);
    assert_eq!(files[0].record_name, "&mtlauncher/Index&12.59.403");
    assert_eq!(files[0].source_file, "phone|mtlauncher|12.59.403|Index.ts");
}

#[test]
fn module_record_roots_attribute_dependency_sources_without_loader_module_path_map_entries() {
    let app_root = tempfile::tempdir().unwrap();
    let project_root = app_root.path();
    let phone_root = project_root.join("phone");
    let log_root = project_root.join("oh_modules/.ohpm/@example+log@hash=/oh_modules/@example/log");
    fs_err::create_dir_all(&log_root).unwrap();
    fs_err::write(
        log_root.join("oh-package.json5"),
        r#"{
  "name": "@example/log",
  "version": "0.0.18",
  "main": "Index.ets"
}"#,
    )
    .unwrap();
    let log_entry = log_root.join("Index.ets");
    fs_err::write(&log_entry, "export const log = 1;").unwrap();
    let log_entry_id = log_entry.to_string_lossy().into_owned();
    let mut module_path_map = HvigorModulePathMap::new(
        "phone",
        &phone_root,
        &[("phone".to_string(), phone_root.clone())],
    );
    module_path_map.add_module_record_roots(&[("@example/log".to_string(), log_root.clone())]);

    let ownership = module_path_map
        .source_file_for_entry(&log_entry_id)
        .expect("dependency source ownership");
    assert_eq!(ownership.module_name, "@example/log");
    assert_eq!(ownership.source_file, "Index.ets");

    let mut state = HvigorModuleSourceState::default();
    state.sources.insert(
        log_entry_id.clone(),
        HvigorCollectedModuleSource {
            code: "export const log = 1;".to_string().into(),
        },
    );
    let facts = ModuleGraphFacts {
        full_module_ids: vec![log_entry_id.clone()],
        full_module_is_commonjs: FxHashMap::default(),
    };
    let mut runtime_packages = runtime_packages(&[("phone", "phone", "", "", "", false)]);
    runtime_packages.packages.insert(
        "@example/log".to_string(),
        BundleRuntimePackageContext {
            package_name: "@example/log".to_string(),
            version: "0.0.18".to_string(),
            entry_path: "Index.ets".to_string(),
            ..BundleRuntimePackageContext::default()
        },
    );

    let files = HvigorModuleSourceWriter::new(
        "com.example.hmosworld",
        "phone",
        project_root,
        &module_path_map,
        &runtime_packages,
        &[],
        true,
        &facts,
        facts.full_module_ids.clone(),
        state,
        project_root.join("cache"),
    )
    .write()
    .unwrap();

    assert_eq!(files.len(), 1);
    assert_eq!(files[0].record_name, "&@example/log/Index&0.0.18");
    assert_eq!(files[0].source_file, "phone|@example/log|0.0.18|Index.ts");
}

#[test]
fn module_source_state_keeps_lazy_compile_arkts_override_after_rolldown_scan() {
    let module_id = "/workspace/oh_modules/@scope/speech/FaceSpeech.ets";
    let standard_source = r#"import textToSpeech from "@hms.ai.textToSpeech";
textToSpeech.createEngine({});"#;
    let lazy_source = r#"import lazy textToSpeech from "@hms.ai.textToSpeech";
textToSpeech.createEngine({});"#;
    let state = HvigorModuleSourceState::from_rolldown_scan(
        rolldown::ModuleSourceScanOutput {
            warnings: Vec::new(),
            modules: vec![rolldown::ModuleSourceScanModule {
                id: module_id.into(),
                source: standard_source.into(),
                module_type: rolldown_common::ModuleType::Ts,
                exports_kind: rolldown_common::ExportsKind::Esm,
                imports: vec![rolldown::ModuleSourceScanImport {
                    module_request: "@hms.ai.textToSpeech".to_string(),
                    resolved_id: None,
                    kind: rolldown_common::ImportKind::Import,
                    importer_span: rolldown::ModuleSourceScanSpan { start: 0, end: 52 },
                }],
            }],
        },
        HvigorModuleTransformSnapshot {
            module_source_overrides: FxHashMap::from_iter([(
                module_id.to_string(),
                Arc::<str>::from(lazy_source),
            )]),
            source_maps: FxHashMap::default(),
            javascript_module_ids: FxHashSet::default(),
            shared_module_ids: FxHashSet::default(),
            insight_intent_sources: FxHashMap::default(),
        },
    );

    let stored_source = state.sources.get(module_id).expect("module source");
    assert_eq!(stored_source.code.as_str(), lazy_source);
    assert_eq!(
        HvigorModuleRequestSpecifiers::new(stored_source.code.as_str(), module_id).collect(),
        vec!["@hms.ai.textToSpeech".to_string()]
    );

    let imports = HvigorChunkImportMap::from_iter([(
        "@hms.ai.textToSpeech".to_string(),
        "@hms:ai.textToSpeech".to_string(),
    )]);
    let rewritten =
        HvigorModuleSourceImportRewriter::new(stored_source.code.as_str(), module_id, &imports)
            .rewrite();
    assert!(
        rewritten.contains("import lazy textToSpeech from \"@hms:ai.textToSpeech\";"),
        "the final CompileArkTS writer must normalize lazy SDK requests: {rewritten}"
    );
}

#[test]
fn module_source_state_prefers_ets_transform_snapshot_over_rolldown_printer_output() {
    let module_id = "/workspace/entry/src/main/ets/model/Local.ets";
    let ets_transform_source =
        "import type { Local } from './LocalTypes';\nexport const value: Local = {} as Local;\n";
    let rolldown_scanned_source =
        "import type { Local } from \"./LocalTypes\";\nexport const value: Local = {} as Local;\n";
    let state = HvigorModuleSourceState::from_rolldown_scan(
        rolldown::ModuleSourceScanOutput {
            warnings: Vec::new(),
            modules: vec![rolldown::ModuleSourceScanModule {
                id: module_id.into(),
                source: rolldown_scanned_source.into(),
                module_type: rolldown_common::ModuleType::Ts,
                exports_kind: rolldown_common::ExportsKind::Esm,
                imports: Vec::new(),
            }],
        },
        HvigorModuleTransformSnapshot {
            module_source_overrides: FxHashMap::from_iter([(
                module_id.to_string(),
                Arc::<str>::from(ets_transform_source),
            )]),
            source_maps: FxHashMap::default(),
            javascript_module_ids: FxHashSet::default(),
            shared_module_ids: FxHashSet::default(),
            insight_intent_sources: FxHashMap::default(),
        },
    );

    assert_eq!(
        state.sources.get(module_id).map(|source| source.code.as_str()),
        Some(ets_transform_source),
        "ModuleMode must materialize etsTransform output instead of Rolldown's parser-normalized source"
    );
}

#[test]
fn coverage_retransform_replays_resolved_module_requests() {
    let module_id = PathBuf::from("workspace")
        .join("entry")
        .join("src")
        .join("main")
        .join("ets")
        .join("EntryAbility.ets");
    let transformed = r#"import UIAbility from "@ohos.app.ability.UIAbility";
import { helper } from "./helper";
export default class EntryAbility extends UIAbility {}"#;
    let materialized = r#"import UIAbility from "@ohos:app.ability.UIAbility";
import { helper } from "@bundle:com.example/entry/ets/helper";
export default class EntryAbility extends UIAbility {}"#;
    let instrumented = r#"import UIAbility from "@ohos.app.ability.UIAbility";
import { helper } from "./helper";
class BjcCov {}
export default class EntryAbility extends UIAbility {}"#;

    let plan = HvigorModuleRequestRewritePlan::from_baseline(
        module_id.to_string_lossy().as_ref(),
        transformed,
        materialized,
    )
    .expect("capture module request rewrite plan");
    let rewritten = plan.rewrite(instrumented);

    assert!(rewritten.contains("from \"@ohos:app.ability.UIAbility\""));
    assert!(rewritten.contains("from \"@bundle:com.example/entry/ets/helper\""));
    assert!(rewritten.contains("class BjcCov {}"));
}

#[test]
fn module_source_state_consumes_rolldown_compact_scan_facts() {
    let importer = "/workspace/oh_modules/@example/passport/RSAEncryptionUtils.ets";
    let resolved = "/workspace/oh_modules/@ohos/hypium/index.js";
    let cjs_module = "/workspace/oh_modules/@sgfe/flashbuy_price_harmony/build/index.cjs.js";
    let mut source_maps = FxHashMap::default();
    source_maps.insert(importer.to_string(), Arc::from("{\"version\":3}"));
    let state = HvigorModuleSourceState::from_rolldown_scan(
        rolldown::ModuleSourceScanOutput {
            warnings: Vec::new(),
            modules: vec![
                rolldown::ModuleSourceScanModule {
                    id: importer.into(),
                    source: r#"import { HostType } from "@sgfe/flashbuy_price_harmony";
import hypium from "@ohos/hypium";
const dynamicHar = import("bytecode-har-dep");"#
                        .into(),
                    module_type: rolldown_common::ModuleType::Ts,
                    exports_kind: rolldown_common::ExportsKind::Esm,
                    imports: vec![
                        rolldown::ModuleSourceScanImport {
                            module_request: "@sgfe/flashbuy_price_harmony".to_string(),
                            resolved_id: Some(cjs_module.into()),
                            kind: rolldown_common::ImportKind::Import,
                            importer_span: rolldown::ModuleSourceScanSpan { start: 0, end: 62 },
                        },
                        rolldown::ModuleSourceScanImport {
                            module_request: "@ohos/hypium".to_string(),
                            resolved_id: Some(resolved.into()),
                            kind: rolldown_common::ImportKind::Import,
                            importer_span: rolldown::ModuleSourceScanSpan { start: 63, end: 91 },
                        },
                        rolldown::ModuleSourceScanImport {
                            module_request: "bytecode-har-dep".to_string(),
                            resolved_id: None,
                            kind: rolldown_common::ImportKind::DynamicImport,
                            importer_span: rolldown::ModuleSourceScanSpan {
                                start: 92,
                                end: 142,
                            },
                        },
                    ],
                },
                rolldown::ModuleSourceScanModule {
                    id: cjs_module.into(),
                    source: "exports.HostType = void 0;".into(),
                    module_type: rolldown_common::ModuleType::Js,
                    exports_kind: rolldown_common::ExportsKind::CommonJs,
                    imports: Vec::new(),
                },
            ],
        },
        HvigorModuleTransformSnapshot {
            module_source_overrides: FxHashMap::default(),
            source_maps,
            javascript_module_ids: FxHashSet::from_iter([importer.to_string()]),
            shared_module_ids: FxHashSet::default(),
            insight_intent_sources: FxHashMap::default(),
        },
    );
    assert_eq!(
        state
            .resolved_imports
            .get("/workspace/oh_modules/@example/passport/RSAEncryptionUtils.ets")
            .and_then(|imports| imports.get("@sgfe/flashbuy_price_harmony"))
            .map(String::as_str),
        Some(cjs_module)
    );
    assert_eq!(
        state
            .resolved_imports
            .get("/workspace/oh_modules/@example/passport/RSAEncryptionUtils.ets")
            .and_then(|imports| imports.get("@ohos/hypium"))
            .map(String::as_str),
        Some(resolved)
    );
    assert_eq!(
        state
            .resolved_imports
            .get("/workspace/oh_modules/@example/passport/RSAEncryptionUtils.ets")
            .and_then(|imports| imports.get("bytecode-har-dep"))
            .map(String::as_str),
        Some("bytecode-har-dep")
    );
    assert_eq!(
        state
            .full_module_is_commonjs
            .get("/workspace/oh_modules/@example/passport/RSAEncryptionUtils.ets"),
        Some(&false)
    );
    assert_eq!(state.full_module_is_commonjs.get(cjs_module), Some(&true));
    assert!(state
        .source_maps
        .contains_key("/workspace/oh_modules/@example/passport/RSAEncryptionUtils.ets"));
    assert!(state.javascript_module_ids.contains(importer));
    assert_eq!(
        state
            .resolved_import_records
            .get("/workspace/oh_modules/@example/passport/RSAEncryptionUtils.ets")
            .map(Vec::len),
        Some(3)
    );
}

#[tokio::test]
async fn rolldown_scan_preserves_variable_dynamic_import_without_request() {
    let temp = tempfile::tempdir().expect("tempdir");
    let build_profile = temp.path().join("BuildProfile.js");
    std::fs::write(
        &build_profile,
        r#"export default class BuildProfile {
}
BuildProfile.USE_ANT_NETWORK_LIB = false;
"#,
    )
    .expect("write build profile");
    let entry = temp.path().join("entry.js");
    std::fs::write(
        &entry,
        r#"import BuildProfile from "./BuildProfile.js";
const USE_ANT_NETWORK_LIB = BuildProfile.USE_ANT_NETWORK_LIB;
export class APDIDNetworkUtil {
static async executeRPC(value) {
    if (USE_ANT_NETWORK_LIB) {
        let request = "@alipay/network";
        return import(request);
    }
    return value;
}
}
"#,
    )
    .expect("write entry");

    let mut bundler = rolldown::Bundler::with_plugins(
        BundlerOptions {
            input: Some(vec![InputItem {
                name: None,
                import: entry.to_string_lossy().into_owned(),
            }]),
            cwd: Some(temp.path().to_path_buf()),
            format: Some(OutputFormat::Esm),
            transform: Some(BundlerTransformOptions {
                typescript: Some(TypeScriptOptions::default()),
                ..Default::default()
            }),
            treeshake: TreeshakeOptions::Boolean(false),
            ..Default::default()
        },
        Vec::new(),
    )
    .expect("bundler");

    let scan = bundler.scan_module_sources().await.expect("scan");
    bundler.close().await.expect("close");

    let imports = scan
        .modules
        .iter()
        .flat_map(|module| module.imports.iter())
        .map(|import| import.module_request.as_str())
        .collect::<Vec<_>>();
    assert!(
        !imports.contains(&"@alipay/network"),
        "non-static dynamic import must not become a module request: {imports:?}"
    );
}
