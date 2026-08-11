use std::path::PathBuf;

use arkdown::SourceMapInputFingerprint;
use tempfile::TempDir;

use super::intermediates::{IntermediateArtifactWriter, SourceMapPackageHandoff};
use super::layout::RestoolLayout;

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
  std::fs::write(&stale_package_intermediate, b"stale-source-map").expect("write stale source map");
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
      SourceMapPackageHandoff::Materialize(&temp.path().join("source_map/default/sourceMaps.map")),
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
  std::fs::write(&origin, map).expect("write source maps with an unchanged serialized Index entry");

  writer
    .stage_source_maps(
      &origin,
      &compile_cache,
      &release_output,
      SourceMapPackageHandoff::Materialize(&temp.path().join("source_map/default/sourceMaps.map")),
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
    let value: serde_json::Value = serde_json::from_slice(&std::fs::read(path).expect("read map"))
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
  let module_layout = super::layout::ModuleBuildLayout::new(temp.path());
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
  let module_layout = super::layout::ModuleBuildLayout::new(temp.path());
  let writer = IntermediateArtifactWriter::new(&temp.path().join("sdk/toolchains/restool"));

  writer
    .write_empty_router_map_intermediates(temp.path())
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
    .write_module_info(&module_root, "default", true)
    .expect("write enabled ModuleInfo.ts");
  assert_eq!(
    std::fs::read_to_string(&output).expect("read ModuleInfo.ts"),
    "\n//@ts-noCheck\nexport const __MODULE_NAME__: string = __getCurrentModuleName__()\nexport const __BUNDLE_NAME__: string = __getCurrentBundleName__()\n"
  );

  writer
    .write_module_info(&module_root, "default", false)
    .expect("remove stale disabled ModuleInfo.ts");
  assert!(!output.exists());
}

#[test]
fn package_har_cache_path_matches_latest_target_scoped_task_cache() {
  let module_root = PathBuf::from("project").join("entry");
  let layout = super::layout::ModuleBuildLayout::new(&module_root);

  assert_eq!(
    layout.package_har_cache_dir("default"),
    module_root
      .join("build/default/cache/default")
      .join("default@PackageHar")
  );
}

#[test]
fn resource_table_outputs_match_latest_process_resource_contract() {
  let temp = TempDir::new().expect("create resource table fixture root");
  let module_layout = super::layout::ModuleBuildLayout::new(temp.path());
  let writer = IntermediateArtifactWriter::new(&temp.path().join("sdk/toolchains/restool"));

  let hap = writer.resource_table_outputs(
    &module_layout,
    &arkdown_core::OhosModuleType::Entry,
    &arkdown_core::OutputTarget::Hap,
  );
  assert_eq!(
    hap
      .iter()
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

  writer
    .write_profile_intermediates(
      &module_root,
      &arkdown_core::OutputTarget::Hap,
      "default",
      &final_module_json,
      2,
      &[],
    )
    .expect("write staged profiles");

  let layout = super::layout::ModuleBuildLayout::new(&module_root);
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
fn restool_layout_matches_hvigor_sdk_specific_image_transcoder_contract() {
  let temp = TempDir::new().expect("create SDK fixture root");

  let openharmony_toolchains = temp.path().join("openharmony-sdk").join("toolchains");
  std::fs::create_dir_all(&openharmony_toolchains).expect("create OpenHarmony toolchains");
  std::fs::write(openharmony_toolchains.join("oh-uni-package.json"), b"{}")
    .expect("write OpenHarmony component marker");
  let openharmony_layout =
    RestoolLayout::from_restool_path(&openharmony_toolchains.join("restool"));
  assert_eq!(
    openharmony_layout.image_transcoder_library(),
    None,
    "hvigor OhosSdkInfo.getLibimageTranscoderShared() returns an empty path"
  );
  assert_eq!(
    openharmony_layout.defined_sys_ids(),
    None,
    "hvigor ProcessResource omits definedSysIds for OpenHarmony SDKs"
  );

  let default_sdk = temp.path().join("deveco-sdk").join("default");
  let openharmony_restool = default_sdk
    .join("openharmony")
    .join("toolchains")
    .join("restool");
  let hms_toolchains = default_sdk.join("hms").join("toolchains");
  std::fs::create_dir_all(&hms_toolchains).expect("create HarmonyOS toolchains");
  std::fs::write(hms_toolchains.join("id_defined.json"), b"[]")
    .expect("write HarmonyOS system ID definitions");
  let harmony_layout = RestoolLayout::from_restool_path(&openharmony_restool);
  let library = harmony_layout
    .image_transcoder_library()
    .expect("HarmonyOS SDK must expose the image transcoder path");
  assert_eq!(library.parent(), Some(hms_toolchains.join("lib").as_path()));
  assert_eq!(
    library.file_name().and_then(|name| name.to_str()),
    Some(platform_image_transcoder_name())
  );
  assert_eq!(
    harmony_layout.defined_sys_ids(),
    Some(hms_toolchains.join("id_defined.json"))
  );
}

fn platform_image_transcoder_name() -> &'static str {
  if cfg!(target_os = "windows") {
    "libimage_transcoder_shared.dll"
  } else if cfg!(target_os = "linux") {
    "libimage_transcoder_shared.so"
  } else {
    "libimage_transcoder_shared.dylib"
  }
}

#[tokio::test]
async fn partial_hvigor_execution_stops_only_after_every_selected_root() {
  use super::args::NativeTaskRoot;
  use super::coordinator::{BuildTaskLifecycle, NativeTaskCutoffReached};

  let mut lifecycle = BuildTaskLifecycle::new(
    None,
    vec![
      NativeTaskRoot {
        module_name: "entry".to_string(),
        target_name: Some("default".to_string()),
        task_name: "CreateBuildProfile".to_string(),
      },
      NativeTaskRoot {
        module_name: "entry".to_string(),
        target_name: Some("default".to_string()),
        task_name: "CompileArkTS".to_string(),
      },
    ],
  );

  lifecycle
    .after("entry", Some("default"), "CreateBuildProfile")
    .await
    .expect("the first selected root must not stop a multi-root command");
  lifecycle
    .after("entry", Some("default"), "GenerateLoaderJson")
    .await
    .expect("an unrelated dependency boundary must not stop the command");
  let cutoff = lifecycle
    .after("entry", Some("default"), "CompileArkTS")
    .await
    .expect_err("the final selected root must stop the native pipeline");

  assert!(cutoff.is::<NativeTaskCutoffReached>());
}

#[tokio::test]
async fn unqualified_module_hook_root_matches_a_targetless_boundary() {
  use super::args::NativeTaskRoot;
  use super::coordinator::{BuildTaskLifecycle, NativeTaskCutoffReached};

  let mut lifecycle = BuildTaskLifecycle::new(
    None,
    vec![NativeTaskRoot {
      module_name: "entry".to_string(),
      target_name: None,
      task_name: "GenerateBuildProfile".to_string(),
    }],
  );

  lifecycle
    .after("entry", Some("default"), "CreateBuildProfile")
    .await
    .expect("the qualified dependency must not complete the unqualified hook root");
  let cutoff = lifecycle
    .after("entry", None, "GenerateBuildProfile")
    .await
    .expect_err("the unqualified module hook boundary must stop the partial command");

  assert!(cutoff.is::<NativeTaskCutoffReached>());
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
  let index_golden = std::fs::read(format!("{GOLDEN_UNIT_TEST_DIR}/Index.ets.golden"))
    .expect("hvigor-generated Index.ets golden must exist");
  let ability_golden = std::fs::read(format!("{GOLDEN_UNIT_TEST_DIR}/TestAbility.ets.golden"))
    .expect("hvigor-generated TestAbility.ets golden must exist");
  let phone_golden =
    std::fs::read(format!("{GOLDEN_UNIT_TEST_DIR}/phoneSettingConfig_Phone.json.golden"))
      .expect("hvigor-generated phoneSettingConfig golden must exist");

  let params = super::unit_test::UnitTestTemplateParams {
    bundle_name: "com.example.arkdown.test",
    module_name: "entry",
    scope: "",
    // Golden files captured from a `hvigorw test -p ohos-test-coverage=false` run.
    coverage: false,
    module_path: std::path::Path::new(
      "/Volumes/PSSD/code/arkdown/tests/fixtures/minimal-hap/entry",
    ),
  };
  let rendered = params.render_page_index().expect("render template");
  assert_eq!(
    rendered.as_bytes(),
    index_golden.as_slice(),
    "rendered Index.ets must equal the hvigor-generated file"
  );

  let dir = tempfile::tempdir().expect("tempdir");
  super::unit_test::generate_unit_test_template(dir.path(), &params)
    .expect("template generation");
  assert_eq!(
    std::fs::read(dir.path().join(".test/testability/TestAbility.ets")).unwrap(),
    ability_golden,
    "TestAbility.ets must equal the hvigor-generated file"
  );
  assert_eq!(
    std::fs::read(dir.path().join(".test/testability/phoneSettingConfig_Phone.json")).unwrap(),
    phone_golden,
    "phoneSettingConfig must equal the hvigor-generated file"
  );
}

#[test]
fn unit_test_template_coverage_false_and_scope_are_substituted() {
  let params = super::unit_test::UnitTestTemplateParams {
    bundle_name: "com.example.arkdown.test",
    module_name: "entry",
    scope: "ActsAbilityTest",
    coverage: false,
    module_path: std::path::Path::new("/tmp/example/entry"),
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
  assert!(
    rendered.contains("global[savePath] = \"/tmp/example/entry/.test/default/intermediates/test/coverage_data/js_coverage.json\""),
    "module path must be embedded verbatim"
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
  super::unit_test::inject_unit_test_ability(&mut document, "entry").expect("injection");
  let abilities = document["module"]["abilities"].as_array().unwrap();
  assert_eq!(abilities.len(), 2, "one TestAbility appended");
  assert_eq!(abilities[1]["name"], "TestAbility");
  assert_eq!(abilities[1]["srcEntrance"], "../../.test/testability/TestAbility.ets");
  assert_eq!(abilities[1]["startWindowIcon"], "$media:startIcon");
  assert_eq!(
    abilities[1]["startWindowBackground"],
    "$color:start_window_background"
  );
  assert_eq!(document["module"]["deliveryWithInstall"], serde_json::Value::Null);
  // entry module keeps deliveryWithInstall untouched
}

#[test]
fn unit_test_process_profile_har_rewrite_matches_hvigor() {
  let mut document = serde_json::json!({
    "app": {"icon": "$media:layered_image", "label": "$string:app_name"},
    "module": {"name": "library", "type": "har", "abilities": []}
  });
  super::unit_test::inject_unit_test_ability(&mut document, "har").expect("injection");
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
  super::unit_test::replace_unit_test_index_file(
    &res_module_json,
    &process_profile,
    &profile_dir,
    "entry",
    super::unit_test::UNIT_TEST_REPLACE_PAGE,
  )
  .expect("replace");
  let module_doc: serde_json::Value =
    serde_json::from_str(&std::fs::read_to_string(&res_module_json).unwrap()).unwrap();
  assert_eq!(module_doc["module"]["pages"], "$profile:main_pages");
  let main_pages: serde_json::Value = serde_json::from_str(
    &std::fs::read_to_string(profile_dir.join("main_pages.json")).unwrap(),
  )
  .unwrap();
  assert_eq!(
    main_pages["src"][0],
    super::unit_test::UNIT_TEST_REPLACE_PAGE
  );

  // HAP with existing pages -> keep, but still write main_pages.json.
  let res_module_json2 = res.join("module2.json");
  std::fs::write(
    &res_module_json2,
    r#"{"module": {"pages": "$profile:existing"}}"#,
  )
  .unwrap();
  super::unit_test::replace_unit_test_index_file(
    &res_module_json2,
    &process_profile,
    &profile_dir,
    "entry",
    super::unit_test::UNIT_TEST_REPLACE_PAGE,
  )
  .expect("replace keeps pages");
  let kept: serde_json::Value =
    serde_json::from_str(&std::fs::read_to_string(&res_module_json2).unwrap()).unwrap();
  assert_eq!(kept["module"]["pages"], "$profile:existing");
  assert!(profile_dir.join("main_pages.json").is_file());
}

#[test]
fn unit_test_artifact_handoffs_use_the_typed_module_layout() {
  use super::layout::ModuleBuildLayout;
  use super::unit_test::UnitTestArtifactWriter;

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

  let layout = ModuleBuildLayout::for_test(&module_root);
  let writer = UnitTestArtifactWriter::new(&module_root);
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
