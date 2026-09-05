use std::path::Path;

use arkdown_abc::bytecode::Ts2abcReloadMode;
use arkdown_core::HvigorModuleSourceFile;
use arkdown_watcher::{WatchEvent, WatchEventKind};
use serde_json::json;

use super::hot_reload::{
    HotReloadCompileInput, HotReloadCompilePhase, HotReloadProcessor, HotReloadTaskBoundary,
    HotReloadTaskPlan, ReloadResourceTaskPlan,
};
use super::layout::ModuleBuildLayout;
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
fn hot_reload_task_plan_preserves_targetless_build_boundary_and_resource_hook() {
    let action = BuildAction::HotReloadBuild;
    let plan = HotReloadTaskPlan::for_action(action).expect("hot reload plan");
    assert_eq!(plan.boundary(), HotReloadTaskBoundary::Build);
    assert!(action.is_hot_reload());
    assert!(ReloadResourceTaskPlan::requested(action));
    assert!(!ReloadResourceTaskPlan::requested(BuildAction::Assemble));
}

#[test]
fn initial_hot_reload_compiles_all_files_and_dumps_the_configured_symbol_map() {
    let temp = tempfile::tempdir().expect("tempdir");
    let project_root = temp.path();
    let module_root = project_root.join("entry");
    let layout = ModuleBuildLayout::for_target_product(&module_root, "free", "default");
    let index = layout.source().ets().join("pages").join("Index.ets");
    let about = layout.source().ets().join("pages").join("About.ets");
    let sources = vec![
        source(&index, &module_root.join("cache/Index.js"), "index-key"),
        source(&about, &module_root.join("cache/About.js"), "about-key"),
    ];
    let old_map = project_root.join("ide").join("old-map");
    let patch_config = json!({
      "mode": "hotReload",
      "oldMapFilePath": old_map,
      "changedFileList": project_root.join("ide").join("changed.json"),
      "patchAbcPath": project_root.join("ide").join("patch")
    });

    let plan = HotReloadProcessor::new(&layout, project_root)
        .compile_plan(HotReloadCompileInput {
            patch_config: Some(&patch_config),
            compile_mode: "esmodule",
            compile_api: 12,
            incremental: false,
            module_source_files: &sources,
            base_modules_abc_path: &layout.loader_out_dir().join("modules.abc"),
            source_maps_path: None,
        })
        .expect("initial hot reload plan")
        .expect("initial compilation");

    assert_eq!(plan.phase, HotReloadCompilePhase::Initial);
    assert_eq!(plan.module_source_files, sources);
    assert_eq!(plan.reload.mode, Ts2abcReloadMode::Dump);
    assert_eq!(plan.reload.symbol_table_path, old_map.join("symbolMap.map"));
    assert!(plan.compile_entry_override.is_none());
}

#[test]
fn incremental_hot_reload_v1_uses_ets_relative_paths_and_changed_order() {
    let temp = tempfile::tempdir().expect("tempdir");
    let project_root = temp.path();
    let module_root = project_root.join("entry");
    let layout = ModuleBuildLayout::for_target_product(&module_root, "free", "default");
    let index = layout.source().ets().join("pages").join("Index.ets");
    let about = layout.source().ets().join("pages").join("About.ets");
    write_file(&index, b"index");
    write_file(&about, b"about");
    let sources = vec![
        source(&index, &module_root.join("cache/Index.js"), "index-key"),
        source(&about, &module_root.join("cache/About.js"), "about-key"),
    ];
    let changed = project_root.join("ide").join("changed-v1.json");
    write_file(
        &changed,
        serde_json::to_vec(&json!({
          "modifiedFiles": ["pages/About.ets", "pages/Index.ets"]
        }))
        .expect("changed list"),
    );
    let source_maps = project_root.join("sourceMaps.map");
    write_file(
        &source_maps,
        serde_json::to_vec(&json!({
          "index-key": { "map": "index" },
          "about-key": { "map": "about" }
        }))
        .expect("source maps"),
    );
    let patch_dir = project_root.join("ide").join("patch-v1");
    let patch_config = json!({
      "mode": "hotReload",
      "oldMapFilePath": project_root.join("ide").join("old-map"),
      "changedFileList": changed,
      "patchAbcPath": patch_dir
    });

    let plan = HotReloadProcessor::new(&layout, project_root)
        .compile_plan(HotReloadCompileInput {
            patch_config: Some(&patch_config),
            compile_mode: "esmodule",
            compile_api: 12,
            incremental: true,
            module_source_files: &sources,
            base_modules_abc_path: &layout.loader_out_dir().join("modules.abc"),
            source_maps_path: Some(&source_maps),
        })
        .expect("incremental hot reload plan")
        .expect("changed compilation");

    assert_eq!(plan.phase, HotReloadCompilePhase::Incremental);
    assert_eq!(
        plan.module_source_files
            .iter()
            .map(|source| source.source_file.as_str())
            .collect::<Vec<_>>(),
        vec!["about-key", "index-key"]
    );
    assert_eq!(plan.reload.mode, Ts2abcReloadMode::Hot);
    assert_eq!(plan.modules_abc_path, patch_dir.join("modules.abc"));
    let written: serde_json::Value = serde_json::from_slice(
        &std::fs::read(patch_dir.join("sourceMaps.map")).expect("patch source map"),
    )
    .expect("valid patch source map");
    assert_eq!(
        written,
        json!({
          "about-key": { "map": "about" },
          "index-key": { "map": "index" }
        })
    );
}

#[test]
fn incremental_hot_reload_v2_prefers_absolute_file_path_entries() {
    let temp = tempfile::tempdir().expect("tempdir");
    let project_root = temp.path();
    let module_root = project_root.join("entry");
    let layout = ModuleBuildLayout::for_target_product(&module_root, "free", "default");
    let index = layout.source().ets().join("pages").join("Index.ets");
    write_file(&index, b"index");
    let sources = vec![source(
        &index,
        &module_root.join("cache/Index.js"),
        "index-key",
    )];
    let changed = project_root.join("changed-v2.json");
    write_file(
        &changed,
        serde_json::to_vec(&json!({
          "modifiedFilesV2": [
            { "filePath": index },
            { "kind": "ignored-without-filePath" }
          ],
          "modifiedFiles": ["pages/DoesNotWin.ets"]
        }))
        .expect("changed list"),
    );
    let source_maps = project_root.join("sourceMaps.map");
    write_file(
        &source_maps,
        serde_json::to_vec(&json!({ "index-key": { "map": "index" } })).expect("source maps"),
    );
    let patch_config = json!({
      "mode": "hotReload",
      "oldMapFilePath": project_root.join("old-map"),
      "changedFileList": changed,
      "patchAbcPath": project_root.join("patch")
    });

    let plan = HotReloadProcessor::new(&layout, project_root)
        .compile_plan(HotReloadCompileInput {
            patch_config: Some(&patch_config),
            compile_mode: "esmodule",
            compile_api: 12,
            incremental: true,
            module_source_files: &sources,
            base_modules_abc_path: &layout.loader_out_dir().join("modules.abc"),
            source_maps_path: Some(&source_maps),
        })
        .expect("v2 hot reload plan")
        .expect("v2 changed compilation");

    assert_eq!(plan.module_source_files, sources);
    assert_eq!(
        plan.compile_entry_override.as_deref(),
        Some([index.to_string_lossy().into_owned()].as_slice())
    );
}

#[test]
fn incremental_hot_reload_skips_json_changes_like_the_sdk() {
    let temp = tempfile::tempdir().expect("tempdir");
    let project_root = temp.path();
    let module_root = project_root.join("entry");
    let layout = ModuleBuildLayout::for_target_product(&module_root, "free", "default");
    let changed = project_root.join("changed.json");
    write_file(
        &changed,
        serde_json::to_vec(&json!({ "modifiedFiles": ["config/data.json"] }))
            .expect("changed list"),
    );
    let patch_config = json!({
      "mode": "hotReload",
      "oldMapFilePath": project_root.join("old-map"),
      "changedFileList": changed,
      "patchAbcPath": project_root.join("patch")
    });

    let plan = HotReloadProcessor::new(&layout, project_root)
        .compile_plan(HotReloadCompileInput {
            patch_config: Some(&patch_config),
            compile_mode: "esmodule",
            compile_api: 12,
            incremental: true,
            module_source_files: &[],
            base_modules_abc_path: &layout.loader_out_dir().join("modules.abc"),
            source_maps_path: None,
        })
        .expect("json change is a valid no-op");

    assert!(plan.is_none());
}

#[test]
fn watch_handoff_matches_rollup_hot_reload_event_cache_semantics() {
    let temp = tempfile::tempdir().expect("tempdir");
    let project_root = temp.path();
    let module_root = project_root.join("entry");
    let layout = ModuleBuildLayout::for_target_product(&module_root, "free", "default");
    let source_root = layout.source().ets();
    let first = source_root.join("pages").join("First.ets");
    let latest = source_root.join("pages").join("Latest.ets");
    let created = source_root.join("pages").join("Created.ets");
    write_file(&first, b"first");
    write_file(&latest, b"latest");
    write_file(&created, b"created");
    let changed_file_list = project_root.join("ide").join("changed.json");
    write_file(
        &changed_file_list,
        serde_json::to_vec(&json!({
          "modifiedFiles": [
            Path::new("pages").join("Existing.ets").to_string_lossy(),
            Path::new("pages").join("Created.ets").to_string_lossy()
          ],
          "removedFiles": ["stale-value-is-not-persistent"]
        }))
        .expect("existing changed list"),
    );
    let patch_config = json!({
      "mode": "hotReload",
      "oldMapFilePath": project_root.join("old-map"),
      "changedFileList": changed_file_list,
      "patchAbcPath": project_root.join("patch")
    });

    HotReloadProcessor::new(&layout, project_root)
        .write_watch_changed_file_list(
            Some(&patch_config),
            &[
                WatchEvent::new(first, WatchEventKind::Modified),
                WatchEvent::new(latest, WatchEventKind::Modified),
                WatchEvent::new(created, WatchEventKind::Created),
            ],
        )
        .expect("write watch handoff");

    let written: serde_json::Value = serde_json::from_slice(
        &std::fs::read(
            patch_config["changedFileList"]
                .as_str()
                .expect("changed list path"),
        )
        .expect("read changed list"),
    )
    .expect("valid changed list");
    assert_eq!(
        written,
        json!({
          "modifiedFiles": [
            Path::new("pages").join("Existing.ets").to_string_lossy(),
            Path::new("pages").join("Latest.ets").to_string_lossy()
          ],
          "removedFiles": [
            Path::new("pages").join("Created.ets").to_string_lossy()
          ]
        })
    );
}

#[test]
fn watch_handoff_respects_sdk_owned_changed_list_switch() {
    let temp = tempfile::tempdir().expect("tempdir");
    let project_root = temp.path();
    let module_root = project_root.join("entry");
    let layout = ModuleBuildLayout::for_target_product(&module_root, "free", "default");
    let changed_file_list = project_root.join("ide").join("changed.json");
    write_file(&changed_file_list, b"sentinel");
    let patch_config = json!({
      "mode": "hotReload",
      "oldMapFilePath": project_root.join("old-map"),
      "changedFileList": changed_file_list,
      "patchAbcPath": project_root.join("patch"),
      "removeChangedFileListInSdk": "true"
    });

    HotReloadProcessor::new(&layout, project_root)
        .write_watch_changed_file_list(Some(&patch_config), &[])
        .expect("SDK-owned changed list is untouched");

    assert_eq!(
        std::fs::read(
            patch_config["changedFileList"]
                .as_str()
                .expect("changed list path")
        )
        .expect("read sentinel"),
        b"sentinel"
    );
}
