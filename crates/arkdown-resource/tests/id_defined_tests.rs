use arkdown_resource::IdDefinedFileProcessor;

#[test]
fn process_id_defined_file_rewrites_json5_as_tab_indented_json() {
    let temp = tempfile::tempdir().expect("tempdir");
    let source = temp.path().join("ids.json5");
    let output = temp.path().join("build/ids_map/id_defined.json");
    std::fs::write(&source, "{ // seed\n app: 0x01000000,\n}").expect("write seed");

    let produced = IdDefinedFileProcessor::new(Some(&source), &output)
        .process()
        .expect("process seed");

    assert_eq!(produced.as_deref(), Some(output.as_path()));
    assert_eq!(
        std::fs::read_to_string(output).expect("read output"),
        "{\n\t\"app\": 16777216\n}"
    );
}

#[test]
fn process_id_defined_file_removes_stale_output_without_a_seed() {
    let temp = tempfile::tempdir().expect("tempdir");
    let output = temp.path().join("id_defined.json");
    std::fs::write(&output, "stale").expect("write stale output");

    let produced = IdDefinedFileProcessor::new(None, &output)
        .process()
        .expect("remove stale output");

    assert!(produced.is_none());
    assert!(!output.exists());
}
