pub mod error;
pub mod integrated_hsp;
pub mod packager;
pub mod zip_writer;

pub use error::PackError;
pub use integrated_hsp::{normalize_integrated_hsps, IntegratedHspEntry};
pub use packager::{HarPackInput, PackAbility, PackInput, Packager};

#[cfg(test)]
mod tests {
    use std::fs;
    use std::sync::Arc;

    use arkdown_core::{
        ArkdownConfig, BuildMode, BuildOption, OutputTarget, ProductConfig, SdkConfig,
        StrictModeConfig,
    };

    use super::*;

    fn make_config() -> Arc<ArkdownConfig> {
        Arc::new(ArkdownConfig {
            project_root: std::path::PathBuf::from("/tmp"),
            build_mode: BuildMode::Debug,
            output_target: OutputTarget::Hap,
            product: ProductConfig {
                name: "default".to_string(),
                bundle_name: "com.test.app".to_string(),
                compatible_sdk_version: "5.0.0(12)".to_string(),
                runtime_os: "HarmonyOS".to_string(),
                signing_config: None,
                build_option: BuildOption {
                    preload_so: vec![],
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

    #[test]
    fn test_pack_hap_produces_zip() {
        let tmp = tempfile::tempdir().unwrap();
        let abc = tmp.path().join("modules.abc");
        let res = tmp.path().join("resources.index");
        let modj = tmp.path().join("module.json");
        let out = tmp.path().join("entry.hap");

        fs::write(&abc, b"PAND\x00\x00\x00\x00").unwrap();
        fs::write(&res, b"\xf0\x0f\x12\x34\x00\x00\x00\x00").unwrap();
        fs::write(&modj, r#"{"module":{"name":"entry","type":"entry"}}"#).unwrap();

        let packager = Packager::new(make_config());
        let result = packager.pack_hap(PackInput {
            module_name: "entry".to_string(),
            abc_path: abc,
            resources_index: res,
            module_json: modj,
            native_libs: vec![],
            output_path: out.clone(),
            resources_src_dir: None,
            bundle_version_code: 1000000,
            bundle_version_name: "1.0.0".to_string(),
            bundle_type: "app".to_string(),
            integrated_hsps: vec![],
            privacy_profile_path: None,
            source_maps_path: None,
            abilities: vec![],
        });
        assert!(result.is_ok(), "{:?}", result.err());
        assert!(out.exists());

        // Verify it's a valid zip by reading it
        let file = fs::File::open(&out).unwrap();
        let archive = zip::ZipArchive::new(file).unwrap();
        let names: Vec<_> = archive.file_names().collect();
        assert!(
            names.iter().any(|n| *n == "module.json"),
            "missing module.json; got: {:?}",
            names
        );
        assert!(
            names.iter().any(|n| n.contains(".abc")),
            "missing .abc; got: {:?}",
            names
        );
    }

    #[test]
    fn test_pack_hap_abc_stored_at_ets_path_for_api12() {
        let tmp = tempfile::tempdir().unwrap();
        let abc = tmp.path().join("modules.abc");
        let res = tmp.path().join("resources.index");
        let modj = tmp.path().join("module.json");
        let out = tmp.path().join("entry.hap");

        fs::write(&abc, b"PAND").unwrap();
        fs::write(&res, b"\x00\x00").unwrap();
        fs::write(&modj, r#"{"module":{"name":"entry","type":"entry"}}"#).unwrap();

        let packager = Packager::new(make_config()); // compile_api_version = 12
        packager
            .pack_hap(PackInput {
                module_name: "entry".to_string(),
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
            })
            .unwrap();

        let file = fs::File::open(&out).unwrap();
        let archive = zip::ZipArchive::new(file).unwrap();
        let names: Vec<_> = archive.file_names().collect();
        assert!(
            names.iter().any(|n| *n == "ets/modules.abc"),
            "expected ets/modules.abc for API 12; got: {:?}",
            names
        );
    }

    #[test]
    fn test_pack_hap_missing_module_json_errors() {
        let tmp = tempfile::tempdir().unwrap();
        let packager = Packager::new(make_config());
        let result = packager.pack_hap(PackInput {
            module_name: "entry".to_string(),
            abc_path: tmp.path().join("missing.abc"),
            resources_index: tmp.path().join("missing.index"),
            module_json: tmp.path().join("missing.json"),
            native_libs: vec![],
            output_path: tmp.path().join("out.hap"),
            resources_src_dir: None,
            bundle_version_code: 1000000,
            bundle_version_name: "1.0.0".to_string(),
            bundle_type: "app".to_string(),
            integrated_hsps: vec![],
            privacy_profile_path: None,
            source_maps_path: None,
            abilities: vec![],
        });
        assert!(result.is_err());
        let err = result.unwrap_err();
        assert!(
            matches!(err, PackError::MissingInput(_)),
            "expected MissingInput, got: {:?}",
            err
        );
    }

    #[test]
    fn test_pack_har_includes_ets_sources() {
        let tmp = tempfile::tempdir().unwrap();
        let src_dir = tmp.path().join("src/main/ets");
        fs::create_dir_all(&src_dir).unwrap();
        fs::write(src_dir.join("Index.ets"), b"export struct Index {}").unwrap();

        let pkg_json = tmp.path().join("oh-package.json5");
        fs::write(&pkg_json, r#"{"name":"mylib"}"#).unwrap();

        let out = tmp.path().join("mylib.har");

        let packager = Packager::new(make_config());
        let result = packager.pack_har(HarPackInput {
            module_name: "mylib".to_string(),
            source_dir: src_dir.clone(),
            resources_dir: tmp.path().join("src/main/resources"),
            oh_package_json: pkg_json,
            output_path: out.clone(),
        });
        assert!(result.is_ok(), "{:?}", result.err());
        assert!(out.exists());

        let file = fs::File::open(&out).unwrap();
        let archive = zip::ZipArchive::new(file).unwrap();
        let names: Vec<_> = archive.file_names().collect();
        assert!(
            names.iter().any(|n| n.ends_with("Index.ets")),
            "missing Index.ets; got: {:?}",
            names
        );
        assert!(
            names.iter().any(|n| *n == "oh-package.json5"),
            "missing oh-package.json5; got: {:?}",
            names
        );
    }

    #[test]
    fn test_pack_hsp_same_as_hap() {
        let tmp = tempfile::tempdir().unwrap();
        let abc = tmp.path().join("modules.abc");
        let res = tmp.path().join("resources.index");
        let modj = tmp.path().join("module.json");
        let out = tmp.path().join("shared.hsp");

        fs::write(&abc, b"PAND").unwrap();
        fs::write(&res, b"\x00\x00").unwrap();
        fs::write(&modj, r#"{"module":{"name":"shared","type":"shared"}}"#).unwrap();

        let packager = Packager::new(make_config());
        let result = packager.pack_hsp(PackInput {
            module_name: "shared".to_string(),
            abc_path: abc,
            resources_index: res,
            module_json: modj,
            native_libs: vec![],
            output_path: out.clone(),
            resources_src_dir: None,
            bundle_version_code: 1000000,
            bundle_version_name: "1.0.0".to_string(),
            bundle_type: "app".to_string(),
            integrated_hsps: vec![],
            privacy_profile_path: None,
            source_maps_path: None,
            abilities: vec![],
        });
        assert!(result.is_ok(), "{:?}", result.err());
        assert!(out.exists());
    }

    #[test]
    #[ignore = "preload SO embedding not yet functional"]
    fn test_pack_hap_embeds_preload_so() {
        let tmp = tempfile::tempdir().unwrap();
        let abc = tmp.path().join("modules.abc");
        let res = tmp.path().join("resources.index");
        let modj = tmp.path().join("module.json");
        let so_file = tmp.path().join("libpreload.so");
        let out = tmp.path().join("entry.hap");

        fs::write(&abc, b"PAND").unwrap();
        fs::write(&res, b"\x00\x00").unwrap();
        fs::write(&modj, r#"{"module":{"name":"entry","type":"entry"}}"#).unwrap();
        fs::write(&so_file, b"\x7fELF").unwrap();

        let mut cfg = (*make_config()).clone();
        cfg.product.build_option.preload_so = vec![so_file.clone()];
        let packager = Packager::new(Arc::new(cfg));

        packager
            .pack_hap(PackInput {
                module_name: "entry".to_string(),
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
                privacy_profile_path: None,
                integrated_hsps: vec![],
            })
            .unwrap();

        let file = fs::File::open(&out).unwrap();
        let archive = zip::ZipArchive::new(file).unwrap();
        let names: Vec<_> = archive.file_names().collect();
        assert!(
            names.iter().any(|n| *n == "libs/arm64-v8a/libpreload.so"),
            "missing preload SO in HAP; got: {:?}",
            names
        );
    }

    #[test]
    #[ignore = "privacy profile embedding not yet functional"]
    fn test_pack_hap_embeds_privacy_profile() {
        let tmp = tempfile::tempdir().unwrap();
        let abc = tmp.path().join("modules.abc");
        let res = tmp.path().join("resources.index");
        let modj = tmp.path().join("module.json");
        let privacy = tmp.path().join("privacy.json");
        let out = tmp.path().join("entry.hap");

        fs::write(&abc, b"PAND").unwrap();
        fs::write(&res, b"\x00\x00").unwrap();
        fs::write(&modj, r#"{"module":{"name":"entry","type":"entry"}}"#).unwrap();
        fs::write(&privacy, r#"{"dataProcess":[]}"#).unwrap();

        let packager = Packager::new(make_config());
        packager
            .pack_hap(PackInput {
                module_name: "entry".to_string(),
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
                privacy_profile_path: Some(privacy),
                integrated_hsps: vec![],
            })
            .unwrap();

        let file = fs::File::open(&out).unwrap();
        let archive = zip::ZipArchive::new(file).unwrap();
        let names: Vec<_> = archive.file_names().collect();
        assert!(
            names.iter().any(|n| *n == "module.privacy.json"),
            "missing module.privacy.json in HAP; got: {:?}",
            names
        );
    }
}
