//! Integration tests for `arkdown-pack` public API.

use std::fs;
use std::io::Read;
use std::sync::Arc;

use arkdown_core::{
    ArkdownConfig, BuildMode, BuildOption, OutputTarget, ProductConfig, SdkConfig, StrictModeConfig,
};
use arkdown_pack::{
    normalize_integrated_hsps, HarPackInput, IntegratedHspEntry, PackError, PackInput, Packager,
};

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
            compatible_sdk_version_stage: None,
            runtime_os: "HarmonyOS".to_string(),
            signing_config: None,
            build_option: BuildOption {
                preload_system_so: false,
                preload_so: vec![],
                remove_permissions: vec![],
                aot_compile_mode: None,
                host_pgo: None,
                ap_path: None,
                abi_filters: vec![],
                strict_mode: StrictModeConfig {
                    case_sensitive_check: false,
                    use_normalized_ohm_url: true,
                },
                ark_options: None,
                res_options: None,
                source_option: None,
            },
            version_code: None,
            version_name: None,
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
        integrated_hsps: vec![],
        privacy_profile_path: None,
        device_types: vec![],
        installation_free: false,
        delivery_with_install: true,
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
fn test_hap_does_not_package_pkg_context_info_for_hap() {
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
        !names.iter().any(|n| *n == "pkgContextInfo.json"),
        "HAP must not package 'pkgContextInfo.json'; entries: {:?}",
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
fn test_hap_contains_profile_guided_ap_from_hvigor_intermediate() {
    let tmp = tempfile::tempdir().expect("tempdir");
    let module_root = tmp.path().join("entry");
    fs::create_dir_all(&module_root).expect("create module root");
    let (abc, res, modj) = write_minimal_inputs(&module_root);
    let ap_dir = module_root.join("build/default/intermediates/ap");
    fs::create_dir_all(&ap_dir).expect("create ap dir");
    fs::write(ap_dir.join("entry.ap"), b"profile guided data").expect("write ap");
    let out = module_root.join("build/default/outputs/default/entry-default-unsigned.hap");

    Packager::new(make_config())
        .pack_hap(minimal_pack_input(abc, res, modj, out.clone()))
        .expect("pack_hap must succeed");

    let file = fs::File::open(&out).expect("open HAP");
    let mut archive = zip::ZipArchive::new(file).expect("valid ZIP");
    let mut entry = archive.by_name("ap/entry.ap").expect("embedded ap");
    let mut contents = Vec::new();
    entry.read_to_end(&mut contents).expect("read embedded ap");
    assert_eq!(contents, b"profile guided data");
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
            integrated_hsps: vec![],
            privacy_profile_path: None,
            device_types: vec![],
            installation_free: false,
            delivery_with_install: true,
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
        integrated_hsps: vec![],
        privacy_profile_path: None,
        device_types: vec![],
        installation_free: false,
        delivery_with_install: true,
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
        integrated_hsps: vec![],
        privacy_profile_path: None,
        device_types: vec![],
        installation_free: false,
        delivery_with_install: true,
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
            integrated_hsps: vec![],
            privacy_profile_path: None,
            device_types: vec![],
            installation_free: false,
            delivery_with_install: true,
        })
        .expect("pack_hsp must succeed");

    let bytes = fs::read(&out).expect("read HSP");
    assert_eq!(&bytes[0..4], b"PK\x03\x04", "HSP must be a ZIP file");
}

#[test]
fn test_hsp_pack_info_uses_shared_bundle_type() {
    let tmp = tempfile::tempdir().expect("tempdir");
    let (abc, res, modj) = write_minimal_inputs(tmp.path());
    let out = tmp.path().join("shared_pack_info.hsp");
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
            integrated_hsps: vec![],
            privacy_profile_path: None,
            device_types: vec!["phone".to_string()],
            installation_free: false,
            delivery_with_install: true,
        })
        .expect("pack_hsp must succeed");

    let file = fs::File::open(&out).expect("open HSP");
    let mut archive = zip::ZipArchive::new(file).expect("valid ZIP");
    let mut entry = archive.by_name("pack.info").expect("pack.info");
    let mut content = String::new();
    entry.read_to_string(&mut content).unwrap();
    let pack_info: serde_json::Value = serde_json::from_str(&content).unwrap();

    assert_eq!(pack_info["summary"]["app"]["bundleType"], "shared");
    assert_eq!(
        pack_info["summary"]["modules"][0]["distro"]["moduleType"],
        "shared"
    );
    assert_eq!(pack_info["packages"][0]["moduleType"], "shared");
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
            resource_str_dir: None,
            oh_package_json: pkg_json,
            output_path: out.clone(),
            declarations_dir: None,
            consumer_rules_path: None,
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
            resource_str_dir: None,
            oh_package_json: pkg_json,
            output_path: out.clone(),
            declarations_dir: None,
            consumer_rules_path: None,
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

#[test]
fn test_har_contains_permission_reason_resource_str() {
    let tmp = tempfile::tempdir().expect("tempdir");
    let src_dir = tmp.path().join("src/main/ets");
    fs::create_dir_all(&src_dir).unwrap();
    fs::write(src_dir.join("Entry.ets"), b"export class Entry {}").unwrap();

    let resource_str_dir = tmp.path().join("resource_str");
    fs::create_dir_all(resource_str_dir.join("base/element")).unwrap();
    fs::write(
        resource_str_dir.join("base/element/string.json"),
        r#"{"string":[{"name":"camera_reason","value":"Need camera"}]}"#,
    )
    .unwrap();

    let pkg_json = tmp.path().join("oh-package.json5");
    fs::write(&pkg_json, r#"{"name":"lib","version":"0.1.0"}"#).unwrap();

    let out = tmp.path().join("lib.har");
    Packager::new(make_config())
        .pack_har(HarPackInput {
            module_name: "lib".into(),
            source_dir: src_dir,
            resources_dir: tmp.path().join("src/main/resources"),
            resource_str_dir: Some(resource_str_dir),
            oh_package_json: pkg_json,
            output_path: out.clone(),
            declarations_dir: None,
            consumer_rules_path: None,
        })
        .expect("pack_har must succeed");

    let file = fs::File::open(&out).expect("open HAR");
    let mut archive = zip::ZipArchive::new(file).expect("valid ZIP");
    let names: Vec<_> = archive.file_names().map(str::to_string).collect();
    assert!(
        names
            .iter()
            .any(|n| n == "src/main/resource/base/element/string.json"),
        "HAR must contain permission reason resource_str; entries: {:?}",
        names
    );

    let mut content = String::new();
    archive
        .by_name("src/main/resource/base/element/string.json")
        .expect("resource_str entry")
        .read_to_string(&mut content)
        .unwrap();
    assert_eq!(
        content,
        r#"{"string":[{"name":"camera_reason","value":"Need camera"}]}"#
    );
}

// ---------------------------------------------------------------------------
// Integrated HSP (SPEC-044)
// ---------------------------------------------------------------------------

/// Build a minimal `.hsp` zip in memory and write it to `path`.
fn write_minimal_hsp(path: &std::path::Path, bundle_name: &str, version_code: u64) {
    use std::io::Write as _;
    use zip::write::SimpleFileOptions;
    use zip::CompressionMethod;

    let pack_info = serde_json::json!({
        "summary": {
            "app": {
                "bundleName": bundle_name,
                "bundleType": "shared",
                "version": { "code": version_code, "name": "1.0.0" }
            },
            "modules": []
        },
        "packages": []
    })
    .to_string();

    let buf = Vec::new();
    let cursor = std::io::Cursor::new(buf);
    let mut zw = zip::ZipWriter::new(cursor);

    let opts = SimpleFileOptions::default().compression_method(CompressionMethod::Deflated);
    zw.start_file("pack.info", opts).unwrap();
    zw.write_all(pack_info.as_bytes()).unwrap();

    let opts = SimpleFileOptions::default().compression_method(CompressionMethod::Stored);
    zw.start_file("ets/modules.abc", opts).unwrap();
    zw.write_all(b"PAND").unwrap();

    let bytes = zw.finish().unwrap().into_inner();
    fs::write(path, bytes).unwrap();
}

#[test]
fn test_normalize_integrated_hsps_patches_pack_info() {
    let tmp = tempfile::tempdir().expect("tempdir");
    let hsp_path = tmp.path().join("mylib.hsp");
    write_minimal_hsp(&hsp_path, "com.remote.hsp", 1);

    let entry = IntegratedHspEntry {
        hsp_dir_name: "mylib@1.0.0".to_string(),
        hsp_file_name: "mylib.hsp".to_string(),
        hsp_path,
    };

    let out_dir = tmp.path().join("integrated");
    let paths = normalize_integrated_hsps(&[entry], "com.host.app", 2000000, &out_dir).unwrap();

    assert_eq!(paths.len(), 1);
    let out_path = &paths[0];
    assert_eq!(out_path, &out_dir.join("mylib@1.0.0").join("mylib.hsp"));

    let out_file = fs::File::open(out_path).unwrap();
    let mut archive = zip::ZipArchive::new(out_file).unwrap();
    let mut entry = archive.by_name("pack.info").unwrap();
    let mut content = String::new();
    entry.read_to_string(&mut content).unwrap();
    let v: serde_json::Value = serde_json::from_str(&content).unwrap();

    assert_eq!(
        v["summary"]["app"]["bundleName"].as_str().unwrap(),
        "com.host.app",
        "bundleName must be patched to host value"
    );
    assert_eq!(
        v["summary"]["app"]["version"]["code"].as_u64().unwrap(),
        2000000,
        "version.code must be patched to host value"
    );
}

#[test]
fn test_pack_hap_embeds_integrated_hsp() {
    let tmp = tempfile::tempdir().expect("tempdir");
    let (abc, res, modj) = write_minimal_inputs(tmp.path());
    let out = tmp.path().join("entry.hap");

    // Build and normalize the remote HSP.
    let hsp_src = tmp.path().join("remote.hsp");
    write_minimal_hsp(&hsp_src, "com.remote.hsp", 1);
    let integrated_dir = tmp.path().join("integrated");
    let normalized = normalize_integrated_hsps(
        &[IntegratedHspEntry {
            hsp_dir_name: "remote@1.0.0".to_string(),
            hsp_file_name: "remote.hsp".to_string(),
            hsp_path: hsp_src,
        }],
        "com.test.packtest",
        1000000,
        &integrated_dir,
    )
    .unwrap();

    let integrated_entry = IntegratedHspEntry {
        hsp_dir_name: "remote@1.0.0".to_string(),
        hsp_file_name: "remote.hsp".to_string(),
        hsp_path: normalized[0].clone(),
    };

    Packager::new(make_config())
        .pack_hap(PackInput {
            module_name: "entry".into(),
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
            integrated_hsps: vec![integrated_entry],
            privacy_profile_path: None,
            device_types: vec![],
            installation_free: false,
            delivery_with_install: true,
        })
        .expect("pack_hap must succeed");

    let file = fs::File::open(&out).expect("open HAP");
    let archive = zip::ZipArchive::new(file).expect("valid ZIP");
    let names: Vec<_> = archive.file_names().collect();

    assert!(
        names.iter().any(|n| *n == "libs/remote@1.0.0/remote.hsp"),
        "HAP must contain embedded HSP at libs/<hsp_dir_name>/<hsp_file_name>; entries: {:?}",
        names
    );
    // Standard HAP entries must still be present.
    assert!(
        names.iter().any(|n| *n == "module.json"),
        "module.json missing"
    );
    assert!(
        names.iter().any(|n| *n == "ets/modules.abc"),
        "ets/modules.abc missing"
    );
}

#[test]
fn test_pack_hap_empty_integrated_hsps_no_libs_entry() {
    let tmp = tempfile::tempdir().expect("tempdir");
    let (abc, res, modj) = write_minimal_inputs(tmp.path());
    let out = tmp.path().join("entry.hap");

    Packager::new(make_config())
        .pack_hap(minimal_pack_input(abc, res, modj, out.clone()))
        .expect("pack_hap must succeed");

    let file = fs::File::open(&out).expect("open HAP");
    let archive = zip::ZipArchive::new(file).expect("valid ZIP");
    let names: Vec<_> = archive.file_names().collect();

    assert!(
        !names
            .iter()
            .any(|n| n.starts_with("libs/") && n.ends_with(".hsp")),
        "HAP with no integrated_hsps must not contain any .hsp under libs/; entries: {:?}",
        names
    );
}

/// Verify that declaration stubs (`index.d.ts`, `index.d.ets`) from the
/// declarations directory are included in the HAR archive at the root level.
///
/// hvigor calls `processDeclarationEntry()` before packing and places the
/// resulting stubs at the archive root so consumers can use `types` to
/// resolve type information without running `tsc`.
#[test]
fn test_har_includes_declaration_stubs() {
    let tmp = tempfile::tempdir().expect("tempdir");
    let src_dir = tmp.path().join("src/main/ets");
    fs::create_dir_all(&src_dir).unwrap();
    fs::write(src_dir.join("Index.ets"), b"export class Index {}").unwrap();

    let declarations_dir = tmp.path().join("declarations");
    fs::create_dir_all(&declarations_dir).unwrap();
    fs::write(
        declarations_dir.join("index.d.ts"),
        b"export declare class Index {}\n",
    )
    .unwrap();
    fs::write(
        declarations_dir.join("index.d.ets"),
        b"export declare class Index {}\n",
    )
    .unwrap();
    // Add an unrelated file that should NOT appear.
    fs::write(declarations_dir.join("other.json"), b"{}").unwrap();

    let pkg_json = tmp.path().join("oh-package.json5");
    fs::write(&pkg_json, r#"{"name":"lib"}"#).unwrap();

    let out = tmp.path().join("lib.har");
    Packager::new(make_config())
        .pack_har(HarPackInput {
            module_name: "lib".into(),
            source_dir: src_dir,
            resources_dir: tmp.path().join("src/main/resources"),
            resource_str_dir: None,
            oh_package_json: pkg_json,
            output_path: out.clone(),
            declarations_dir: Some(declarations_dir),
            consumer_rules_path: None,
        })
        .expect("pack_har must succeed");

    let file = fs::File::open(&out).expect("open HAR");
    let archive = zip::ZipArchive::new(file).expect("valid ZIP");
    let names: Vec<_> = archive.file_names().collect();

    assert!(
        names.iter().any(|n| *n == "index.d.ts"),
        "HAR must contain index.d.ts declaration stub; entries: {:?}",
        names
    );
    assert!(
        names.iter().any(|n| *n == "index.d.ets"),
        "HAR must contain index.d.ets declaration stub; entries: {:?}",
        names
    );
    assert!(
        !names.iter().any(|n| *n == "other.json"),
        "HAR must not contain non-declaration files from declarations dir; entries: {:?}",
        names
    );
}

/// Verify that consumer obfuscation rules are included in the HAR archive
/// at the root level as `consumer-rules.txt`.
///
/// hvigor calls `generateConsumerObConfigFile()` and includes the rules so
/// downstream consumers can extend their own obfuscation configuration.
#[test]
fn test_har_includes_consumer_rules() {
    let tmp = tempfile::tempdir().expect("tempdir");
    let src_dir = tmp.path().join("src/main/ets");
    fs::create_dir_all(&src_dir).unwrap();
    fs::write(src_dir.join("Index.ets"), b"export class Index {}").unwrap();

    let consumer_rules = tmp.path().join("consumer-rules.txt");
    fs::write(
        &consumer_rules,
        b"-keep public class com.example.lib.*\n",
    )
    .unwrap();

    let pkg_json = tmp.path().join("oh-package.json5");
    fs::write(&pkg_json, r#"{"name":"lib"}"#).unwrap();

    let out = tmp.path().join("lib.har");
    Packager::new(make_config())
        .pack_har(HarPackInput {
            module_name: "lib".into(),
            source_dir: src_dir,
            resources_dir: tmp.path().join("src/main/resources"),
            resource_str_dir: None,
            oh_package_json: pkg_json,
            output_path: out.clone(),
            declarations_dir: None,
            consumer_rules_path: Some(consumer_rules),
        })
        .expect("pack_har must succeed");

    let file = fs::File::open(&out).expect("open HAR");
    let mut archive = zip::ZipArchive::new(file).expect("valid ZIP");
    let names: Vec<_> = archive.file_names().map(str::to_string).collect();

    assert!(
        names.iter().any(|n| n == "consumer-rules.txt"),
        "HAR must contain consumer-rules.txt; entries: {:?}",
        names
    );

    let mut content = String::new();
    archive
        .by_name("consumer-rules.txt")
        .expect("consumer-rules.txt entry")
        .read_to_string(&mut content)
        .expect("read consumer-rules.txt");
    assert!(
        content.contains("-keep public class"),
        "consumer-rules.txt must contain keep rules; got: {:?}",
        content
    );
}

/// Verify that when declaration stubs exist, `oh-package.json5` in the HAR
/// archive has a `"types": "index.d.ts"` field added.
///
/// hvigor's `processDeclarationEntry()` writes this so downstream consumers can
/// locate TypeScript declaration stubs without guessing the path.
#[test]
fn test_har_oh_package_json5_has_types_field_when_declarations_present() {
    let tmp = tempfile::tempdir().expect("tempdir");
    let src_dir = tmp.path().join("src/main/ets");
    fs::create_dir_all(&src_dir).unwrap();
    fs::write(src_dir.join("Index.ets"), b"export class Index {}").unwrap();

    // Create declaration directory with stubs.
    let declarations_dir = tmp.path().join("declarations");
    fs::create_dir_all(&declarations_dir).unwrap();
    fs::write(
        declarations_dir.join("index.d.ts"),
        b"export declare class Index {}\n",
    )
    .unwrap();
    fs::write(
        declarations_dir.join("index.d.ets"),
        b"export declare class Index {}\n",
    )
    .unwrap();

    let pkg_json = tmp.path().join("oh-package.json5");
    // Source oh-package.json5 without types field.
    fs::write(
        &pkg_json,
        r#"{ "name": "mylib", "version": "1.0.0", "main": "src/main/ets/Index.ets" }"#,
    )
    .unwrap();

    let out = tmp.path().join("mylib.har");
    Packager::new(make_config())
        .pack_har(HarPackInput {
            module_name: "mylib".into(),
            source_dir: src_dir,
            resources_dir: tmp.path().join("src/main/resources"),
            resource_str_dir: None,
            oh_package_json: pkg_json,
            output_path: out.clone(),
            declarations_dir: Some(declarations_dir),
            consumer_rules_path: None,
        })
        .expect("pack_har must succeed");

    let file = fs::File::open(&out).expect("open HAR");
    let mut archive = zip::ZipArchive::new(file).expect("valid ZIP");

    // Read the oh-package.json5 from the archive.
    let mut content = String::new();
    archive
        .by_name("oh-package.json5")
        .expect("oh-package.json5 entry")
        .read_to_string(&mut content)
        .expect("read oh-package.json5");

    let pkg: serde_json::Value = serde_json::from_str(&content).expect("parse oh-package.json5");
    assert_eq!(
        pkg.get("types").and_then(|v| v.as_str()),
        Some("index.d.ts"),
        "oh-package.json5 must have types: 'index.d.ts' when declarations are present; got: {}",
        content
    );
    assert_eq!(
        pkg.get("name").and_then(|v| v.as_str()),
        Some("mylib"),
        "name field must be preserved"
    );
}

/// Verify `clean_har_sources` (hvigor `cleanEtsForTgz`): produces a cleaned HAR
/// directory where original .ets/.ts sources are replaced by declaration stubs.
///
/// Addresses section 3.8: 'cleanEtsForTgz' gap
#[test]
fn test_clean_har_sources_removes_ets_adds_declarations() {
    let tmp = tempfile::tempdir().expect("tempdir");
    let src_dir = tmp.path().join("src/main/ets");
    fs::create_dir_all(&src_dir).unwrap();
    fs::write(src_dir.join("Index.ets"), b"export class Index {}").unwrap();
    fs::write(src_dir.join("Utils.ts"), b"export function util() {}").unwrap();
    // A JSON config file should be preserved.
    let cfg_dir = tmp.path().join("src/main/resources/base/profile");
    fs::create_dir_all(&cfg_dir).unwrap();
    fs::write(cfg_dir.join("main_pages.json"), b"{\"src\":[]}").unwrap();

    // Declarations directory with stubs.
    let decl_dir = tmp.path().join("declarations");
    fs::create_dir_all(&decl_dir).unwrap();
    fs::write(decl_dir.join("index.d.ts"), b"export declare class Index {}").unwrap();
    fs::write(decl_dir.join("index.d.ets"), b"export declare class Index {}").unwrap();

    let output_dir = tmp.path().join("cleaned");
    let cleaned_ets = Packager::clean_har_sources(
        &tmp.path().join("src/main"),
        Some(&decl_dir),
        &output_dir,
    )
    .expect("clean_har_sources must succeed");

    // The cleaned/ets directory should contain declaration stubs.
    assert!(cleaned_ets.join("index.d.ts").exists(), "index.d.ts must be in cleaned ets");
    assert!(cleaned_ets.join("index.d.ets").exists(), "index.d.ets must be in cleaned ets");

    // Original .ets/.ts files must NOT be present.
    assert!(
        !output_dir.join("ets/Index.ets").exists(),
        "original .ets source must be removed (cleanEtsForTgz)"
    );
    assert!(
        !output_dir.join("ets/Utils.ts").exists(),
        "original .ts source must be removed (cleanEtsForTgz)"
    );
}
