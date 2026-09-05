use std::sync::Arc;

use arkdown::SourceMapInputFingerprint;
use arkdown_session::{ModuleSnapshot, ProjectSnapshot};
use indexmap::IndexMap;
use tempfile::TempDir;

use super::coordinator::PackInfoFormResourceResolver;
use crate::intermediates::{IntermediateArtifactWriter, SourceMapPackageHandoff};
use crate::unit_test::{
    inject_unit_test_ability, UnitTestArtifactWriter, UnitTestProfileWriter,
    UnitTestTemplateParams, UnitTestTemplateWriter, UNIT_TEST_REPLACE_PAGE,
};

#[test]
fn make_pack_info_form_profiles_follow_form_widget_module_source_set() {
    let temp = TempDir::new().expect("create form resource fixture root");
    let entry_root = temp.path().join("entry");
    let widget_root = temp.path().join("widget");
    let default_widget_root = temp.path().join("default-widget");
    for (root, name, module_type) in [
        (&entry_root, "entry", "entry"),
        (&widget_root, "widget", "shared"),
        (&default_widget_root, "default-widget", "shared"),
    ] {
        let source = root.join("src").join("main");
        std::fs::create_dir_all(&source).expect("create module source set");
        std::fs::write(
            source.join("module.json5"),
            serde_json::json!({
              "module": {
                "name": name,
                "type": module_type
              }
            })
            .to_string(),
        )
        .expect("write module manifest");
    }
    let widget_test = widget_root.join("src").join("ohosTest");
    std::fs::create_dir_all(&widget_test).expect("create widget test source set");
    std::fs::write(
        widget_test.join("module.json5"),
        r#"{"module":{"name":"widget_test","type":"shared"}}"#,
    )
    .expect("write widget test manifest");

    let mut modules = IndexMap::new();
    for (root, name) in [
        (&entry_root, "entry"),
        (&widget_root, "widget"),
        (&default_widget_root, "default-widget"),
    ] {
        modules.insert(
            name.to_string(),
            Arc::new(ModuleSnapshot::from_root(root, name).expect("snapshot module")),
        );
    }
    let project = ProjectSnapshot {
        root: temp.path().to_path_buf(),
        bundle_name: "com.example.form".to_string(),
        version_code: 1,
        version_name: "1.0.0".to_string(),
        build_version: None,
        min_compatible_version_code: None,
        bundle_type: "app".to_string(),
        product_name: "default".to_string(),
        module_order: modules.keys().cloned().collect(),
        modules,
        root_package: None,
        dependency_graphs: IndexMap::new(),
    };
    let resolver = PackInfoFormResourceResolver::new(&project);

    let redirected = resolver
        .resolve(
            "entry",
            "ohosTest",
            &serde_json::json!({
              "module": { "formWidgetModule": "widget" }
            }),
        )
        .expect("resolve widget test resources");
    assert_eq!(
        redirected,
        [widget_root.join("src").join("ohosTest").join("resources")]
    );

    let target_fallback = resolver
        .resolve(
            "entry",
            "ohosTest",
            &serde_json::json!({
              "module": { "formWidgetModule": "default-widget" }
            }),
        )
        .expect("fall back to widget default resources");
    assert_eq!(
        target_fallback,
        [default_widget_root
            .join("src")
            .join("main")
            .join("resources")]
    );

    let missing_widget_fallback = resolver
        .resolve(
            "entry",
            "default",
            &serde_json::json!({
              "module": { "formWidgetModule": "missing-widget" }
            }),
        )
        .expect("fall back to the current module like FormUtil");
    assert_eq!(
        missing_widget_fallback,
        [entry_root.join("src").join("main").join("resources")]
    );
}

#[test]
fn bytecode_har_release_source_maps_move_to_compile_cache_like_hvigor() {
    let temp = TempDir::new().expect("create source-map fixture root");
    let origin = temp.path().join("loader_out/default/ets/sourceMaps.map");
    let compile_cache = temp.path().join("CompileArkTS/sourceMaps.json");
    let release_output = temp.path().join("CompileArkTS/sourceMaps.map");
    let stale_package_intermediate = temp.path().join("source_map/default/sourceMaps.map");
    std::fs::create_dir_all(origin.parent().unwrap()).expect("create loader output");
    std::fs::write(&origin, b"source-map").expect("write source map");
    std::fs::create_dir_all(stale_package_intermediate.parent().unwrap())
        .expect("create stale source-map directory");
    std::fs::write(&stale_package_intermediate, b"stale-source-map")
        .expect("write stale source map");
    let writer = IntermediateArtifactWriter::new(&temp.path().join("sdk/toolchains/restool"));

    let staged = writer
        .stage_source_maps(
            &origin,
            &compile_cache,
            &release_output,
            SourceMapPackageHandoff::Suppress(&stale_package_intermediate),
            &arkdown_core::BuildMode::Release,
            &[],
        )
        .expect("stage release source map");

    assert_eq!(staged, release_output);
    assert_eq!(
        std::fs::read(&release_output).expect("read release source map"),
        b"source-map"
    );
    assert!(!origin.exists());
    assert!(
        !stale_package_intermediate.exists(),
        "ByteCode HAR must remove the obsolete package source-map intermediate"
    );
    assert_eq!(
        std::fs::read(&compile_cache).expect("read CompileArkTS source map"),
        b"source-map"
    );
}

#[test]
fn debug_source_maps_keep_loader_and_package_intermediate_for_hap_like_hvigor() {
    let temp = TempDir::new().expect("create source-map fixture root");
    let origin = temp.path().join("loader_out/default/ets/sourceMaps.map");
    let compile_cache = temp.path().join("CompileArkTS/sourceMaps.json");
    let release_output = temp.path().join("CompileArkTS/sourceMaps.map");
    let package_intermediate = temp.path().join("source_map/default/sourceMaps.map");
    std::fs::create_dir_all(origin.parent().unwrap()).expect("create loader output");
    std::fs::write(&origin, b"source-map").expect("write source map");
    let writer = IntermediateArtifactWriter::new(&temp.path().join("sdk/toolchains/restool"));

    let staged = writer
        .stage_source_maps(
            &origin,
            &compile_cache,
            &release_output,
            SourceMapPackageHandoff::Materialize(&package_intermediate),
            &arkdown_core::BuildMode::Debug,
            &[],
        )
        .expect("stage debug source map");

    assert_eq!(staged, package_intermediate);
    assert!(origin.is_file());
    assert!(!release_output.exists());
    assert!(compile_cache.is_file());
    assert_eq!(
        std::fs::read(&package_intermediate).expect("read package source map"),
        b"source-map"
    );
}

#[test]
fn release_source_maps_keep_package_intermediate_for_hap_like_hvigor() {
    let temp = TempDir::new().expect("create source-map fixture root");
    let origin = temp.path().join("loader_out/default/ets/sourceMaps.map");
    let compile_cache = temp.path().join("CompileArkTS/sourceMaps.json");
    let release_output = temp.path().join("CompileArkTS/sourceMaps.map");
    let package_intermediate = temp.path().join("source_map/default/sourceMaps.map");
    std::fs::create_dir_all(origin.parent().unwrap()).expect("create loader output");
    std::fs::write(&origin, b"source-map").expect("write source map");
    let writer = IntermediateArtifactWriter::new(&temp.path().join("sdk/toolchains/restool"));

    let staged = writer
        .stage_source_maps(
            &origin,
            &compile_cache,
            &release_output,
            SourceMapPackageHandoff::Materialize(&package_intermediate),
            &arkdown_core::BuildMode::Release,
            &[],
        )
        .expect("stage release source map");

    assert_eq!(staged, package_intermediate);
    assert!(!origin.exists());
    for path in [&compile_cache, &release_output, &package_intermediate] {
        assert_eq!(
            std::fs::read(path).expect("read staged source map"),
            b"source-map",
            "{}",
            path.display()
        );
    }
}

#[test]
fn incremental_source_maps_prepend_regenerated_entries_like_latest_ets_loader() {
    let temp = TempDir::new().expect("create incremental source-map fixture root");
    let origin = temp.path().join("loader_out/default/ets/sourceMaps.map");
    let compile_cache = temp.path().join("CompileArkTS/sourceMaps.json");
    let release_output = temp.path().join("CompileArkTS/sourceMaps.map");
    std::fs::create_dir_all(origin.parent().unwrap()).expect("create loader output");

    // SDK 6.1.1 `SourceMapGenerator` first emits this pass's `Index` record,
    // then appends cached ResourceTable and EntryAbility records in cache order.
    // The source edit below deliberately leaves `mappings` unchanged so this
    // regression protects the native source-fingerprint handoff, not a JSON
    // source-map difference.
    let map = r#"{
  "ResourceTable.js": { "mappings": "resource" },
  "EntryAbility.ts": { "mappings": "ability" },
  "Index.ts": { "mappings": "stable-index" }
}"#;
    std::fs::write(&origin, map).expect("write cold source maps");
    let writer = IntermediateArtifactWriter::new(&temp.path().join("sdk/toolchains/restool"));

    writer
        .stage_source_maps(
            &origin,
            &compile_cache,
            &release_output,
            SourceMapPackageHandoff::Materialize(
                &temp.path().join("source_map/default/sourceMaps.map"),
            ),
            &arkdown_core::BuildMode::Debug,
            &[SourceMapInputFingerprint {
                key: "Index.ts".to_string(),
                content_hash: "cold-index-source".to_string(),
            }],
        )
        .expect("stage cold source maps");
    writer
        .commit_source_map_input_fingerprints(
            &compile_cache,
            &[SourceMapInputFingerprint {
                key: "Index.ts".to_string(),
                content_hash: "cold-index-source".to_string(),
            }],
        )
        .expect("commit cold source-map input fingerprints");
    std::fs::write(&origin, map)
        .expect("write source maps with an unchanged serialized Index entry");

    writer
        .stage_source_maps(
            &origin,
            &compile_cache,
            &release_output,
            SourceMapPackageHandoff::Materialize(
                &temp.path().join("source_map/default/sourceMaps.map"),
            ),
            &arkdown_core::BuildMode::Debug,
            &[SourceMapInputFingerprint {
                key: "Index.ts".to_string(),
                content_hash: "changed-index-source".to_string(),
            }],
        )
        .expect("stage incrementally regenerated source maps");
    writer
        .commit_source_map_input_fingerprints(
            &compile_cache,
            &[SourceMapInputFingerprint {
                key: "Index.ts".to_string(),
                content_hash: "changed-index-source".to_string(),
            }],
        )
        .expect("commit changed source-map input fingerprints");

    let package_intermediate = temp.path().join("source_map/default/sourceMaps.map");
    for path in [&origin, &compile_cache, &package_intermediate] {
        let value: serde_json::Value =
            serde_json::from_slice(&std::fs::read(path).expect("read map"))
                .expect("parse staged source map");
        assert_eq!(
            value
                .as_object()
                .expect("source map object")
                .keys()
                .map(String::as_str)
                .collect::<Vec<_>>(),
            ["Index.ts", "ResourceTable.js", "EntryAbility.ts"],
            "{} must preserve ets-loader's incremental map insertion order",
            path.display()
        );
        assert_eq!(value["Index.ts"]["mappings"], "stable-index");
    }
}

#[test]
fn package_har_publishes_source_maps_to_outputs_mapping_like_latest_hvigor() {
    let temp = TempDir::new().expect("create PackageHar source-map fixture root");
    let module_layout = crate::layout::ModuleBuildLayout::new(temp.path());
    let source_maps = module_layout.source_maps_intermediate();
    std::fs::create_dir_all(source_maps.parent().unwrap()).expect("create source-map directory");
    std::fs::write(&source_maps, b"package-har-source-map").expect("write source map");
    let writer = IntermediateArtifactWriter::new(&temp.path().join("sdk/toolchains/restool"));

    writer
        .publish_package_source_maps(
            Some(&source_maps),
            &module_layout.output_mapping_source_maps(),
        )
        .expect("publish PackageHar source map");

    assert_eq!(
        std::fs::read(module_layout.output_mapping_source_maps())
            .expect("read published PackageHar source map"),
        b"package-har-source-map"
    );
}

#[test]
fn empty_router_map_writes_latest_process_router_map_task_outputs() {
    let temp = TempDir::new().expect("create empty router-map fixture root");
    let module_layout = crate::layout::ModuleBuildLayout::new(temp.path());
    let writer = IntermediateArtifactWriter::new(&temp.path().join("sdk/toolchains/restool"));

    writer
        .write_empty_router_map_intermediates(temp.path(), "default")
        .expect("write empty ProcessRouterMap outputs");

    for output in [
        module_layout.router_map_loader(),
        module_layout.router_map_temp(),
    ] {
        assert_eq!(
            std::fs::read_to_string(&output).expect("read empty router-map output"),
            r#"{"routerMap":[]}"#
        );
    }
}

#[test]
fn module_info_writer_matches_latest_conditional_output_and_stale_cleanup() {
    let temp = TempDir::new().expect("create module-info fixture root");
    let module_root = temp.path().join("entry");
    let output = module_root.join("build/default/generated/profile/default/ModuleInfo.ts");
    let writer = IntermediateArtifactWriter::new(&temp.path().join("sdk/toolchains/restool"));

    writer
        .write_module_info(&module_root, "default", "default", true)
        .expect("write enabled ModuleInfo.ts");
    assert_eq!(
    std::fs::read_to_string(&output).expect("read ModuleInfo.ts"),
    "\n//@ts-noCheck\nexport const __MODULE_NAME__: string = __getCurrentModuleName__()\nexport const __BUNDLE_NAME__: string = __getCurrentBundleName__()\n"
  );

    writer
        .write_module_info(&module_root, "default", "default", false)
        .expect("remove stale disabled ModuleInfo.ts");
    assert!(!output.exists());
}

#[test]
fn resource_table_outputs_match_latest_process_resource_contract() {
    let temp = TempDir::new().expect("create resource table fixture root");
    let module_layout = crate::layout::ModuleBuildLayout::new(temp.path());
    let writer = IntermediateArtifactWriter::new(&temp.path().join("sdk/toolchains/restool"));

    let hap = writer.resource_table_outputs(
        &module_layout,
        &arkdown_core::OhosModuleType::Entry,
        &arkdown_core::OutputTarget::Hap,
    );
    assert_eq!(
        hap.iter()
            .filter_map(|path| path.file_name().and_then(|name| name.to_str()))
            .collect::<Vec<_>>(),
        ["ResourceTable.h", "ResourceTable.ts"]
    );

    let har = writer.resource_table_outputs(
        &module_layout,
        &arkdown_core::OhosModuleType::Har,
        &arkdown_core::OutputTarget::Har,
    );
    assert_eq!(har.len(), 1);
    let preview = writer.resource_table_outputs(
        &module_layout,
        &arkdown_core::OhosModuleType::Entry,
        &arkdown_core::OutputTarget::Preview,
    );
    assert_eq!(preview.len(), 1);
}

#[test]
fn compile_resource_metadata_is_added_after_profile_intermediates() {
    let temp = TempDir::new().expect("create resource profile fixture root");
    let module_json = temp.path().join("res/default/module.json");
    let insight_intent = temp
        .path()
        .join("res/default/resources/base/profile/insight_intent.json");
    std::fs::create_dir_all(insight_intent.parent().unwrap()).expect("create profile directory");
    std::fs::write(&module_json, r#"{"app":{},"module":{"name":"entry"}}"#)
        .expect("write resource module profile");
    std::fs::write(
        &insight_intent,
        r#"{"insightIntents":[{"intentName":"Search"}]}"#,
    )
    .expect("write insight intent profile");
    let writer = IntermediateArtifactWriter::new(&temp.path().join("sdk/toolchains/restool"));

    writer
        .apply_compile_resource_module_metadata(&module_json, true, &insight_intent)
        .expect("apply compile-resource metadata");

    let profile: serde_json::Value =
        serde_json::from_slice(&std::fs::read(module_json).expect("read resource module profile"))
            .expect("parse resource module profile");
    assert_eq!(
        profile["module"]["routerMap"],
        "$profile:default-router-map"
    );
    assert_eq!(profile["module"]["hasInsightIntent"], true);
}

#[test]
fn merge_profile_precedes_dependency_extension_ability_collection() {
    let temp = TempDir::new().expect("create profile stage fixture root");
    let module_root = temp.path().join("entry");
    let final_module_json = temp.path().join("final-module.json");
    std::fs::write(
        &final_module_json,
        r#"{
      "app": {},
      "module": {
        "name": "entry",
        "virtualMachine": "ark12.0.6.0",
        "compileMode": "esmodule",
        "extensionAbilities": [
          { "name": "CurrentOne" },
          { "name": "CurrentTwo" },
          { "name": "DependencyHarExtension" }
        ]
      }
    }"#,
    )
    .expect("write final module profile");
    let writer = IntermediateArtifactWriter::new(&temp.path().join("sdk/toolchains/restool"));
    let final_value: serde_json::Value = serde_json::from_slice(
        &std::fs::read(&final_module_json).expect("read final module profile"),
    )
    .expect("parse final module profile");
    let layout = crate::layout::ModuleBuildLayout::new(&module_root);

    writer
        .write_merge_profile_intermediate(&layout, &final_value, 2)
        .expect("write MergeProfile output");

    assert!(layout.merge_profile_module_json().is_file());
    assert!(!layout.process_profile_module_json().exists());
    assert!(!layout.res_ark_module_json().exists());

    writer
        .write_process_profile_intermediates(&layout, &final_value, 2, &[])
        .expect("write ProcessProfile outputs");

    let merge: serde_json::Value = serde_json::from_slice(
        &std::fs::read(layout.merge_profile_module_json()).expect("read merge profile"),
    )
    .expect("parse merge profile");
    let process: serde_json::Value = serde_json::from_slice(
        &std::fs::read(layout.process_profile_module_json()).expect("read process profile"),
    )
    .expect("parse process profile");
    let resource_ark: serde_json::Value = serde_json::from_slice(
        &std::fs::read(layout.res_ark_module_json()).expect("read resource ark profile"),
    )
    .expect("parse resource ark profile");

    assert_eq!(
        merge["module"]["extensionAbilities"]
            .as_array()
            .unwrap()
            .len(),
        2
    );
    assert_eq!(
        process["module"]["extensionAbilities"]
            .as_array()
            .unwrap()
            .len(),
        3
    );
    assert!(merge["module"].get("virtualMachine").is_none());
    assert_eq!(process["module"]["virtualMachine"], "ark12.0.6.0");
    assert_eq!(
        resource_ark["module"]["extensionAbilities"]
            .as_array()
            .unwrap()
            .len(),
        2
    );
    assert_eq!(resource_ark["module"]["virtualMachine"], "ark12.0.6.0");
}

#[test]
fn sanitizer_profile_flags_merge_extra_config_and_cmake_arguments() {
    use crate::compile_context::SanitizerProfileFlags;
    use arkdown_core::ExternalNativeOptions;

    let native = ExternalNativeOptions {
        arguments: vec![
            "-DOTHER=ON -DOHOS_ENABLE_TSAN=ON".to_string(),
            "-DOHOS_ENABLE_UBSAN=ON".to_string(),
        ],
        ..ExternalNativeOptions::default()
    };
    let flags =
        SanitizerProfileFlags::new(true, false, false, false).with_cmake_arguments(Some(&native));
    let temp = tempfile::tempdir().expect("tempdir");
    let module_json = temp.path().join("module.json");
    std::fs::write(&module_json, r#"{"app":{},"module":{"name":"entry"}}"#)
        .expect("write module json");

    flags
        .apply_to_module_json(&module_json)
        .expect("apply sanitizer flags");

    let value: serde_json::Value =
        serde_json::from_slice(&std::fs::read(module_json).expect("read module json"))
            .expect("parse module json");
    assert_eq!(value["app"]["asanEnabled"], true);
    assert_eq!(value["app"]["tsanEnabled"], true);
    assert_eq!(value["app"]["ubsanEnabled"], true);
    assert_eq!(value["app"]["hwasanEnabled"], serde_json::Value::Null);
}

// ── Local unit-test chain oracle tests ──────────────────────────────────────
// Golden files captured from a real hvigor 6.24.3 `hvigorw test` run over the
// minimal-hap fixture (see meta/design/hvigor-remaining-diff-audit.md E).
// The templates must match `@ohos/coverage` testTemplate.js byte for byte.

// Golden files captured from a real hvigor 6.24.3 `hvigorw test
// -p ohos-test-coverage=false` run over the minimal-hap fixture; they are
// committed under tests/fixtures/golden/unit-test/ so the oracle does not
// depend on a live `.test` build root.
const GOLDEN_UNIT_TEST_DIR: &str = concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/../../tests/fixtures/golden/unit-test"
);

#[test]
fn unit_test_template_matches_hvigor_generated_files_byte_for_byte() {
    let module_path = std::fs::canonicalize(
        std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .join("..")
            .join("..")
            .join("tests")
            .join("fixtures")
            .join("minimal-hap")
            .join("entry"),
    )
    .expect("minimal-hap entry fixture must exist");
    let serialized_module_path =
        arkdown_path::ArtifactPathSerializer::path_to_slash_string(&module_path)
            .expect("fixture path must serialize");
    let index_golden_template =
        std::fs::read_to_string(format!("{GOLDEN_UNIT_TEST_DIR}/Index.ets.golden"))
            .expect("hvigor-generated Index.ets golden must exist");
    let index_golden = index_golden_template
        .strip_suffix('\n')
        .unwrap_or(&index_golden_template)
        .replace("__MODULE_PATH__", &serialized_module_path);
    let ability_golden = std::fs::read(format!("{GOLDEN_UNIT_TEST_DIR}/TestAbility.ets.golden"))
        .expect("hvigor-generated TestAbility.ets golden must exist");
    let phone_golden = std::fs::read(format!(
        "{GOLDEN_UNIT_TEST_DIR}/phoneSettingConfig_Phone.json.golden"
    ))
    .expect("hvigor-generated phoneSettingConfig golden must exist");

    let params = UnitTestTemplateParams {
        bundle_name: "com.example.arkdown.test",
        module_name: "entry",
        scope: "",
        // Golden files captured from a `hvigorw test -p ohos-test-coverage=false` run.
        coverage: false,
        module_path: &module_path,
    };
    let rendered = params.render_page_index().expect("render template");
    assert_eq!(
        rendered.as_bytes(),
        index_golden.as_bytes(),
        "rendered Index.ets must equal the hvigor-generated file"
    );

    let dir = tempfile::tempdir().expect("tempdir");
    UnitTestTemplateWriter::new(dir.path(), "default")
        .generate_local(&params)
        .expect("template generation");
    assert_eq!(
        std::fs::read(dir.path().join(".test/testability/TestAbility.ets")).unwrap(),
        ability_golden,
        "TestAbility.ets must equal the hvigor-generated file"
    );
    assert_eq!(
        std::fs::read(
            dir.path()
                .join(".test/testability/phoneSettingConfig_Phone.json")
        )
        .unwrap(),
        phone_golden,
        "phoneSettingConfig must equal the hvigor-generated file"
    );
}

#[test]
fn unit_test_template_coverage_false_and_scope_are_substituted() {
    let dir = tempfile::tempdir().expect("tempdir");
    let module_path = dir.path().join("example").join("entry");
    let params = UnitTestTemplateParams {
        bundle_name: "com.example.arkdown.test",
        module_name: "entry",
        scope: "ActsAbilityTest",
        coverage: false,
        module_path: &module_path,
    };
    let rendered = params.render_page_index().expect("render template");
    assert!(
        rendered.contains("\"-s coverage\": \"false\""),
        "coverage=false must render false"
    );
    assert!(
        rendered.contains("\"-s class\": \"ActsAbilityTest\""),
        "scope must be injected into -s class"
    );
    assert!(
        rendered.contains("abilityDelegatorArguments.testCaseNames = \"ActsAbilityTest\""),
        "scope must be injected into testCaseNames"
    );
    let serialized_module_path =
        arkdown_path::ArtifactPathSerializer::path_to_slash_string(&module_path)
            .expect("test module path must serialize");
    assert!(
    rendered.contains(&format!(
      "global[savePath] = \"{serialized_module_path}/.test/default/intermediates/test/coverage_data/js_coverage.json\""
    )),
    "module path must be embedded through the portable artifact serializer"
  );
}

#[test]
fn unit_test_process_profile_injects_test_ability_like_hvigor() {
    let process_profile: serde_json::Value = serde_json::from_str(
        r#"{
      "app": {"bundleName": "com.example.arkdown.test", "icon": "$media:layered_image"},
      "module": {
        "name": "entry",
        "type": "entry",
        "mainElement": "EntryAbility",
        "abilities": [
          {
            "name": "EntryAbility",
            "srcEntry": "./ets/entryability/EntryAbility.ets",
            "startWindowIcon": "$media:startIcon",
            "startWindowBackground": "$color:start_window_background"
          }
        ]
      }
    }"#,
    )
    .unwrap();
    let mut document = process_profile.clone();
    inject_unit_test_ability(&mut document, "entry").expect("injection");
    let abilities = document["module"]["abilities"].as_array().unwrap();
    assert_eq!(abilities.len(), 2, "one TestAbility appended");
    assert_eq!(abilities[1]["name"], "TestAbility");
    assert_eq!(
        abilities[1]["srcEntrance"],
        "../../.test/testability/TestAbility.ets"
    );
    assert_eq!(abilities[1]["startWindowIcon"], "$media:startIcon");
    assert_eq!(
        abilities[1]["startWindowBackground"],
        "$color:start_window_background"
    );
    assert_eq!(
        document["module"]["deliveryWithInstall"],
        serde_json::Value::Null
    );
    // entry module keeps deliveryWithInstall untouched
}

#[test]
fn unit_test_process_profile_har_rewrite_matches_hvigor() {
    let mut document = serde_json::json!({
      "app": {"icon": "$media:layered_image", "label": "$string:app_name"},
      "module": {"name": "library", "type": "har", "abilities": []}
    });
    inject_unit_test_ability(&mut document, "har").expect("injection");
    assert_eq!(document["module"]["type"], "entry");
    // hvigor: `s.module.deliveryWithInstall = n.isHarModule()` — HAR is true.
    assert_eq!(document["module"]["deliveryWithInstall"], true);
    assert_eq!(document["app"]["icon"], "icon");
    assert_eq!(document["app"]["label"], "label");
    let ability = &document["module"]["abilities"][0];
    assert_eq!(ability["startWindowIcon"], "startIcon");
    assert_eq!(ability["startWindowBackground"], "#FFFFFF");
}

#[test]
fn replace_unit_test_index_file_writes_pages_and_main_pages() {
    let dir = tempfile::tempdir().unwrap();
    let res = dir.path().join("res");
    std::fs::create_dir_all(&res).unwrap();
    let res_module_json = res.join("module.json");
    let profile_dir = res.join("resources/base/profile");
    let process_profile = serde_json::json!({
      "module": {"pages": "$profile:main_pages"}
    });
    // HAP without pages -> rewrite.
    UnitTestProfileWriter::new(
        &res_module_json,
        &process_profile,
        &profile_dir,
        "entry",
        UNIT_TEST_REPLACE_PAGE,
    )
    .replace_index()
    .expect("replace");
    let module_doc: serde_json::Value =
        serde_json::from_str(&std::fs::read_to_string(&res_module_json).unwrap()).unwrap();
    assert_eq!(module_doc["module"]["pages"], "$profile:main_pages");
    let main_pages: serde_json::Value = serde_json::from_str(
        &std::fs::read_to_string(profile_dir.join("main_pages.json")).unwrap(),
    )
    .unwrap();
    assert_eq!(main_pages["src"][0], UNIT_TEST_REPLACE_PAGE);

    // HAP with existing pages -> keep, but still write main_pages.json.
    let res_module_json2 = res.join("module2.json");
    std::fs::write(
        &res_module_json2,
        r#"{"module": {"pages": "$profile:existing"}}"#,
    )
    .unwrap();
    UnitTestProfileWriter::new(
        &res_module_json2,
        &process_profile,
        &profile_dir,
        "entry",
        UNIT_TEST_REPLACE_PAGE,
    )
    .replace_index()
    .expect("replace keeps pages");
    let kept: serde_json::Value =
        serde_json::from_str(&std::fs::read_to_string(&res_module_json2).unwrap()).unwrap();
    assert_eq!(kept["module"]["pages"], "$profile:existing");
    assert!(profile_dir.join("main_pages.json").is_file());
}

#[test]
fn unit_test_artifact_handoffs_use_the_typed_module_layout() {
    use crate::layout::ModuleBuildLayout;

    let temp = tempfile::tempdir().expect("tempdir");
    let module_root = temp.path().join("entry");
    let compile_output = temp.path().join("compile-output");
    std::fs::create_dir_all(&compile_output).expect("compile output");
    let modules_abc = compile_output.join("modules.abc");
    let source_maps = compile_output.join("sourceMaps.map");
    std::fs::write(&modules_abc, b"abc").expect("ABC fixture");
    std::fs::write(
        &source_maps,
        br#"{"entry":{"sources":["src/main/ets/Index.ets"],"mappings":"old"}}"#,
    )
    .expect("source map fixture");

    let layout = ModuleBuildLayout::for_test_product(&module_root, "default");
    let writer = UnitTestArtifactWriter::new(&module_root, "default");
    writer
        .stage_compiled_assets(&modules_abc, Some(&source_maps))
        .expect("stage UnitTestArkTS assets");
    assert_eq!(
        std::fs::read(layout.test_assets_modules_abc()).expect("staged ABC"),
        b"abc"
    );
    assert!(layout.test_assets_source_maps().is_file());

    let mock_input = layout.intermediates_mock_config_json();
    std::fs::create_dir_all(mock_input.parent().expect("mock input parent"))
        .expect("mock input directory");
    std::fs::write(&mock_input, b"{}").expect("mock config fixture");
    writer.copy_mock_config_json().expect("copy mock config");
    assert_eq!(
        std::fs::read(layout.test_mock_config_output_json()).expect("copied mock config"),
        b"{}"
    );

    let replacement = layout.test_coverage_source_map();
    std::fs::create_dir_all(replacement.parent().expect("coverage map parent"))
        .expect("coverage map directory");
    std::fs::write(
        &replacement,
        br#""src/main/ets/Index.ets":{"mappings":"new"},"#,
    )
    .expect("coverage source map fixture");
    writer
        .replace_source_map_in_coverage_mode(true)
        .expect("replace mappings");
    let merged: serde_json::Value = serde_json::from_slice(
        &std::fs::read(layout.test_assets_source_maps()).expect("merged source maps"),
    )
    .expect("parse merged source maps");
    assert_eq!(merged["entry"]["mappings"], "new");

    writer.write_hsp_build_config().expect("HSP buildConfig");
    let build_config: serde_json::Value = serde_json::from_slice(
        &std::fs::read(layout.test_build_config_json()).expect("HSP buildConfig output"),
    )
    .expect("parse HSP buildConfig");
    assert_eq!(
        build_config["aceModuleBuild"],
        arkdown_path::ArtifactPathSerializer::path_to_slash_string(&layout.test_assets_ets_dir())
            .expect("portable artifact path")
    );
}
