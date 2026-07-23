use super::har_dedup::HarDedupPlugin;

#[test]
fn collect_pass_is_scoped_to_planned_source_har_roots() {
    let temp = tempfile::tempdir().expect("create temp dir");
    let selected_root = temp.path().join("oh_modules/selected");
    let other_root = temp.path().join("oh_modules/other");
    let plugin = HarDedupPlugin::collect_pass(
        temp.path().join("sourceHarDuplicateFilesInfo.txt"),
        vec![selected_root.clone()],
    );

    assert!(plugin.should_collect_source(&selected_root.join("src/index.ets").to_string_lossy()));
    assert!(!plugin.should_collect_source(&other_root.join("src/index.ets").to_string_lossy()));
}

#[test]
fn real_build_empties_only_exact_collected_modules() {
    let temp = tempfile::tempdir().expect("create temp dir");
    let collect_file = temp.path().join("sourceHarDuplicateFilesInfo.txt");
    let duplicate = temp.path().join("oh_modules/selected/src/index.ets");
    let sibling = temp.path().join("oh_modules/selected/src/index_extra.ets");
    std::fs::write(&collect_file, duplicate.to_string_lossy().as_bytes())
        .expect("write collect file");
    let plugin = HarDedupPlugin::real_build(collect_file).expect("load collect file");

    let duplicate_output = plugin
        .load_module(&duplicate.to_string_lossy())
        .expect("load duplicate module");
    assert!(duplicate_output.is_some());
    let sibling_output = plugin
        .load_module(&sibling.to_string_lossy())
        .expect("load sibling module");
    assert!(sibling_output.is_none());
}
