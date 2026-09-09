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
    assert!(!plugin.should_collect_source(&format!(
        "\0{}",
        selected_root.join("src/virtual.ets").display()
    )));
}

#[test]
fn collect_pass_merges_the_final_module_graph_deterministically() {
    let temp = tempfile::tempdir().expect("create temp dir");
    let selected_root = temp.path().join("oh_modules/selected");
    let collect_file = temp.path().join("sourceHarDuplicateFilesInfo.txt");
    let plugin = HarDedupPlugin::collect_pass(collect_file.clone(), vec![selected_root.clone()]);
    let first = selected_root
        .join("src/a.ets")
        .to_string_lossy()
        .into_owned();
    let second = selected_root
        .join("src/b.ets")
        .to_string_lossy()
        .into_owned();
    let unrelated = temp
        .path()
        .join("oh_modules/other/src/index.ets")
        .to_string_lossy()
        .into_owned();

    plugin
        .flush_collect([
            second.as_str(),
            unrelated.as_str(),
            first.as_str(),
            second.as_str(),
        ])
        .expect("write collect graph");

    assert_eq!(
        std::fs::read_to_string(collect_file).expect("read collect graph"),
        format!("{first}\n{second}")
    );
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
