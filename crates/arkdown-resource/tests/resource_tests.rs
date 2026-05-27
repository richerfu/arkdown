//! Integration tests for `arkdown-resource` public API.

use arkdown_resource::{ResourceCompileConfig, ResourceCompileInput, ResourceCompiler};
use std::fs;

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

fn make_compiler() -> ResourceCompiler {
    ResourceCompiler::new(ResourceCompileConfig {
        package_name: "com.example.tests".into(),
        module_name: "entry".into(),
        device_types: vec!["phone".into()],
        is_debug: false,
    })
}

fn write_string_json(dir: &std::path::Path, name: &str, value: &str) {
    let content = format!(r#"{{"string":[{{"name":"{}","value":"{}"}}]}}"#, name, value);
    fs::write(dir.join("string.json"), content).unwrap();
}

// ---------------------------------------------------------------------------
// Baseline: compile with an empty resources directory
// ---------------------------------------------------------------------------

#[test]
fn test_empty_resources_dir_returns_ok() {
    let tmp = tempfile::tempdir().expect("tempdir");
    let res_dir = tmp.path().join("resources");
    fs::create_dir_all(&res_dir).unwrap();

    let out_dir = tmp.path().join("out");
    let result = make_compiler().compile(ResourceCompileInput {
        resources_dirs: vec![res_dir],
        output_dir: out_dir.clone(),
    });
    assert!(result.is_ok(), "empty resources dir must not error: {:?}", result.err());
    assert!(out_dir.join("resources.index").exists(), "resources.index must be created");
    assert!(out_dir.join("ResourceTable.txt").exists(), "ResourceTable.txt must be created");
}

#[test]
fn test_nonexistent_dir_is_silently_skipped() {
    // scan_resources returns empty for non-existent paths — no error expected.
    let tmp = tempfile::tempdir().expect("tempdir");
    let out_dir = tmp.path().join("out");
    let result = make_compiler().compile(ResourceCompileInput {
        resources_dirs: vec![tmp.path().join("does_not_exist")],
        output_dir: out_dir.clone(),
    });
    assert!(
        result.is_ok(),
        "non-existent resource dir should be skipped silently: {:?}",
        result.err()
    );
}

// ---------------------------------------------------------------------------
// resources.index binary format
// ---------------------------------------------------------------------------

#[test]
fn test_resources_index_has_restool_magic() {
    let tmp = tempfile::tempdir().expect("tempdir");
    let el_dir = tmp.path().join("res/base/element");
    fs::create_dir_all(&el_dir).unwrap();
    write_string_json(&el_dir, "app_name", "My App");

    let out_dir = tmp.path().join("out");
    let result = make_compiler()
        .compile(ResourceCompileInput {
            resources_dirs: vec![tmp.path().join("res")],
            output_dir: out_dir,
        })
        .expect("compile must succeed");

    let bytes = fs::read(&result.resources_index).expect("read resources.index");
    // Magic constant: 0xF00F1234 (LE)
    assert_eq!(
        &bytes[0..4],
        &0xF00F_1234_u32.to_le_bytes(),
        "resources.index must start with restool magic 0xF00F1234"
    );
}

#[test]
fn test_resources_index_is_nonempty_with_entries() {
    let tmp = tempfile::tempdir().expect("tempdir");
    let el_dir = tmp.path().join("res/base/element");
    fs::create_dir_all(&el_dir).unwrap();
    write_string_json(&el_dir, "greeting", "Hello");

    let out_dir = tmp.path().join("out");
    let result = make_compiler()
        .compile(ResourceCompileInput {
            resources_dirs: vec![tmp.path().join("res")],
            output_dir: out_dir,
        })
        .expect("compile must succeed");

    let meta = fs::metadata(&result.resources_index).expect("metadata");
    assert!(meta.len() > 4, "resources.index must contain more than just the magic header");
}

// ---------------------------------------------------------------------------
// ResourceTable.txt content
// ---------------------------------------------------------------------------

#[test]
fn test_resource_table_lists_string_entries() {
    let tmp = tempfile::tempdir().expect("tempdir");
    let el_dir = tmp.path().join("res/base/element");
    fs::create_dir_all(&el_dir).unwrap();
    write_string_json(&el_dir, "app_title", "TestApp");

    let out_dir = tmp.path().join("out");
    let result = make_compiler()
        .compile(ResourceCompileInput {
            resources_dirs: vec![tmp.path().join("res")],
            output_dir: out_dir,
        })
        .expect("compile must succeed");

    let table = fs::read_to_string(&result.resource_table).expect("read ResourceTable.txt");
    assert!(
        table.contains("app_title"),
        "ResourceTable.txt must contain resource name 'app_title'"
    );
    assert!(
        table.contains("string"),
        "ResourceTable.txt must contain type 'string'"
    );
}

#[test]
fn test_resource_table_lists_media_entries() {
    let tmp = tempfile::tempdir().expect("tempdir");
    let media_dir = tmp.path().join("res/base/media");
    fs::create_dir_all(&media_dir).unwrap();
    fs::write(media_dir.join("icon.png"), b"\x89PNG\r\n\x1a\n").unwrap();

    let out_dir = tmp.path().join("out");
    let result = make_compiler()
        .compile(ResourceCompileInput {
            resources_dirs: vec![tmp.path().join("res")],
            output_dir: out_dir,
        })
        .expect("compile must succeed");

    let table = fs::read_to_string(&result.resource_table).expect("read ResourceTable.txt");
    assert!(
        table.contains("icon"),
        "ResourceTable.txt must list 'icon' media entry"
    );
}

// ---------------------------------------------------------------------------
// Output paths returned in ResourceCompileOutput
// ---------------------------------------------------------------------------

#[test]
fn test_output_paths_point_inside_output_dir() {
    let tmp = tempfile::tempdir().expect("tempdir");
    let el_dir = tmp.path().join("res/base/element");
    fs::create_dir_all(&el_dir).unwrap();
    write_string_json(&el_dir, "x", "y");

    let out_dir = tmp.path().join("build");
    let result = make_compiler()
        .compile(ResourceCompileInput {
            resources_dirs: vec![tmp.path().join("res")],
            output_dir: out_dir.clone(),
        })
        .expect("compile must succeed");

    assert!(
        result.resources_index.starts_with(&out_dir),
        "resources_index path must be inside output_dir"
    );
    assert!(
        result.resource_table.starts_with(&out_dir),
        "resource_table path must be inside output_dir"
    );
}

// ---------------------------------------------------------------------------
// Media file copy
// ---------------------------------------------------------------------------

#[test]
fn test_media_file_copied_to_output_dir() {
    let tmp = tempfile::tempdir().expect("tempdir");
    let media_dir = tmp.path().join("res/base/media");
    fs::create_dir_all(&media_dir).unwrap();
    let img_data = b"\x89PNG\r\n\x1a\n\x00\x00";
    fs::write(media_dir.join("logo.png"), img_data).unwrap();

    let out_dir = tmp.path().join("out");
    make_compiler()
        .compile(ResourceCompileInput {
            resources_dirs: vec![tmp.path().join("res")],
            output_dir: out_dir.clone(),
        })
        .expect("compile must succeed");

    let copied = out_dir.join("media/logo.png");
    assert!(copied.exists(), "logo.png must be copied to output/media/");
    let copied_bytes = fs::read(&copied).expect("read copied media file");
    assert_eq!(copied_bytes, img_data, "copied media file content must match original");
}

// ---------------------------------------------------------------------------
// Profile file copy
// ---------------------------------------------------------------------------

#[test]
fn test_profile_file_copied_and_returned() {
    let tmp = tempfile::tempdir().expect("tempdir");
    let profile_dir = tmp.path().join("res/base/profile");
    fs::create_dir_all(&profile_dir).unwrap();
    fs::write(profile_dir.join("main_pages.json"), r#"{"src":["pages/Index"]}"#).unwrap();

    let out_dir = tmp.path().join("out");
    let result = make_compiler()
        .compile(ResourceCompileInput {
            resources_dirs: vec![tmp.path().join("res")],
            output_dir: out_dir.clone(),
        })
        .expect("compile must succeed");

    assert_eq!(result.profiles.len(), 1, "one profile should be reported");
    assert_eq!(result.profiles[0].0, "main_pages", "profile name should be the file stem");
    assert!(
        out_dir.join("base/profile/main_pages.json").exists(),
        "profile JSON must be copied to base/profile/"
    );
}

// ---------------------------------------------------------------------------
// Multi-directory merge
// ---------------------------------------------------------------------------

#[test]
fn test_two_resource_dirs_are_merged() {
    let tmp = tempfile::tempdir().expect("tempdir");

    let dir_a = tmp.path().join("r1/base/element");
    fs::create_dir_all(&dir_a).unwrap();
    write_string_json(&dir_a, "alpha", "Alpha");

    let dir_b = tmp.path().join("r2/base/element");
    fs::create_dir_all(&dir_b).unwrap();
    write_string_json(&dir_b, "beta", "Beta");

    let out_dir = tmp.path().join("out");
    let result = make_compiler()
        .compile(ResourceCompileInput {
            resources_dirs: vec![tmp.path().join("r1"), tmp.path().join("r2")],
            output_dir: out_dir,
        })
        .expect("compile must succeed");

    let table = fs::read_to_string(&result.resource_table).expect("read table");
    assert!(table.contains("alpha"), "merged table must contain 'alpha'");
    assert!(table.contains("beta"), "merged table must contain 'beta'");
}
