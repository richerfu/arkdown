pub mod app;
pub mod error;
pub mod integrated_hsp;
pub mod packager;
pub mod packing_check;
pub mod plan;
pub mod zip_writer;

pub use app::{
    AppArchiveUnpacker, AppArtifactFormat, AppArtifactInput, AppArtifactRole, AppArtifactStager,
    AppModulePackageKind, AppNormalizePlan, AppPackageMode, AppPackagePlan, AppPackageRunner,
    AppPackingCommand, HqfPackagePlan, ProjectPackInfoBuilder, ProjectPackInfoOutput,
    RemoteHspCandidate, RemoteHspCollector, RemoteHspDependencyScope, StagedAppArtifacts,
    UnpackedAppArtifacts,
};
pub use error::PackError;
pub use integrated_hsp::{
    normalize_integrated_hsps, IntegratedHspCandidate, IntegratedHspCollector, IntegratedHspEntry,
};
pub use packager::{
    minify_package_intermediate_module_json, BundledHarResourceInput, HarBundledDependencyInput,
    HarPackInput, HarPackageParameterInput, HarVariantInput, HspInterfaceHarInput, HspTgzInput,
    PackInfoInput, PackInfoModuleInput, PackInput, Packager,
};
pub use packing_check::PackingCheck;
pub use plan::{
    PackageEntry, PackageEntrySource, PackageKind, PackageOutput, PackagePlan, PackageToolMode,
};

#[cfg(test)]
mod tests {
    use std::fs;
    use std::io::Read;
    use std::sync::Arc;

    use arkdown_core::{
        ArkdownConfig, BuildMode, BuildOption, CompileMode, ExtensionAbilityEntry, HapManifest,
        ModuleConfig, NativeLibOption, OutputTarget, ProductConfig, SdkConfig, StrictModeConfig,
    };

    use super::*;

    fn make_config() -> Arc<ArkdownConfig> {
        Arc::new(ArkdownConfig {
            project_root: std::path::PathBuf::from("/tmp"),
            build_mode: BuildMode::Debug,
            output_target: OutputTarget::Hap,
            product: ProductConfig {
                name: "default".to_string(),
                artifact_name: None,
                arkts_version: Default::default(),
                bundle_name: "com.test.app".to_string(),
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
                        no_external_import_by_path: None,
                        duplicate_dependency_check: false,
                        api_compatibility_check: None,
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
            target_name: "default".to_string(),
            abc_path: abc,
            additional_abc_paths: Vec::new(),
            resources_index: res,
            module_json: modj,
            pack_info_module_json: None,
            pack_info_path: None,
            native_libs: vec![],
            compress_native_libs: false,
            output_path: out.clone(),
            resources_src_dir: None,
            bundle_version_code: 1000000,
            bundle_version_name: "1.0.0".to_string(),
            bundle_build_version: None,
            bundle_type: "app".to_string(),
            integrated_hsps: vec![],
            privacy_profile_path: None,
            syscap_path: None,
            device_types: vec![],
            installation_free: false,
            delivery_with_install: true,
            source_maps_path: None,
            aot_mode: None,
            pkg_context_info_path: None,
        });
        assert!(result.is_ok(), "{:?}", result.err());
        assert!(out.exists());

        // Verify it's a valid zip by reading it
        let file = fs::File::open(&out).unwrap();
        let archive = zip::ZipArchive::new(file).unwrap();
        let names: Vec<_> = archive.file_names().collect();
        assert!(
            names.contains(&"module.json"),
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
                target_name: "default".to_string(),
                abc_path: abc,
                additional_abc_paths: Vec::new(),
                resources_index: res,
                module_json: modj,
                pack_info_module_json: None,
                pack_info_path: None,
                native_libs: vec![],
                compress_native_libs: false,
                output_path: out.clone(),
                resources_src_dir: None,
                bundle_version_code: 1000000,
                bundle_version_name: "1.0.0".to_string(),
                bundle_build_version: None,
                bundle_type: "app".to_string(),
                source_maps_path: None,
                aot_mode: None,
                pkg_context_info_path: None,
                integrated_hsps: vec![],
                privacy_profile_path: None,
                syscap_path: None,
                device_types: vec![],
                installation_free: false,
                delivery_with_install: true,
            })
            .unwrap();

        let file = fs::File::open(&out).unwrap();
        let archive = zip::ZipArchive::new(file).unwrap();
        let names: Vec<_> = archive.file_names().collect();
        assert!(
            names.contains(&"ets/modules.abc"),
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
            target_name: "default".to_string(),
            abc_path: tmp.path().join("missing.abc"),
            additional_abc_paths: Vec::new(),
            resources_index: tmp.path().join("missing.index"),
            module_json: tmp.path().join("missing.json"),
            pack_info_module_json: None,
            pack_info_path: None,
            native_libs: vec![],
            compress_native_libs: false,
            output_path: tmp.path().join("out.hap"),
            resources_src_dir: None,
            bundle_version_code: 1000000,
            bundle_version_name: "1.0.0".to_string(),
            bundle_build_version: None,
            bundle_type: "app".to_string(),
            integrated_hsps: vec![],
            privacy_profile_path: None,
            syscap_path: None,
            device_types: vec![],
            installation_free: false,
            delivery_with_install: true,
            source_maps_path: None,
            aot_mode: None,
            pkg_context_info_path: None,
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
            resource_str_dir: None,
            oh_package_json: pkg_json,
            output_path: out.clone(),
            declarations_dir: None,
            consumer_rules_path: None,
        });
        assert!(result.is_ok(), "{:?}", result.err());
        assert!(out.exists());

        let file = fs::File::open(&out).unwrap();
        let gzip = flate2::read::GzDecoder::new(file);
        let mut archive = tar::Archive::new(gzip);
        let names = archive
            .entries()
            .unwrap()
            .map(|entry| {
                entry
                    .unwrap()
                    .path()
                    .unwrap()
                    .to_string_lossy()
                    .into_owned()
            })
            .collect::<Vec<_>>();
        assert!(
            names.iter().any(|n| n.ends_with("Index.ets")),
            "missing Index.ets; got: {:?}",
            names
        );
        assert!(
            names.iter().any(|n| n == "package/oh-package.json5"),
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
            target_name: "default".to_string(),
            abc_path: abc,
            additional_abc_paths: Vec::new(),
            resources_index: res,
            module_json: modj,
            pack_info_module_json: None,
            pack_info_path: None,
            native_libs: vec![],
            compress_native_libs: false,
            output_path: out.clone(),
            resources_src_dir: None,
            bundle_version_code: 1000000,
            bundle_version_name: "1.0.0".to_string(),
            bundle_build_version: None,
            bundle_type: "app".to_string(),
            integrated_hsps: vec![],
            privacy_profile_path: None,
            syscap_path: None,
            device_types: vec![],
            installation_free: false,
            delivery_with_install: true,
            source_maps_path: None,
            aot_mode: None,
            pkg_context_info_path: None,
        });
        assert!(result.is_ok(), "{:?}", result.err());
        assert!(out.exists());
    }

    #[test]
    fn test_pack_hap_does_not_embed_build_option_preload_so_as_native_lib() {
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
                target_name: "default".to_string(),
                abc_path: abc,
                additional_abc_paths: Vec::new(),
                resources_index: res,
                module_json: modj,
                pack_info_module_json: None,
                pack_info_path: None,
                native_libs: vec![],
                compress_native_libs: false,
                output_path: out.clone(),
                resources_src_dir: None,
                bundle_version_code: 1000000,
                bundle_version_name: "1.0.0".to_string(),
                bundle_build_version: None,
                bundle_type: "app".to_string(),
                source_maps_path: None,
                aot_mode: None,
                pkg_context_info_path: None,
                privacy_profile_path: None,
                syscap_path: None,
                device_types: vec![],
                installation_free: false,
                delivery_with_install: true,
                integrated_hsps: vec![],
            })
            .unwrap();

        let file = fs::File::open(&out).unwrap();
        let archive = zip::ZipArchive::new(file).unwrap();
        let names: Vec<_> = archive.file_names().collect();
        assert!(
            !names.contains(&"libs/arm64-v8a/libpreload.so"),
            "buildOption.preloadSo must not be treated as a native lib; got: {:?}",
            names
        );
    }

    #[test]
    fn test_pack_hap_embeds_privacy_profile() {
        let tmp = tempfile::tempdir().unwrap();
        let abc = tmp.path().join("modules.abc");
        let res = tmp.path().join("resources.index");
        let modj = tmp.path().join("module.json");
        let privacy = tmp.path().join("pac.json5");
        let out = tmp.path().join("entry.hap");

        fs::write(&abc, b"PAND").unwrap();
        fs::write(&res, b"\x00\x00").unwrap();
        fs::write(&modj, r#"{"module":{"name":"entry","type":"entry"}}"#).unwrap();
        fs::write(&privacy, r#"{ dataProcess: [], specialAPIs: [] }"#).unwrap();

        let packager = Packager::new(make_config());
        packager
            .pack_hap(PackInput {
                module_name: "entry".to_string(),
                target_name: "default".to_string(),
                abc_path: abc,
                additional_abc_paths: Vec::new(),
                resources_index: res,
                module_json: modj,
                pack_info_module_json: None,
                pack_info_path: None,
                native_libs: vec![],
                compress_native_libs: false,
                output_path: out.clone(),
                resources_src_dir: None,
                bundle_version_code: 1000000,
                bundle_version_name: "1.0.0".to_string(),
                bundle_build_version: None,
                bundle_type: "app".to_string(),

                source_maps_path: None,
                aot_mode: None,
                pkg_context_info_path: None,
                privacy_profile_path: Some(privacy),
                syscap_path: None,
                integrated_hsps: vec![],
                device_types: vec![],
                installation_free: false,
                delivery_with_install: true,
            })
            .unwrap();

        let file = fs::File::open(&out).unwrap();
        let mut archive = zip::ZipArchive::new(file).unwrap();
        let names: Vec<_> = archive.file_names().map(str::to_string).collect();
        assert!(
            names.iter().any(|n| n == "module.privacy.json"),
            "missing module.privacy.json in HAP; got: {:?}",
            names
        );
        let mut privacy_entry = String::new();
        archive
            .by_name("module.privacy.json")
            .unwrap()
            .read_to_string(&mut privacy_entry)
            .unwrap();
        assert_eq!(privacy_entry, r#"{"dataProcess":[],"specialAPIs":[]}"#);
    }

    #[test]
    fn packing_check_requires_native_extension_entry_for_default_abi() {
        let temp = tempfile::tempdir().expect("temporary packing layout");
        let module_root = temp.path().join("entry");
        let module_json = module_root.join("src/main/module.json5");
        let config = make_config();
        let module = ModuleConfig {
            name: "entry".to_string(),
            src_path: module_root,
            module_type: arkdown_core::OhosModuleType::Entry,
        };
        let manifest = HapManifest {
            module_name: "entry".to_string(),
            module_type: arkdown_core::OhosModuleType::Entry,
            device_types: Vec::new(),
            virtual_machine: "ark12.0.6.0".to_string(),
            compile_mode: CompileMode::EsModule,
            abilities: Vec::new(),
            extension_abilities: vec![ExtensionAbilityEntry {
                name: "NativeAbility".to_string(),
                src_entry: "libability.so".to_string(),
                extension_type: "service".to_string(),
                label: None,
                description: None,
                metadata: Vec::new(),
            }],
            dependencies: Vec::new(),
            router_map: None,
            pages: None,
            main_element: None,
            description: None,
            src_entry: None,
        };
        let stripped = temp.path().join("stripped_native_libs");
        let check = PackingCheck::new(
            &config,
            &module,
            &manifest,
            temp.path().join("loader_out"),
            stripped.clone(),
            temp.path().join("resources"),
        );

        let error = check
            .validate()
            .expect_err("missing extension ability library must fail packing");
        assert_eq!(
            error.to_string(),
            format!(
                "MODULE_SRC_ENTRY_NOT_FOUND: 'libability.so' is not present under stripped_native_libs/arm64-v8a (referenced by {})",
                module_json.display()
            )
        );

        fs::create_dir_all(stripped.join("arm64-v8a")).expect("create stripped ABI directory");
        fs::write(stripped.join("arm64-v8a/libability.so"), b"native")
            .expect("write extension ability library");
        check
            .validate()
            .expect("present extension ability library must pass packing");
    }
}
