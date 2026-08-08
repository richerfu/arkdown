//! Integration tests for `arkdown-pack` public API.

use std::fs;
use std::io::{Read, Write};
use std::sync::Arc;

use arkdown_core::{
    ArkdownConfig, BuildMode, BuildOption, HarArtifactType, NativeLibOption, OutputTarget,
    ProductConfig, SdkConfig, StrictModeConfig,
};
use arkdown_pack::{
    normalize_integrated_hsps, AppArchiveUnpacker, AppArtifactInput, AppArtifactStager,
    AppNormalizePlan, AppPackagePlan, AppPackageRunner, BundledHarResourceInput,
    HarBundledDependencyInput, HarPackInput, HarPackageParameterInput, HarVariantInput,
    HspInterfaceHarInput, HspTgzInput, IntegratedHspEntry, PackError, PackInput, Packager,
    ProjectPackInfoBuilder, RemoteHspCollector, RemoteHspDependencyScope,
};
use arkdown_path::{ArtifactPath, HostPlatform};
use flate2::write::DeflateEncoder;
use indexmap::IndexMap;

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
            artifact_name: None,
            bundle_name: "com.test.packtest".to_string(),
            compatible_sdk_version: "5.0.0(12)".to_string(),
            compatible_sdk_version_stage: None,
            target_sdk_version: None,
            runtime_os: "HarmonyOS".to_string(),
            signing_config: None,
            build_option: BuildOption {
                preload_system_so: false,
                preload_so: vec![],
                deduplicate_har: false,
                enable_source_code_check: true,
            app_with_signed_pkg: false,
                generate_shared_tgz: None,
                customized_har_base_package: None,
                remove_permissions: vec![],
                aot_compile_mode: None,
                host_pgo: None,
                ap_path: None,
                abi_filters: vec![],
                external_native_options: None,
                har_artifact_type: arkdown_core::HarArtifactType::Original,
                native_lib: NativeLibOption::default(),
                strict_mode: StrictModeConfig {
                    case_sensitive_check: false,
                    use_normalized_ohm_url: true,
                    duplicate_dependency_check: false,
                },
                ark_options: None,
                res_options: None,
                source_option: None,
            },
            version_code: None,
            version_name: None,
            build_version: None,
            app_options: Default::default(),
        },
        modules: vec![],
        sdk_config: SdkConfig {
            ets_loader_path: "/sdk/ets-loader".into(),
            es2abc_path: "/sdk/es2abc".into(),
            restool_path: "/sdk/restool".into(),
            compile_api_version: 12,
            sdk_component_api_version: 12,
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

fn rawzip_deflated_len_for_level(content: &[u8], level: flate2::Compression) -> usize {
    let mut encoder = DeflateEncoder::new(Vec::new(), level);
    encoder.write_all(content).unwrap();
    encoder.flush().unwrap();
    encoder.finish().unwrap().len()
}

fn read_har_entries(path: &std::path::Path) -> std::collections::BTreeMap<String, Vec<u8>> {
    let file = fs::File::open(path).expect("open HAR");
    let gzip = flate2::read::GzDecoder::new(file);
    let mut archive = tar::Archive::new(gzip);
    archive
        .entries()
        .expect("valid TAR entries")
        .map(|entry| {
            let mut entry = entry.expect("valid TAR entry");
            let path = entry
                .path()
                .expect("valid TAR path")
                .to_string_lossy()
                .into_owned();
            let mut bytes = Vec::new();
            entry.read_to_end(&mut bytes).expect("read TAR entry");
            (path, bytes)
        })
        .collect()
}

fn read_har_entry_names(path: &std::path::Path) -> Vec<String> {
    let file = fs::File::open(path).expect("open HAR");
    let gzip = flate2::read::GzDecoder::new(file);
    let mut archive = tar::Archive::new(gzip);
    archive
        .entries()
        .expect("valid TAR entries")
        .map(|entry| {
            entry
                .expect("valid TAR entry")
                .path()
                .expect("valid TAR path")
                .to_string_lossy()
                .into_owned()
        })
        .collect()
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
        pack_info_module_json: None,
        native_libs: vec![],
        compress_native_libs: false,
        output_path: out,
        resources_src_dir: None,
        bundle_version_code: 1000000,
        bundle_version_name: "1.0.0".to_string(),
        bundle_build_version: None,
        bundle_type: "app".to_string(),
        source_maps_path: None,
        pkg_context_info_path: None,
        pkg_sdk_info_path: None,
        abilities: vec![],
        integrated_hsps: vec![],
        privacy_profile_path: None,
        syscap_path: None,
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

#[test]
fn test_pack_info_matches_hvigor_optional_build_version_contract() {
    let tmp = tempfile::tempdir().expect("tempdir");
    let (abc, res, modj) = write_minimal_inputs(tmp.path());

    let with_build_path = tmp.path().join("with-build-version.hap");
    let mut with_build = minimal_pack_input(
        abc.clone(),
        res.clone(),
        modj.clone(),
        with_build_path.clone(),
    );
    with_build.bundle_build_version = Some("2026.07.23".to_string());
    Packager::new(make_config())
        .pack_hap(with_build)
        .expect("pack HAP with buildVersion");

    let file = fs::File::open(&with_build_path).expect("open HAP with buildVersion");
    let mut archive = zip::ZipArchive::new(file).expect("valid ZIP");
    let mut entry = archive.by_name("pack.info").expect("pack.info");
    let mut content = String::new();
    entry.read_to_string(&mut content).expect("read pack.info");
    let pack_info: serde_json::Value = serde_json::from_str(&content).expect("parse pack.info");
    assert_eq!(
        pack_info["summary"]["app"]["version"]["build"].as_str(),
        Some("2026.07.23")
    );

    let without_build_path = tmp.path().join("without-build-version.hap");
    Packager::new(make_config())
        .pack_hap(minimal_pack_input(
            abc,
            res,
            modj,
            without_build_path.clone(),
        ))
        .expect("pack HAP without buildVersion");

    let file = fs::File::open(&without_build_path).expect("open HAP without buildVersion");
    let mut archive = zip::ZipArchive::new(file).expect("valid ZIP");
    let mut entry = archive.by_name("pack.info").expect("pack.info");
    let mut content = String::new();
    entry.read_to_string(&mut content).expect("read pack.info");
    let pack_info: serde_json::Value = serde_json::from_str(&content).expect("parse pack.info");
    assert!(
        pack_info["summary"]["app"]["version"]
            .get("build")
            .is_none(),
        "hvigor omits summary.app.version.build when buildVersion is undefined"
    );
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
fn test_hap_metadata_entries_are_stored() {
    let tmp = tempfile::tempdir().expect("tempdir");
    let (abc, res, modj) = write_minimal_inputs(tmp.path());
    let out = tmp.path().join("stored_metadata.hap");

    Packager::new(make_config())
        .pack_hap(minimal_pack_input(abc, res, modj, out.clone()))
        .expect("pack_hap must succeed");

    let file = fs::File::open(&out).expect("open HAP");
    let mut archive = zip::ZipArchive::new(file).expect("valid ZIP");
    assert_eq!(
        archive.by_name("module.json").unwrap().compression(),
        zip::CompressionMethod::Stored
    );
    assert_eq!(
        archive.by_name("pack.info").unwrap().compression(),
        zip::CompressionMethod::Stored
    );
}

#[test]
fn test_hap_deflates_native_libs_when_compress_native_libs_enabled() {
    let tmp = tempfile::tempdir().expect("tempdir");
    let (abc, res, modj) = write_minimal_inputs(tmp.path());
    let out = tmp.path().join("native_deflated.hap");
    let so_dir = tmp.path().join("src/main/libs/arm64-v8a");
    fs::create_dir_all(&so_dir).unwrap();
    let so = so_dir.join("libnative.so");
    let second_so = so_dir.join("libnative2.so");
    let first_payload = b"native payload one".repeat(64);
    fs::write(&so, &first_payload).unwrap();
    fs::write(&second_so, b"native payload two".repeat(64)).unwrap();

    let mut input = minimal_pack_input(abc, res, modj, out.clone());
    input.native_libs = vec![so, second_so];
    input.compress_native_libs = true;
    Packager::new(make_config())
        .pack_hap(input)
        .expect("pack_hap must succeed");

    let file = fs::File::open(&out).expect("open HAP");
    let mut archive = zip::ZipArchive::new(file).expect("valid ZIP");
    let names: Vec<_> = archive.file_names().map(str::to_string).collect();
    let first_index = names
        .iter()
        .position(|name| name == "libs/arm64-v8a/libnative.so")
        .expect("first native lib");
    let second_index = names
        .iter()
        .position(|name| name == "libs/arm64-v8a/libnative2.so")
        .expect("second native lib");
    assert!(
        first_index < second_index,
        "native lib entry order must follow input order; entries: {names:?}"
    );
    assert_eq!(
        archive
            .by_name("libs/arm64-v8a/libnative.so")
            .unwrap()
            .compression(),
        zip::CompressionMethod::Deflated
    );
    assert_eq!(
        archive
            .by_name("libs/arm64-v8a/libnative.so")
            .unwrap()
            .compressed_size() as usize,
        rawzip_deflated_len_for_level(&first_payload, flate2::Compression::new(1)),
        "native lib deflate level must match SDK app_packing_tool.jar ohos.Utility default compressLevel=1"
    );
    assert_ne!(
        archive
            .by_name("libs/arm64-v8a/libnative.so")
            .unwrap()
            .compressed_size() as usize,
        rawzip_deflated_len_for_level(&first_payload, flate2::Compression::default()),
        "regression guard: native lib packing must not fall back to flate2 default compression level"
    );
    assert_eq!(
        archive
            .by_name("libs/arm64-v8a/libnative2.so")
            .unwrap()
            .compression(),
        zip::CompressionMethod::Deflated
    );
    let mut contents = String::new();
    archive
        .by_name("libs/arm64-v8a/libnative.so")
        .unwrap()
        .read_to_string(&mut contents)
        .unwrap();
    assert_eq!(contents, "native payload one".repeat(64));
}

#[test]
fn test_hap_applies_latest_hvigor_standard_native_compress_level() {
    let tmp = tempfile::tempdir().expect("tempdir");
    let (abc, res, modj) = write_minimal_inputs(tmp.path());
    let out = tmp.path().join("native_standard.hap");
    let so_dir = tmp.path().join("src/main/libs/arm64-v8a");
    fs::create_dir_all(&so_dir).unwrap();
    let so = so_dir.join("libnative.so");
    let payload = b"compress-level-parity".repeat(512);
    fs::write(&so, &payload).unwrap();

    let mut input = minimal_pack_input(abc, res, modj, out.clone());
    input.native_libs = vec![so];
    input.compress_native_libs = true;
    Packager::new(make_config())
        .with_compress_level(5)
        .pack_hap(input)
        .expect("pack HAP with standard compression");

    let file = fs::File::open(&out).expect("open HAP");
    let mut archive = zip::ZipArchive::new(file).expect("valid ZIP");
    assert_eq!(
        archive
            .by_name("libs/arm64-v8a/libnative.so")
            .unwrap()
            .compressed_size() as usize,
        rawzip_deflated_len_for_level(&payload, flate2::Compression::new(5))
    );
}

#[test]
fn test_hap_preserves_native_lib_abi_directory() {
    let tmp = tempfile::tempdir().expect("tempdir");
    let (abc, res, modj) = write_minimal_inputs(tmp.path());
    let out = tmp.path().join("native_abis.hap");
    let arm64_dir = tmp.path().join("src/main/libs/arm64-v8a");
    let x64_dir = tmp.path().join("src/main/libs/x86_64");
    fs::create_dir_all(&arm64_dir).unwrap();
    fs::create_dir_all(&x64_dir).unwrap();
    let arm64_so = arm64_dir.join("libsame.so");
    let x64_so = x64_dir.join("libsame.so");
    fs::write(&arm64_so, b"\x7fELF arm64").unwrap();
    fs::write(&x64_so, b"\x7fELF x64").unwrap();

    let mut input = minimal_pack_input(abc, res, modj, out.clone());
    input.native_libs = vec![arm64_so, x64_so];
    Packager::new(make_config())
        .pack_hap(input)
        .expect("pack_hap must succeed");

    let file = fs::File::open(&out).expect("open HAP");
    let archive = zip::ZipArchive::new(file).expect("valid ZIP");
    let names: Vec<_> = archive.file_names().collect();
    assert!(
        names
            .iter()
            .any(|name| *name == "libs/arm64-v8a/libsame.so"),
        "missing arm64-v8a native lib; entries: {:?}",
        names
    );
    assert!(
        names.iter().any(|name| *name == "libs/x86_64/libsame.so"),
        "missing x86_64 native lib; entries: {:?}",
        names
    );
}

#[test]
fn test_pack_info_splits_api_versions_and_includes_form_extensions() {
    let tmp = tempfile::tempdir().expect("tempdir");
    let (abc, res, modj) = write_minimal_inputs(tmp.path());
    let out = tmp.path().join("form_pack_info.hap");
    fs::write(
        &modj,
        r#"{
          "module": {
            "name": "entry",
            "type": "entry",
            "extensionAbilities": [{
              "name": "EntryFormAbility",
              "type": "form",
              "metadata": [{
                "name": "ohos.extension.form",
                "resource": "$profile:form_config"
              }]
            }, {
              "name": "DependencyExtensionAbility",
              "type": "workScheduler"
            }]
          }
        }"#,
    )
    .unwrap();
    let pack_info_module_json = tmp.path().join("current-module.json");
    fs::write(
        &pack_info_module_json,
        r#"{
          "module": {
            "name": "entry",
            "type": "entry",
            "extensionAbilities": [{
              "name": "EntryFormAbility",
              "type": "form",
              "metadata": [{
                "name": "ohos.extension.form",
                "resource": "$profile:form_config"
              }]
            }, {
              "name": "BackupExtensionAbility",
              "type": "backup",
              "metadata": [{
                "name": "ohos.extension.backup",
                "resource": "$profile:backup_config"
              }]
            }, {
              "name": "LiveViewExtensionAbility",
              "type": "liveViewLockScreen"
            }]
          }
        }"#,
    )
    .unwrap();

    let resources_dir = tmp.path().join("compiled/resources/base/profile");
    fs::create_dir_all(&resources_dir).unwrap();
    fs::write(
        resources_dir.join("form_config.json"),
        r#"{
          "forms": [{
            "name": "widget",
            "src": "./ets/widget/pages/WidgetCard.ets",
            "updateEnabled": false,
            "scheduledUpdateTime": "10:30",
            "updateDuration": 1,
            "supportDimensions": ["4*4"],
            "defaultDimension": "4*4"
          }]
        }"#,
    )
    .unwrap();

    let mut config = (*make_config()).clone();
    config.product.compatible_sdk_version = "6.0.0(20)".to_string();
    config.sdk_config.compile_api_version = 21;

    let mut input = minimal_pack_input(abc, res, modj, out.clone());
    input.resources_src_dir = Some(tmp.path().join("compiled/resources"));
    input.pack_info_module_json = Some(pack_info_module_json);

    Packager::new(Arc::new(config))
        .pack_hap(input)
        .expect("pack_hap must succeed");

    let file = fs::File::open(&out).expect("open HAP");
    let mut archive = zip::ZipArchive::new(file).expect("valid ZIP");
    let mut entry = archive.by_name("pack.info").expect("pack.info");
    let mut content = String::new();
    entry.read_to_string(&mut content).unwrap();
    let pack_info: serde_json::Value = serde_json::from_str(&content).unwrap();
    let module = &pack_info["summary"]["modules"][0];

    assert_eq!(module["apiVersion"]["compatible"].as_u64(), Some(20));
    assert_eq!(module["apiVersion"]["target"].as_u64(), Some(21));
    assert_eq!(
        module["extensionAbilities"][0]["name"].as_str(),
        Some("EntryFormAbility")
    );
    assert_eq!(
        module["extensionAbilities"][0]["forms"][0]["name"].as_str(),
        Some("widget")
    );
    assert_eq!(
        module["extensionAbilities"][0]["forms"][0]["type"].as_str(),
        Some("JS")
    );
    assert!(
        module["extensionAbilities"][0]["forms"][0]
            .get("src")
            .is_none(),
        "pack.info form summary must not include source-only fields"
    );
    assert_eq!(
        module["extensionAbilities"][1]["name"],
        "BackupExtensionAbility"
    );
    assert_eq!(
        module["extensionAbilities"][1]["forms"],
        serde_json::json!([])
    );
    assert_eq!(
        module["extensionAbilities"][2]["name"],
        "LiveViewExtensionAbility"
    );
    assert!(module["extensionAbilities"][2].get("forms").is_none());
    assert!(module["extensionAbilities"]
        .as_array()
        .unwrap()
        .iter()
        .all(|ability| ability["name"] != "DependencyExtensionAbility"));
}

#[test]
fn test_hap_omits_pkg_context_info_when_not_provided() {
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
fn test_hap_packages_pkg_context_info_when_provided_like_hvigor() {
    let tmp = tempfile::tempdir().expect("tempdir");
    let (abc, res, modj) = write_minimal_inputs(tmp.path());
    let out = tmp.path().join("pkg_ctx.hap");
    let pkg_context_info = tmp.path().join("pkgContextInfo.json");
    fs::write(
        &pkg_context_info,
        r#"{"entry":{"bundleName":"com.example","moduleName":"entry","entryPath":"ets/modules.abc"}}"#,
    )
    .expect("write pkgContextInfo.json");

    let mut input = minimal_pack_input(abc, res, modj, out.clone());
    input.pkg_context_info_path = Some(pkg_context_info);

    Packager::new(make_config())
        .pack_hap(input)
        .expect("pack_hap must succeed");

    let file = fs::File::open(&out).expect("open HAP");
    let mut archive = zip::ZipArchive::new(file).expect("valid ZIP");
    let mut entry = archive
        .by_name("pkgContextInfo.json")
        .expect("pkgContextInfo.json must be packaged");
    let mut content = String::new();
    entry.read_to_string(&mut content).unwrap();
    assert!(
        content.contains(r#""entryPath":"ets/modules.abc""#),
        "pkgContextInfo.json content must survive package-tool normalization, got: {content}"
    );
    drop(entry);
    assert_eq!(
        archive.file_names().last(),
        Some("pkgContextInfo.json"),
        "SDK app_packing_tool writes pkgContextInfo.json after pack.info"
    );
}

#[test]
fn test_hap_packages_pkg_sdk_info_when_provided_like_hvigor() {
    let tmp = tempfile::tempdir().expect("tempdir");
    let (abc, res, modj) = write_minimal_inputs(tmp.path());
    let out = tmp.path().join("pkg_sdk.hap");
    let pkg_sdk_info = tmp.path().join("pkgSdkInfo.json");
    let expected = r#"{"library":{"version":"1.2.3","type":"har","isDirectDep":true},"entry":{"version":"1.0.0","type":"entry","isDirectDep":false}}"#;
    fs::write(&pkg_sdk_info, expected).expect("write pkgSdkInfo.json");

    let mut input = minimal_pack_input(abc, res, modj, out.clone());
    input.pkg_sdk_info_path = Some(pkg_sdk_info);

    Packager::new(make_config())
        .pack_hap(input)
        .expect("pack_hap must succeed");

    let file = fs::File::open(&out).expect("open HAP");
    let mut archive = zip::ZipArchive::new(file).expect("valid ZIP");
    let mut entry = archive
        .by_name("pkgSdkInfo.json")
        .expect("pkgSdkInfo.json must be packaged");
    let mut content = String::new();
    entry.read_to_string(&mut content).unwrap();
    assert_eq!(
        content, expected,
        "pkgSdkInfo.json must remain byte-identical"
    );
    drop(entry);
    assert_eq!(
        archive.file_names().last(),
        Some("pkgSdkInfo.json"),
        "SDK app_packing_tool writes pkgSdkInfo.json after pack.info"
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
fn test_hap_packages_source_maps_after_modules_abc_like_hvigor_ets_path() {
    let tmp = tempfile::tempdir().expect("tempdir");
    let (abc, res, modj) = write_minimal_inputs(tmp.path());
    let source_maps = tmp.path().join("sourceMaps.map");
    let out = tmp.path().join("source_maps.hap");
    fs::write(&source_maps, br#"{"version":3,"sources":[],"mappings":""}"#).unwrap();

    let mut input = minimal_pack_input(abc, res, modj, out.clone());
    input.source_maps_path = Some(source_maps);
    Packager::new(make_config())
        .pack_hap(input)
        .expect("pack_hap must succeed");

    let file = fs::File::open(&out).expect("open HAP");
    let mut archive = zip::ZipArchive::new(file).expect("valid ZIP");
    let names: Vec<_> = archive.file_names().map(str::to_string).collect();
    let abc_index = names
        .iter()
        .position(|name| name == "ets/modules.abc")
        .expect("ets/modules.abc");
    let source_map_index = names
        .iter()
        .position(|name| name == "ets/sourceMaps.map")
        .expect("ets/sourceMaps.map");
    let pack_info_index = names
        .iter()
        .position(|name| name == "pack.info")
        .expect("pack.info");
    assert_eq!(source_map_index, abc_index + 1, "entries: {names:?}");
    assert!(source_map_index < pack_info_index, "entries: {names:?}");

    let mut entry = archive.by_name("ets/sourceMaps.map").unwrap();
    assert_eq!(entry.compression(), zip::CompressionMethod::Stored);
    let mut content = String::new();
    entry.read_to_string(&mut content).unwrap();
    assert_eq!(content, r#"{"version":3,"sources":[],"mappings":""}"#);
}

#[test]
fn test_hap_rawzip_entries_use_dos_creator_metadata() {
    let tmp = tempfile::tempdir().expect("tempdir");
    let (abc, res, modj) = write_minimal_inputs(tmp.path());
    let out = tmp.path().join("rawzip_metadata.hap");

    Packager::new(make_config())
        .pack_hap(minimal_pack_input(abc, res, modj, out.clone()))
        .expect("pack_hap must succeed");

    let file = fs::File::open(&out).expect("open HAP");
    let mut archive = zip::ZipArchive::new(file).expect("valid ZIP");
    let entry = archive.by_name("module.json").unwrap();
    assert_eq!(entry.version_made_by(), (2, 0));
    assert_eq!(entry.unix_mode(), None);
}

#[test]
fn test_hap_stored_local_headers_match_sdk_packing_tool_shape() {
    let tmp = tempfile::tempdir().expect("tempdir");
    let (abc, res, modj) = write_minimal_inputs(tmp.path());
    let out = tmp.path().join("local_header.hap");

    Packager::new(make_config())
        .pack_hap(minimal_pack_input(abc, res, modj, out.clone()))
        .expect("pack_hap must succeed");

    // SDK app_packing_tool.jar emits stored entries with extract version 1.0,
    // concrete CRC/sizes in the local header, and no data descriptor flag.
    let bytes = fs::read(out).expect("read HAP");
    let read_u16 = |offset| u16::from_le_bytes([bytes[offset], bytes[offset + 1]]);
    let read_u32 = |offset| {
        u32::from_le_bytes([
            bytes[offset],
            bytes[offset + 1],
            bytes[offset + 2],
            bytes[offset + 3],
        ])
    };
    assert_eq!(&bytes[..4], b"PK\x03\x04");
    assert_eq!(read_u16(4), 10);
    assert_eq!(read_u16(6), 0x0800);
    assert_ne!(read_u32(14), 0);
    assert_eq!(read_u32(18), read_u32(22));

    let next_header =
        30 + usize::from(read_u16(26)) + usize::from(read_u16(28)) + read_u32(18) as usize;
    assert_eq!(&bytes[next_header..next_header + 4], b"PK\x03\x04");
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
            pack_info_module_json: None,
            native_libs: vec![],
            compress_native_libs: false,
            output_path: out.clone(),
            resources_src_dir: Some(tmp.path().join("compiled/resources")),
            bundle_version_code: 1000000,
            bundle_version_name: "1.0.0".to_string(),
            bundle_build_version: None,
            bundle_type: "app".to_string(),
            source_maps_path: None,
            pkg_context_info_path: None,
            pkg_sdk_info_path: None,
            abilities: vec![],
            integrated_hsps: vec![],
            privacy_profile_path: None,
            syscap_path: None,
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
        pack_info_module_json: None,
        native_libs: vec![],
        compress_native_libs: false,
        output_path: tmp.path().join("out.hap"),
        resources_src_dir: None,
        bundle_version_code: 1000000,
        bundle_version_name: "1.0.0".to_string(),
        bundle_build_version: None,
        bundle_type: "app".to_string(),
        source_maps_path: None,
        pkg_context_info_path: None,
        pkg_sdk_info_path: None,
        abilities: vec![],
        integrated_hsps: vec![],
        privacy_profile_path: None,
        syscap_path: None,
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
        pack_info_module_json: None,
        native_libs: vec![],
        compress_native_libs: false,
        output_path: tmp.path().join("out.hap"),
        resources_src_dir: None,
        bundle_version_code: 1000000,
        bundle_version_name: "1.0.0".to_string(),
        bundle_build_version: None,
        bundle_type: "app".to_string(),
        source_maps_path: None,
        pkg_context_info_path: None,
        pkg_sdk_info_path: None,
        abilities: vec![],
        integrated_hsps: vec![],
        privacy_profile_path: None,
        syscap_path: None,
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
            pack_info_module_json: None,
            native_libs: vec![],
            compress_native_libs: false,
            output_path: out.clone(),
            resources_src_dir: None,
            bundle_version_code: 1000000,
            bundle_version_name: "1.0.0".to_string(),
            bundle_build_version: None,
            bundle_type: "app".to_string(),
            source_maps_path: None,
            pkg_context_info_path: None,
            pkg_sdk_info_path: None,
            abilities: vec![],
            integrated_hsps: vec![],
            privacy_profile_path: None,
            syscap_path: None,
            device_types: vec![],
            installation_free: false,
            delivery_with_install: true,
        })
        .expect("pack_hsp must succeed");

    let bytes = fs::read(&out).expect("read HSP");
    assert_eq!(&bytes[0..4], b"PK\x03\x04", "HSP must be a ZIP file");
}

#[test]
fn test_hsp_pack_info_keeps_app_bundle_type_and_uses_shared_module_type() {
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
            pack_info_module_json: None,
            native_libs: vec![],
            compress_native_libs: false,
            output_path: out.clone(),
            resources_src_dir: None,
            bundle_version_code: 1000000,
            bundle_version_name: "1.0.0".to_string(),
            bundle_build_version: None,
            bundle_type: "app".to_string(),
            source_maps_path: None,
            pkg_context_info_path: None,
            pkg_sdk_info_path: None,
            abilities: vec![],
            integrated_hsps: vec![],
            privacy_profile_path: None,
            syscap_path: None,
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

    assert_eq!(pack_info["summary"]["app"]["bundleType"], "app");
    assert!(pack_info["summary"]["modules"][0]
        .get("mainAbility")
        .is_none());
    assert!(pack_info["summary"]["modules"][0]
        .get("abilities")
        .is_none());
    assert_eq!(
        pack_info["summary"]["modules"][0]["distro"]["moduleType"],
        "shared"
    );
    assert_eq!(pack_info["packages"][0]["moduleType"], "shared");
}

#[test]
fn test_hsp_interface_har_matches_prepare_shared_har_resource_shape() {
    let tmp = tempfile::tempdir().unwrap();
    let module_root = tmp.path().join("entry");
    let declarations = module_root.join("build/default/intermediates/loader_out/default/etsFortgz");
    let declaration = declarations.join("entry/src/main/ets/Index.d.ets");
    let processed_module =
        module_root.join("build/default/intermediates/process_profile/default/module.json");
    let resource_table =
        module_root.join("build/default/intermediates/res/default/ResourceTable.txt");
    let native_lib =
        module_root.join("build/default/intermediates/stripped_native_libs/arm64-v8a/libshared.so");
    fs::create_dir_all(declaration.parent().unwrap()).unwrap();
    fs::create_dir_all(processed_module.parent().unwrap()).unwrap();
    fs::create_dir_all(resource_table.parent().unwrap()).unwrap();
    fs::create_dir_all(native_lib.parent().unwrap()).unwrap();
    fs::write(
        module_root.join("oh-package.json5"),
        r#"{
  "modelVersion": "6.0.0",
  "name": "entry",
  "version": "1.0.0",
  "description": "Shared module",
  "main": "",
  "dependencies": {}
}"#,
    )
    .unwrap();
    fs::write(&declaration, "export declare const value: string;\r\n").unwrap();
    fs::write(&resource_table, "string app_name 0x01000000\n").unwrap();
    fs::write(&native_lib, b"ELF").unwrap();
    fs::write(
        &processed_module,
        r#"{"app":{"bundleName":"com.example.shared","bundleType":"app","debug":true,"versionCode":1000000,"versionName":"1.0.0","minAPIVersion":50005017,"targetAPIVersion":50005017,"apiReleaseType":"Release","targetMinorAPIVersion":0,"targetPatchAPIVersion":0,"compileSdkVersion":"ignored"},"module":{"name":"entry","packageName":"entry","type":"shared","deliveryWithInstall":true,"deviceTypes":["phone"],"dependencies":[],"srcEntry":"./ets/Index.ets"}}"#,
    )
    .unwrap();

    let output = module_root.join("build/default/outputs/default/entry.har");
    Packager::new(make_config())
        .pack_hsp_interface_har(HspInterfaceHarInput {
            module_name: "entry".to_string(),
            module_root: module_root.clone(),
            target_name: "default".to_string(),
            oh_package_json: module_root.join("oh-package.json5"),
            processed_module_json: processed_module,
            declarations_dir: declarations,
            resource_table_path: Some(resource_table),
            resource_str_dir: None,
            privacy_profile_path: None,
            obfuscation_rules_path: None,
            native_libs: vec![native_lib],
            native_header_dirs: Vec::new(),
            exclude_so_from_interface_har: true,
            output_path: output.clone(),
            compatible_sdk_version: 17,
            compatible_sdk_type: "HarmonyOS".to_string(),
            use_normalized_ohm_url: false,
            integrated_hsp: None,
            source_roots: Vec::new(),
            dependencies: std::collections::BTreeMap::new(),
            dynamic_dependencies: std::collections::BTreeMap::new(),
            debug: true,
            obfuscated: false,
        })
        .expect("PackageSharedHar must succeed");

    let entries = read_har_entries(&output);
    assert_eq!(
        entries.keys().cloned().collect::<Vec<_>>(),
        vec![
            "package/ResourceTable.txt",
            "package/oh-package.json5",
            "package/src/",
            "package/src/main/",
            "package/src/main/ets/",
            "package/src/main/ets/Index.d.ets",
            "package/src/main/module.json",
        ]
    );
    assert_eq!(
        std::str::from_utf8(&entries["package/src/main/module.json"]).unwrap(),
        r#"{"app":{"bundleName":"com.example.shared","bundleType":"app","debug":true,"versionCode":1000000,"versionName":"1.0.0","minAPIVersion":50005017,"targetAPIVersion":50005017,"apiReleaseType":"Release","targetMinorAPIVersion":0,"targetPatchAPIVersion":0},"module":{"name":"entry","packageName":"entry","type":"shared","deliveryWithInstall":true,"deviceTypes":["phone"],"dependencies":[]}}
"#
    );
    let package: serde_json::Value =
        serde_json::from_slice(&entries["package/oh-package.json5"]).unwrap();
    assert_eq!(package["types"], "index.d.ets");
    assert_eq!(package["packageType"], "InterfaceHar");
    assert_eq!(package["compatibleSdkVersion"], 17);
    assert_eq!(
        package["metadata"]["sourceRoots"],
        serde_json::json!(["./src/main", "./build/default/generated/profile/default"])
    );
}

#[test]
fn test_hsp_tgz_matches_package_shared_tgz_members_and_is_deterministic() {
    let tmp = tempfile::tempdir().unwrap();
    let hsp = tmp.path().join("entry-default-unsigned.hsp");
    let har = tmp.path().join("entry.har");
    let first = tmp.path().join("entry-default.tgz");
    let second = tmp.path().join("second.tgz");
    fs::write(&hsp, b"HSP archive bytes").unwrap();
    fs::write(&har, b"interface HAR bytes").unwrap();

    let packager = Packager::new(make_config());
    for output in [&first, &second] {
        packager
            .pack_hsp_tgz(HspTgzInput {
                hsp_path: hsp.clone(),
                interface_har_path: har.clone(),
                output_path: output.clone(),
            })
            .expect("PackageSharedTgz must succeed");
    }

    let entries = read_har_entries(&first);
    assert_eq!(
        entries.keys().cloned().collect::<Vec<_>>(),
        vec!["entry-default.hsp", "entry.har"]
    );
    assert_eq!(entries["entry-default.hsp"], b"HSP archive bytes");
    assert_eq!(entries["entry.har"], b"interface HAR bytes");
    assert_eq!(fs::read(first).unwrap(), fs::read(second).unwrap());
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

    let entries = read_har_entries(&out);
    let names: Vec<_> = entries.keys().collect();
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

    let entries = read_har_entries(&out);
    let names: Vec<_> = entries.keys().collect();
    assert!(
        names
            .iter()
            .any(|n| n.as_str() == "package/oh-package.json5"),
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

    let entries = read_har_entries(&out);
    let names: Vec<_> = entries.keys().collect();
    assert!(
        names
            .iter()
            .any(|n| n.as_str() == "package/src/main/resource/base/element/string.json"),
        "HAR must contain permission reason resource_str; entries: {:?}",
        names
    );

    let content = std::str::from_utf8(
        entries
            .get("package/src/main/resource/base/element/string.json")
            .expect("resource_str entry"),
    )
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
    let paths = normalize_integrated_hsps(
        &[entry],
        "com.host.app",
        2000000,
        Some("host-build"),
        &out_dir,
    )
    .unwrap();

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
    assert_eq!(
        v["summary"]["app"]["version"]["build"].as_str(),
        Some("host-build"),
        "version.build must be patched when the host defines buildVersion"
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
        None,
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
            pack_info_module_json: None,
            native_libs: vec![],
            compress_native_libs: false,
            output_path: out.clone(),
            resources_src_dir: None,
            bundle_version_code: 1000000,
            bundle_version_name: "1.0.0".to_string(),
            bundle_build_version: None,
            bundle_type: "app".to_string(),
            source_maps_path: None,
            pkg_context_info_path: None,
            pkg_sdk_info_path: None,
            abilities: vec![],
            integrated_hsps: vec![integrated_entry],
            privacy_profile_path: None,
            syscap_path: None,
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

/// Original source HARs retain ArkTS sources and do not copy generated
/// declaration intermediates. Hvigor only copies declarations for customized
/// or bytecode layouts in `ProcessHarArtifacts.copyDeclareFileToTempDir` /
/// `copyCompiledSourceFileToTempDir`.
#[test]
fn test_original_har_omits_generated_declaration_stubs() {
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

    let entries = read_har_entries(&out);
    let names: Vec<_> = entries.keys().collect();

    assert!(
        !names.iter().any(|n| n.as_str() == "package/index.d.ts"),
        "original HAR must omit generated index.d.ts; entries: {:?}",
        names
    );
    assert!(
        !names.iter().any(|n| n.as_str() == "package/index.d.ets"),
        "original HAR must omit generated index.d.ets; entries: {:?}",
        names
    );
    assert!(
        !names.iter().any(|n| n.as_str() == "package/other.json"),
        "HAR must not contain non-declaration files from declarations dir; entries: {:?}",
        names
    );
}

/// Verify that consumer obfuscation rules are included in the HAR archive
/// at the root level as `obfuscation.txt`.
///
/// hvigor calls `generateConsumerObConfigFile()` and includes the rules so
/// downstream consumers can extend their own obfuscation configuration.
#[test]
fn test_har_includes_consumer_rules() {
    let tmp = tempfile::tempdir().expect("tempdir");
    let src_dir = tmp.path().join("src/main/ets");
    fs::create_dir_all(&src_dir).unwrap();
    fs::write(src_dir.join("Index.ets"), b"export class Index {}").unwrap();

    let consumer_rules = tmp.path().join("obfuscation.txt");
    fs::write(&consumer_rules, b"-keep public class com.example.lib.*\n").unwrap();

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

    let entries = read_har_entries(&out);
    let names: Vec<_> = entries.keys().collect();

    assert!(
        names
            .iter()
            .any(|n| n.as_str() == "package/obfuscation.txt"),
        "HAR must contain obfuscation.txt; entries: {:?}",
        names
    );

    let content = std::str::from_utf8(
        entries
            .get("package/obfuscation.txt")
            .expect("obfuscation.txt entry"),
    )
    .expect("read obfuscation.txt");
    assert!(
        content.contains("-keep public class"),
        "obfuscation.txt must contain keep rules; got: {:?}",
        content
    );
}

/// Merely producing declaration intermediates must not mutate an original
/// source HAR's package manifest.
#[test]
fn test_original_har_does_not_invent_types_field() {
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

    let entries = read_har_entries(&out);
    let content = std::str::from_utf8(
        entries
            .get("package/oh-package.json5")
            .expect("oh-package.json5 entry"),
    )
    .expect("read oh-package.json5");

    let pkg: serde_json::Value = serde_json::from_str(&content).expect("parse oh-package.json5");
    assert_eq!(
        pkg.get("types").and_then(|v| v.as_str()),
        None,
        "original HAR must not invent a types field from generated intermediates; got: {}",
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
    fs::write(
        decl_dir.join("index.d.ts"),
        b"export declare class Index {}",
    )
    .unwrap();
    fs::write(
        decl_dir.join("index.d.ets"),
        b"export declare class Index {}",
    )
    .unwrap();

    let output_dir = tmp.path().join("cleaned");
    let cleaned_ets =
        Packager::clean_har_sources(&tmp.path().join("src/main"), Some(&decl_dir), &output_dir)
            .expect("clean_har_sources must succeed");

    // The cleaned/ets directory should contain declaration stubs.
    assert!(
        cleaned_ets.join("index.d.ts").exists(),
        "index.d.ts must be in cleaned ets"
    );
    assert!(
        cleaned_ets.join("index.d.ets").exists(),
        "index.d.ets must be in cleaned ets"
    );

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

#[test]
fn process_oh_package_json_matches_latest_hvigor_bytecode_har_output() {
    let tmp = tempfile::tempdir().expect("tempdir");
    let module_root = tmp.path().join("entry");
    let source_dir = module_root.join("src/main/ets");
    fs::create_dir_all(&source_dir).unwrap();
    fs::write(source_dir.join("Index.ets"), "export const value = 1").unwrap();
    let manifest_path = module_root.join("oh-package.json5");
    let source_manifest = r#"{
  "modelVersion": "6.0.0",
  "name": "entry",
  "version": "1.0.0",
  "description": "Minimal HAR library module",
  "main": "src/main/ets/Index.ets",
  "author": "",
  "license": "Apache-2.0",
  "dependencies": {
    "share-config-har": "1.0.0",
    "byte-har": "1.0.0"
  },
  "devDependencies": {
    "dev-source": "1.0.0"
  },
  "dynamicDependencies": {
    "dyn-source": "1.0.0",
    "dyn-byte": "1.0.0"
  }
}"#;
    fs::write(&manifest_path, source_manifest).unwrap();
    let generated_manifest =
        module_root.join("build/default/generated/pm/default/oh-package.json5");

    Packager::new(make_config())
        .process_har_package_json(
            &HarPackInput {
                module_name: "entry".into(),
                source_dir,
                resources_dir: module_root.join("src/main/resources"),
                resource_str_dir: None,
                oh_package_json: manifest_path.clone(),
                output_path: module_root.join("build/default/outputs/default/entry.har"),
                declarations_dir: None,
                consumer_rules_path: None,
            },
            &HarVariantInput {
                artifact_type: HarArtifactType::Bytecode,
                module_root,
                compiled_output_dir: std::path::PathBuf::new(),
                processed_module_json: None,
                resource_table_path: None,
                native_libs: Vec::new(),
                native_header_dirs: Vec::new(),
                preload_system_so_json: None,
                source_maps_path: None,
                build_profile_path: None,
                declaration_entries: Vec::new(),
                package_parameters: HarPackageParameterInput::default(),
                worker_entries: Vec::new(),
                runtime_only: None,
                resource_directories: Vec::new(),
                bundled_dependency_rewrite: None,
                dependency_pkg_version: IndexMap::new(),
                bundled_dependencies: false,
                bundled_resources: Vec::new(),
                bundled_resource_directories: Vec::new(),
                compiled_package_names: Vec::new(),
                extra_importees: Vec::new(),
                global_dependencies: Vec::new(),
                ignore_resource_patterns: Vec::new(),
                debug: true,
                target_name: "default".into(),
                package_cache_dir: None,
            },
            &generated_manifest,
        )
        .expect("ProcessOHPackageJson must succeed");

    assert_eq!(fs::read_to_string(&manifest_path).unwrap(), source_manifest);
    assert_eq!(
        fs::read_to_string(generated_manifest).unwrap(),
        concat!(
            "{\"modelVersion\":\"6.0.0\",\"name\":\"entry\",\"version\":\"1.0.0\",",
            "\"description\":\"Minimal HAR library module\",\"author\":\"\",",
            "\"license\":\"Apache-2.0\",\"dependencies\":{",
            "\"share-config-har\":\"1.0.0\",\"byte-har\":\"1.0.0\"},",
            "\"devDependencies\":{\"dev-source\":\"1.0.0\"},",
            "\"dynamicDependencies\":{\"dyn-source\":\"1.0.0\",",
            "\"dyn-byte\":\"1.0.0\"},\"types\":\"src/main/ets/Index.d.ets\",",
            "\"artifactType\":\"obfuscation\",\"metadata\":{\"byteCodeHar\":true}}\n"
        )
    );
}

#[test]
fn test_bytecode_har_uses_compiled_layout_and_does_not_mutate_source_manifest() {
    let tmp = tempfile::tempdir().expect("tempdir");
    let module_root = tmp.path().join("library");
    let source_dir = module_root.join("src/main/ets");
    let resources_dir = module_root.join("src/main/resources");
    let compiled_dir = module_root.join("build/default/intermediates/loader_out/default");
    let declarations_dir = module_root.join("build/default/intermediates/har/default");
    fs::create_dir_all(&source_dir).unwrap();
    fs::create_dir_all(resources_dir.join("base/element")).unwrap();
    fs::create_dir_all(compiled_dir.join("ets")).unwrap();
    fs::create_dir_all(declarations_dir.join("library/types")).unwrap();
    fs::write(source_dir.join("Index.ets"), "export class Index {}").unwrap();
    fs::write(
        resources_dir.join("base/element/string.json"),
        r#"{"string":[]}"#,
    )
    .unwrap();
    fs::write(compiled_dir.join("ets/modules.abc"), b"abc").unwrap();
    fs::write(
        declarations_dir.join("library/types/index.d.ts"),
        "export declare class Index {}",
    )
    .unwrap();
    let generated_module_info =
        declarations_dir.join("library/build/default/generated/profile/default/ModuleInfo.d.ts");
    fs::create_dir_all(generated_module_info.parent().unwrap()).unwrap();
    fs::write(
        &generated_module_info,
        "export declare const __MODULE_NAME__: string;\r\n",
    )
    .unwrap();
    let processed_module = module_root.join("processed-module.json");
    let resource_table = module_root.join("ResourceTable.txt");
    fs::write(&processed_module, r#"{"module":{"name":"library"}}"#).unwrap();
    fs::write(&resource_table, "resource table").unwrap();
    let manifest_path = module_root.join("oh-package.json5");
    let source_manifest = r#"{
      name: "library",
      version: "1.0.0",
      main: "src/main/ets/Index.ets",
      metadata: { workers: ["src/main/ets/Worker.ets"], runtimeOnly: { sources: ["src/main/ets/Runtime.ets"], packages: ["dep"] } }
    }"#;
    fs::write(&manifest_path, source_manifest).unwrap();
    let output = tmp.path().join("library.har");
    let package_cache = tmp.path().join("default@PackageHar");

    Packager::new(make_config())
        .pack_har_variant(
            HarPackInput {
                module_name: "library".into(),
                source_dir,
                resources_dir,
                resource_str_dir: None,
                oh_package_json: manifest_path.clone(),
                output_path: output.clone(),
                declarations_dir: Some(declarations_dir),
                consumer_rules_path: None,
            },
            HarVariantInput {
                artifact_type: HarArtifactType::Bytecode,
                module_root,
                compiled_output_dir: compiled_dir,
                processed_module_json: Some(processed_module),
                resource_table_path: Some(resource_table),
                native_libs: Vec::new(),
                native_header_dirs: Vec::new(),
                preload_system_so_json: None,
                source_maps_path: None,
                build_profile_path: None,
                declaration_entries: vec![
                    "@normalized:N&&library&library/src/main/ets/Index&1.0.0".into(),
                ],
                package_parameters: HarPackageParameterInput::default(),
                worker_entries: Vec::new(),
                runtime_only: None,
                resource_directories: Vec::new(),
                bundled_dependency_rewrite: None,
                dependency_pkg_version: IndexMap::new(),
                bundled_dependencies: false,
                bundled_resources: Vec::new(),
                bundled_resource_directories: Vec::new(),
                compiled_package_names: Vec::new(),
                extra_importees: Vec::new(),
                global_dependencies: Vec::new(),
                ignore_resource_patterns: Vec::new(),
                debug: true,
                target_name: "default".into(),
                package_cache_dir: Some(package_cache.clone()),
            },
        )
        .expect("pack bytecode HAR");

    assert_eq!(fs::read_to_string(&manifest_path).unwrap(), source_manifest);
    assert_eq!(
        read_har_entry_names(&output),
        [
            "package/ResourceTable.txt",
            "package/ets/",
            "package/libs/",
            "package/oh-package.json5",
            "package/src/",
            "package/types/",
            "package/ets/modules.abc",
            "package/src/main/",
            "package/types/index.d.ts",
            "package/src/main/module.json",
            "package/src/main/resources/",
            "package/src/main/resources/base/",
            "package/src/main/resources/base/element/",
            "package/src/main/resources/base/element/string.json",
        ]
    );
    let entries = read_har_entries(&output);
    assert!(entries.contains_key("package/ets/modules.abc"));
    assert!(!entries.contains_key("package/src/main/ets/Index.ets"));
    assert!(entries.contains_key("package/src/main/module.json"));
    assert!(entries.contains_key("package/ResourceTable.txt"));
    assert!(entries.contains_key("package/types/index.d.ts"));
    assert!(
        !entries.contains_key("package/build/default/generated/profile/default/ModuleInfo.d.ts"),
        "ByteCode HAR must keep generated ModuleInfo declarations in etsFortgz only"
    );
    assert!(!entries.contains_key("package/BuildProfile.ets"));
    assert_eq!(
        fs::read(package_cache.join("ets/modules.abc")).unwrap(),
        b"abc"
    );
    assert_eq!(
        fs::read_to_string(package_cache.join("src/main/module.json")).unwrap(),
        r#"{"module":{"name":"library"}}"#
    );
    assert!(package_cache.join("libs").is_dir());
    assert!(!package_cache.join("BuildProfile.ets").exists());
    let manifest_bytes = entries
        .get("package/oh-package.json5")
        .expect("oh-package.json5");
    assert_eq!(
        std::str::from_utf8(manifest_bytes).unwrap(),
        concat!(
            "{\"name\":\"library\",\"version\":\"1.0.0\",\"metadata\":{",
            "\"runtimeOnly\":{\"packages\":[\"dep\"]},\"byteCodeHar\":true,",
            "\"sourceRoots\":[\"./src/main\"],\"debug\":true,",
            "\"dependencyPkgVersion\":{},\"declarationEntry\":[",
            "\"@normalized:N&&library&library/src/main/ets/Index&1.0.0\"],",
            "\"useNormalizedOHMUrl\":true},",
            "\"types\":\"src/main/ets/Index.d.ets\",",
            "\"artifactType\":\"obfuscation\",\"compatibleSdkVersion\":12,",
            "\"compatibleSdkType\":\"HarmonyOS\",\"obfuscated\":false}\n"
        )
    );
    let manifest: serde_json::Value = serde_json::from_slice(manifest_bytes).unwrap();
    assert_eq!(manifest["artifactType"], "obfuscation");
    assert_eq!(manifest["metadata"]["byteCodeHar"], true);
    assert_eq!(
        manifest["metadata"]["dependencyPkgVersion"],
        serde_json::json!({})
    );
    assert_eq!(
        manifest["metadata"]["declarationEntry"]
            .as_array()
            .unwrap()
            .len(),
        1
    );
    assert!(manifest.get("main").is_none());
    assert!(manifest["metadata"].get("workers").is_none());
    assert!(manifest["metadata"]["runtimeOnly"].get("sources").is_none());
}

#[test]
fn bundled_bytecode_har_matches_hvigor_metadata_resources_and_breadth_first_order() {
    let tmp = tempfile::tempdir().expect("tempdir");
    let module_root = tmp.path().join("entry");
    let source_dir = module_root.join("src/main/ets");
    let resources_dir = module_root.join("src/main/resources");
    let compiled_dir = module_root.join("build/default/intermediates/loader_out/default");
    let dependency_resources = tmp.path().join("share-config-har/src/main/resources");
    fs::create_dir_all(&source_dir).unwrap();
    fs::create_dir_all(resources_dir.join("base/element")).unwrap();
    fs::create_dir_all(compiled_dir.join("ets")).unwrap();
    fs::create_dir_all(dependency_resources.join("base/profile")).unwrap();
    fs::write(source_dir.join("Index.ets"), "export const value = 1").unwrap();
    fs::write(
        resources_dir.join("base/element/string.json"),
        r#"{"string":[]}"#,
    )
    .unwrap();
    fs::write(
        dependency_resources.join("base/profile/config.json"),
        r#"{"value":true}"#,
    )
    .unwrap();
    fs::write(compiled_dir.join("ets/modules.abc"), b"abc").unwrap();
    let processed_module = module_root.join("module.json");
    fs::write(&processed_module, r#"{"module":{"name":"entry"}}"#).unwrap();
    let manifest_path = module_root.join("oh-package.json5");
    fs::write(
        &manifest_path,
        r#"{"modelVersion":"6.0.0","name":"entry","version":"1.0.0","main":"src/main/ets/Index.ets","dependencies":{"share-config-har":"@param:versions.share","byte-har":"1.0.0"},"devDependencies":{"dev-source":"1.0.0"},"dynamicDependencies":{"dyn-source":"1.0.0","dyn-byte":"1.0.0"}}"#,
    )
    .unwrap();
    let output = tmp.path().join("entry.har");

    Packager::new(make_config())
        .pack_har_variant(
            HarPackInput {
                module_name: "entry".into(),
                source_dir,
                resources_dir,
                resource_str_dir: None,
                oh_package_json: manifest_path,
                output_path: output.clone(),
                declarations_dir: None,
                consumer_rules_path: None,
            },
            HarVariantInput {
                artifact_type: HarArtifactType::Bytecode,
                module_root,
                compiled_output_dir: compiled_dir,
                processed_module_json: Some(processed_module),
                resource_table_path: None,
                native_libs: Vec::new(),
                native_header_dirs: Vec::new(),
                preload_system_so_json: None,
                source_maps_path: None,
                build_profile_path: None,
                declaration_entries: Vec::new(),
                package_parameters: HarPackageParameterInput {
                    dependencies: IndexMap::from_iter([(
                        "share-config-har".into(),
                        "1.0.0".into(),
                    )]),
                    ..Default::default()
                },
                worker_entries: Vec::new(),
                runtime_only: None,
                resource_directories: Vec::new(),
                bundled_dependency_rewrite: Some(HarBundledDependencyInput {
                    dependencies: IndexMap::from_iter([("byte-har".into(), "2.0.0".into())]),
                    dynamic_dependencies: IndexMap::from_iter([(
                        "dyn-byte".into(),
                        "2.0.0".into(),
                    )]),
                }),
                dependency_pkg_version: IndexMap::new(),
                bundled_dependencies: true,
                bundled_resources: vec![BundledHarResourceInput {
                    source_dir: dependency_resources,
                    archive_directory: ArtifactPath::from_slash(
                        "src/main/resources-share-config-har",
                    ),
                }],
                bundled_resource_directories: vec![
                    ArtifactPath::from_slash("src/main/resources"),
                    ArtifactPath::from_slash("src/main/resources-share-config-har"),
                ],
                compiled_package_names: vec!["share-config-har".into()],
                extra_importees: Vec::new(),
                global_dependencies: Vec::new(),
                ignore_resource_patterns: Vec::new(),
                debug: true,
                target_name: "default".into(),
                package_cache_dir: None,
            },
        )
        .expect("pack bundled bytecode HAR");

    assert_eq!(
        read_har_entry_names(&output),
        [
            "package/ets/",
            "package/libs/",
            "package/oh-package.json5",
            "package/src/",
            "package/ets/modules.abc",
            "package/src/main/",
            "package/src/main/module.json",
            "package/src/main/resources/",
            "package/src/main/resources-share-config-har/",
            "package/src/main/resources/base/",
            "package/src/main/resources-share-config-har/base/",
            "package/src/main/resources/base/element/",
            "package/src/main/resources-share-config-har/base/profile/",
            "package/src/main/resources/base/element/string.json",
            "package/src/main/resources-share-config-har/base/profile/config.json",
        ]
    );
    let entries = read_har_entries(&output);
    assert_eq!(
        std::str::from_utf8(&entries["package/oh-package.json5"]).unwrap(),
        concat!(
            "{\"modelVersion\":\"6.0.0\",\"name\":\"entry\",\"version\":\"1.0.0\",",
            "\"dependencies\":{\"byte-har\":\"2.0.0\"},",
            "\"dynamicDependencies\":{\"dyn-byte\":\"2.0.0\"},",
            "\"types\":\"src/main/ets/Index.d.ets\",\"artifactType\":\"obfuscation\",",
            "\"metadata\":{\"byteCodeHar\":true,\"sourceRoots\":[\"./src/main\"],",
            "\"debug\":true,\"dependencyPkgVersion\":{},\"bundleDependencies\":true,",
            "\"resource\":{\"directories\":[\"src/main/resources\",",
            "\"src/main/resources-share-config-har\"]},\"declarationEntry\":[],",
            "\"compiledPackageNames\":[\"share-config-har\"],\"extraImportees\":[],",
            "\"useNormalizedOHMUrl\":true},\"compatibleSdkVersion\":12,",
            "\"compatibleSdkType\":\"HarmonyOS\",\"obfuscated\":false}\n"
        )
    );
}

fn write_app_test_artifact(path: &std::path::Path, module_name: &str, package_name: &str) {
    use zip::write::SimpleFileOptions;

    let file = fs::File::create(path).unwrap();
    let mut archive = zip::ZipWriter::new(file);
    let options = SimpleFileOptions::default().compression_method(zip::CompressionMethod::Stored);
    archive.start_file("module.json", options).unwrap();
    write!(archive, "{{\"module\":{{\"name\":\"{module_name}\"}}}}").unwrap();
    archive.start_file("pack.info", options).unwrap();
    write!(
        archive,
        "{{\"summary\":{{\"app\":{{\"bundleName\":\"com.example.app\",\"bundleType\":\"app\",\"version\":{{\"code\":1,\"name\":\"1.0.0\"}}}},\"modules\":[{{\"deviceType\":[\"phone\"],\"distro\":{{\"moduleType\":\"entry\",\"moduleName\":\"{module_name}\"}}}}]}},\"packages\":[{{\"deviceType\":[\"phone\"],\"moduleType\":\"entry\",\"name\":\"{package_name}\"}}]}}"
    )
    .unwrap();
    archive.finish().unwrap();
}

#[test]
fn project_pack_info_matches_latest_hvigor_merge_and_format() {
    let tmp = tempfile::tempdir().unwrap();
    let first = tmp.path().join("entry-phone.hap");
    let second = tmp.path().join("entry-tablet.hap");
    write_app_test_artifact(&first, "entry", "entry-phone");
    write_app_test_artifact(&second, "entry", "entry-tablet");
    let artifacts = vec![
        AppArtifactInput::local(first, "entry"),
        AppArtifactInput::local(second, "entry"),
    ];
    let output = tmp.path().join("build/outputs/default/pack.info");

    let result = ProjectPackInfoBuilder::new(&artifacts)
        .write(&output)
        .unwrap();

    let bytes = fs::read(&output).unwrap();
    assert!(bytes.starts_with(b"{\n\t\"summary\""));
    assert_eq!(bytes.last(), Some(&b'\n'));
    let value: serde_json::Value = serde_json::from_slice(&bytes).unwrap();
    assert_eq!(value["summary"]["app"]["bundleName"], "com.example.app");
    assert_eq!(value["summary"]["modules"].as_array().unwrap().len(), 1);
    assert_eq!(value["packages"].as_array().unwrap().len(), 2);
    assert_eq!(
        result.package_file_names,
        [
            "entry-phone.hap".to_string(),
            "entry-tablet.hap".to_string()
        ]
        .into_iter()
        .collect()
    );
}

#[test]
fn project_pack_info_renames_remote_hsp_package_like_hvigor() {
    let tmp = tempfile::tempdir().unwrap();
    let remote = tmp.path().join("shared-runtime.hsp");
    write_app_test_artifact(&remote, "shared", "published-name");
    let artifacts = vec![AppArtifactInput::remote_hsp(remote)];
    let output = tmp.path().join("pack.info");

    ProjectPackInfoBuilder::new(&artifacts)
        .write(&output)
        .unwrap();

    let value: serde_json::Value = serde_json::from_slice(&fs::read(output).unwrap()).unwrap();
    assert_eq!(value["packages"][0]["name"], "shared-runtime");
}

#[test]
fn remote_hsp_collector_uses_dependency_filter_semver_and_integrated_cache() {
    let tmp = tempfile::tempdir().unwrap();
    let module_root = tmp.path().join("entry");
    let module_remote = module_root.join("oh_modules/.hsp/shared-module");
    let project_remote = tmp.path().join("oh_modules/.hsp/shared-project");
    fs::create_dir_all(&module_remote).unwrap();
    fs::create_dir_all(&project_remote).unwrap();
    fs::write(
        module_remote.join("oh-package.json5"),
        r#"{"name":"@scope/shared","version":"1.0.0","metadata":{"integratedHsp":true}}"#,
    )
    .unwrap();
    fs::write(
        project_remote.join("oh-package.json5"),
        r#"{"name":"@scope/shared","version":"2.0.0","metadata":{"integratedHsp":true}}"#,
    )
    .unwrap();
    write_app_test_artifact(&module_remote.join("shared.hsp"), "shared", "shared");
    write_app_test_artifact(&project_remote.join("shared.hsp"), "shared", "shared");
    let scopes = vec![RemoteHspDependencyScope::new(
        module_root,
        ["@scope/shared".to_string()],
    )];
    let cache = tmp.path().join("build/cache/default/integrated_hsp");

    let artifacts = RemoteHspCollector::new(tmp.path(), &scopes)
        .collect_for_app("com.host.app", 42, Some("2026.8.1"), &cache)
        .unwrap();

    assert_eq!(artifacts.len(), 1);
    assert_eq!(artifacts[0].path, cache.join("shared-project/shared.hsp"));
    let file = fs::File::open(&artifacts[0].path).unwrap();
    let mut archive = zip::ZipArchive::new(file).unwrap();
    let mut pack_info = String::new();
    archive
        .by_name("pack.info")
        .unwrap()
        .read_to_string(&mut pack_info)
        .unwrap();
    let value: serde_json::Value = serde_json::from_str(&pack_info).unwrap();
    assert_eq!(value["summary"]["app"]["bundleName"], "com.host.app");
    assert_eq!(value["summary"]["app"]["version"]["code"], 42);
    assert_eq!(value["summary"]["app"]["version"]["build"], "2026.8.1");
}

#[test]
fn app_artifact_stager_uses_hvigor_app_directory_and_unsigned_name_rule() {
    let tmp = tempfile::tempdir().unwrap();
    let output_dir = tmp.path().join("entry/build/default/outputs/default");
    fs::create_dir_all(&output_dir).unwrap();
    let source = output_dir.join("entry-default-unsigned.hap");
    fs::write(&source, b"hap").unwrap();
    let artifacts = vec![AppArtifactInput::local(source, "entry")];

    let staged = AppArtifactStager::new(&artifacts).stage().unwrap();

    assert_eq!(
        staged.hap_paths,
        vec![output_dir.join("app/entry-default.hap")]
    );
    assert_eq!(fs::read(&staged.hap_paths[0]).unwrap(), b"hap");
}

#[test]
fn app_packing_command_matches_latest_hvigor_on_windows() {
    let tmp = tempfile::tempdir().unwrap();
    let tool = tmp.path().join("app_packing_tool.jar");
    let pack_info = tmp.path().join("pack.info");
    let hap = tmp.path().join("entry-default.hap");
    let pac = tmp.path().join("pac.json");
    for path in [&tool, &pack_info, &hap, &pac] {
        fs::write(path, b"input").unwrap();
    }
    let mut plan = AppPackagePlan::new(
        pack_info.clone(),
        vec![hap.clone()],
        vec![],
        tmp.path().join("project-default-unsigned.app"),
    );
    plan.pac_json_path = Some(pac.clone());
    plan.replace_pack_info = Some(false);
    let runner = AppPackageRunner::for_host(
        tool.clone(),
        std::ffi::OsString::from("java.exe"),
        HostPlatform::Windows,
    );

    let command = runner.pack_command(&plan).unwrap();
    let args = command
        .args()
        .iter()
        .map(|value| value.to_string_lossy().into_owned())
        .collect::<Vec<_>>();

    assert_eq!(command.program(), std::ffi::OsStr::new("java.exe"));
    assert_eq!(args[0], "-Dfile.encoding=GBK");
    assert_eq!(args[1], "-jar");
    assert_eq!(args[2], tool.to_string_lossy());
    assert_eq!(
        args[3..7],
        [
            "--mode",
            "app",
            "--pack-info-path",
            pack_info.to_str().unwrap()
        ]
    );
    assert!(args
        .windows(2)
        .any(|pair| pair == ["--hap-path", hap.to_str().unwrap()]));
    assert!(args.windows(2).any(|pair| pair == ["--hsp-path", ""]));
    assert!(args
        .windows(2)
        .any(|pair| pair == ["--replace-pack-info", "false"]));
    assert!(args
        .windows(2)
        .any(|pair| pair == ["--pac-json-path", pac.to_str().unwrap()]));
}

#[test]
fn app_general_normalize_command_matches_latest_hvigor_on_windows() {
    let tmp = tempfile::tempdir().unwrap();
    let tool = tmp.path().join("app_packing_tool.jar");
    let hap = tmp.path().join("entry-default.hap");
    let hsp = tmp.path().join("shared-default.hsp");
    for path in [&tool, &hap, &hsp] {
        fs::write(path, b"input").unwrap();
    }
    let output = tmp.path().join("package_app");
    let plan = AppNormalizePlan::new(
        vec![hap.clone(), hsp.clone()],
        vec!["phone".to_string(), "tablet".to_string()],
        output.clone(),
    );
    let runner = AppPackageRunner::for_host(
        tool.clone(),
        std::ffi::OsString::from("java.exe"),
        HostPlatform::Windows,
    );

    let command = runner.normalize_device_types_command(&plan).unwrap();
    let args = command
        .args()
        .iter()
        .map(|value| value.to_string_lossy().into_owned())
        .collect::<Vec<_>>();

    assert_eq!(args[0], "-Dfile.encoding=GBK");
    assert_eq!(
        args[3..],
        [
            "--mode".to_string(),
            "generalNormalize".to_string(),
            "--device-types".to_string(),
            "phone,tablet".to_string(),
            "--input-list".to_string(),
            format!("{},{}", hap.display(), hsp.display()),
            "--out-path".to_string(),
            output.to_string_lossy().into_owned(),
        ]
    );
}

#[test]
fn app_archive_unpacker_extracts_signable_packages_and_cleans_exact_output() {
    use zip::write::SimpleFileOptions;

    let tmp = tempfile::tempdir().unwrap();
    let app = tmp.path().join("project-default-unsigned.app");
    let file = fs::File::create(&app).unwrap();
    let mut archive = zip::ZipWriter::new(file);
    let stored = SimpleFileOptions::default().compression_method(zip::CompressionMethod::Stored);
    archive.start_file("entry-default.hap", stored).unwrap();
    archive.write_all(b"hap bytes").unwrap();
    archive.start_file("shared-default.hsp", stored).unwrap();
    archive.write_all(b"hsp bytes").unwrap();
    archive.start_file("pack.info", stored).unwrap();
    archive.write_all(b"{}").unwrap();
    archive.finish().unwrap();

    let unpacker = AppArchiveUnpacker::new(&app);
    let unpacked = unpacker.unpack().unwrap();

    assert_eq!(
        unpacked.hap_paths,
        vec![unpacked.root_path.join("entry-default.hap")]
    );
    assert_eq!(
        unpacked.hsp_paths,
        vec![unpacked.root_path.join("shared-default.hsp")]
    );
    assert_eq!(fs::read(&unpacked.hap_paths[0]).unwrap(), b"hap bytes");
    assert!(unpacked.root_path.join("pack.info").is_file());

    unpacker.cleanup(&unpacked).unwrap();
    assert!(!unpacked.root_path.exists());
}
