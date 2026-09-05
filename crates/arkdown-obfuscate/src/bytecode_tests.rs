#![cfg(unix)]

use std::os::unix::fs::PermissionsExt;
use std::path::Path;
use std::sync::Arc;

use serde_json::Value;
use tempfile::TempDir;

use crate::{
    ArkguardAdapterOptions, BytecodeObfuscationInput, BytecodeObfuscationModuleRecord,
    ObfuscationModuleFile, ObfuscationModuleInput, ObfuscationOptions, ObfuscationSource,
    Obfuscator,
};

struct NativeBytecodeFixture {
    temp: TempDir,
    options: ObfuscationOptions,
    input: BytecodeObfuscationInput,
}

impl NativeBytecodeFixture {
    fn new() -> Self {
        let temp = tempfile::tempdir().expect("tempdir");
        let root = temp.path();
        let module_root = root.join("entry");
        let cache_path = module_root.join("build/cache/CompileArkTS");
        let ace_module_build = module_root.join("build/intermediates/loader");
        let obfuscation_cache = cache_path.join("obfuscation");
        let source_maps_input = module_root.join("build/intermediates/sourceMaps.map");
        let modules_abc = ace_module_build.join("modules.abc");
        let rule_file = module_root.join("obfuscation-rules.txt");
        let arkguard_package = root.join("sdk/arkguard");
        let panda_guard = root.join("sdk/bin/panda_guard");

        for directory in [
            &cache_path,
            &ace_module_build,
            source_maps_input.parent().expect("source map parent"),
            &obfuscation_cache,
            panda_guard.parent().expect("panda guard parent"),
            &arkguard_package.join("lib/configs/preset"),
        ] {
            fs_err::create_dir_all(directory).expect("fixture directory");
        }
        fs_err::write(&modules_abc, b"original-abc").expect("modules abc");
        fs_err::write(cache_path.join("filesInfo.txt"), "files-info\n").expect("files info");
        fs_err::write(cache_path.join("sourceMaps.json"), "{\"previous\":{}}")
            .expect("previous source maps");
        fs_err::write(
            &source_maps_input,
            serde_json::to_vec(&serde_json::json!({
                "entry/src/main/ets/Index.ets": {
                    "version": 3,
                    "sources": ["Index.ets"],
                    "names": [],
                    "mappings": "AAAA;AAEA",
                    "sourcesContent": ["export class PublicApi {}\n"]
                }
            }))
            .expect("source maps JSON"),
        )
        .expect("source maps");
        fs_err::write(
            arkguard_package.join("lib/configs/preset/es_reserved_properties.json"),
            r#"{"builtins":["prototype","length"]}"#,
        )
        .expect("ArkGuard ES preset");
        fs_err::write(
            &rule_file,
            "-enable-bytecode-obfuscation\n-enable-property-obfuscation\n-enable-toplevel-obfuscation\n-enable-filename-obfuscation\n-enable-export-obfuscation\n-remove-comments\n-keep-comments\nkeptProperty\n-keep-property-name\nkeptProperty\n-keep-global-name\nKeptGlobal\n-keep-file-name\nPublic.ets\nfoo.bar.ets\n",
        )
        .expect("rule file");

        let name_cache = obfuscation_cache.join("nameCache.json");
        let obfuscated_abc = obfuscation_cache.join("obf/modules.abc");
        let script = format!(
            "#!/bin/sh\nset -eu\nprintf '%s' 'obfuscated-abc' > '{}'\nprintf '%s' '{}' > '{}'\n",
            Self::shell_path(&obfuscated_abc),
            r##"{"entry":{"OriSourceFile":"entry/src/main/ets/Index.ets","ObfSourceFile":"a.ets","IdentifierCache":{"#scope:1:2":"x"},"MemberMethodCache":{}}}"##,
            Self::shell_path(&name_cache),
        );
        fs_err::write(&panda_guard, script).expect("fake panda_guard");
        let mut permissions = fs_err::metadata(&panda_guard)
            .expect("panda_guard metadata")
            .permissions();
        permissions.set_mode(0o755);
        fs_err::set_permissions(&panda_guard, permissions).expect("executable panda_guard");

        let original_source = module_root.join("src/main/ets/Index.ets");
        let transformed_source = cache_path.join("entry/src/main/ets/Index.ts");
        let adapter = ArkguardAdapterOptions {
            panda_guard_path: panda_guard,
            arkguard_package,
            module_root: module_root.clone(),
            project_root: root.to_path_buf(),
            project_path: module_root.join("src/main/ets"),
            cache_path: cache_path.clone(),
            ace_module_build: ace_module_build.clone(),
            compile_sdk_version: "5.0.0.71".to_string(),
            compatible_sdk_version: 13,
            compatible_sdk_version_stage: None,
            entry_package_info: "entry|1.0.0".to_string(),
            resource_table_path: Some(module_root.join("build/generated/r/ResourceTable.ts")),
            use_normalized: true,
            byte_code_har_info: serde_json::json!({"remote-har": {}}),
        };
        let options = ObfuscationOptions {
            enable_obfuscation: true,
            rule_files: vec![rule_file],
            dependency_rule_files: Vec::new(),
            cache_dir: obfuscation_cache,
            export_rule_path: None,
            arkguard: Some(adapter),
        };
        let declaration_path = cache_path.join("entry/src/main/ets/Api.d.ts");
        fs_err::create_dir_all(declaration_path.parent().expect("declaration parent"))
            .expect("declaration directory");
        let declaration_source = "// removed declaration comment\nexport declare class DeclarationApi {\n// kept member comment\nkeptProperty: string;\n// removed member comment\nrenamedProperty: string;\n}";
        fs_err::write(&declaration_path, declaration_source).expect("declaration source");
        let module = ObfuscationModuleInput {
            files: vec![
                ObfuscationModuleFile {
                    build_path: transformed_source,
                    original_path: original_source.clone(),
                    relative_path: "entry/src/main/ets/Index.ets".to_string(),
                    record_name: None,
                    rename_file: true,
                    source_name_cache_key: None,
                    source_map_key: "entry/src/main/ets/Index.ets".to_string(),
                    previous_source_map: None,
                    source: Arc::from("export class PublicApi {}"),
                    insight_intent_global_names: vec!["InsightIntentEntry".to_string()],
                    insight_intent_property_names: vec!["execute".to_string()],
                },
                ObfuscationModuleFile {
                    build_path: declaration_path,
                    original_path: module_root.join("src/main/ets/Api.ts"),
                    relative_path: "entry/src/main/ets/Api.d.ts".to_string(),
                    record_name: None,
                    rename_file: true,
                    source_name_cache_key: Some("entry/src/main/ets/Api.ts".to_string()),
                    source_map_key: "entry/src/main/ets/Api.d.ts".to_string(),
                    previous_source_map: None,
                    source: Arc::from(declaration_source),
                    insight_intent_global_names: Vec::new(),
                    insight_intent_property_names: Vec::new(),
                },
            ],
            inventory_sources: Vec::new(),
            module_dependencies: std::collections::BTreeMap::new(),
            preserved_source_maps: std::collections::BTreeMap::new(),
            source_maps_path: Some(source_maps_input),
            source_map_mirror_paths: Vec::new(),
        };
        let input = BytecodeObfuscationInput {
            module,
            modules_abc_path: modules_abc,
            module_records: vec![
                BytecodeObfuscationModuleRecord {
                    source_path: original_source.clone(),
                    record_name: "&entry/src/main/ets/Index&".to_string(),
                },
                BytecodeObfuscationModuleRecord {
                    source_path: module_root.join("build/generated/r/ResourceTable.ts"),
                    record_name: "&entry/build/generated/r/ResourceTable&".to_string(),
                },
            ],
            record_name_white_list: vec!["&entry/build/generated/r/ResourceTable&".to_string()],
            reserved_name_sources: vec![
                ObfuscationSource {
                    path: original_source,
                    source: Arc::from(
                        "@Entry export class PublicApi { public visibleProperty: number = 1; }",
                    ),
                    sdk_declaration: false,
                },
                ObfuscationSource {
                    path: root.join("sdk/api/global.d.ts"),
                    source: Arc::from("declare interface SystemApi { systemMember: string; }"),
                    sdk_declaration: true,
                },
            ],
        };
        Self {
            temp,
            options,
            input,
        }
    }

    fn shell_path(path: &Path) -> String {
        path.to_str()
            .expect("UTF-8 fixture path")
            .replace('\'', "'\\''")
    }
}

#[test]
fn native_panda_guard_executes_and_updates_hvigor_artifacts() {
    let fixture = NativeBytecodeFixture::new();
    let modules_abc = fixture.input.modules_abc_path.clone();
    let output = Obfuscator::new(fixture.options)
        .expect("obfuscator")
        .obfuscate_bytecode(fixture.input)
        .expect("native bytecode obfuscation");

    assert_eq!(
        fs_err::read_to_string(&modules_abc).expect("rewritten modules.abc"),
        "obfuscated-abc"
    );
    assert_eq!(
        fs_err::read_to_string(&output.origin_abc_path).expect("origin modules.abc"),
        "original-abc"
    );
    assert!(output.obfuscated_abc_path.is_file());
    let source_maps: Value = serde_json::from_slice(
        &fs_err::read(&output.source_maps_path).expect("updated source maps"),
    )
    .expect("source maps JSON");
    assert!(source_maps.get("a.ets").is_some());
    assert!(source_maps.get("entry/src/main/ets/Index.ets").is_none());
    let published_source_maps: Value = serde_json::from_slice(
        &fs_err::read(
            fixture
                .temp
                .path()
                .join("entry/build/intermediates/sourceMaps.map"),
        )
        .expect("published source maps"),
    )
    .expect("published source maps JSON");
    assert!(published_source_maps.get("a.ets").is_some());
    let name_cache: Value =
        serde_json::from_slice(&fs_err::read(&output.name_cache_path).expect("name cache"))
            .expect("name cache JSON");
    assert_eq!(name_cache["entry"]["IdentifierCache"]["#scope:1:3"], "x");

    let config: Value = serde_json::from_slice(&fs_err::read(&output.config_path).expect("config"))
        .expect("config JSON");
    let reserved_names = config["obfuscationRules"]["reservedNames"]
        .as_array()
        .expect("reserved names");
    for expected in [
        "PublicApi",
        "SystemApi",
        "systemMember",
        "InsightIntentEntry",
        "execute",
        "_ESAnnotation",
        "prototype",
    ] {
        assert!(
            reserved_names.iter().any(|value| value == expected),
            "missing reserved name {expected}"
        );
    }
    let reserved_properties = config["obfuscationRules"]["propertyObfuscation"]
        ["reservedProperties"]
        .as_array()
        .expect("reserved properties");
    for expected in ["keptProperty", "componentClass", "getReuseId"] {
        assert!(
            reserved_properties.iter().any(|value| value == expected),
            "missing reserved property {expected}"
        );
    }
    assert_eq!(
        config["obfuscationRules"]["fileNameObfuscation"]["reservedFileNames"][0],
        "Public"
    );
    assert_eq!(
        config["obfuscationRules"]["fileNameObfuscation"]["reservedFileNames"][1],
        "foo.bar.ets"
    );
    assert_eq!(config["obfuscationRules"]["keepOptions"]["enable"], false);
    assert_eq!(
        config["obfuscationRules"]["keepOptions"]["keepPaths"][0],
        "&entry/build/generated/r/ResourceTable&"
    );
    assert_eq!(config["skippedRemoteHarList"][0], "remote-har");
    assert_eq!(config["useNormalizedOHMUrl"], true);
    assert!(config.get("useNormalizedOhmUrl").is_none());
    assert!(fixture
        .temp
        .path()
        .join("entry/build/cache/CompileArkTS/sourceMaps.json")
        .is_file());

    let declaration =
        walkdir::WalkDir::new(fixture.temp.path().join("entry/build/cache/CompileArkTS"))
            .into_iter()
            .filter_map(Result::ok)
            .find(|entry| {
                entry.file_type().is_file()
                    && entry
                        .file_name()
                        .to_str()
                        .is_some_and(|name| name.ends_with(".d.ts"))
            })
            .expect("native ArkGuard declaration output");
    let declaration = fs_err::read_to_string(declaration.path()).expect("read declaration output");
    assert!(declaration.contains("kept member comment"));
    assert!(!declaration.contains("removed declaration comment"));
    assert!(!declaration.contains("removed member comment"));
    assert!(declaration.contains("keptProperty"));
    assert!(!declaration.contains("renamedProperty"));
}
