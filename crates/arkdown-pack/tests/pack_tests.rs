//! Integration tests for `arkdown-pack` public API.

use std::fs;
use std::io::Read;
use std::sync::Arc;

use arkdown_core::{
    ArkdownConfig, BuildMode, BuildOption, OutputTarget, ProductConfig, SdkConfig, StrictModeConfig,
};
use arkdown_pack::{HarPackInput, PackError, PackInput, Packager};

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

fn make_config() -> Arc<ArkdownConfig> {
    Arc::new(ArkdownConfig {
        project_root: std::path::PathBuf::from("/tmp"),
        build_mode: BuildMode::Debug,
        output_target: OutputTarget::Hap,
        product: ProductConfig {
            name: "default".to_string(),
            bundle_name: "com.test.packtest".to_string(),
            compatible_sdk_version: "5.0.0(12)".to_string(),
            runtime_os: "HarmonyOS".to_string(),
            signing_config: None,
            build_option: BuildOption {
                strict_mode: StrictModeConfig {
                    case_sensitive_check: false,
                    use_normalized_ohm_url: true,
                },
                ark_options: None,
                res_options: None,
                source_option: None,
            },
        },
        modules: vec![],
        sdk_config: SdkConfig {
            ets_loader_path: "/sdk/ets-loader".into(),
            es2abc_path: "/sdk/es2abc".into(),
            restool_path: "/sdk/restool".into(),
            compile_api_version: 12,
            has_rollup_plugin: true,
            compile_sdk_version: "6.0.0.47".to_string(),
        },
    })
}

fn write_minimal_inputs(
    tmp: &std::path::Path,
) -> (std::path::PathBuf, std::path::PathBuf, std::path::PathBuf) {
    let abc = tmp.join("modules.abc");
    let res = tmp.join("resources.index");
    let modj = tmp.join("module.json");
    fs::write(&abc, b"PAND\x00\x00\x00\x00").unwrap();
    fs::write(&res, b"\xf0\x0f\x12\x34\x00\x00\x00\x00").unwrap();
    fs::write(&modj, r#"{"module":{"name":"entry","type":"entry"}}"#).unwrap();
    (abc, res, modj)
}

fn minimal_pack_input(
    abc: std::path::PathBuf,
    res: std::path::PathBuf,
    modj: std::path::PathBuf,
    out: std::path::PathBuf,
) -> PackInput {
    PackInput {
        module_name: "entry".into(),
        abc_path: abc,
        resources_index: res,
        module_json: modj,
        native_libs: vec![],
        output_path: out,
        resources_src_dir: None,
        bundle_version_code: 1000000,
        bundle_version_name: "1.0.0".to_string(),
        bundle_type: "app".to_string(),
        source_maps_path: None,
        abilities: vec![],
    }
}

// ---------------------------------------------------------------------------
// HAP output format
// ---------------------------------------------------------------------------

#[test]
fn test_hap_output_is_valid_zip() {
    let tmp = tempfile::tempdir().expect("tempdir");
    let (abc, res, modj) = write_minimal_inputs(tmp.path());
    let out = tmp.path().join("entry.hap");

    Packager::new(make_config())
        .pack_hap(minimal_pack_input(abc, res, modj, out.clone()))
        .expect("pack_hap must succeed");

    // Verify ZIP magic bytes: PK\x03\x04
    let bytes = fs::read(&out).expect("read HAP");
    assert_eq!(
        &bytes[0..4],
        b"PK\x03\x04",
        "HAP file must begin with ZIP local file header magic (PK\\x03\\x04)"
    );
}

#[test]
fn test_hap_output_is_readable_by_zip_crate() {
    let tmp = tempfile::tempdir().expect("tempdir");
    let (abc, res, modj) = write_minimal_inputs(tmp.path());
    let out = tmp.path().join("readable.hap");

    Packager::new(make_config())
        .pack_hap(minimal_pack_input(abc, res, modj, out.clone()))
        .expect("pack_hap must succeed");

    let file = fs::File::open(&out).expect("open HAP");
    // zip::ZipArchive will error if the file is not a well-formed ZIP
    let archive = zip::ZipArchive::new(file).expect("HAP must be a valid ZIP archive");
    assert!(archive.len() > 0, "HAP ZIP must contain at least one entry");
}

// ---------------------------------------------------------------------------
// HAP required entries
// ---------------------------------------------------------------------------

#[test]
fn test_hap_contains_module_json() {
    let tmp = tempfile::tempdir().expect("tempdir");
    let (abc, res, modj) = write_minimal_inputs(tmp.path());
    let out = tmp.path().join("entries.hap");

    Packager::new(make_config())
        .pack_hap(minimal_pack_input(abc, res, modj, out.clone()))
        .expect("pack_hap must succeed");

    let file = fs::File::open(&out).expect("open HAP");
    let archive = zip::ZipArchive::new(file).expect("valid ZIP");
    let names: Vec<_> = archive.file_names().collect();
    assert!(
        names.iter().any(|n| *n == "module.json"),
        "HAP must contain 'module.json'; entries: {:?}",
        names
    );
}

#[test]
fn test_hap_contains_pack_info() {
    let tmp = tempfile::tempdir().expect("tempdir");
    let (abc, res, modj) = write_minimal_inputs(tmp.path());
    let out = tmp.path().join("pack_info.hap");

    Packager::new(make_config())
        .pack_hap(minimal_pack_input(abc, res, modj, out.clone()))
        .expect("pack_hap must succeed");

    let file = fs::File::open(&out).expect("open HAP");
    let archive = zip::ZipArchive::new(file).expect("valid ZIP");
    let names: Vec<_> = archive.file_names().collect();
    assert!(
        names.iter().any(|n| *n == "pack.info"),
        "HAP must contain 'pack.info'; entries: {:?}",
        names
    );
}

#[test]
fn test_hap_contains_pkg_context_info() {
    let tmp = tempfile::tempdir().expect("tempdir");
    let (abc, res, modj) = write_minimal_inputs(tmp.path());
    let out = tmp.path().join("pkg_ctx.hap");

    Packager::new(make_config())
        .pack_hap(minimal_pack_input(abc, res, modj, out.clone()))
        .expect("pack_hap must succeed");

    let file = fs::File::open(&out).expect("open HAP");
    let archive = zip::ZipArchive::new(file).expect("valid ZIP");
    let names: Vec<_> = archive.file_names().collect();
    assert!(
        names.iter().any(|n| *n == "pkgContextInfo.json"),
        "HAP must contain 'pkgContextInfo.json'; entries: {:?}",
        names
    );
}

#[test]
fn test_hap_contains_abc_at_ets_path_for_api12() {
    let tmp = tempfile::tempdir().expect("tempdir");
    let (abc, res, modj) = write_minimal_inputs(tmp.path());
    let out = tmp.path().join("abc_path.hap");

    // Default config uses compile_api_version = 12
    Packager::new(make_config())
        .pack_hap(minimal_pack_input(abc, res, modj, out.clone()))
        .expect("pack_hap must succeed");

    let file = fs::File::open(&out).expect("open HAP");
    let archive = zip::ZipArchive::new(file).expect("valid ZIP");
    let names: Vec<_> = archive.file_names().collect();
    assert!(
        names.iter().any(|n| *n == "ets/modules.abc"),
        "API 12 HAP must store the ABC at 'ets/modules.abc'; entries: {:?}",
        names
    );
}

#[test]
fn test_hap_contains_resources_index() {
    let tmp = tempfile::tempdir().expect("tempdir");
    let (abc, res, modj) = write_minimal_inputs(tmp.path());
    let out = tmp.path().join("res_idx.hap");

    Packager::new(make_config())
        .pack_hap(minimal_pack_input(abc, res, modj, out.clone()))
        .expect("pack_hap must succeed");

    let file = fs::File::open(&out).expect("open HAP");
    let archive = zip::ZipArchive::new(file).expect("valid ZIP");
    let names: Vec<_> = archive.file_names().collect();
    assert!(
        names.iter().any(|n| n.contains("resources")),
        "HAP must contain a resources entry; entries: {:?}",
        names
    );
}

#[test]
fn test_hap_resource_files_copied_from_processed_dir() {
    let tmp = tempfile::tempdir().expect("tempdir");
    let (abc, res, modj) = write_minimal_inputs(tmp.path());
    let out = tmp.path().join("res_copy.hap");

    let media_dir = tmp.path().join("compiled/resources/base/media");
    let profile_dir = tmp.path().join("compiled/resources/base/profile");
    fs::create_dir_all(&media_dir).unwrap();
    fs::create_dir_all(&profile_dir).unwrap();
    fs::write(
        media_dir.join("layered_image.json"),
        b"{\n  \"layered-image\": {\n    \"background\": \"$media:16777217\"\n  }\n}",
    )
    .unwrap();
    fs::write(
        profile_dir.join("main_pages.json"),
        b"{\n  \"src\": [\"pages/Index\"]\n}",
    )
    .unwrap();

    Packager::new(make_config())
        .pack_hap(PackInput {
            module_name: "entry".into(),
            abc_path: abc,
            resources_index: res,
            module_json: modj,
            native_libs: vec![],
            output_path: out.clone(),
            resources_src_dir: Some(tmp.path().join("compiled/resources")),
            bundle_version_code: 1000000,
            bundle_version_name: "1.0.0".to_string(),
            bundle_type: "app".to_string(),
            source_maps_path: None,
            abilities: vec![],
        })
        .expect("pack_hap must succeed");

    let file = fs::File::open(&out).expect("open HAP");
    let mut archive = zip::ZipArchive::new(file).expect("valid ZIP");
    let names: Vec<_> = archive.file_names().map(ToOwned::to_owned).collect();
    assert!(
        names
            .iter()
            .any(|n| n == "resources/base/media/layered_image.json"),
        "HAP must contain processed layered image JSON; entries: {:?}",
        names
    );
    assert!(
        names.iter().all(|n| !n.contains("/element/")),
        "HAP must not include raw element JSON files; entries: {:?}",
        names
    );

    let mut layered = String::new();
    archive
        .by_name("resources/base/media/layered_image.json")
        .unwrap()
        .read_to_string(&mut layered)
        .unwrap();
    assert_eq!(
        layered,
        r#"{"layered-image":{"background":"$media:16777217"}}"#
    );
}

// ---------------------------------------------------------------------------
// Error cases
// ---------------------------------------------------------------------------

#[test]
fn test_missing_abc_returns_missing_input_error() {
    let tmp = tempfile::tempdir().expect("tempdir");
    let res = tmp.path().join("resources.index");
    let modj = tmp.path().join("module.json");
    fs::write(&res, b"\x00").unwrap();
    fs::write(&modj, r#"{"module":{"name":"x","type":"entry"}}"#).unwrap();

    let result = Packager::new(make_config()).pack_hap(PackInput {
        module_name: "entry".into(),
        abc_path: tmp.path().join("missing.abc"), // does not exist
        resources_index: res,
        module_json: modj,
        native_libs: vec![],
        output_path: tmp.path().join("out.hap"),
        resources_src_dir: None,
        bundle_version_code: 1000000,
        bundle_version_name: "1.0.0".to_string(),
        bundle_type: "app".to_string(),
        source_maps_path: None,
        abilities: vec![],
    });
    assert!(result.is_err(), "missing ABC must return Err");
    assert!(
        matches!(result.unwrap_err(), PackError::MissingInput(_)),
        "error must be PackError::MissingInput"
    );
}

#[test]
fn test_missing_module_json_returns_missing_input_error() {
    let tmp = tempfile::tempdir().expect("tempdir");
    let result = Packager::new(make_config()).pack_hap(PackInput {
        module_name: "entry".into(),
        abc_path: tmp.path().join("m.abc"),
        resources_index: tmp.path().join("m.index"),
        module_json: tmp.path().join("m.json"), // does not exist
        native_libs: vec![],
        output_path: tmp.path().join("out.hap"),
        resources_src_dir: None,
        bundle_version_code: 1000000,
        bundle_version_name: "1.0.0".to_string(),
        bundle_type: "app".to_string(),
        source_maps_path: None,
        abilities: vec![],
    });
    assert!(result.is_err());
    assert!(matches!(result.unwrap_err(), PackError::MissingInput(_)));
}

// ---------------------------------------------------------------------------
// HSP packing
// ---------------------------------------------------------------------------

#[test]
fn test_hsp_output_is_valid_zip() {
    let tmp = tempfile::tempdir().expect("tempdir");
    let (abc, res, modj) = write_minimal_inputs(tmp.path());
    let out = tmp.path().join("shared.hsp");
    // Rewrite module.json with type "shared"
    fs::write(&modj, r#"{"module":{"name":"shared","type":"shared"}}"#).unwrap();

    Packager::new(make_config())
        .pack_hsp(PackInput {
            module_name: "shared".into(),
            abc_path: abc,
            resources_index: res,
            module_json: modj,
            native_libs: vec![],
            output_path: out.clone(),
            resources_src_dir: None,
            bundle_version_code: 1000000,
            bundle_version_name: "1.0.0".to_string(),
            bundle_type: "app".to_string(),
            source_maps_path: None,
            abilities: vec![],
        })
        .expect("pack_hsp must succeed");

    let bytes = fs::read(&out).expect("read HSP");
    assert_eq!(&bytes[0..4], b"PK\x03\x04", "HSP must be a ZIP file");
}

// ---------------------------------------------------------------------------
// HAR packing
// ---------------------------------------------------------------------------

#[test]
fn test_har_contains_ets_source_files() {
    let tmp = tempfile::tempdir().expect("tempdir");
    let src_dir = tmp.path().join("src/main/ets");
    fs::create_dir_all(&src_dir).unwrap();
    fs::write(src_dir.join("Index.ets"), b"export struct Index {}").unwrap();
    fs::write(src_dir.join("utils.ts"), b"export function noop() {}").unwrap();

    let pkg_json = tmp.path().join("oh-package.json5");
    fs::write(&pkg_json, r#"{"name":"mylib","version":"1.0.0"}"#).unwrap();

    let out = tmp.path().join("mylib.har");
    Packager::new(make_config())
        .pack_har(HarPackInput {
            module_name: "mylib".into(),
            source_dir: src_dir,
            resources_dir: tmp.path().join("src/main/resources"),
            oh_package_json: pkg_json,
            output_path: out.clone(),
        })
        .expect("pack_har must succeed");

    let file = fs::File::open(&out).expect("open HAR");
    let archive = zip::ZipArchive::new(file).expect("valid ZIP");
    let names: Vec<_> = archive.file_names().collect();
    assert!(
        names.iter().any(|n| n.ends_with("Index.ets")),
        "HAR must contain Index.ets; entries: {:?}",
        names
    );
    assert!(
        names.iter().any(|n| n.ends_with("utils.ts")),
        "HAR must contain utils.ts; entries: {:?}",
        names
    );
}

#[test]
fn test_har_contains_oh_package_json5() {
    let tmp = tempfile::tempdir().expect("tempdir");
    let src_dir = tmp.path().join("src/main/ets");
    fs::create_dir_all(&src_dir).unwrap();
    fs::write(src_dir.join("Entry.ets"), b"export class Entry {}").unwrap();

    let pkg_json = tmp.path().join("oh-package.json5");
    fs::write(&pkg_json, r#"{"name":"lib","version":"0.1.0"}"#).unwrap();

    let out = tmp.path().join("lib.har");
    Packager::new(make_config())
        .pack_har(HarPackInput {
            module_name: "lib".into(),
            source_dir: src_dir,
            resources_dir: tmp.path().join("src/main/resources"),
            oh_package_json: pkg_json,
            output_path: out.clone(),
        })
        .expect("pack_har must succeed");

    let file = fs::File::open(&out).expect("open HAR");
    let archive = zip::ZipArchive::new(file).expect("valid ZIP");
    let names: Vec<_> = archive.file_names().collect();
    assert!(
        names.iter().any(|n| *n == "oh-package.json5"),
        "HAR must contain oh-package.json5; entries: {:?}",
        names
    );
}
