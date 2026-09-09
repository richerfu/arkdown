use std::path::Path;

use arkdown_core::HvigorModuleSourceFile;
use serde_json::json;

use super::layout::ModuleBuildLayout;
use super::quickfix::{
    ColdReloadProcessor, QuickFixArtifactInput, QuickFixArtifactProcessor, QuickFixTaskBoundary,
    QuickFixTaskPlan, QuickFixVersionStore,
};
use super::request::BuildAction;

fn write_file(path: &Path, contents: impl AsRef<[u8]>) {
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent).expect("create fixture parent");
    }
    std::fs::write(path, contents).expect("write fixture");
}

fn source(module_id: &Path, emitted: &Path, source_file: &str) -> HvigorModuleSourceFile {
    HvigorModuleSourceFile {
        module_id: module_id.to_string_lossy().into_owned(),
        path: emitted.to_path_buf(),
        record_name: format!("record:{source_file}"),
        record_name_is_explicit: false,
        source_file: source_file.to_string(),
        package_name: Some("entry".to_string()),
        is_commonjs: false,
        is_shared_module: false,
        origin_source_lang: "ets".to_string(),
        imported_record_names: Vec::new(),
        is_package_module: false,
    }
}

#[test]
fn quickfix_task_plan_promotes_transitive_hqf_roots_to_the_full_boundary() {
    let action = BuildAction::AssembleQuickfix;
    let plan = QuickFixTaskPlan::for_action(action).expect("quickfix plan");
    assert_eq!(plan.boundary(), QuickFixTaskBoundary::Assemble);
    assert!(plan.needs_patch_artifacts());
    assert!(QuickFixTaskPlan::patch_artifacts_requested(action));
    assert!(QuickFixTaskPlan::for_action(BuildAction::Assemble).is_none());
}

#[test]
fn cold_reload_selects_only_modified_ets_and_writes_patch_source_maps() {
    let temp = tempfile::tempdir().expect("tempdir");
    let layout = ModuleBuildLayout::for_target_product(temp.path(), "free", "release");
    let cache_dir = temp.path().join("cache");
    let ets_root = temp.path().join("src").join("main").join("ets");
    let changed_source = ets_root.join("pages").join("Index.ets");
    let unchanged_source = ets_root.join("pages").join("About.ets");
    let changed_output = temp.path().join("emitted").join("Index.js");
    let unchanged_output = temp.path().join("emitted").join("About.js");
    write_file(&changed_source, b"export default 1");
    write_file(&unchanged_source, b"export default 2");
    write_file(&changed_output, b"export default 1");
    write_file(&unchanged_output, b"export default 2");
    let changed_file_list = temp.path().join("ide").join("changed.json");
    write_file(
        &changed_file_list,
        serde_json::to_vec(&json!({ "modifiedFiles": ["pages/Index.ets"] })).expect("changed list"),
    );
    let old_map_dir = temp.path().join("ide").join("old-map");
    write_file(&old_map_dir.join("symbolMap.map"), b"base-symbol-map");
    let patch_abc_dir = temp.path().join("ide").join("patch-abc");
    let source_maps = temp.path().join("sourceMaps.map");
    write_file(
        &source_maps,
        serde_json::to_vec(&json!({
          "entry|pages/Index.ets": { "map": "changed" },
          "entry|pages/About.ets": { "map": "unchanged" }
        }))
        .expect("source maps"),
    );
    let sources = vec![
        source(&changed_source, &changed_output, "entry|pages/Index.ets"),
        source(
            &unchanged_source,
            &unchanged_output,
            "entry|pages/About.ets",
        ),
    ];

    let plan = ColdReloadProcessor::new(&layout, &cache_dir)
        .compile_plan(
            Some(&json!({
                "mode": "coldReload",
                "oldMapFilePath": old_map_dir,
                "changedFileList": changed_file_list,
                "patchAbcPath": patch_abc_dir,
            })),
            &sources,
            Some(&source_maps),
        )
        .expect("cold reload plan")
        .expect("modified source");

    assert_eq!(plan.module_source_files, vec![sources[0].clone()]);
    assert_eq!(plan.modules_abc_path, patch_abc_dir.join("modules.abc"));
    assert_eq!(
        plan.reload.symbol_table_path,
        old_map_dir.join("symbolMap.map")
    );
    let filtered: serde_json::Value = serde_json::from_slice(
        &std::fs::read(patch_abc_dir.join("sourceMaps.map")).expect("patch source maps"),
    )
    .expect("valid patch source maps");
    assert_eq!(
        filtered,
        json!({ "entry|pages/Index.ets": { "map": "changed" } })
    );
}

#[test]
fn cold_reload_uses_sdk_cache_default_for_missing_changed_file_list() {
    let temp = tempfile::tempdir().expect("tempdir");
    let layout = ModuleBuildLayout::for_target_product(temp.path(), "free", "release");
    let cache_dir = temp.path().join("compile-cache");
    write_file(
        &cache_dir.join("changedFileList.json"),
        serde_json::to_vec(&json!({ "modifiedFiles": [] })).expect("changed list"),
    );
    let old_map_dir = temp.path().join("old-map");

    let plan = ColdReloadProcessor::new(&layout, &cache_dir)
        .compile_plan(
            Some(&json!({
                "mode": "coldReload",
                "oldMapFilePath": old_map_dir,
                "changedFileList": "",
            })),
            &[],
            None,
        )
        .expect("missing changedFileList uses the SDK cache default");

    assert!(plan.is_none());
}

#[test]
fn cold_reload_does_not_select_the_sdk_hot_reload_mode() {
    let temp = tempfile::tempdir().expect("tempdir");
    let layout = ModuleBuildLayout::for_target_product(temp.path(), "free", "release");

    let plan = ColdReloadProcessor::new(&layout, &temp.path().join("cache"))
        .compile_plan(Some(&json!({ "mode": "hotReload" })), &[], None)
        .expect("hot reload is handled by ModuleHotreloadMode");

    assert!(plan.is_none());
}

#[test]
fn prepare_quickfix_materializes_native_resource_logs_and_product_scoped_patch() {
    let temp = tempfile::tempdir().expect("tempdir");
    let module_root = temp.path().join("entry");
    let layout = ModuleBuildLayout::for_target_product(&module_root, "paid", "default");
    let stripped = layout
        .stripped_native_libs_dir()
        .join("arm64-v8a")
        .join("libentry.so");
    let processed = layout
        .process_libs_dir()
        .join("arm64-v8a")
        .join("libentry.so");
    write_file(&stripped, b"stripped");
    write_file(&processed, b"processed");
    let resource_root = module_root.join("src").join("main").join("resources");
    let resource = resource_root.join("base").join("media").join("logo.txt");
    write_file(&resource, b"logo");
    let changed_file_list = layout.patch_changed_file_list();
    write_file(
        &changed_file_list,
        serde_json::to_vec(&json!({
          "modifiedFiles": ["pages/Index.ets"],
          "resources": {
            "resFile": [{
              "filePath": resource,
              "resourcePath": resource_root
            }],
            "rawFile": []
          }
        }))
        .expect("changed list"),
    );
    let patch_config = json!({
      "mode": "coldReload",
      "changedFileList": changed_file_list
    });
    let device_types = ["phone".to_string()];
    let processor = QuickFixArtifactProcessor::new(QuickFixArtifactInput {
        layout: &layout,
        module_name: "entry",
        target_name: "default",
        bundle_name: "com.example.quickfix",
        version_code: 12,
        version_name: "1.2.0",
        device_types: &device_types,
        debug_symbol: None,
        patch_version: 3_000_007,
    });

    processor
        .prepare(Some(&patch_config))
        .expect("prepare quickfix");

    assert_eq!(
        std::fs::read(
            layout
                .patch_libs_dir()
                .join("arm64-v8a")
                .join("libentry.so")
        )
        .expect("patched native library"),
        b"stripped"
    );
    assert_eq!(
        std::fs::read(
            layout
                .patch_resources_dir()
                .join("base")
                .join("media")
                .join("logo.txt")
        )
        .expect("patched resource"),
        b"logo"
    );
    let patch: serde_json::Value =
        serde_json::from_slice(&std::fs::read(layout.patch_json()).expect("patch manifest"))
            .expect("valid patch manifest");
    assert_eq!(patch["app"]["patchVersionCode"], 3_000_007);
    assert_eq!(patch["module"]["name"], "entry");
    let log: serde_json::Value =
        serde_json::from_slice(&std::fs::read(layout.quickfix_json()).expect("quickfix log"))
            .expect("valid quickfix log");
    assert_eq!(log["state"], 2);
    assert_eq!(log["file"], "entry-default-unsigned.hqf");
    let native_cache: serde_json::Value = serde_json::from_slice(
        &std::fs::read(layout.patch_last_libs_json()).expect("quickfix native cache"),
    )
    .expect("valid quickfix native cache");
    assert!(native_cache.get("debugSymbol").is_none());
    assert!(layout
        .quickfix_json()
        .starts_with(module_root.join("build").join("paid")));
}

#[test]
fn assemble_hqf_version_store_starts_at_official_value_and_advances_once() {
    let temp = tempfile::tempdir().expect("tempdir");
    let store = QuickFixVersionStore::new(temp.path());
    assert_eq!(store.current().expect("initial version"), 3_000_000);
    store
        .advance_after_assemble(3_000_000)
        .expect("advance version");
    assert_eq!(store.current().expect("advanced version"), 3_000_001);
}

#[test]
fn quickfix_paths_use_native_path_components_instead_of_encoded_separators() {
    let temp = tempfile::tempdir().expect("tempdir");
    let module_root = temp.path().join("feature");
    let layout = ModuleBuildLayout::for_target_product(&module_root, "staging", "release");

    assert_eq!(
        layout.patch_dir(),
        module_root
            .join("build")
            .join("staging")
            .join("intermediates")
            .join("patch")
            .join("release")
    );
    assert_eq!(
        layout.target_output_dir(),
        module_root
            .join("build")
            .join("staging")
            .join("outputs")
            .join("release")
    );
}
