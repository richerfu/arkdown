use super::*;
use crate::plan::{BundleMockConfig, BundleRuntimePackageContext, BundleRuntimePackageInfo};
use rustc_hash::FxHashMap;

fn runtime_packages(contexts: &[(&str, &str, &str, &str, &str, bool)]) -> BundleRuntimePackageInfo {
    let mut packages = FxHashMap::default();
    for (key, package_name, bundle_name, module_name, version, is_so) in contexts {
        packages.insert(
            (*key).to_string(),
            BundleRuntimePackageContext {
                package_name: (*package_name).to_string(),
                bundle_name: (*bundle_name).to_string(),
                module_name: (*module_name).to_string(),
                version: (*version).to_string(),
                entry_path: if *key == "phone" || *key == "audioplayer" {
                    "src/main/".to_string()
                } else {
                    "index".to_string()
                },
                is_so: *is_so,
            },
        );
    }
    BundleRuntimePackageInfo { packages }
}

/// Test-only replacement for the removed
/// `HvigorSourceSpan::for_module_request_literal` helper: locate a module
/// request literal's span through the shared ArkTS inventory.
fn source_span_for_module_request_literal(
    source: &str,
    module_id: &str,
    specifier: &str,
) -> Option<HvigorSourceSpan> {
    HvigorModuleRequestSpecifiers::new(source, module_id)
        .collect_literals()
        .into_iter()
        .find(|literal| literal.specifier == specifier)
        .map(|literal| literal.source_span)
}

fn record_resolved_import(
    state: &mut HvigorModuleSourceState,
    importer: &str,
    specifier: &str,
    resolved_id: &str,
) {
    state
        .resolved_imports
        .entry(importer.to_string())
        .or_default()
        .insert(specifier.to_string(), resolved_id.to_string());
    if let Some(source) = state.sources.get(importer) {
        if let Some(source_span) =
            source_span_for_module_request_literal(source.code.as_ref(), importer, specifier)
        {
            state
                .resolved_import_records
                .entry(importer.to_string())
                .or_default()
                .push(HvigorResolvedImportRecord {
                    specifier: specifier.to_string(),
                    source_span,
                });
        }
    }
}

#[test]
fn module_source_writer_rewrites_unresolved_native_so_static_import() {
    let app_root = tempfile::tempdir().unwrap();
    let project_root = app_root.path();
    let phone_root = project_root.join("phone");
    let entry_id = phone_root
        .join("src/main/ets/entryability/EntryAbility.ets")
        .to_string_lossy()
        .into_owned();
    let module_path_map = HvigorModulePathMap::new("phone", &phone_root, &[]);
    let mut state = HvigorModuleSourceState::default();
    state.sources.insert(
        entry_id.clone(),
        HvigorCollectedModuleSource {
            code: "import { setHornLxDict } from \"liblx.so\";"
                .to_string()
                .into(),
        },
    );
    record_resolved_import(&mut state, &entry_id, "liblx.so", "liblx.so");
    let facts = ModuleGraphFacts {
        full_module_ids: vec![entry_id.clone()],
        full_module_is_commonjs: FxHashMap::default(),
    };
    let out_dir = app_root
        .path()
        .join("cache")
        .join("default@CompileArkTS")
        .join("esmodule")
        .join("debug");
    let mut packages = FxHashMap::default();
    packages.insert(
        "phone".to_string(),
        BundleRuntimePackageContext {
            package_name: "phone".to_string(),
            ..BundleRuntimePackageContext::default()
        },
    );
    packages.insert(
        "liblx.so".to_string(),
        BundleRuntimePackageContext {
            package_name: "liblx.so".to_string(),
            is_so: true,
            ..BundleRuntimePackageContext::default()
        },
    );
    let runtime_packages = BundleRuntimePackageInfo { packages };

    let files = HvigorModuleSourceWriter::new(
        "com.example.hmosworld",
        "phone",
        project_root,
        &module_path_map,
        &runtime_packages,
        &[],
        true,
        &facts,
        facts.full_module_ids.clone(),
        state,
        out_dir.clone(),
    )
    .write()
    .unwrap();

    let entry_file = files
        .iter()
        .find(|file| file.module_id == entry_id)
        .expect("entry module source");
    assert_eq!(entry_file.imported_record_names, vec!["&liblx.so&"]);
    let output = std::fs::read_to_string(&entry_file.path).unwrap();
    assert!(output.contains("\"@normalized:Y&&&liblx.so&\""), "{output}");
    assert!(!output.contains("@app:"), "{output}");

    let stale_file = out_dir.join("stale").join("Old.ts");
    fs_err::create_dir_all(stale_file.parent().unwrap()).unwrap();
    fs_err::write(&stale_file, "old").unwrap();
    let source_maps_cache = out_dir.join("sourceMaps.json");
    let program_cache = out_dir.join("modules.cache");
    fs_err::write(&source_maps_cache, r#"{ "Index.ts": {} }"#).unwrap();
    fs_err::write(&program_cache, "module-cache").unwrap();

    let mut next_state = HvigorModuleSourceState::default();
    next_state.sources.insert(
        entry_id.clone(),
        HvigorCollectedModuleSource {
            code: "import { setHornLxDict } from \"liblx.so\";"
                .to_string()
                .into(),
        },
    );
    record_resolved_import(&mut next_state, &entry_id, "liblx.so", "liblx.so");

    let next_files = HvigorModuleSourceWriter::new(
        "com.example.hmosworld",
        "phone",
        project_root,
        &module_path_map,
        &runtime_packages,
        &[],
        true,
        &facts,
        facts.full_module_ids.clone(),
        next_state,
        out_dir,
    )
    .write()
    .unwrap();
    let next_entry_file = next_files
        .iter()
        .find(|file| file.module_id == entry_id)
        .expect("entry module source after second write");
    assert!(!stale_file.exists(), "stale module source must be removed");
    assert_eq!(
        fs_err::read_to_string(&source_maps_cache).unwrap(),
        r#"{ "Index.ts": {} }"#,
        "CompileArkTS source-map cache is not a stale module source"
    );
    assert_eq!(
        fs_err::read_to_string(&program_cache).unwrap(),
        "module-cache",
        "CompileArkTS ProgramCache is not a stale module source"
    );
    assert_eq!(
        std::fs::read_to_string(&next_entry_file.path).unwrap(),
        output
    );
}

#[test]
fn module_source_writer_rewrites_type_only_static_imports_with_hvigor_requests() {
    let app_root = tempfile::tempdir().unwrap();
    let project_root = app_root.path();
    let meituan_root = project_root.join("meituan");
    let entry_id = meituan_root
        .join("src/main/ets/pages/TypeOnly.ets")
        .to_string_lossy()
        .into_owned();
    let module_path_map = HvigorModulePathMap::new("meituan", &meituan_root, &[]);
    let mut state = HvigorModuleSourceState::default();
    state.sources.insert(
        entry_id.clone(),
        HvigorCollectedModuleSource {
            code: r#"
import type Want from "@ohos.app.ability.Want";
import type { MapBridge } from "libmsimap.so";
import type { IMtBluetoothAccess } from "@meituan/privacy";

export type Use = Want | MapBridge | IMtBluetoothAccess;
"#
            .to_string()
            .into(),
        },
    );
    let facts = ModuleGraphFacts {
        full_module_ids: vec![entry_id.clone()],
        full_module_is_commonjs: FxHashMap::default(),
    };
    let out_dir = app_root
        .path()
        .join("cache")
        .join("default@CompileArkTS")
        .join("esmodule")
        .join("debug");
    let mut packages = FxHashMap::default();
    packages.insert(
        "meituan".to_string(),
        BundleRuntimePackageContext {
            package_name: "meituan".to_string(),
            ..BundleRuntimePackageContext::default()
        },
    );
    packages.insert(
        "libmsimap.so".to_string(),
        BundleRuntimePackageContext {
            package_name: "libmsimap.so".to_string(),
            is_so: true,
            ..BundleRuntimePackageContext::default()
        },
    );
    packages.insert(
        "@meituan/privacy".to_string(),
        BundleRuntimePackageContext {
            package_name: "@meituan/privacy".to_string(),
            version: "0.3.20".to_string(),
            entry_path: "Index.d.ets".to_string(),
            ..BundleRuntimePackageContext::default()
        },
    );
    let runtime_packages = BundleRuntimePackageInfo { packages };
    let byte_code_har_packages = vec!["@meituan/privacy".to_string()];

    let files = HvigorModuleSourceWriter::new(
        "com.sankuai.hmeituan",
        "meituan",
        project_root,
        &module_path_map,
        &runtime_packages,
        &byte_code_har_packages,
        true,
        &facts,
        facts.full_module_ids.clone(),
        state,
        out_dir,
    )
    .write()
    .unwrap();

    let entry_file = files
        .iter()
        .find(|file| file.module_id == entry_id)
        .expect("entry module source");
    assert_eq!(
        entry_file.imported_record_names,
        vec![
            "&@meituan/privacy/Index&0.3.20".to_string(),
            "&libmsimap.so&".to_string()
        ]
    );
    let output = std::fs::read_to_string(&entry_file.path).unwrap();
    assert!(output.contains("\"@ohos:app.ability.Want\""), "{output}");
    assert!(
        output.contains("\"@normalized:Y&&&libmsimap.so&\""),
        "{output}"
    );
    assert!(
        output.contains("\"@normalized:N&&&@meituan/privacy/Index&0.3.20\""),
        "{output}"
    );
}

#[test]
fn module_source_writer_records_bytecode_har_import_records_for_compile_context() {
    let app_root = tempfile::tempdir().unwrap();
    let project_root = app_root.path();
    let phone_root = project_root.join("phone");
    let entry_id = phone_root
        .join("src/main/ets/entryability/EntryAbility.ets")
        .to_string_lossy()
        .into_owned();
    let module_path_map = HvigorModulePathMap::new("phone", &phone_root, &[]);
    let mut state = HvigorModuleSourceState::default();
    state.sources.insert(
        entry_id.clone(),
        HvigorCollectedModuleSource {
            code: "import { IQQOpenApi } from \"@tencent/qq-open-sdk\";"
                .to_string()
                .into(),
        },
    );
    state
        .resolved_imports
        .entry(entry_id.clone())
        .or_default()
        .insert(
            "@tencent/qq-open-sdk".to_string(),
            "@tencent/qq-open-sdk".to_string(),
        );
    let facts = ModuleGraphFacts {
        full_module_ids: vec![entry_id.clone()],
        full_module_is_commonjs: FxHashMap::default(),
    };
    let out_dir = app_root
        .path()
        .join("cache")
        .join("default@CompileArkTS")
        .join("esmodule")
        .join("release");
    let mut packages = FxHashMap::default();
    packages.insert(
        "phone".to_string(),
        BundleRuntimePackageContext {
            package_name: "phone".to_string(),
            ..BundleRuntimePackageContext::default()
        },
    );
    packages.insert(
        "@tencent/qq-open-sdk".to_string(),
        BundleRuntimePackageContext {
            package_name: "@tencent/qq-open-sdk".to_string(),
            version: "1.0.4".to_string(),
            entry_path: "Index".to_string(),
            ..BundleRuntimePackageContext::default()
        },
    );
    let runtime_packages = BundleRuntimePackageInfo { packages };
    let byte_code_har_packages = vec!["@tencent/qq-open-sdk".to_string()];

    let files = HvigorModuleSourceWriter::new(
        "com.example.hmosworld",
        "phone",
        project_root,
        &module_path_map,
        &runtime_packages,
        &byte_code_har_packages,
        true,
        &facts,
        facts.full_module_ids.clone(),
        state,
        out_dir,
    )
    .write()
    .unwrap();

    let entry_file = files
        .iter()
        .find(|file| file.module_id == entry_id)
        .expect("entry module source");
    assert_eq!(
        entry_file.imported_record_names,
        vec!["&@tencent/qq-open-sdk/Index&1.0.4".to_string()]
    );
    let output = std::fs::read_to_string(&entry_file.path).unwrap();
    assert!(
        output.contains("\"@normalized:N&&&@tencent/qq-open-sdk/Index&1.0.4\""),
        "{output}"
    );
}

#[test]
fn module_source_writer_rewrites_case_mismatched_bare_package_entry_request() {
    let app_root = tempfile::tempdir().unwrap();
    let project_root = app_root.path();
    let phone_root = project_root.join("phone");
    let abtest_root =
        project_root.join("oh_modules/.ohpm/@meituan+abtest@0.0.23/oh_modules/@meituan/abtest");
    std::fs::create_dir_all(&abtest_root).unwrap();
    std::fs::write(
        abtest_root.join("oh-package.json5"),
        r#"{ name: "@meituan/abtest", version: "0.0.23", main: "Index.ets" }"#,
    )
    .unwrap();
    std::fs::write(
        abtest_root.join("Index.ets"),
        "export class ABTestService {}",
    )
    .unwrap();
    let entry_id = phone_root
        .join("src/main/ets/entryability/EntryAbility.ets")
        .to_string_lossy()
        .into_owned();
    let abtest_id = abtest_root.join("Index.ets").to_string_lossy().into_owned();
    let module_path_map = HvigorModulePathMap::new(
        "phone",
        &phone_root,
        &[("@meituan/abtest".to_string(), abtest_root.clone())],
    );
    let mut state = HvigorModuleSourceState::default();
    state.sources.insert(
        entry_id.clone(),
        HvigorCollectedModuleSource {
            code: "import { ABTestService } from \"@meituan/abTest\";"
                .to_string()
                .into(),
        },
    );
    state.sources.insert(
        abtest_id.clone(),
        HvigorCollectedModuleSource {
            code: "export class ABTestService {}".to_string().into(),
        },
    );
    state
        .resolved_imports
        .entry(entry_id.clone())
        .or_default()
        .insert("@meituan/abTest".to_string(), abtest_id.clone());
    let facts = ModuleGraphFacts {
        full_module_ids: vec![entry_id.clone(), abtest_id.clone()],
        full_module_is_commonjs: FxHashMap::default(),
    };
    let out_dir = app_root
        .path()
        .join("cache")
        .join("default@CompileArkTS")
        .join("esmodule")
        .join("release");
    let mut packages = FxHashMap::default();
    packages.insert(
        "phone".to_string(),
        BundleRuntimePackageContext {
            package_name: "phone".to_string(),
            ..BundleRuntimePackageContext::default()
        },
    );
    packages.insert(
        "@meituan/abtest".to_string(),
        BundleRuntimePackageContext {
            package_name: "@meituan/abtest".to_string(),
            version: "0.0.23".to_string(),
            entry_path: "Index.ets".to_string(),
            ..BundleRuntimePackageContext::default()
        },
    );
    let runtime_packages = BundleRuntimePackageInfo { packages };

    let files = HvigorModuleSourceWriter::new(
        "com.example.hmosworld",
        "phone",
        project_root,
        &module_path_map,
        &runtime_packages,
        &[],
        true,
        &facts,
        facts.full_module_ids.clone(),
        state,
        out_dir,
    )
    .write()
    .unwrap();

    let entry_file = files
        .iter()
        .find(|file| file.module_id == entry_id)
        .expect("entry module source");
    assert_eq!(
        entry_file.imported_record_names,
        vec!["&@meituan/abtest/Index&0.0.23".to_string()]
    );
    let output = std::fs::read_to_string(&entry_file.path).unwrap();
    assert!(
        output.contains("\"@normalized:N&&&@meituan/abtest/Index&0.0.23\""),
        "{output}"
    );
}

#[test]
fn module_source_writer_rewrites_source_har_imports_with_pkg_context_module_name() {
    let app_root = tempfile::tempdir().unwrap();
    let project_root = app_root.path();
    let phone_root = project_root.join("phone");
    let audioplayer_root = project_root.join("commons/audioplayer");
    let entry_id = phone_root
        .join("src/main/ets/entryability/EntryAbility.ets")
        .to_string_lossy()
        .into_owned();
    let speech_id = audioplayer_root
        .join("src/main/ets/service/SpeechPlayerService.ets")
        .to_string_lossy()
        .into_owned();
    let module_path_map = HvigorModulePathMap::new(
        "phone",
        &phone_root,
        &[("audioplayer".to_string(), audioplayer_root.clone())],
    );
    let mut state = HvigorModuleSourceState::default();
    state.sources.insert(
        entry_id.clone(),
        HvigorCollectedModuleSource {
            code: "import { SpeechPlayerService } from \"../SpeechPlayerService\";"
                .to_string()
                .into(),
        },
    );
    state.sources.insert(
        speech_id.clone(),
        HvigorCollectedModuleSource {
            code: "export class SpeechPlayerService {}".to_string().into(),
        },
    );
    state
        .resolved_imports
        .entry(entry_id.clone())
        .or_default()
        .insert("../SpeechPlayerService".to_string(), speech_id.clone());
    let facts = ModuleGraphFacts {
        full_module_ids: vec![entry_id.clone(), speech_id.clone()],
        full_module_is_commonjs: FxHashMap::default(),
    };
    let out_dir = app_root
        .path()
        .join("cache")
        .join("default@CompileArkTS")
        .join("esmodule")
        .join("release");
    let runtime_packages = runtime_packages(&[
        ("phone", "phone", "", "", "", false),
        ("audioplayer", "audioplayer", "", "", "1.0.0", false),
    ]);

    let files = HvigorModuleSourceWriter::new(
        "com.example.hmosworld",
        "phone",
        project_root,
        &module_path_map,
        &runtime_packages,
        &[],
        true,
        &facts,
        facts.full_module_ids.clone(),
        state,
        out_dir,
    )
    .write()
    .unwrap();

    assert_eq!(files.len(), 2);
    let entry_file = files
        .iter()
        .find(|file| file.module_id == entry_id)
        .expect("entry module source");
    let speech_file = files
        .iter()
        .find(|file| file.module_id == speech_id)
        .expect("dependency module source");
    assert_eq!(
        entry_file.record_name,
        "&phone/src/main/ets/entryability/EntryAbility&"
    );
    assert_eq!(
        entry_file.source_file,
        "phone|phone|1.0.0|src/main/ets/entryability/EntryAbility.ts"
    );
    assert_eq!(
        entry_file.path,
        app_root
            .path()
            .join("cache")
            .join("default@CompileArkTS")
            .join("esmodule")
            .join("release")
            .join("phone")
            .join("src")
            .join("main")
            .join("ets")
            .join("entryability")
            .join("EntryAbility.ts")
    );
    assert_eq!(entry_file.package_name.as_deref(), Some("phone"));
    assert_eq!(
        entry_file.imported_record_names,
        vec!["&audioplayer/src/main/ets/service/SpeechPlayerService&1.0.0".to_string()]
    );
    assert_eq!(
        speech_file.record_name,
        "&audioplayer/src/main/ets/service/SpeechPlayerService&1.0.0"
    );
    assert_eq!(
        speech_file.source_file,
        "phone|audioplayer|1.0.0|src/main/ets/service/SpeechPlayerService.ts"
    );
    assert_eq!(speech_file.package_name.as_deref(), Some("audioplayer"));
    let output = std::fs::read_to_string(&entry_file.path).unwrap();
    assert!(
        output.contains(
            "\"@normalized:N&&&audioplayer/src/main/ets/service/SpeechPlayerService&1.0.0\""
        ),
        "{output}"
    );
}

#[test]
fn module_source_writer_rewrites_static_re_exports_from_rolldown_graph() {
    let app_root = tempfile::tempdir().unwrap();
    let project_root = app_root.path();
    let phone_root = project_root.join("products/phone");
    let mrn_root = project_root.join("oh_modules/.ohpm/@mrn+mrn@12.59.403/oh_modules/@mrn/mrn");
    let index_id = mrn_root.join("Index.ets").to_string_lossy().into_owned();
    let config_id = mrn_root
        .join("src/main/ets/config/MRNConfigCenter.ets")
        .to_string_lossy()
        .into_owned();
    std::fs::create_dir_all(&mrn_root).unwrap();
    std::fs::write(
        mrn_root.join("oh-package.json5"),
        r#"{
            "name": "@mrn/mrn",
            "version": "12.59.403",
            "main": "Index.ets"
        }"#,
    )
    .unwrap();
    std::fs::write(mrn_root.join("Index.ets"), "export {};").unwrap();
    std::fs::create_dir_all(mrn_root.join("src/main/ets/config")).unwrap();
    std::fs::write(
        mrn_root.join("src/main/ets/config/MRNConfigCenter.ets"),
        "export class MRNConfigCenter {}",
    )
    .unwrap();

    let module_path_map = HvigorModulePathMap::new(
        "phone",
        &phone_root,
        &[("@mrn/mrn".to_string(), mrn_root.clone())],
    );
    let mut state = HvigorModuleSourceState::default();
    state.sources.insert(
        index_id.clone(),
        HvigorCollectedModuleSource {
            code: "export { MRNConfigCenter } from \"./src/main/ets/config/MRNConfigCenter\";"
                .to_string()
                .into(),
        },
    );
    state.sources.insert(
        config_id.clone(),
        HvigorCollectedModuleSource {
            code: "export class MRNConfigCenter {}".to_string().into(),
        },
    );
    record_resolved_import(
        &mut state,
        &index_id,
        "./src/main/ets/config/MRNConfigCenter",
        &config_id,
    );
    let facts = ModuleGraphFacts {
        full_module_ids: vec![index_id.clone(), config_id.clone()],
        full_module_is_commonjs: FxHashMap::default(),
    };
    let out_dir = app_root
        .path()
        .join("cache")
        .join("default@CompileArkTS")
        .join("esmodule")
        .join("release");
    let mut runtime_packages =
        runtime_packages(&[("@mrn/mrn", "@mrn/mrn", "", "", "12.59.403", false)]);
    runtime_packages
        .packages
        .get_mut("@mrn/mrn")
        .expect("mrn package context")
        .entry_path = "Index.ets".to_string();

    let files = HvigorModuleSourceWriter::new(
        "com.example.hmosworld",
        "phone",
        project_root,
        &module_path_map,
        &runtime_packages,
        &[],
        true,
        &facts,
        facts.full_module_ids.clone(),
        state,
        out_dir,
    )
    .write()
    .unwrap();

    let index_file = files
        .iter()
        .find(|file| file.module_id == index_id)
        .expect("package index source");
    assert_eq!(index_file.record_name, "&@mrn/mrn/Index&12.59.403");
    assert_eq!(
        index_file.imported_record_names,
        vec!["&@mrn/mrn/src/main/ets/config/MRNConfigCenter&12.59.403".to_string()]
    );
    let output = std::fs::read_to_string(&index_file.path).unwrap();
    assert!(
        output
            .contains("\"@normalized:N&&&@mrn/mrn/src/main/ets/config/MRNConfigCenter&12.59.403\""),
        "{output}"
    );
}

#[test]
fn module_source_writer_rewrites_relative_package_imports_from_resolved_module_ids() {
    let app_root = tempfile::tempdir().unwrap();
    let project_root = app_root.path();
    let phone_root = project_root.join("products/phone");
    let launcher_root =
        project_root.join("oh_modules/.ohpm/mtlauncher@12.59.403/oh_modules/mtlauncher");
    let index_id = launcher_root
        .join("Index.ets")
        .to_string_lossy()
        .into_owned();
    let build_profile_id = launcher_root
        .join("BuildProfile.ets")
        .to_string_lossy()
        .into_owned();
    let resolved_build_profile_id = launcher_root
        .join(".")
        .join("BuildProfile.ets")
        .to_string_lossy()
        .into_owned();
    std::fs::create_dir_all(&launcher_root).unwrap();
    std::fs::write(
        launcher_root.join("oh-package.json5"),
        r#"{
            "name": "mtlauncher",
            "version": "12.59.403",
            "main": "Index.ets"
        }"#,
    )
    .unwrap();
    std::fs::write(launcher_root.join("Index.ets"), "export {};").unwrap();
    std::fs::write(
        launcher_root.join("BuildProfile.ets"),
        "export default class BuildProfile {}",
    )
    .unwrap();

    let module_path_map = HvigorModulePathMap::new(
        "phone",
        &phone_root,
        &[("mtlauncher".to_string(), launcher_root.clone())],
    );
    let mut state = HvigorModuleSourceState::default();
    state.sources.insert(
        index_id.clone(),
        HvigorCollectedModuleSource {
            code: "import BuildProfile from \"./BuildProfile\"; export { BuildProfile };"
                .to_string()
                .into(),
        },
    );
    state.sources.insert(
        build_profile_id.clone(),
        HvigorCollectedModuleSource {
            code: "export default class BuildProfile {}".to_string().into(),
        },
    );
    record_resolved_import(
        &mut state,
        &index_id,
        "./BuildProfile",
        &resolved_build_profile_id,
    );
    let facts = ModuleGraphFacts {
        full_module_ids: vec![index_id.clone(), build_profile_id.clone()],
        full_module_is_commonjs: FxHashMap::default(),
    };
    let out_dir = app_root
        .path()
        .join("cache")
        .join("default@CompileArkTS")
        .join("esmodule")
        .join("release");
    let mut runtime_packages =
        runtime_packages(&[("mtlauncher", "mtlauncher", "", "", "12.59.403", false)]);
    runtime_packages
        .packages
        .get_mut("mtlauncher")
        .expect("mtlauncher package context")
        .entry_path = "Index.ets".to_string();

    let files = HvigorModuleSourceWriter::new(
        "com.example.hmosworld",
        "phone",
        project_root,
        &module_path_map,
        &runtime_packages,
        &[],
        true,
        &facts,
        facts.full_module_ids.clone(),
        state,
        out_dir,
    )
    .write()
    .unwrap();

    let index_file = files
        .iter()
        .find(|file| file.module_id == index_id)
        .expect("launcher index source");
    assert_eq!(index_file.record_name, "&mtlauncher/Index&12.59.403");
    assert_eq!(
        index_file.imported_record_names,
        vec!["&mtlauncher/BuildProfile&12.59.403".to_string()]
    );
    let output = std::fs::read_to_string(&index_file.path).unwrap();
    assert!(
        output.contains("\"@normalized:N&&&mtlauncher/BuildProfile&12.59.403\""),
        "{output}"
    );
    assert!(!output.contains("\"./BuildProfile\""), "{output}");
}

#[test]
fn module_source_writer_preserves_duplicate_specifiers_in_resolved_id_map() {
    let app_root = tempfile::tempdir().unwrap();
    let project_root = app_root.path();
    let phone_root = project_root.join("products/phone");
    let package_root =
        project_root.join("oh_modules/.ohpm/mtlauncher@12.59.403/oh_modules/mtlauncher");
    let index_id = package_root
        .join("Index.ets")
        .to_string_lossy()
        .into_owned();
    let constant_id = package_root
        .join("constant/index.ets")
        .to_string_lossy()
        .into_owned();
    let reporter_id = package_root
        .join("reporter/ApiCallReporter.ets")
        .to_string_lossy()
        .into_owned();
    std::fs::create_dir_all(package_root.join("constant")).unwrap();
    std::fs::create_dir_all(package_root.join("reporter")).unwrap();
    std::fs::write(
        package_root.join("oh-package.json5"),
        r#"{
            "name": "mtlauncher",
            "version": "12.59.403",
            "main": "Index.ets"
        }"#,
    )
    .unwrap();
    std::fs::write(package_root.join("Index.ets"), "export {};").unwrap();
    std::fs::write(
        package_root.join("constant/index.ets"),
        "export const ASYNC_OR_SYNC_MODE = 1; export const INNER_SCOPE = 2;",
    )
    .unwrap();
    std::fs::write(
        package_root.join("reporter/ApiCallReporter.ets"),
        "export const KEY_SCOPE = 'scope';",
    )
    .unwrap();

    let module_path_map = HvigorModulePathMap::new(
        "phone",
        &phone_root,
        &[("mtlauncher".to_string(), package_root.clone())],
    );
    let mut state = HvigorModuleSourceState::default();
    state.sources.insert(
        index_id.clone(),
        HvigorCollectedModuleSource {
            code: r#"import { ASYNC_OR_SYNC_MODE } from "./constant";
import { INNER_SCOPE } from "./constant/index";
import { KEY_SCOPE } from "./reporter/ApiCallReporter";
export { ASYNC_OR_SYNC_MODE, INNER_SCOPE, KEY_SCOPE };"#
                .to_string()
                .into(),
        },
    );
    state.sources.insert(
        constant_id.clone(),
        HvigorCollectedModuleSource {
            code: "export const ASYNC_OR_SYNC_MODE = 1; export const INNER_SCOPE = 2;"
                .to_string()
                .into(),
        },
    );
    state.sources.insert(
        reporter_id.clone(),
        HvigorCollectedModuleSource {
            code: "export const KEY_SCOPE = 'scope';".to_string().into(),
        },
    );
    record_resolved_import(&mut state, &index_id, "./constant", &constant_id);
    record_resolved_import(&mut state, &index_id, "./constant/index", &constant_id);
    record_resolved_import(
        &mut state,
        &index_id,
        "./reporter/ApiCallReporter",
        &reporter_id,
    );
    let facts = ModuleGraphFacts {
        full_module_ids: vec![index_id.clone(), constant_id.clone(), reporter_id.clone()],
        full_module_is_commonjs: FxHashMap::default(),
    };
    let out_dir = app_root
        .path()
        .join("cache")
        .join("default@CompileArkTS")
        .join("esmodule")
        .join("release");
    let mut runtime_packages =
        runtime_packages(&[("mtlauncher", "mtlauncher", "", "", "12.59.403", false)]);
    runtime_packages
        .packages
        .get_mut("mtlauncher")
        .expect("mtlauncher package context")
        .entry_path = "Index.ets".to_string();

    let files = HvigorModuleSourceWriter::new(
        "com.example.hmosworld",
        "phone",
        project_root,
        &module_path_map,
        &runtime_packages,
        &[],
        true,
        &facts,
        facts.full_module_ids.clone(),
        state,
        out_dir,
    )
    .write()
    .unwrap();

    let index_file = files
        .iter()
        .find(|file| file.module_id == index_id)
        .expect("launcher index source");
    assert_eq!(
        index_file.imported_record_names,
        vec![
            "&mtlauncher/constant/index&12.59.403".to_string(),
            "&mtlauncher/reporter/ApiCallReporter&12.59.403".to_string(),
        ]
    );
    let output = std::fs::read_to_string(&index_file.path).unwrap();
    assert!(
        output.contains("\"@normalized:N&&&mtlauncher/constant/index&12.59.403\""),
        "{output}"
    );
    assert!(
        output.contains("\"@normalized:N&&&mtlauncher/reporter/ApiCallReporter&12.59.403\""),
        "{output}"
    );
    assert!(!output.contains("\"./constant\""), "{output}");
    assert!(
        !output.contains("\"./reporter/ApiCallReporter\""),
        "{output}"
    );
}

#[test]
fn module_source_writer_preserves_arkts_syntax_when_rewriting_requests() {
    let app_root = tempfile::tempdir().unwrap();
    let project_root = app_root.path();
    let phone_root = project_root.join("products/phone");
    let lx_root = project_root.join("oh_modules/.ohpm/@meituan+lx@4.119.1/oh_modules/@meituan/lx");
    let manager_id = lx_root
        .join("src/main/ets/channel/ChannelManager.ets")
        .to_string_lossy()
        .into_owned();
    let utils_id = lx_root
        .join("src/main/ets/utils/Utils.ets")
        .to_string_lossy()
        .into_owned();
    std::fs::create_dir_all(lx_root.join("src/main/ets/channel")).unwrap();
    std::fs::create_dir_all(lx_root.join("src/main/ets/utils")).unwrap();
    std::fs::write(
        lx_root.join("oh-package.json5"),
        r#"{
            "name": "@meituan/lx",
            "version": "4.119.1",
            "main": "Index.ets"
        }"#,
    )
    .unwrap();
    let manager_code = [
        "import type { PageInfo } from '../pageinfo/PageInfo';",
        "import { isEmptyString } from '../utils/Utils';",
        "export default class ChannelManager {",
        "  private defaultChannelName: string;",
        "  private static instance: ChannelManager;",
        "  constructor(defaultChannelName: string) {",
        "    this.defaultChannelName = isEmptyString(defaultChannelName) ? '' : defaultChannelName;",
        "  }",
        "}",
    ]
    .join("\n");
    let utils_code =
        "export function isEmptyString(value?: string): boolean { return !value; }".to_string();
    std::fs::write(&manager_id, &manager_code).unwrap();
    std::fs::write(&utils_id, &utils_code).unwrap();
    let module_path_map = HvigorModulePathMap::new(
        "phone",
        &phone_root,
        &[("@meituan/lx".to_string(), lx_root.clone())],
    );
    let mut state = HvigorModuleSourceState::default();
    state.sources.insert(
        manager_id.clone(),
        HvigorCollectedModuleSource {
            code: manager_code.into(),
        },
    );
    state.sources.insert(
        utils_id.clone(),
        HvigorCollectedModuleSource {
            code: utils_code.into(),
        },
    );
    state
        .resolved_imports
        .entry(manager_id.clone())
        .or_default()
        .insert("../utils/Utils".to_string(), utils_id.clone());
    let facts = ModuleGraphFacts {
        full_module_ids: vec![manager_id.clone(), utils_id.clone()],
        full_module_is_commonjs: FxHashMap::default(),
    };
    let out_dir = app_root
        .path()
        .join("cache")
        .join("default@CompileArkTS")
        .join("esmodule")
        .join("release");
    let runtime_packages =
        runtime_packages(&[("@meituan/lx", "@meituan/lx", "", "", "4.119.1", false)]);

    let files = HvigorModuleSourceWriter::new(
        "com.example.hmosworld",
        "phone",
        project_root,
        &module_path_map,
        &runtime_packages,
        &[],
        true,
        &facts,
        facts.full_module_ids.clone(),
        state,
        out_dir,
    )
    .write()
    .unwrap();

    let manager_file = files
        .iter()
        .find(|file| file.module_id == manager_id)
        .expect("manager module source");
    let output = std::fs::read_to_string(&manager_file.path).unwrap();
    assert!(
        output.contains("import type { PageInfo } from '../pageinfo/PageInfo';"),
        "{output}"
    );
    assert!(
        output.contains("\"@normalized:N&&&@meituan/lx/src/main/ets/utils/Utils&4.119.1\""),
        "{output}"
    );
    assert!(
        output.contains("private defaultChannelName: string;"),
        "{output}"
    );
    assert!(
        output.contains("private static instance: ChannelManager;"),
        "{output}"
    );
    assert!(
        output.contains("constructor(defaultChannelName: string)"),
        "{output}"
    );
}

#[test]
fn module_source_writer_rewrites_lowercase_index_request_to_pascal_index_record() {
    let app_root = tempfile::tempdir().unwrap();
    let project_root = app_root.path();
    let phone_root = project_root.join("products/phone");
    let mrn_root = project_root.join("oh_modules/.ohpm/@mrn+mrn@12.59.403/oh_modules/@mrn/mrn");
    let manager_id = mrn_root
        .join("src/main/ets/engine/MRNInstanceManager.ets")
        .to_string_lossy()
        .into_owned();
    let route_index_id = mrn_root
        .join("src/main/ets/route/Index.ets")
        .to_string_lossy()
        .into_owned();
    std::fs::create_dir_all(mrn_root.join("src/main/ets/engine")).unwrap();
    std::fs::create_dir_all(mrn_root.join("src/main/ets/route")).unwrap();
    std::fs::write(
        mrn_root.join("oh-package.json5"),
        r#"{
            "name": "@mrn/mrn",
            "version": "12.59.403",
            "main": "Index.ets"
        }"#,
    )
    .unwrap();
    std::fs::write(
        mrn_root.join("src/main/ets/engine/MRNInstanceManager.ets"),
        "import { getMRNRouteURL } from \"../route/index\";",
    )
    .unwrap();
    std::fs::write(
        mrn_root.join("src/main/ets/route/Index.ets"),
        "export function getMRNRouteURL() { return \"imeituan://www.meituan.com/mrn\"; }",
    )
    .unwrap();

    let module_path_map = HvigorModulePathMap::new(
        "phone",
        &phone_root,
        &[("@mrn/mrn".to_string(), mrn_root.clone())],
    );
    let mut state = HvigorModuleSourceState::default();
    state.sources.insert(
        manager_id.clone(),
        HvigorCollectedModuleSource {
            code: "import { getMRNRouteURL } from \"../route/index\";"
                .to_string()
                .into(),
        },
    );
    state.sources.insert(
        route_index_id.clone(),
        HvigorCollectedModuleSource {
            code: "export function getMRNRouteURL() { return \"imeituan://www.meituan.com/mrn\"; }"
                .to_string()
                .into(),
        },
    );
    record_resolved_import(&mut state, &manager_id, "../route/index", &route_index_id);
    let facts = ModuleGraphFacts {
        full_module_ids: vec![manager_id.clone(), route_index_id.clone()],
        full_module_is_commonjs: FxHashMap::default(),
    };
    let out_dir = app_root
        .path()
        .join("cache")
        .join("default@CompileArkTS")
        .join("esmodule")
        .join("release");
    let runtime_packages =
        runtime_packages(&[("@mrn/mrn", "@mrn/mrn", "", "", "12.59.403", false)]);

    let files = HvigorModuleSourceWriter::new(
        "com.example.hmosworld",
        "phone",
        project_root,
        &module_path_map,
        &runtime_packages,
        &[],
        true,
        &facts,
        facts.full_module_ids.clone(),
        state,
        out_dir,
    )
    .write()
    .unwrap();

    let manager_file = files
        .iter()
        .find(|file| file.module_id == manager_id)
        .expect("manager module source");
    assert_eq!(
        manager_file.imported_record_names,
        vec!["&@mrn/mrn/src/main/ets/route/Index&12.59.403".to_string()]
    );
    let output = std::fs::read_to_string(&manager_file.path).unwrap();
    assert!(
        output.contains("\"@normalized:N&&&@mrn/mrn/src/main/ets/route/Index&12.59.403\""),
        "{output}"
    );
}

#[test]
fn module_source_writer_rewrites_mixed_case_request_to_uppercase_record() {
    let app_root = tempfile::tempdir().unwrap();
    let project_root = app_root.path();
    let phone_root = project_root.join("products/phone");
    let lx_root = project_root.join("oh_modules/.ohpm/@meituan+lx@4.119.1/oh_modules/@meituan/lx");
    let inner_id = lx_root
        .join("src/main/ets/event/inner/InnerDataManager.ets")
        .to_string_lossy()
        .into_owned();
    let lx_id = lx_root
        .join("src/main/ets/LX.ets")
        .to_string_lossy()
        .into_owned();
    std::fs::create_dir_all(lx_root.join("src/main/ets/event/inner")).unwrap();
    std::fs::create_dir_all(lx_root.join("src/main/ets")).unwrap();
    std::fs::write(
        lx_root.join("oh-package.json5"),
        r#"{
            "name": "@meituan/lx",
            "version": "4.119.1",
            "main": "Index.ets"
        }"#,
    )
    .unwrap();
    std::fs::write(
        lx_root.join("src/main/ets/event/inner/InnerDataManager.ets"),
        "import { isInitialized } from \"../../Lx\";",
    )
    .unwrap();
    std::fs::write(
        lx_root.join("src/main/ets/LX.ets"),
        "export const isInitialized = true;",
    )
    .unwrap();

    let module_path_map = HvigorModulePathMap::new(
        "phone",
        &phone_root,
        &[("@meituan/lx".to_string(), lx_root.clone())],
    );
    let mut state = HvigorModuleSourceState::default();
    state.sources.insert(
        inner_id.clone(),
        HvigorCollectedModuleSource {
            code: "import { isInitialized } from \"../../Lx\";"
                .to_string()
                .into(),
        },
    );
    state.sources.insert(
        lx_id.clone(),
        HvigorCollectedModuleSource {
            code: "export const isInitialized = true;".to_string().into(),
        },
    );
    record_resolved_import(&mut state, &inner_id, "../../Lx", &lx_id);
    let facts = ModuleGraphFacts {
        full_module_ids: vec![inner_id.clone(), lx_id.clone()],
        full_module_is_commonjs: FxHashMap::default(),
    };
    let out_dir = app_root
        .path()
        .join("cache")
        .join("default@CompileArkTS")
        .join("esmodule")
        .join("release");
    let runtime_packages =
        runtime_packages(&[("@meituan/lx", "@meituan/lx", "", "", "4.119.1", false)]);

    let files = HvigorModuleSourceWriter::new(
        "com.example.hmosworld",
        "phone",
        project_root,
        &module_path_map,
        &runtime_packages,
        &[],
        true,
        &facts,
        facts.full_module_ids.clone(),
        state,
        out_dir,
    )
    .write()
    .unwrap();

    let inner_file = files
        .iter()
        .find(|file| file.module_id == inner_id)
        .expect("inner module source");
    assert_eq!(
        inner_file.imported_record_names,
        vec!["&@meituan/lx/src/main/ets/LX&4.119.1".to_string()]
    );
    let output = std::fs::read_to_string(&inner_file.path).unwrap();
    assert!(
        output.contains("\"@normalized:N&&&@meituan/lx/src/main/ets/LX&4.119.1\""),
        "{output}"
    );
}

#[test]
fn module_source_writer_rewrites_case_mismatched_relative_file_request() {
    let app_root = tempfile::tempdir().unwrap();
    let project_root = app_root.path();
    let phone_root = project_root.join("products/phone");
    let network_root =
        project_root.join("oh_modules/.ohpm/@waimai+network@1.0.32/oh_modules/@waimai/network");
    let index_id = network_root
        .join("src/main/ets/network/index.ets")
        .to_string_lossy()
        .into_owned();
    let init_id = network_root
        .join("src/main/ets/network/NetworkInit.ets")
        .to_string_lossy()
        .into_owned();
    std::fs::create_dir_all(network_root.join("src/main/ets/network")).unwrap();
    std::fs::write(
        network_root.join("oh-package.json5"),
        r#"{
            "name": "@waimai/network",
            "version": "1.0.32",
            "main": "Index.ets"
        }"#,
    )
    .unwrap();
    std::fs::write(
        network_root.join("src/main/ets/network/index.ets"),
        "export { NetWorkInit } from \"./NetWorkInit\";",
    )
    .unwrap();
    std::fs::write(
        network_root.join("src/main/ets/network/NetworkInit.ets"),
        "export class NetWorkInit {}",
    )
    .unwrap();

    let module_path_map = HvigorModulePathMap::new(
        "phone",
        &phone_root,
        &[("@waimai/network".to_string(), network_root.clone())],
    );
    let mut state = HvigorModuleSourceState::default();
    state.sources.insert(
        index_id.clone(),
        HvigorCollectedModuleSource {
            code: "export { NetWorkInit } from \"./NetWorkInit\";"
                .to_string()
                .into(),
        },
    );
    state.sources.insert(
        init_id.clone(),
        HvigorCollectedModuleSource {
            code: "export class NetWorkInit {}".to_string().into(),
        },
    );
    record_resolved_import(&mut state, &index_id, "./NetWorkInit", &init_id);
    let facts = ModuleGraphFacts {
        full_module_ids: vec![index_id.clone(), init_id.clone()],
        full_module_is_commonjs: FxHashMap::default(),
    };
    let out_dir = app_root
        .path()
        .join("cache")
        .join("default@CompileArkTS")
        .join("esmodule")
        .join("release");
    let runtime_packages = runtime_packages(&[(
        "@waimai/network",
        "@waimai/network",
        "",
        "",
        "1.0.32",
        false,
    )]);

    let files = HvigorModuleSourceWriter::new(
        "com.example.hmosworld",
        "phone",
        project_root,
        &module_path_map,
        &runtime_packages,
        &[],
        true,
        &facts,
        facts.full_module_ids.clone(),
        state,
        out_dir,
    )
    .write()
    .unwrap();

    let index_file = files
        .iter()
        .find(|file| file.module_id == index_id)
        .expect("network index module source");
    assert_eq!(
        index_file.imported_record_names,
        vec!["&@waimai/network/src/main/ets/network/NetworkInit&1.0.32".to_string()]
    );
    let output = std::fs::read_to_string(&index_file.path).unwrap();
    assert!(
        output.contains(
            "\"@normalized:N&&&@waimai/network/src/main/ets/network/NetworkInit&1.0.32\""
        ),
        "{output}"
    );
}

#[test]
fn module_source_writer_preserves_standard_source_root_index_record() {
    let app_root = tempfile::tempdir().unwrap();
    let project_root = app_root.path();
    let phone_root = project_root.join("products/phone");
    let msi_root = project_root.join("oh_modules/.ohpm/@msi+msi@12.59.400/oh_modules/@msi/msi");
    let package_index_id = msi_root.join("Index.ets").to_string_lossy().into_owned();
    let source_root_index_id = msi_root
        .join("src/main/ets/index.ets")
        .to_string_lossy()
        .into_owned();
    std::fs::create_dir_all(msi_root.join("src/main/ets")).unwrap();
    std::fs::write(
        msi_root.join("oh-package.json5"),
        r#"{
            "name": "@msi/msi",
            "version": "12.59.400",
            "main": "Index.ets"
        }"#,
    )
    .unwrap();
    std::fs::write(
        msi_root.join("Index.ets"),
        "export * from \"./src/main/ets/index\";",
    )
    .unwrap();
    std::fs::write(
        msi_root.join("src/main/ets/index.ets"),
        "export class MSI {}",
    )
    .unwrap();

    let module_path_map = HvigorModulePathMap::new(
        "phone",
        &phone_root,
        &[("@msi/msi".to_string(), msi_root.clone())],
    );
    let mut state = HvigorModuleSourceState::default();
    state.sources.insert(
        package_index_id.clone(),
        HvigorCollectedModuleSource {
            code: "export * from \"./src/main/ets/index\";".to_string().into(),
        },
    );
    state.sources.insert(
        source_root_index_id.clone(),
        HvigorCollectedModuleSource {
            code: "export class MSI {}".to_string().into(),
        },
    );
    record_resolved_import(
        &mut state,
        &package_index_id,
        "./src/main/ets/index",
        &source_root_index_id,
    );
    let facts = ModuleGraphFacts {
        full_module_ids: vec![package_index_id.clone(), source_root_index_id.clone()],
        full_module_is_commonjs: FxHashMap::default(),
    };
    let out_dir = app_root
        .path()
        .join("cache")
        .join("default@CompileArkTS")
        .join("esmodule")
        .join("release");
    let runtime_packages =
        runtime_packages(&[("@msi/msi", "@msi/msi", "", "", "12.59.400", false)]);

    let files = HvigorModuleSourceWriter::new(
        "com.example.hmosworld",
        "phone",
        project_root,
        &module_path_map,
        &runtime_packages,
        &[],
        true,
        &facts,
        facts.full_module_ids.clone(),
        state,
        out_dir,
    )
    .write()
    .unwrap();

    let source_root_index_file = files
        .iter()
        .find(|file| file.module_id == source_root_index_id)
        .expect("source root index module source");
    assert_eq!(
        source_root_index_file.record_name,
        "&@msi/msi/src/main/ets/index&12.59.400"
    );
    let package_index_file = files
        .iter()
        .find(|file| file.module_id == package_index_id)
        .expect("package index module source");
    assert_eq!(
        package_index_file.imported_record_names,
        vec!["&@msi/msi/src/main/ets/index&12.59.400".to_string()]
    );
    let output = std::fs::read_to_string(&package_index_file.path).unwrap();
    assert!(
        output.contains("\"@normalized:N&&&@msi/msi/src/main/ets/index&12.59.400\""),
        "{output}"
    );
}

#[test]
fn module_source_writer_preserves_package_source_index_subpath_record() {
    let app_root = tempfile::tempdir().unwrap();
    let project_root = app_root.path();
    let phone_root = project_root.join("products/phone");
    let privacy_root =
        project_root.join("oh_modules/.ohpm/@meituan+privacy@0.3.20/oh_modules/@meituan/privacy");
    let privacy_impl_root = project_root
        .join("oh_modules/.ohpm/@meituan+privacy_impl@1.0.0/oh_modules/@meituan/privacy_impl");
    let importer_id = privacy_impl_root
        .join("src/main/ets/monitor/BabelMonitor.ets")
        .to_string_lossy()
        .into_owned();
    let def_index_id = privacy_root
        .join("src/main/ets/def/index.ets")
        .to_string_lossy()
        .into_owned();
    std::fs::create_dir_all(privacy_root.join("src/main/ets/def")).unwrap();
    std::fs::create_dir_all(privacy_impl_root.join("src/main/ets/monitor")).unwrap();
    std::fs::write(
        privacy_root.join("oh-package.json5"),
        r#"{
            "name": "@meituan/privacy",
            "version": "0.3.20",
            "main": "Index.ets"
        }"#,
    )
    .unwrap();
    std::fs::write(
        privacy_impl_root.join("oh-package.json5"),
        r#"{
            "name": "@meituan/privacy_impl",
            "version": "1.0.0",
            "main": "Index.ets"
        }"#,
    )
    .unwrap();
    std::fs::write(
        privacy_impl_root.join("src/main/ets/monitor/BabelMonitor.ets"),
        "import { DefFilterResult } from \"@meituan/privacy/src/main/ets/def\";",
    )
    .unwrap();
    std::fs::write(
        privacy_root.join("src/main/ets/def/index.ets"),
        "export class DefFilterResult {}",
    )
    .unwrap();

    let module_path_map = HvigorModulePathMap::new(
        "phone",
        &phone_root,
        &[
            ("@meituan/privacy".to_string(), privacy_root.clone()),
            (
                "@meituan/privacy_impl".to_string(),
                privacy_impl_root.clone(),
            ),
        ],
    );
    let mut state = HvigorModuleSourceState::default();
    state.sources.insert(
        importer_id.clone(),
        HvigorCollectedModuleSource {
            code: "import { DefFilterResult } from \"@meituan/privacy/src/main/ets/def\";"
                .to_string()
                .into(),
        },
    );
    state.sources.insert(
        def_index_id.clone(),
        HvigorCollectedModuleSource {
            code: "export class DefFilterResult {}".to_string().into(),
        },
    );
    record_resolved_import(
        &mut state,
        &importer_id,
        "@meituan/privacy/src/main/ets/def",
        &def_index_id,
    );
    let facts = ModuleGraphFacts {
        full_module_ids: vec![importer_id.clone(), def_index_id.clone()],
        full_module_is_commonjs: FxHashMap::default(),
    };
    let out_dir = app_root
        .path()
        .join("cache")
        .join("default@CompileArkTS")
        .join("esmodule")
        .join("release");
    let runtime_packages = runtime_packages(&[
        (
            "@meituan/privacy",
            "@meituan/privacy",
            "",
            "",
            "0.3.20",
            false,
        ),
        (
            "@meituan/privacy_impl",
            "@meituan/privacy_impl",
            "",
            "",
            "1.0.0",
            false,
        ),
    ]);

    let files = HvigorModuleSourceWriter::new(
        "com.example.hmosworld",
        "phone",
        project_root,
        &module_path_map,
        &runtime_packages,
        &[],
        true,
        &facts,
        facts.full_module_ids.clone(),
        state,
        out_dir,
    )
    .write()
    .unwrap();

    let def_index_file = files
        .iter()
        .find(|file| file.module_id == def_index_id)
        .expect("privacy def index module source");
    assert_eq!(
        def_index_file.record_name,
        "&@meituan/privacy/src/main/ets/def/index&0.3.20"
    );
    let importer_file = files
        .iter()
        .find(|file| file.module_id == importer_id)
        .expect("privacy impl importer module source");
    assert_eq!(
        importer_file.imported_record_names,
        vec!["&@meituan/privacy/src/main/ets/def/index&0.3.20".to_string()]
    );
    let output = std::fs::read_to_string(&importer_file.path).unwrap();
    assert!(
        output.contains("\"@normalized:N&&&@meituan/privacy/src/main/ets/def/index&0.3.20\""),
        "{output}"
    );
}

#[test]
fn module_source_writer_uses_local_dependency_module_name_for_cache_path() {
    let app_root = tempfile::tempdir().unwrap();
    let project_root = app_root.path();
    let phone_root = project_root.join("products/phone");
    let runtime_root = project_root.join("repo/repos/_msi_runtime/msi_runtime");
    let runtime_api_id = runtime_root
        .join("src/main/ets/ApiPortal.ts")
        .to_string_lossy()
        .into_owned();
    std::fs::create_dir_all(runtime_root.join("src/main/ets")).unwrap();
    std::fs::write(
        runtime_root.join("oh-package.json5"),
        r#"{
            "name": "@msi/runtime",
            "version": "12.59.400",
            "main": "Index.ets"
        }"#,
    )
    .unwrap();
    std::fs::write(
        runtime_root.join("src/main/ets/ApiPortal.ts"),
        "export class ApiPortal {}",
    )
    .unwrap();

    let mut module_path_map = HvigorModulePathMap::new(
        "phone",
        &phone_root,
        &[
            ("phone".to_string(), phone_root.clone()),
            ("msi_runtime".to_string(), runtime_root.clone()),
        ],
    );
    module_path_map.add_module_record_roots(&[("@msi/runtime".to_string(), runtime_root.clone())]);

    let mut state = HvigorModuleSourceState::default();
    state.sources.insert(
        runtime_api_id.clone(),
        HvigorCollectedModuleSource {
            code: "export class ApiPortal {}".to_string().into(),
        },
    );
    let facts = ModuleGraphFacts {
        full_module_ids: vec![runtime_api_id.clone()],
        full_module_is_commonjs: FxHashMap::default(),
    };
    let out_dir = app_root
        .path()
        .join("cache")
        .join("default@CompileArkTS")
        .join("esmodule")
        .join("debug");
    let mut runtime_packages = runtime_packages(&[("phone", "phone", "", "", "", false)]);
    runtime_packages.packages.insert(
        "@msi/runtime".to_string(),
        BundleRuntimePackageContext {
            package_name: "@msi/runtime".to_string(),
            version: "12.59.400".to_string(),
            entry_path: "Index.ets".to_string(),
            ..BundleRuntimePackageContext::default()
        },
    );

    let files = HvigorModuleSourceWriter::new(
        "com.example.hmosworld",
        "phone",
        project_root,
        &module_path_map,
        &runtime_packages,
        &[],
        true,
        &facts,
        facts.full_module_ids.clone(),
        state,
        out_dir.clone(),
    )
    .write()
    .unwrap();

    let api_file = files
        .iter()
        .find(|file| file.module_id == runtime_api_id)
        .expect("local dependency source");
    assert_eq!(
        api_file.record_name,
        "&@msi/runtime/src/main/ets/ApiPortal&12.59.400"
    );
    assert_eq!(
        api_file.path,
        out_dir.join("msi_runtime/src/main/ets/ApiPortal.ts")
    );
    assert_eq!(
        api_file.source_file,
        "phone|@msi/runtime|12.59.400|src/main/ets/ApiPortal.ts"
    );
    assert_eq!(api_file.package_name.as_deref(), Some("@msi/runtime"));
}

#[test]
fn module_source_writer_uses_actual_package_file_path_for_package_root_record() {
    let app_root = tempfile::tempdir().unwrap();
    let project_root = app_root.path();
    let phone_root = project_root.join("products/phone");
    let imageknife_root = project_root
        .join("oh_modules/.ohpm/@meituan+imageknife@5.0.8/oh_modules/@meituan/imageknife");
    let entry_id = phone_root
        .join("src/main/ets/abilitystage/EntryAbilityStage.ets")
        .to_string_lossy()
        .into_owned();
    let imageknife_id = imageknife_root
        .join("index.ets")
        .to_string_lossy()
        .into_owned();
    std::fs::create_dir_all(&imageknife_root).unwrap();
    std::fs::write(
        imageknife_root.join("oh-package.json5"),
        r#"{
            "name": "@meituan/imageknife",
            "version": "5.0.8",
            "main": "Index.ets"
        }"#,
    )
    .unwrap();
    std::fs::write(
        imageknife_root.join("index.ets"),
        "export class ImageKnife {}",
    )
    .unwrap();

    let module_path_map = HvigorModulePathMap::new(
        "phone",
        &phone_root,
        &[("@meituan/imageknife".to_string(), imageknife_root.clone())],
    );
    let mut state = HvigorModuleSourceState::default();
    state.sources.insert(
        entry_id.clone(),
        HvigorCollectedModuleSource {
            code: "import { ImageKnife } from \"@meituan/imageknife\";"
                .to_string()
                .into(),
        },
    );
    state.sources.insert(
        imageknife_id.clone(),
        HvigorCollectedModuleSource {
            code: "export class ImageKnife {}".to_string().into(),
        },
    );
    record_resolved_import(&mut state, &entry_id, "@meituan/imageknife", &imageknife_id);
    let facts = ModuleGraphFacts {
        full_module_ids: vec![entry_id.clone(), imageknife_id.clone()],
        full_module_is_commonjs: FxHashMap::default(),
    };
    let out_dir = app_root
        .path()
        .join("cache")
        .join("default@CompileArkTS")
        .join("esmodule")
        .join("release");
    let mut packages = FxHashMap::default();
    packages.insert(
        "phone".to_string(),
        BundleRuntimePackageContext {
            package_name: "phone".to_string(),
            entry_path: "src/main/".to_string(),
            ..BundleRuntimePackageContext::default()
        },
    );
    packages.insert(
        "@meituan/imageknife".to_string(),
        BundleRuntimePackageContext {
            package_name: "@meituan/imageknife".to_string(),
            version: "5.0.8".to_string(),
            entry_path: "Index.ets".to_string(),
            ..BundleRuntimePackageContext::default()
        },
    );
    let runtime_packages = BundleRuntimePackageInfo { packages };

    let files = HvigorModuleSourceWriter::new(
        "com.example.hmosworld",
        "phone",
        project_root,
        &module_path_map,
        &runtime_packages,
        &[],
        true,
        &facts,
        facts.full_module_ids.clone(),
        state,
        out_dir,
    )
    .write()
    .unwrap();

    let imageknife_file = files
        .iter()
        .find(|file| file.module_id == imageknife_id)
        .expect("imageknife package entry module source");
    assert_eq!(
        imageknife_file.record_name,
        "&@meituan/imageknife/index&5.0.8"
    );
    let entry_file = files
        .iter()
        .find(|file| file.module_id == entry_id)
        .expect("entry module source");
    assert_eq!(
        entry_file.imported_record_names,
        vec!["&@meituan/imageknife/index&5.0.8".to_string()]
    );
    let output = std::fs::read_to_string(&entry_file.path).unwrap();
    assert!(
        output.contains("\"@normalized:N&&&@meituan/imageknife/index&5.0.8\""),
        "{output}"
    );
}

#[test]
fn module_source_writer_rewrites_bare_package_imports_from_rolldown_graph() {
    let app_root = tempfile::tempdir().unwrap();
    let project_root = app_root.path();
    let phone_root = project_root.join("products/phone");
    let log_root =
        project_root.join("oh_modules/.ohpm/@meituan+log@0.0.18/oh_modules/@meituan/log");
    let entry_id = phone_root
        .join("src/main/ets/abilitystage/EntryAbilityStage.ets")
        .to_string_lossy()
        .into_owned();
    let log_id = log_root.join("Index.ets").to_string_lossy().into_owned();
    std::fs::create_dir_all(&log_root).unwrap();
    std::fs::write(
        log_root.join("oh-package.json5"),
        r#"{
            "name": "@meituan/log",
            "version": "0.0.18",
            "main": "Index.ets"
        }"#,
    )
    .unwrap();
    std::fs::write(log_root.join("Index.ets"), "export class MLog {}").unwrap();

    let module_path_map = HvigorModulePathMap::new(
        "phone",
        &phone_root,
        &[("@meituan/log".to_string(), log_root.clone())],
    );
    let mut state = HvigorModuleSourceState::default();
    state.sources.insert(
        entry_id.clone(),
        HvigorCollectedModuleSource {
            code: "import { MLog } from \"@meituan/log\";".to_string().into(),
        },
    );
    state.sources.insert(
        log_id.clone(),
        HvigorCollectedModuleSource {
            code: "export class MLog {}".to_string().into(),
        },
    );
    state
        .resolved_imports
        .entry(entry_id.clone())
        .or_default()
        .insert("@meituan/log".to_string(), log_id.clone());
    let facts = ModuleGraphFacts {
        full_module_ids: vec![entry_id.clone(), log_id.clone()],
        full_module_is_commonjs: FxHashMap::default(),
    };
    let out_dir = app_root
        .path()
        .join("cache")
        .join("default@CompileArkTS")
        .join("esmodule")
        .join("release");
    let mut packages = FxHashMap::default();
    packages.insert(
        "phone".to_string(),
        BundleRuntimePackageContext {
            package_name: "phone".to_string(),
            entry_path: "src/main/".to_string(),
            ..BundleRuntimePackageContext::default()
        },
    );
    packages.insert(
        "@meituan/log".to_string(),
        BundleRuntimePackageContext {
            package_name: "@meituan/log".to_string(),
            version: "0.0.18".to_string(),
            entry_path: "Index.ets".to_string(),
            ..BundleRuntimePackageContext::default()
        },
    );
    let runtime_packages = BundleRuntimePackageInfo { packages };

    let files = HvigorModuleSourceWriter::new(
        "com.example.hmosworld",
        "phone",
        project_root,
        &module_path_map,
        &runtime_packages,
        &[],
        true,
        &facts,
        facts.full_module_ids.clone(),
        state,
        out_dir,
    )
    .write()
    .unwrap();

    let entry_file = files
        .iter()
        .find(|file| file.module_id == entry_id)
        .expect("entry module source");
    assert_eq!(
        entry_file.imported_record_names,
        vec!["&@meituan/log/Index&0.0.18".to_string()]
    );
    let output = std::fs::read_to_string(&entry_file.path).unwrap();
    assert!(
        output.contains("\"@normalized:N&&&@meituan/log/Index&0.0.18\""),
        "{output}"
    );
}

#[test]
fn module_source_writer_uses_actual_entry_file_when_pkg_context_entry_path_is_empty() {
    let app_root = tempfile::tempdir().unwrap();
    let project_root = app_root.path();
    let phone_root = project_root.join("products/phone");
    let pako_root = project_root.join("oh_modules/.ohpm/pako@2.1.0/oh_modules/pako");
    let entry_id = phone_root
        .join("src/main/ets/entryability/EntryAbility.ets")
        .to_string_lossy()
        .into_owned();
    let pako_id = pako_root
        .join("dist/pako.esm.mjs")
        .to_string_lossy()
        .into_owned();
    std::fs::create_dir_all(pako_root.join("dist")).unwrap();
    std::fs::write(
        pako_root.join("oh-package.json5"),
        r#"{
            "name": "pako",
            "version": "2.1.0",
            "module": "./dist/pako.esm.mjs",
            "exports": {
                ".": {
                    "import": "./dist/pako.esm.mjs"
                }
            }
        }"#,
    )
    .unwrap();
    std::fs::write(pako_root.join("dist/pako.esm.mjs"), "export const zip = 1;").unwrap();

    let module_path_map = HvigorModulePathMap::new(
        "phone",
        &phone_root,
        &[("pako".to_string(), pako_root.clone())],
    );
    let mut state = HvigorModuleSourceState::default();
    state.sources.insert(
        entry_id.clone(),
        HvigorCollectedModuleSource {
            code: "import { zip } from \"pako\";".to_string().into(),
        },
    );
    state.sources.insert(
        pako_id.clone(),
        HvigorCollectedModuleSource {
            code: "export const zip = 1;".to_string().into(),
        },
    );
    state
        .resolved_imports
        .entry(entry_id.clone())
        .or_default()
        .insert("pako".to_string(), pako_id.clone());
    let facts = ModuleGraphFacts {
        full_module_ids: vec![entry_id.clone(), pako_id.clone()],
        full_module_is_commonjs: FxHashMap::default(),
    };
    let out_dir = app_root
        .path()
        .join("cache")
        .join("default@CompileArkTS")
        .join("esmodule")
        .join("release");
    let mut packages = FxHashMap::default();
    packages.insert(
        "phone".to_string(),
        BundleRuntimePackageContext {
            package_name: "phone".to_string(),
            entry_path: "src/main/".to_string(),
            ..BundleRuntimePackageContext::default()
        },
    );
    packages.insert(
        "pako".to_string(),
        BundleRuntimePackageContext {
            package_name: "pako".to_string(),
            version: "2.1.0".to_string(),
            entry_path: String::new(),
            ..BundleRuntimePackageContext::default()
        },
    );
    let runtime_packages = BundleRuntimePackageInfo { packages };

    let files = HvigorModuleSourceWriter::new(
        "com.example.hmosworld",
        "phone",
        project_root,
        &module_path_map,
        &runtime_packages,
        &[],
        true,
        &facts,
        facts.full_module_ids.clone(),
        state,
        out_dir,
    )
    .write()
    .unwrap();

    let entry_file = files
        .iter()
        .find(|file| file.module_id == entry_id)
        .expect("entry module source");
    let pako_file = files
        .iter()
        .find(|file| file.module_id == pako_id)
        .expect("pako module source");
    assert_eq!(pako_file.record_name, "&pako/dist/pako.esm&2.1.0");
    assert_eq!(
        entry_file.imported_record_names,
        vec!["&pako/dist/pako.esm&2.1.0".to_string()]
    );
    let output = std::fs::read_to_string(&entry_file.path).unwrap();
    assert!(
        output.contains("\"@normalized:N&&&pako/dist/pako.esm&2.1.0\""),
        "{output}"
    );
}

#[test]
fn module_source_writer_matches_index_source_for_empty_main_package_root_record() {
    let app_root = tempfile::tempdir().unwrap();
    let project_root = app_root.path();
    let phone_root = project_root.join("products/phone");
    let subscribe_root = project_root
        .join("oh_modules/.ohpm/@meituan+card_subscribe@1.0.32/oh_modules/@meituan/card_subscribe");
    let entry_id = phone_root
        .join("src/main/ets/form/FormAbility.ets")
        .to_string_lossy()
        .into_owned();
    let subscribe_id = subscribe_root
        .join("index.ets")
        .to_string_lossy()
        .into_owned();
    std::fs::create_dir_all(&subscribe_root).unwrap();
    std::fs::write(
        subscribe_root.join("oh-package.json5"),
        r#"{
            "name": "@meituan/card_subscribe",
            "version": "1.0.32",
            "main": "",
            "metadata": {
                "sourceRoots": ["./src/main"],
                "useNormalizedOHMUrl": true
            }
        }"#,
    )
    .unwrap();
    std::fs::write(subscribe_root.join("index.ets"), "export const status = 1;").unwrap();

    let module_path_map = HvigorModulePathMap::new(
        "phone",
        &phone_root,
        &[(
            "@meituan/card_subscribe".to_string(),
            subscribe_root.clone(),
        )],
    );
    let mut state = HvigorModuleSourceState::default();
    state.sources.insert(
        entry_id.clone(),
        HvigorCollectedModuleSource {
            code: "import { status } from \"@meituan/card_subscribe\";"
                .to_string()
                .into(),
        },
    );
    state.sources.insert(
        subscribe_id.clone(),
        HvigorCollectedModuleSource {
            code: "export const status = 1;".to_string().into(),
        },
    );
    state
        .resolved_imports
        .entry(entry_id.clone())
        .or_default()
        .insert("@meituan/card_subscribe".to_string(), subscribe_id.clone());
    let facts = ModuleGraphFacts {
        full_module_ids: vec![entry_id.clone(), subscribe_id.clone()],
        full_module_is_commonjs: FxHashMap::default(),
    };
    let out_dir = app_root
        .path()
        .join("cache")
        .join("default@CompileArkTS")
        .join("esmodule")
        .join("release");
    let mut packages = FxHashMap::default();
    packages.insert(
        "phone".to_string(),
        BundleRuntimePackageContext {
            package_name: "phone".to_string(),
            entry_path: "src/main/".to_string(),
            ..BundleRuntimePackageContext::default()
        },
    );
    packages.insert(
        "@meituan/card_subscribe".to_string(),
        BundleRuntimePackageContext {
            package_name: "@meituan/card_subscribe".to_string(),
            version: "1.0.32".to_string(),
            entry_path: String::new(),
            ..BundleRuntimePackageContext::default()
        },
    );
    let runtime_packages = BundleRuntimePackageInfo { packages };

    let files = HvigorModuleSourceWriter::new(
        "com.example.hmosworld",
        "phone",
        project_root,
        &module_path_map,
        &runtime_packages,
        &[],
        true,
        &facts,
        facts.full_module_ids.clone(),
        state,
        out_dir,
    )
    .write()
    .unwrap();

    let entry_file = files
        .iter()
        .find(|file| file.module_id == entry_id)
        .expect("entry module source");
    let subscribe_file = files
        .iter()
        .find(|file| file.module_id == subscribe_id)
        .expect("subscribe package source");
    assert_eq!(
        subscribe_file.record_name,
        "&@meituan/card_subscribe/index&1.0.32"
    );
    assert_eq!(
        entry_file.imported_record_names,
        vec!["&@meituan/card_subscribe/index&1.0.32".to_string()]
    );
    let output = std::fs::read_to_string(&entry_file.path).unwrap();
    assert!(
        output.contains("\"@normalized:N&&&@meituan/card_subscribe/index&1.0.32\""),
        "{output}"
    );
}

#[test]
fn module_source_output_path_uses_transform_language_metadata() {
    let ownership = HvigorChunkSource::source_module(
        "entry".to_string(),
        "entry".to_string(),
        String::new(),
        "ets/Zip.ts".to_string(),
    );
    let output = HvigorModuleSourceOutputPath::new(
        Path::new("/workspace/cache"),
        Path::new("/workspace"),
        &ownership,
        "entry",
        "ets/Zip.ts",
        "/workspace/entry/src/main/ets/Zip.ts",
        true,
    )
    .path()
    .expect("output path");

    assert_eq!(
        output,
        Path::new("/workspace/cache/entry/src/main/ets/Zip.js")
    );
    assert_eq!(
        ownership.file_info_source_file("entry", "1.0.0", false, true),
        "entry|entry|1.0.0|src/main/ets/Zip.js"
    );

    let mjs_ownership = HvigorChunkSource::source_module(
        "entry".to_string(),
        "entry".to_string(),
        String::new(),
        "ets/vendor/pako.esm.mjs".to_string(),
    );
    assert_eq!(
        mjs_ownership.file_info_source_file("entry", "1.0.0", false, false),
        "entry|entry|1.0.0|src/main/ets/vendor/pako.esm.js"
    );

    let cjs_ownership = HvigorChunkSource::source_module(
        "entry".to_string(),
        "entry".to_string(),
        String::new(),
        "ets/vendor/index.cjs".to_string(),
    );
    assert_eq!(
        cjs_ownership.file_info_source_file("entry", "1.0.0", false, false),
        "entry|entry|1.0.0|src/main/ets/vendor/index.js"
    );
}

#[test]
fn module_source_writer_matches_latest_module_mode_final_newline_contract() {
    assert_eq!(
        HvigorModuleSourceWriter::finalize_source_text(
            "export const value = 1;".to_string(),
            false,
        ),
        "export const value = 1;\n"
    );
    assert_eq!(
        HvigorModuleSourceWriter::finalize_source_text(
            concat!(
                "//@ts-noCheck\r\n",
                "export const value = runtime();\r\n",
                "//# sourceMappingURL=ModuleInfo.js.map"
            )
            .to_string(),
            true,
        ),
        concat!(
            "//@ts-noCheck\r\n",
            "export const value = runtime();\r\n",
            "//# sourceMappingURL=ModuleInfo.js.map"
        )
    );
}

#[test]
fn module_source_writer_generates_hvigor_mock_config_and_origin_import() {
    let app_root = tempfile::tempdir().unwrap();
    let project_root = app_root.path();
    let module_root = project_root.join("entry");
    let dependency_root = project_root.join("oh_modules").join("service-lib");
    let src_root = module_root.join("src");
    let ets_root = src_root.join("main").join("ets");
    let main_source = ets_root.join("pages").join("Index.ets");
    let service_source = ets_root.join("service").join("Service.ets");
    let mock_root = src_root.join("mock");
    let mock_source = mock_root.join("ServiceMock.ets");
    let mock_config_path = mock_root.join("mock-config.json5");
    let dependency_main = dependency_root.join("index.ets");
    for path in [
        &main_source,
        &service_source,
        &mock_source,
        &mock_config_path,
        &dependency_main,
    ] {
        fs_err::create_dir_all(path.parent().unwrap()).unwrap();
    }
    fs_err::write(
        &main_source,
        "import { load } from \"../service/Service\"; export const value = load();",
    )
    .unwrap();
    fs_err::write(&service_source, "export const load = () => 1;").unwrap();
    fs_err::write(
        &mock_source,
        "import { load } from \"../main/ets/service/Service\"; export const mock = load;",
    )
    .unwrap();
    fs_err::write(
        &mock_config_path,
        r#"{
          "service/Service.ets": { "source": "src/mock/ServiceMock.ets" },
          "service-lib": { "source": "src/mock/ServiceMock.ets" }
        }"#,
    )
    .unwrap();
    fs_err::write(&dependency_main, "export const dependency = 1;").unwrap();
    fs_err::write(
        dependency_root.join("oh-package.json5"),
        r#"{ "name": "service-lib", "version": "1.0.0", "main": "index.ets" }"#,
    )
    .unwrap();

    let main_id = main_source.to_string_lossy().into_owned();
    let service_id = service_source.to_string_lossy().into_owned();
    let mock_id = mock_source.to_string_lossy().into_owned();
    let mut state = HvigorModuleSourceState::default();
    state.sources.insert(
        main_id.clone(),
        HvigorCollectedModuleSource {
            code: fs_err::read_to_string(&main_source).unwrap().into(),
        },
    );
    state.sources.insert(
        service_id.clone(),
        HvigorCollectedModuleSource {
            code: fs_err::read_to_string(&service_source).unwrap().into(),
        },
    );
    state.sources.insert(
        mock_id.clone(),
        HvigorCollectedModuleSource {
            code: fs_err::read_to_string(&mock_source).unwrap().into(),
        },
    );
    record_resolved_import(&mut state, &main_id, "../service/Service", &service_id);
    record_resolved_import(
        &mut state,
        &mock_id,
        "../main/ets/service/Service",
        &service_id,
    );
    let facts = ModuleGraphFacts {
        full_module_ids: vec![main_id.clone(), service_id, mock_id.clone()],
        full_module_is_commonjs: FxHashMap::default(),
    };
    let module_path_map = HvigorModulePathMap::new(
        "entry",
        &module_root,
        &[
            ("entry".to_string(), module_root.clone()),
            ("service-lib".to_string(), dependency_root.clone()),
        ],
    );
    let runtime_packages =
        runtime_packages(&[("entry", "entry", "com.example", "entry", "1.0.0", false)]);
    let cache_root = project_root.join("cache");
    let transformed_output_path = project_root.join("intermediates").join("mock-config.json");
    let mock_options = BundleMockConfig {
        process: true,
        user_config_path: Some(mock_config_path.clone()),
        ets_source_root: ets_root,
        transformed_output_path: transformed_output_path.clone(),
        transformed_cache_path: cache_root.join("mock-config.json"),
        user_cache_path: cache_root.join("mock-config.json5"),
        dependencies: vec![crate::plan::BundleMockDependency {
            dependency_name: "service-lib".to_string(),
            package_name: "service-lib".to_string(),
            package_root: dependency_root,
            main_file_path: dependency_main,
        }],
    };
    let files = HvigorModuleSourceWriter::new(
        "com.example",
        "entry",
        project_root,
        &module_path_map,
        &runtime_packages,
        &[],
        false,
        &facts,
        facts.full_module_ids.clone(),
        state,
        cache_root.clone(),
    )
    .with_mock_config(Some(&mock_options))
    .write()
    .unwrap();

    let main_output = fs_err::read_to_string(
        &files
            .iter()
            .find(|file| file.module_id == main_id)
            .unwrap()
            .path,
    )
    .unwrap();
    let mock_output = fs_err::read_to_string(
        &files
            .iter()
            .find(|file| file.module_id == mock_id)
            .unwrap()
            .path,
    )
    .unwrap();
    assert!(!main_output.contains(".origin"), "{main_output}");
    assert!(mock_output.contains(".origin\""), "{mock_output}");

    let transformed = serde_json::from_slice::<serde_json::Value>(
        &fs_err::read(&transformed_output_path).unwrap(),
    )
    .unwrap();
    let entries = transformed.as_object().unwrap();
    let (origin_url, origin_entry) = entries
        .iter()
        .find(|(key, _)| key.ends_with(".origin"))
        .expect("origin mock entry");
    let actual_url = origin_url.strip_suffix(".origin").unwrap();
    assert_eq!(origin_entry["source"], actual_url);
    assert!(entries[actual_url]["source"]
        .as_str()
        .is_some_and(|source| source.contains("src/mock/ServiceMock")));
    assert!(
        entries
            .keys()
            .any(|url| url.contains("service-lib") && !url.ends_with(".origin")),
        "dependency mock keys must be eagerly converted like collectMockConfigInfo: {entries:?}"
    );
    assert_eq!(
        fs_err::read(&mock_options.transformed_cache_path).unwrap(),
        fs_err::read(&transformed_output_path).unwrap()
    );
    assert_eq!(
        fs_err::read(&mock_options.user_cache_path).unwrap(),
        fs_err::read(&mock_config_path).unwrap()
    );
}

#[test]
fn module_source_writer_rewrites_ohpm_imports_to_package_records() {
    let app_root = tempfile::tempdir().unwrap();
    let project_root = app_root.path();
    let phone_root = project_root.join("products/phone");
    let hmcore_root = project_root
        .join("oh_modules/.ohpm/@hw-agconnect+hmcore@1.0.6/oh_modules/@hw-agconnect/hmcore");
    let hypium_root =
        project_root.join("oh_modules/.ohpm/@ohos+hypium@1.0.11/oh_modules/@ohos/hypium");
    let entry_id = phone_root
        .join("src/main/ets/entryability/EntryAbility.ets")
        .to_string_lossy()
        .into_owned();
    let hmcore_id = hmcore_root.join("index.ts").to_string_lossy().into_owned();
    let hypium_id = hypium_root.join("index.js").to_string_lossy().into_owned();
    let logger_id = hmcore_root
        .join("src/main/ets/base/log/Logger.ts")
        .to_string_lossy()
        .into_owned();
    let router_id = hmcore_root
        .join("src/main/ets/components/RouterMap.ets")
        .to_string_lossy()
        .into_owned();
    std::fs::create_dir_all(&hmcore_root).unwrap();
    std::fs::write(
        hmcore_root.join("oh-package.json5"),
        r#"{
            "name": "@hw-agconnect/hmcore",
            "version": "1.0.6",
            "main": "index"
        }"#,
    )
    .unwrap();
    std::fs::write(hmcore_root.join("index.ts"), "export {};").unwrap();
    std::fs::create_dir_all(&hypium_root).unwrap();
    std::fs::write(
        hypium_root.join("oh-package.json5"),
        r#"{
            "name": "@ohos/hypium",
            "version": "1.0.11",
            "main": "index.js"
        }"#,
    )
    .unwrap();
    std::fs::write(
        hypium_root.join("index.js"),
        "export const DEFAULT = 'default';",
    )
    .unwrap();
    std::fs::create_dir_all(hmcore_root.join("src/main/ets/base/log")).unwrap();
    std::fs::write(
        hmcore_root.join("src/main/ets/base/log/Logger.ts"),
        "export class Logger {}",
    )
    .unwrap();
    std::fs::create_dir_all(hmcore_root.join("src/main/ets/components")).unwrap();
    std::fs::write(
        hmcore_root.join("src/main/ets/components/RouterMap.ets"),
        "@Component\nstruct RouterMap { build() {} }",
    )
    .unwrap();
    let module_path_map = HvigorModulePathMap::new(
        "phone",
        &phone_root,
        &[
            ("@hw-agconnect/hmcore".to_string(), hmcore_root.clone()),
            ("@ohos/hypium".to_string(), hypium_root.clone()),
        ],
    );
    let mut state = HvigorModuleSourceState::default();
    state.sources.insert(
        entry_id.clone(),
        HvigorCollectedModuleSource {
            code: [
                "import { initialize } from \"@hw-agconnect/hmcore\";",
                "import { DEFAULT } from \"@ohos/hypium\";",
            ]
            .join("\n")
            .into(),
        },
    );
    state.sources.insert(
        hmcore_id.clone(),
        HvigorCollectedModuleSource {
            code: [
                "export { Logger } from \"./src/main/ets/base/log/Logger\";",
                "export { RouterMap } from \"./src/main/ets/components/RouterMap\";",
            ]
            .join("\n")
            .into(),
        },
    );
    state.sources.insert(
        hypium_id.clone(),
        HvigorCollectedModuleSource {
            code: "export const DEFAULT = 'default';".to_string().into(),
        },
    );
    state.sources.insert(
        logger_id.clone(),
        HvigorCollectedModuleSource {
            code: "export class Logger {}".to_string().into(),
        },
    );
    state.sources.insert(
        router_id.clone(),
        HvigorCollectedModuleSource {
            code: "export class RouterMap {}".to_string().into(),
        },
    );
    state
        .resolved_imports
        .entry(entry_id.clone())
        .or_default()
        .insert("@hw-agconnect/hmcore".to_string(), hmcore_id.clone());
    state
        .resolved_imports
        .entry(entry_id.clone())
        .or_default()
        .insert("@ohos/hypium".to_string(), hypium_id.clone());
    state
        .resolved_imports
        .entry(hmcore_id.clone())
        .or_default()
        .insert(
            "./src/main/ets/base/log/Logger".to_string(),
            logger_id.clone(),
        );
    state
        .resolved_imports
        .entry(hmcore_id.clone())
        .or_default()
        .insert(
            "./src/main/ets/components/RouterMap".to_string(),
            router_id.clone(),
        );
    let facts = ModuleGraphFacts {
        full_module_ids: vec![
            entry_id.clone(),
            hmcore_id.clone(),
            hypium_id.clone(),
            logger_id.clone(),
            router_id.clone(),
        ],
        full_module_is_commonjs: FxHashMap::default(),
    };
    let out_dir = app_root
        .path()
        .join("cache")
        .join("default@CompileArkTS")
        .join("esmodule")
        .join("release");
    let runtime_packages = runtime_packages(&[
        ("phone", "phone", "", "", "", false),
        (
            "@hw-agconnect/hmcore",
            "@hw-agconnect/hmcore",
            "",
            "",
            "1.0.6",
            false,
        ),
        ("@ohos/hypium", "@ohos/hypium", "", "", "1.0.11", false),
    ]);

    let files = HvigorModuleSourceWriter::new(
        "com.example.hmosworld",
        "phone",
        project_root,
        &module_path_map,
        &runtime_packages,
        &[],
        true,
        &facts,
        facts.full_module_ids.clone(),
        state,
        out_dir,
    )
    .write()
    .unwrap();

    let entry_file = files
        .iter()
        .find(|file| file.module_id == entry_id)
        .expect("entry module source");
    let hmcore_file = files
        .iter()
        .find(|file| file.module_id == hmcore_id)
        .expect("hmcore module source");
    let hypium_file = files
        .iter()
        .find(|file| file.module_id == hypium_id)
        .expect("hypium module source");
    let logger_file = files
        .iter()
        .find(|file| file.module_id == logger_id)
        .expect("logger module source");
    let router_file = files
        .iter()
        .find(|file| file.module_id == router_id)
        .expect("router module source");
    assert_eq!(hmcore_file.record_name, "&@hw-agconnect/hmcore/index&1.0.6");
    assert_eq!(hypium_file.record_name, "&@ohos/hypium/index&1.0.11");
    assert_eq!(
        hmcore_file.source_file,
        "phone|@hw-agconnect/hmcore|1.0.6|index.ts"
    );
    assert_eq!(
        hypium_file.source_file,
        "phone|@ohos/hypium|1.0.11|index.js"
    );
    assert_eq!(
        entry_file.path,
        app_root
            .path()
            .join("cache")
            .join("default@CompileArkTS")
            .join("esmodule")
            .join("release")
            .join("phone")
            .join("src")
            .join("main")
            .join("ets")
            .join("entryability")
            .join("EntryAbility.ts")
    );
    assert_eq!(
        hmcore_file.path,
        app_root
            .path()
            .join("cache")
            .join("default@CompileArkTS")
            .join("esmodule")
            .join("release")
            .join("oh_modules")
            .join(".ohpm")
            .join("@hw-agconnect+hmcore@1.0.6")
            .join("oh_modules")
            .join("@hw-agconnect")
            .join("hmcore")
            .join("index.ts")
    );
    // `ModuleMode.processModuleInfos` computes packageName via
    // `getPkgModulesFilePkgName()`: the physical OHPM record root
    // (path.join("pkg_modules", pkgPath - projectRoot/packageDir)
    // with packageDir replaced by the constant "pkg_modules"),
    // not the logical package name. The logical name only appears in
    // the `phone|<pkg>|<version>|<entry>` source_file shape.
    // Evidence: SDK ets-loader `module/module_mode.js::getPkgModulesFilePkgName`.
    assert_eq!(
        hmcore_file.package_name.as_deref(),
        Some("pkg_modules/.ohpm/@hw-agconnect+hmcore@1.0.6/pkg_modules/@hw-agconnect/hmcore")
    );
    assert_eq!(
        logger_file.record_name,
        "&@hw-agconnect/hmcore/src/main/ets/base/log/Logger&1.0.6"
    );
    assert_eq!(
        router_file.record_name,
        "&@hw-agconnect/hmcore/src/main/ets/components/RouterMap&1.0.6"
    );
    assert_eq!(
        router_file.source_file,
        "phone|@hw-agconnect/hmcore|1.0.6|src/main/ets/components/RouterMap.ts"
    );
    assert_eq!(
        router_file.path,
        app_root
            .path()
            .join("cache")
            .join("default@CompileArkTS")
            .join("esmodule")
            .join("release")
            .join("oh_modules")
            .join(".ohpm")
            .join("@hw-agconnect+hmcore@1.0.6")
            .join("oh_modules")
            .join("@hw-agconnect")
            .join("hmcore")
            .join("src")
            .join("main")
            .join("ets")
            .join("components")
            .join("RouterMap.ts")
    );
    let entry_output = std::fs::read_to_string(&entry_file.path).unwrap();
    assert!(
        entry_output.contains("\"@normalized:N&&&@hw-agconnect/hmcore/index&1.0.6\""),
        "{entry_output}"
    );
    assert!(
        entry_output.contains("\"@normalized:N&&&@ohos/hypium/index&1.0.11\""),
        "{entry_output}"
    );
    let hmcore_output = std::fs::read_to_string(&hmcore_file.path).unwrap();
    assert!(
        hmcore_output.contains(
            "\"@normalized:N&&&@hw-agconnect/hmcore/src/main/ets/base/log/Logger&1.0.6\""
        ),
        "{hmcore_output}"
    );
    assert!(
        hmcore_output.contains(
            "\"@normalized:N&&&@hw-agconnect/hmcore/src/main/ets/components/RouterMap&1.0.6\""
        ),
        "{hmcore_output}"
    );
}
