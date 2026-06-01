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
    let content = format!(
        r#"{{"string":[{{"name":"{}","value":"{}"}}]}}"#,
        name, value
    );
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
    assert!(
        result.is_ok(),
        "empty resources dir must not error: {:?}",
        result.err()
    );
    assert!(
        out_dir.join("resources.index").exists(),
        "resources.index must be created"
    );
    assert!(
        out_dir.join("ResourceTable.txt").exists(),
        "ResourceTable.txt must be created"
    );
    assert!(
        out_dir.join("ids_map/id_defined.json").exists(),
        "id_defined.json must be created"
    );
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
    // Restool 6.0 magic: b"Restool 6.0.0.001\x00" (18 bytes)
    assert_eq!(
        &bytes[0..18],
        b"Restool 6.0.0.001\x00",
        "resources.index must start with restool 6.0 magic"
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
    assert!(
        meta.len() > 4,
        "resources.index must contain more than just the magic header"
    );
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

#[test]
fn test_resource_infos_use_hvigor_runtime_types() {
    let tmp = tempfile::tempdir().expect("tempdir");
    let element_dir = tmp.path().join("res/base/element");
    let media_dir = tmp.path().join("res/base/media");
    let profile_dir = tmp.path().join("res/base/profile");
    fs::create_dir_all(&element_dir).unwrap();
    fs::create_dir_all(&media_dir).unwrap();
    fs::create_dir_all(&profile_dir).unwrap();
    fs::write(
        element_dir.join("string.json"),
        r#"{"string":[{"name":"title","value":"Title"}]}"#,
    )
    .unwrap();
    fs::write(
        element_dir.join("color.json"),
        r##"{"color":[{"name":"brand","value":"#FFFFFF"}]}"##,
    )
    .unwrap();
    fs::write(media_dir.join("icon.png"), b"png").unwrap();
    fs::write(
        profile_dir.join("main_pages.json"),
        r#"{"src":["pages/Index"]}"#,
    )
    .unwrap();

    let result = make_compiler()
        .compile(ResourceCompileInput {
            resources_dirs: vec![tmp.path().join("res")],
            output_dir: tmp.path().join("out"),
        })
        .expect("compile must succeed");

    assert_eq!(result.resource_infos["string.title"].runtime_type, 10003);
    assert_eq!(result.resource_infos["color.brand"].runtime_type, 10001);
    assert_eq!(result.resource_infos["media.icon"].runtime_type, 20000);
    assert!(
        !result.resource_infos.contains_key("profile.main_pages"),
        "hvigor RESOURCE_TYPE has rawfile but no profile, so profile resources must not be exposed as $r runtime references"
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

    let copied = out_dir.join("resources/base/media/logo.png");
    assert!(
        copied.exists(),
        "logo.png must be copied to output/resources/base/media/"
    );
    let copied_bytes = fs::read(&copied).expect("read copied media file");
    assert_eq!(
        copied_bytes, img_data,
        "copied media file content must match original"
    );
}

// ---------------------------------------------------------------------------
// Profile file copy
// ---------------------------------------------------------------------------

#[test]
fn test_profile_file_copied_and_returned() {
    let tmp = tempfile::tempdir().expect("tempdir");
    let profile_dir = tmp.path().join("res/base/profile");
    fs::create_dir_all(&profile_dir).unwrap();
    fs::write(
        profile_dir.join("main_pages.json"),
        r#"{"src":["pages/Index"]}"#,
    )
    .unwrap();

    let out_dir = tmp.path().join("out");
    let result = make_compiler()
        .compile(ResourceCompileInput {
            resources_dirs: vec![tmp.path().join("res")],
            output_dir: out_dir.clone(),
        })
        .expect("compile must succeed");

    assert_eq!(result.profiles.len(), 1, "one profile should be reported");
    assert_eq!(
        result.profiles[0].0, "main_pages",
        "profile name should be the file stem"
    );
    assert!(
        out_dir
            .join("resources/base/profile/main_pages.json")
            .exists(),
        "profile JSON must be copied to resources/base/profile/"
    );
}

#[test]
fn test_hvigor_resource_layout_dedup_and_processed_media_json() {
    let tmp = tempfile::tempdir().expect("tempdir");

    let app_media = tmp.path().join("app/base/media");
    let app_element = tmp.path().join("app/base/element");
    let module_media = tmp.path().join("module/base/media");
    let module_element = tmp.path().join("module/base/element");
    let module_profile = tmp.path().join("module/base/profile");
    fs::create_dir_all(&app_media).unwrap();
    fs::create_dir_all(&app_element).unwrap();
    fs::create_dir_all(&module_media).unwrap();
    fs::create_dir_all(&module_element).unwrap();
    fs::create_dir_all(&module_profile).unwrap();

    write_string_json(&app_element, "app_name", "ArkdownTest");
    fs::write(app_media.join("background.png"), b"app-bg").unwrap();
    fs::write(app_media.join("foreground.png"), b"app-fg").unwrap();
    fs::write(
        app_media.join("layered_image.json"),
        r#"{"layered-image":{"background":"$media:background","foreground":"$media:foreground"}}"#,
    )
    .unwrap();

    fs::write(module_media.join("background.png"), b"module-bg").unwrap();
    fs::write(module_media.join("foreground.png"), b"module-fg").unwrap();
    fs::write(module_media.join("startIcon.png"), b"start").unwrap();
    fs::write(
        module_element.join("string.json"),
        r#"{"string":[{"name":"module_desc","value":"Test module"},{"name":"EntryAbility_desc","value":"description"},{"name":"EntryAbility_label","value":"Entry"}]}"#,
    )
    .unwrap();
    fs::write(
        module_element.join("color.json"),
        r##"{"color":[{"name":"start_window_background","value":"#FFFFFF"}]}"##,
    )
    .unwrap();
    fs::write(
        module_profile.join("main_pages.json"),
        r#"{"src":["pages/Index"]}"#,
    )
    .unwrap();

    let out_dir = tmp.path().join("out/default");
    let result = ResourceCompiler::new(ResourceCompileConfig {
        package_name: "com.example.tests".into(),
        module_name: "entry".into(),
        device_types: vec!["phone".into()],
        is_debug: false,
    })
    .compile(ResourceCompileInput {
        resources_dirs: vec![tmp.path().join("app"), tmp.path().join("module")],
        output_dir: out_dir.clone(),
    })
    .expect("compile must succeed");

    assert_eq!(result.resource_ids["app_name"], 0x0100_0000);
    assert_eq!(result.resource_ids["background"], 0x0100_0001);
    assert_eq!(result.resource_ids["foreground"], 0x0100_0002);
    assert_eq!(result.resource_ids["layered_image"], 0x0100_0003);
    assert_eq!(result.resource_ids["startIcon"], 0x0100_0008);
    assert_eq!(result.resource_ids["main_pages"], 0x0100_0009);

    assert_eq!(
        fs::read(out_dir.join("resources/base/media/background.png")).unwrap(),
        b"app-bg"
    );
    assert_eq!(
        fs::read(out_dir.join("resources/base/media/foreground.png")).unwrap(),
        b"app-fg"
    );
    assert_eq!(
        fs::read_to_string(out_dir.join("resources/base/media/layered_image.json")).unwrap(),
        "{\n  \"layered-image\": {\n    \"background\": \"$media:16777217\",\n    \"foreground\": \"$media:16777218\"\n  }\n}"
    );

    let table = fs::read_to_string(result.resource_table).unwrap();
    assert_eq!(
        table,
        "\
string EntryAbility_desc 0x01000004\n\
string EntryAbility_label 0x01000005\n\
string app_name 0x01000000\n\
string module_desc 0x01000006\n\
color start_window_background 0x01000007\n\
media background 0x01000001\n\
media foreground 0x01000002\n\
media layered_image 0x01000003\n\
media startIcon 0x01000008\n\
profile main_pages 0x01000009"
    );

    let ids: serde_json::Value =
        serde_json::from_str(&fs::read_to_string(result.ids_defined).unwrap()).unwrap();
    assert_eq!(ids["record"].as_array().unwrap().len(), 10);
    assert_eq!(ids["record"][0]["id"], "0x01000000");
    assert_eq!(ids["record"][9]["type"], "profile");
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
