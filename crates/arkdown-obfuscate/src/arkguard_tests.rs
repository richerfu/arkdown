use std::collections::BTreeMap;
use std::path::{Path, PathBuf};
use std::sync::Arc;

use arkdown_core::HvigorModuleSourceFile;
use tempfile::TempDir;

use crate::{
    ArkguardAdapterOptions, ObfuscationError, ObfuscationModuleFile, ObfuscationModuleInput,
    ObfuscationOptions, ObfuscationSource, Obfuscator,
};

struct NativeArkguardFixture {
    temporary: TempDir,
    cache: PathBuf,
    compile_cache: PathBuf,
    arkguard_package: PathBuf,
}

impl NativeArkguardFixture {
    fn new() -> Self {
        let temporary = tempfile::tempdir().expect("native ArkGuard fixture");
        let cache = temporary.path().join("obfuscation");
        let compile_cache = temporary.path().join("compile-cache");
        let arkguard_package = temporary.path().join("sdk/arkguard");
        let presets = arkguard_package.join("lib/configs/preset");
        fs_err::create_dir_all(&presets).expect("create ArkGuard preset directory");
        fs_err::write(presets.join("es_reserved_properties.json"), b"{}")
            .expect("write reserved-name preset");
        Self {
            temporary,
            cache,
            compile_cache,
            arkguard_package,
        }
    }

    fn adapter(&self) -> ArkguardAdapterOptions {
        ArkguardAdapterOptions {
            panda_guard_path: self.temporary.path().join("sdk/bin/panda_guard"),
            arkguard_package: self.arkguard_package.clone(),
            module_root: self.temporary.path().join("entry"),
            project_root: self.temporary.path().to_path_buf(),
            project_path: self.temporary.path().join("entry/src/main/ets"),
            cache_path: self.compile_cache.clone(),
            ace_module_build: self.compile_cache.clone(),
            compile_sdk_version: "5.0.0.71".to_string(),
            compatible_sdk_version: 13,
            compatible_sdk_version_stage: None,
            entry_package_info: "entry|1.0.0".to_string(),
            compile_har: false,
            resource_table_path: Some(
                self.temporary
                    .path()
                    .join("entry/build/generated/r/ResourceTable.ts"),
            ),
            use_normalized: true,
            byte_code_har_info: serde_json::json!({}),
            reserved_file_paths: Vec::new(),
            arkui_reserved_property_names: Vec::new(),
            arkui_optimized_reserved_property_names: Vec::new(),
        }
    }

    fn obfuscator(&self, rules: &str) -> Obfuscator {
        self.obfuscator_with_module_kind(rules, false)
    }

    fn obfuscator_with_module_kind(&self, rules: &str, compile_har: bool) -> Obfuscator {
        let rule_path = self.temporary.path().join("obfuscation-rules.txt");
        fs_err::write(&rule_path, rules).expect("write ArkGuard rules");
        let mut adapter = self.adapter();
        adapter.compile_har = compile_har;
        Obfuscator::new(ObfuscationOptions {
            enable_obfuscation: true,
            rule_files: vec![rule_path],
            dependency_rule_files: Vec::new(),
            cache_dir: self.cache.clone(),
            export_rule_path: None,
            arkguard: Some(adapter),
        })
        .expect("create native source obfuscator")
    }

    fn source_file(&self, relative: &str, source: &str) -> ObfuscationModuleFile {
        let original_path = self.temporary.path().join(relative);
        fs_err::create_dir_all(original_path.parent().expect("original source parent"))
            .expect("create original source parent");
        fs_err::write(&original_path, source).expect("write original source");
        let mut build_path = self.compile_cache.join(relative);
        build_path.set_extension("js");
        fs_err::create_dir_all(build_path.parent().expect("source parent"))
            .expect("create CompileArkTS source parent");
        fs_err::write(&build_path, source).expect("write CompileArkTS source");
        ObfuscationModuleFile {
            build_path,
            original_path,
            relative_path: relative.to_string(),
            record_name: Some(format!(
                "com.example.arkdown/entry/{}",
                relative
                    .strip_prefix("entry/src/main/")
                    .unwrap_or(relative)
                    .trim_end_matches(".ets")
            )),
            rename_file: true,
            source_name_cache_key: None,
            source_map_key: relative.to_string(),
            previous_source_map: None,
            source: Arc::from(source),
            insight_intent_global_names: Vec::new(),
            insight_intent_property_names: Vec::new(),
        }
    }

    fn run(
        &self,
        obfuscator: &Obfuscator,
        files: Vec<ObfuscationModuleFile>,
    ) -> crate::ObfuscationModuleOutput {
        self.run_with_dependencies(obfuscator, files, BTreeMap::new())
    }

    fn run_with_dependencies(
        &self,
        obfuscator: &Obfuscator,
        files: Vec<ObfuscationModuleFile>,
        module_dependencies: BTreeMap<PathBuf, Vec<PathBuf>>,
    ) -> crate::ObfuscationModuleOutput {
        let inventory_sources = files
            .iter()
            .map(|file| ObfuscationSource {
                path: file.original_path.clone(),
                source: Arc::clone(&file.source),
                kind: crate::ObfuscationSourceKind::Project,
            })
            .collect();
        self.run_with_inventory(obfuscator, files, module_dependencies, inventory_sources)
    }

    fn run_with_inventory(
        &self,
        obfuscator: &Obfuscator,
        files: Vec<ObfuscationModuleFile>,
        module_dependencies: BTreeMap<PathBuf, Vec<PathBuf>>,
        inventory_sources: Vec<ObfuscationSource>,
    ) -> crate::ObfuscationModuleOutput {
        obfuscator
            .obfuscate_module(ObfuscationModuleInput {
                files,
                inventory_sources,
                module_dependencies,
                preserved_source_maps: BTreeMap::new(),
                source_maps_path: Some(self.compile_cache.join("sourceMaps.map")),
                source_map_mirror_paths: vec![self.compile_cache.join("sourceMaps.json")],
            })
            .expect("run native source obfuscation")
    }
}

#[test]
fn native_arkguard_uses_module_ordered_names_and_writes_hvigor_cache_shape() {
    let fixture = NativeArkguardFixture::new();
    let source = r#"
        const LongTopLevel = 1;
        function calculate(longParameter) {
            console.log(longParameter);
            const localObject = { internalProperty: longParameter + LongTopLevel };
            return localObject.internalProperty;
        }
        export { calculate };
    "#;
    let mut file = fixture.source_file("entry/src/main/ets/Index.ets", source);
    file.source_map_key = "entry|entry|1.0.0|src/main/ets/Index.ts".to_string();
    let input_path = file.build_path.clone();
    let obfuscator = fixture.obfuscator(
        "-enable-toplevel-obfuscation\n-enable-property-obfuscation\n-enable-string-property-obfuscation\n-enable-export-obfuscation\n-enable-filename-obfuscation\n-remove-log\n-compact\n",
    );

    let output = fixture.run(&obfuscator, vec![file]);

    assert_eq!(output.files.len(), 1);
    assert_ne!(output.files[0].output_path, input_path);
    assert!(
        !input_path.exists(),
        "renamed source must remove the stale path"
    );
    let code =
        fs_err::read_to_string(&output.files[0].output_path).expect("read obfuscated source");
    assert!(!code.contains("LongTopLevel"));
    assert!(!code.contains("longParameter"));
    assert!(!code.contains("internalProperty"));
    assert!(!code.contains("console"));

    let cache: serde_json::Value = serde_json::from_slice(
        &fs_err::read(&output.name_cache_path).expect("read native name cache"),
    )
    .expect("parse native name cache");
    assert_eq!(cache["compileSdkVersion"], "5.0.0.71");
    assert_eq!(cache["entryPackageInfo"], "entry|1.0.0");
    assert!(cache["PropertyCache"]["internalProperty"].is_string());
    assert_eq!(cache["FileNameCache"]["Index"], "a");
    assert!(cache["entry/src/main/ets/Index.ets"]["IdentifierCache"].is_object());

    let mut module_sources = vec![HvigorModuleSourceFile {
        module_id: fixture
            .temporary
            .path()
            .join("entry/src/main/ets/Index.ets")
            .into_os_string()
            .into_string()
            .expect("UTF-8 test path"),
        path: input_path,
        record_name: "com.example.arkdown/entry/ets/Index".to_string(),
        record_name_is_explicit: false,
        source_file: "entry|entry|1.0.0|src/main/ets/Index.ts".to_string(),
        package_name: Some("entry".to_string()),
        is_commonjs: false,
        is_shared_module: false,
        origin_source_lang: "ets".to_string(),
        imported_record_names: vec!["com.example.arkdown/entry/ets/Index".to_string()],
        is_package_module: false,
    }];
    output
        .apply_to_module_sources(&mut module_sources)
        .expect("apply ArkGuard ModuleMode handoff");
    assert_eq!(module_sources[0].path, output.files[0].output_path);
    assert_eq!(
        module_sources[0].source_file,
        "entry|entry|1.0.0|src/main/ets/a.ts"
    );
    assert_eq!(
        module_sources[0].record_name,
        "com.example.arkdown/entry/ets/a"
    );
    assert_eq!(
        module_sources[0].imported_record_names,
        ["com.example.arkdown/entry/ets/a"]
    );
    let source_maps: serde_json::Value = serde_json::from_slice(
        &fs_err::read(fixture.compile_cache.join("sourceMaps.map"))
            .expect("read obfuscated source maps"),
    )
    .expect("parse obfuscated source maps");
    assert!(source_maps
        .get("entry|entry|1.0.0|src/main/ets/a.ts")
        .is_some());
    assert_eq!(
        fs_err::read(fixture.compile_cache.join("sourceMaps.map"))
            .expect("read primary source maps"),
        fs_err::read(fixture.compile_cache.join("sourceMaps.json"))
            .expect("read mirrored source maps")
    );
}

#[test]
fn merged_source_map_inherits_source_root_presence_from_previous_stage() {
    let fixture = NativeArkguardFixture::new();
    let mut file = fixture.source_file(
        "entry/build/generated/r/ResourceTable.ts",
        "export const ResourceTable = {};",
    );
    file.previous_source_map = Some(serde_json::json!({
        "version": 3,
        "file": "ResourceTable.js",
        "sources": ["ResourceTable.ts"],
        "names": [],
        "mappings": "AAAA"
    }));
    let source_map_key = file.source_map_key.clone();
    let obfuscator = fixture.obfuscator("-enable-toplevel-obfuscation\n");

    fixture.run(&obfuscator, vec![file]);

    let source_maps: serde_json::Value = serde_json::from_slice(
        &fs_err::read(fixture.compile_cache.join("sourceMaps.map"))
            .expect("read obfuscated source maps"),
    )
    .expect("parse obfuscated source maps");
    assert!(
        source_maps[&source_map_key].get("sourceRoot").is_none(),
        "ArkGuard SourceMapMergingUtil.mergeSourceMap preserves an absent previous sourceRoot"
    );
}

#[test]
fn native_arkguard_removes_only_configured_call_expression_statements() {
    let fixture = NativeArkguardFixture::new();
    let file = fixture.source_file(
        "entry/src/main/ets/Index.ets",
        r#"
            service.method();
            service['propertyOne']();
            service[dynamicName]();
            service.keep();
            const retained = service.method();
            if (enabled) {
                nested.remove();
            }
        "#,
    );
    let obfuscator = fixture.obfuscator(
        "-remove-nosideeffects-calls\nservice.method\nservice['property*']\nnested.remove\n",
    );

    let output = fixture.run(&obfuscator, vec![file]);
    let code =
        fs_err::read_to_string(&output.files[0].output_path).expect("read obfuscated source");

    assert_eq!(code.matches("service.method();").count(), 1, "{code}");
    assert!(!code.contains("service['propertyOne']();"), "{code}");
    assert!(!code.contains("nested.remove();"), "{code}");
    assert!(code.contains("service[dynamicName]();"), "{code}");
    assert!(code.contains("service.keep();"), "{code}");
    assert!(
        code.contains("const retained = service.method();"),
        "{code}"
    );
}

#[test]
fn native_arkguard_keeps_selected_source_multiline_when_compact_is_enabled() {
    let fixture = NativeArkguardFixture::new();
    let kept = fixture.source_file(
        "entry/src/main/ets/Kept.ets",
        "export function kept() { const value = 1; return value; }",
    );
    let compact = fixture.source_file(
        "entry/src/main/ets/Compact.ets",
        "export function compact() { const value = 2; return value; }",
    );
    let obfuscator = fixture.obfuscator("-compact\n-keep-uncompact\nentry/src/main/ets/Kept.ets\n");

    let output = fixture.run(&obfuscator, vec![kept, compact]);
    let kept_output = output
        .files
        .iter()
        .find(|file| file.source_map_key.ends_with("Kept.ets"))
        .expect("kept source output");
    let compact_output = output
        .files
        .iter()
        .find(|file| file.source_map_key.ends_with("Compact.ets"))
        .expect("compact source output");
    let kept_code = fs_err::read_to_string(&kept_output.output_path).expect("read kept source");
    let compact_code =
        fs_err::read_to_string(&compact_output.output_path).expect("read compact source");

    assert!(kept_code.lines().count() > 1, "{kept_code}");
    assert_eq!(compact_code.lines().count(), 1, "{compact_code}");
}

#[test]
fn strip_language_default_selects_the_optimized_arkguard_preset() {
    let fixture = NativeArkguardFixture::new();
    let presets = fixture.arkguard_package.join("lib/configs/preset");
    fs_err::write(
        presets.join("es_reserved_properties.json"),
        br#"{"reserved":["standardOnly"]}"#,
    )
    .expect("write standard ArkGuard preset");
    fs_err::write(
        presets.join("es_reserved_properties_optimized.json"),
        br#"{"reserved":["optimizedOnly"]}"#,
    )
    .expect("write optimized ArkGuard preset");
    let file = fixture.source_file(
        "entry/src/main/ets/Index.ets",
        "const standardOnly = 1; const optimizedOnly = standardOnly; export { optimizedOnly };",
    );
    let obfuscator = fixture.obfuscator(
        "-enable-property-obfuscation\n-enable-toplevel-obfuscation\n-enable-export-obfuscation\n-extra-options\nstrip-language-default\n-compact\n",
    );

    let output = fixture.run(&obfuscator, vec![file]);
    let code =
        fs_err::read_to_string(&output.files[0].output_path).expect("read obfuscated source");

    assert!(code.contains("optimizedOnly"), "{code}");
    assert!(!code.contains("standardOnly"), "{code}");
}

#[test]
fn enum_whitelist_follows_arkguard_har_and_initializer_rules() {
    const SOURCE: &str = "enum Status { Alpha, Beta = Alpha + 1 } const current = Status.Beta;";
    const RULES: &str = "-enable-property-obfuscation\n-enable-toplevel-obfuscation\n-enable-export-obfuscation\n-compact\n";

    let application = NativeArkguardFixture::new();
    let mut application_file = application.source_file("entry/src/main/ets/Index.ets", SOURCE);
    application_file.build_path.set_extension("ts");
    fs_err::write(&application_file.build_path, SOURCE).expect("write application enum source");
    let application_output = application.run(
        &application.obfuscator_with_module_kind(RULES, false),
        vec![application_file],
    );
    let application_cache: serde_json::Value = serde_json::from_slice(
        &fs_err::read(application_output.name_cache_path).expect("read application name cache"),
    )
    .expect("parse application name cache");
    assert!(application_cache["PropertyCache"].get("Alpha").is_none());
    assert!(application_cache["PropertyCache"].get("Beta").is_none());

    let har = NativeArkguardFixture::new();
    let mut har_file = har.source_file("library/src/main/ets/Index.ets", SOURCE);
    har_file.build_path.set_extension("ts");
    fs_err::write(&har_file.build_path, SOURCE).expect("write HAR enum source");
    let har_output = har.run(
        &har.obfuscator_with_module_kind(RULES, true),
        vec![har_file],
    );
    let har_cache: serde_json::Value = serde_json::from_slice(
        &fs_err::read(har_output.name_cache_path).expect("read HAR name cache"),
    )
    .expect("parse HAR name cache");
    assert!(har_cache["PropertyCache"].get("Alpha").is_none());
    assert!(har_cache["PropertyCache"].get("Beta").is_none());
}

#[test]
fn filename_obfuscation_rewrites_relative_imports_and_both_physical_outputs() {
    let fixture = NativeArkguardFixture::new();
    let helper = fixture.source_file(
        "entry/src/main/ets/Helper.ets",
        "export const helperValue = 1;",
    );
    let index = fixture.source_file(
        "entry/src/main/ets/Index.ets",
        "import { helperValue } from './Helper'; export const answer = helperValue;",
    );
    let obfuscator = fixture.obfuscator(
        "-enable-toplevel-obfuscation\n-enable-export-obfuscation\n-enable-filename-obfuscation\n-compact\n",
    );

    let output = fixture.run(&obfuscator, vec![index, helper]);

    assert_eq!(output.files.len(), 2);
    assert!(output.files.iter().all(|file| file.output_path.is_file()));
    let index_output = output
        .files
        .iter()
        .find(|file| {
            fs_err::read_to_string(&file.output_path)
                .is_ok_and(|source| source.contains("from\"./a\"") || source.contains("from'./a'"))
        })
        .expect("relative import must use Helper's module-wide filename mapping");
    assert_eq!(
        index_output
            .output_path
            .extension()
            .and_then(|value| value.to_str()),
        Some("js")
    );
}

#[test]
fn filename_obfuscation_rewrites_only_project_ohm_urls_and_keeps_package_paths() {
    let fixture = NativeArkguardFixture::new();
    let helper = fixture.source_file(
        "entry/src/main/ets/Helper.ets",
        "export const helperValue = 1;",
    );
    let index = fixture.source_file(
        "entry/src/main/ets/Index.ets",
        "import { helperValue } from '@bundle:com.example.arkdown/entry/ets/Helper'; export const answer = helperValue;",
    );
    let mut package = fixture.source_file(
        "oh_modules/library/src/main/ets/PackageIndex.ets",
        "export const packageValue = 1;",
    );
    let package_path = package.build_path.clone();
    package.rename_file = false;
    package.record_name = Some("library/src/main/ets/PackageIndex".to_string());
    let obfuscator = fixture.obfuscator(
        "-enable-toplevel-obfuscation\n-enable-export-obfuscation\n-enable-filename-obfuscation\n-compact\n",
    );

    let output = fixture.run(&obfuscator, vec![index, helper, package]);

    let index_source = output
        .files
        .iter()
        .filter_map(|file| fs_err::read_to_string(&file.output_path).ok())
        .find(|source| source.contains("@bundle:"))
        .expect("project OHM import output");
    assert!(
        index_source.contains("@bundle:com.example.arkdown/entry/ets/a"),
        "{index_source}"
    );
    assert!(package_path.is_file());
    assert!(output
        .files
        .iter()
        .any(|file| { file.input_path == package_path && file.output_path == package_path }));
}

#[test]
fn filename_obfuscation_rewrites_normalized_ohm_urls_and_preserves_explicit_records() {
    let fixture = NativeArkguardFixture::new();
    let mut helper = fixture.source_file(
        "entry/src/main/ets/Helper.ets",
        "export const helperValue = 1;",
    );
    helper.record_name = Some("com.example.arkdown&entry/src/main/ets/Helper&1.0.0".to_string());
    let mut index = fixture.source_file(
        "entry/src/main/ets/Index.ets",
        "import { helperValue } from '@normalized:N&entry&com.example.arkdown&entry/src/main/ets/Helper&1.0.0'; export const answer = helperValue;",
    );
    index.record_name = None;
    index.source_map_key = "entry|entry|1.0.0|src/main/ets/Index.ts".to_string();
    let input_path = index.build_path.clone();
    let obfuscator = fixture.obfuscator(
        "-enable-toplevel-obfuscation\n-enable-export-obfuscation\n-enable-filename-obfuscation\n-compact\n",
    );

    let output = fixture.run(&obfuscator, vec![index, helper]);

    let index_output = output
        .files
        .iter()
        .find(|file| file.input_path == input_path)
        .expect("Index output mapping");
    let index_source =
        fs_err::read_to_string(&index_output.output_path).expect("read normalized import output");
    assert!(
        index_source.contains("@normalized:N&entry&com.example.arkdown&entry/src/main/ets/a&1.0.0"),
        "{index_source}"
    );
    let mut module_sources = vec![HvigorModuleSourceFile {
        module_id: "entry/src/main/ets/Index.ets".to_string(),
        path: input_path,
        record_name: "&explicit/Index&".to_string(),
        record_name_is_explicit: true,
        source_file: "entry|entry|1.0.0|src/main/ets/Index.ts".to_string(),
        package_name: Some("entry".to_string()),
        is_commonjs: false,
        is_shared_module: false,
        origin_source_lang: "ets".to_string(),
        imported_record_names: Vec::new(),
        is_package_module: false,
    }];
    output
        .apply_to_module_sources(&mut module_sources)
        .expect("apply explicit-record handoff");
    assert_eq!(module_sources[0].record_name, "&explicit/Index&");
    assert_ne!(module_sources[0].path, index_output.input_path);
    assert_ne!(
        module_sources[0].source_file,
        "entry|entry|1.0.0|src/main/ets/Index.ts"
    );
}

#[test]
fn disabled_string_property_obfuscation_preserves_quoted_property_contract() {
    let fixture = NativeArkguardFixture::new();
    let file = fixture.source_file(
        "entry/src/main/ets/Index.ets",
        "const value = { 'quotedApi': 1, internalApi: 2 }; value['quotedApi']; value.internalApi;",
    );
    let obfuscator = fixture.obfuscator("-enable-property-obfuscation\n-compact\n");

    let output = fixture.run(&obfuscator, vec![file]);
    let code =
        fs_err::read_to_string(&output.files[0].output_path).expect("read obfuscated source");

    assert!(code.contains("quotedApi"));
    assert!(!code.contains("internalApi"));
}

#[test]
fn source_obfuscation_has_no_node_bridge_artifacts() {
    let fixture = NativeArkguardFixture::new();
    let file = fixture.source_file(
        "entry/src/main/ets/Index.ets",
        "function run(value) { return value; }",
    );
    let obfuscator = fixture.obfuscator("-compact\n");

    fixture.run(&obfuscator, vec![file]);

    let bridge = fixture.cache.join("arkdown-arkguard-bridge.cjs");
    assert!(!bridge.exists());
    assert!(!Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("src/arkguard_bridge.cjs")
        .exists());
}

#[test]
fn keep_dts_reserves_declared_global_and_property_names() {
    let fixture = NativeArkguardFixture::new();
    fs_err::write(
        fixture.temporary.path().join("public-api.d.ts"),
        "export interface PublicApi { stableProperty: string; }",
    )
    .expect("write keep-dts declaration");
    let file = fixture.source_file(
        "entry/src/main/ets/Index.ets",
        "const PublicApi = { stableProperty: 1, internalProperty: 2 }; PublicApi.stableProperty; PublicApi.internalProperty;",
    );
    let obfuscator = fixture.obfuscator(
        "-keep-dts\npublic-api.d.ts\n-enable-toplevel-obfuscation\n-enable-property-obfuscation\n-enable-export-obfuscation\n-compact\n",
    );

    let output = fixture.run(&obfuscator, vec![file]);
    let code =
        fs_err::read_to_string(&output.files[0].output_path).expect("read obfuscated source");

    assert!(code.contains("PublicApi"));
    assert!(code.contains("stableProperty"));
    assert!(!code.contains("internalProperty"));
}

#[test]
fn language_property_preset_does_not_reserve_a_local_identifier() {
    let fixture = NativeArkguardFixture::new();
    fs_err::write(
        fixture
            .arkguard_package
            .join("lib/configs/preset/es_reserved_properties.json"),
        br#"{"reserved":["languageProperty"]}"#,
    )
    .expect("write language property preset");
    let file = fixture.source_file(
        "entry/src/main/ets/Index.ets",
        "function holder() { const languageProperty = 1; return languageProperty; }",
    );
    let output = fixture.run(
        &fixture.obfuscator(
            "-enable-toplevel-obfuscation\n-enable-property-obfuscation\n-enable-export-obfuscation\n-compact\n",
        ),
        vec![file],
    );
    let cache: serde_json::Value = serde_json::from_slice(
        &fs_err::read(output.name_cache_path).expect("read ArkGuard name cache"),
    )
    .expect("parse ArkGuard name cache");

    assert!(cache["entry/src/main/ets/Index.ets"]["IdentifierCache"]
        .get("holder#languageProperty")
        .is_some());
    assert!(cache["PropertyCache"].get("languageProperty").is_none());
}

#[test]
fn keep_global_name_does_not_reserve_a_same_named_local_identifier() {
    let fixture = NativeArkguardFixture::new();
    let file = fixture.source_file(
        "entry/src/main/ets/Index.ets",
        "const Stable = 1; function holder() { const Stable = 2; return Stable; }",
    );
    let output = fixture.run(
        &fixture.obfuscator(
            "-keep-global-name\nStable\n-enable-toplevel-obfuscation\n-enable-export-obfuscation\n-compact\n",
        ),
        vec![file],
    );
    let cache: serde_json::Value = serde_json::from_slice(
        &fs_err::read(output.name_cache_path).expect("read ArkGuard name cache"),
    )
    .expect("parse ArkGuard name cache");
    let identifiers = cache["entry/src/main/ets/Index.ets"]["IdentifierCache"]
        .as_object()
        .expect("identifier cache");

    assert!(!identifiers.contains_key("#Stable"));
    assert!(identifiers.contains_key("holder#Stable"));
}

#[test]
fn keep_dts_is_the_configured_name_source_for_local_identifiers() {
    let fixture = NativeArkguardFixture::new();
    fs_err::write(
        fixture.temporary.path().join("public-api.d.ts"),
        "declare const StableLocal: number;",
    )
    .expect("write keep-dts declaration");
    let file = fixture.source_file(
        "entry/src/main/ets/Index.ets",
        "function holder() { const StableLocal = 1; return StableLocal; }",
    );
    let output = fixture.run(
        &fixture.obfuscator(
            "-keep-dts\npublic-api.d.ts\n-enable-toplevel-obfuscation\n-enable-export-obfuscation\n-compact\n",
        ),
        vec![file],
    );
    let cache: serde_json::Value = serde_json::from_slice(
        &fs_err::read(output.name_cache_path).expect("read ArkGuard name cache"),
    )
    .expect("parse ArkGuard name cache");

    assert!(cache["entry/src/main/ets/Index.ets"]["IdentifierCache"]
        .get("holder#StableLocal")
        .is_none());
}

#[test]
fn at_keep_separates_global_declarations_from_property_members() {
    let fixture = NativeArkguardFixture::new();
    let file = fixture.source_file(
        "entry/src/main/ets/Index.ets",
        r#"
            //@KeepSymbol
            class PublicClass { publicMember() {} }
            function holder() {
                //@KeepSymbol
                class LocalClass { localMember() {} }
                return new LocalClass();
            }
        "#,
    );
    let output = fixture.run(
        &fixture.obfuscator(
            "-use-keep-in-source\n-enable-toplevel-obfuscation\n-enable-property-obfuscation\n-enable-export-obfuscation\n-compact\n",
        ),
        vec![file],
    );
    let cache: serde_json::Value = serde_json::from_slice(
        &fs_err::read(output.name_cache_path).expect("read ArkGuard name cache"),
    )
    .expect("parse ArkGuard name cache");
    let identifiers = cache["entry/src/main/ets/Index.ets"]["IdentifierCache"]
        .as_object()
        .expect("identifier cache");

    assert!(!identifiers.contains_key("#PublicClass"));
    assert!(identifiers.contains_key("holder#LocalClass"));
    assert!(cache["PropertyCache"].get("publicMember").is_none());
    assert!(cache["PropertyCache"].get("localMember").is_none());
}

#[test]
fn sdk_property_names_do_not_become_top_level_sdk_names() {
    let fixture = NativeArkguardFixture::new();
    let file = fixture.source_file(
        "entry/src/main/ets/Index.ets",
        "const SystemApi = 1; const systemMember = 2;",
    );
    let inventory_sources = vec![
        ObfuscationSource {
            path: file.original_path.clone(),
            source: Arc::clone(&file.source),
            kind: crate::ObfuscationSourceKind::Project,
        },
        ObfuscationSource {
            path: fixture.temporary.path().join("sdk/api/global.d.ts"),
            source: Arc::from("export declare interface SystemApi { systemMember: string; }"),
            kind: crate::ObfuscationSourceKind::SdkApi,
        },
    ];
    let output = fixture.run_with_inventory(
        &fixture.obfuscator("-enable-toplevel-obfuscation\n-enable-export-obfuscation\n-compact\n"),
        vec![file],
        BTreeMap::new(),
        inventory_sources,
    );
    let cache: serde_json::Value = serde_json::from_slice(
        &fs_err::read(output.name_cache_path).expect("read ArkGuard name cache"),
    )
    .expect("parse ArkGuard name cache");
    let identifiers = cache["entry/src/main/ets/Index.ets"]["IdentifierCache"]
        .as_object()
        .expect("identifier cache");

    assert!(!identifiers.contains_key("#SystemApi"));
    assert!(identifiers.contains_key("#systemMember"));
}

#[test]
fn strip_system_api_args_excludes_sdk_parameters_from_property_whitelist() {
    let fixture = NativeArkguardFixture::new();
    let file = fixture.source_file(
        "entry/src/main/ets/Index.ets",
        "const value = { method: 1, argumentName: 2, parameterName: 3, indexName: 4 };",
    );
    let inventory_sources = vec![
        ObfuscationSource {
            path: file.original_path.clone(),
            source: Arc::clone(&file.source),
            kind: crate::ObfuscationSourceKind::Project,
        },
        ObfuscationSource {
            path: fixture.temporary.path().join("sdk/api/global.d.ts"),
            source: Arc::from(
                "export declare function api(argumentName: string): void; export declare interface Api<TArg> { [indexName: string]: TArg; method(parameterName: TArg): void; }",
            ),
            kind: crate::ObfuscationSourceKind::SdkApi,
        },
    ];
    let output = fixture.run_with_inventory(
        &fixture.obfuscator(
            "-enable-toplevel-obfuscation\n-enable-property-obfuscation\n-enable-export-obfuscation\n-extra-options\nstrip-system-api-args\n-compact\n",
        ),
        vec![file],
        BTreeMap::new(),
        inventory_sources,
    );
    let cache: serde_json::Value = serde_json::from_slice(
        &fs_err::read(output.name_cache_path).expect("read ArkGuard name cache"),
    )
    .expect("parse ArkGuard name cache");

    assert!(cache["PropertyCache"].get("method").is_none());
    assert!(cache["PropertyCache"].get("argumentName").is_some());
    assert!(cache["PropertyCache"].get("parameterName").is_some());
    assert!(cache["PropertyCache"].get("indexName").is_some());
}

#[test]
fn sdk_index_signature_parameter_is_reserved_without_strip_system_api_args() {
    let fixture = NativeArkguardFixture::new();
    let file = fixture.source_file(
        "entry/src/main/ets/Index.ets",
        "const value = { indexName: 1 };",
    );
    let inventory_sources = vec![
        ObfuscationSource {
            path: file.original_path.clone(),
            source: Arc::clone(&file.source),
            kind: crate::ObfuscationSourceKind::Project,
        },
        ObfuscationSource {
            path: fixture.temporary.path().join("sdk/api/global.d.ts"),
            source: Arc::from("export declare interface Api { [indexName: string]: string; }"),
            kind: crate::ObfuscationSourceKind::SdkApi,
        },
    ];
    let output = fixture.run_with_inventory(
        &fixture.obfuscator(
            "-enable-toplevel-obfuscation\n-enable-property-obfuscation\n-enable-export-obfuscation\n-compact\n",
        ),
        vec![file],
        BTreeMap::new(),
        inventory_sources,
    );
    let cache: serde_json::Value = serde_json::from_slice(
        &fs_err::read(output.name_cache_path).expect("read ArkGuard name cache"),
    )
    .expect("parse ArkGuard name cache");

    assert!(cache["PropertyCache"].get("indexName").is_none());
}

#[test]
fn strip_system_api_args_disables_the_sdk_local_identifier_whitelist() {
    fn identifier_cache(strip: bool) -> serde_json::Map<String, serde_json::Value> {
        let fixture = NativeArkguardFixture::new();
        let file = fixture.source_file(
            "entry/src/main/ets/Index.ets",
            "function holder() { const SystemApi = 1; const systemMember = 2; return SystemApi + systemMember; }",
        );
        let inventory_sources = vec![
            ObfuscationSource {
                path: file.original_path.clone(),
                source: Arc::clone(&file.source),
                kind: crate::ObfuscationSourceKind::Project,
            },
            ObfuscationSource {
                path: fixture.temporary.path().join("sdk/api/global.d.ts"),
                source: Arc::from("export declare interface SystemApi { systemMember: string; }"),
                kind: crate::ObfuscationSourceKind::SdkApi,
            },
        ];
        let extra = if strip {
            "-extra-options\nstrip-system-api-args\n"
        } else {
            ""
        };
        let output = fixture.run_with_inventory(
            &fixture.obfuscator(&format!(
                "-enable-toplevel-obfuscation\n-enable-property-obfuscation\n-enable-export-obfuscation\n{extra}-compact\n"
            )),
            vec![file],
            BTreeMap::new(),
            inventory_sources,
        );
        let cache: serde_json::Value = serde_json::from_slice(
            &fs_err::read(output.name_cache_path).expect("read ArkGuard name cache"),
        )
        .expect("parse ArkGuard name cache");
        cache["entry/src/main/ets/Index.ets"]["IdentifierCache"]
            .as_object()
            .expect("identifier cache")
            .clone()
    }

    let default_cache = identifier_cache(false);
    assert!(!default_cache.contains_key("holder#SystemApi"));
    assert!(!default_cache.contains_key("holder#systemMember"));

    let stripped_cache = identifier_cache(true);
    assert!(stripped_cache.contains_key("holder#SystemApi"));
    assert!(stripped_cache.contains_key("holder#systemMember"));
}

#[test]
fn keep_path_directory_and_exclusion_follow_rule_file_relative_paths() {
    let fixture = NativeArkguardFixture::new();
    let kept = fixture.source_file(
        "entry/src/main/ets/Kept.ets",
        "const KeptTopLevel = { keptProperty: 1 };",
    );
    let excluded = fixture.source_file(
        "entry/src/main/ets/Excluded.ets",
        "const ExcludedTopLevel = { changedProperty: 1 };",
    );
    let obfuscator = fixture.obfuscator(
        "-keep\nentry/src/main/ets\n!entry/src/main/ets/Excluded.ets\n-enable-toplevel-obfuscation\n-enable-property-obfuscation\n-enable-export-obfuscation\n-compact\n",
    );

    let output = fixture.run(&obfuscator, vec![kept, excluded]);
    let sources = output
        .files
        .iter()
        .map(|file| fs_err::read_to_string(&file.output_path).expect("read obfuscation output"))
        .collect::<Vec<_>>();

    assert!(sources.iter().any(|source| source.contains("KeptTopLevel")));
    assert!(
        sources
            .iter()
            .all(|source| !source.contains("ExcludedTopLevel")),
        "{sources:?}"
    );
    assert!(sources
        .iter()
        .all(|source| !source.contains("changedProperty")));
}

#[test]
fn keep_source_preserves_dependency_exports_and_still_mangles_file_paths() {
    let fixture = NativeArkguardFixture::new();
    let kept = fixture.source_file(
        "entry/src/main/ets/Kept.ets",
        "import { DependencyApi } from './Dependency'; export const kept = DependencyApi.stableProperty;",
    );
    let dependency = fixture.source_file(
        "entry/src/main/ets/Dependency.ets",
        "export const DependencyApi = { stableProperty: 1 }; const LocalBinding = 2;",
    );
    let kept_path = kept.original_path.clone();
    let dependency_path = dependency.original_path.clone();
    let dependencies = BTreeMap::from([(kept_path, vec![dependency_path])]);
    let obfuscator = fixture.obfuscator(
        "-keep\nentry/src/main/ets/Kept.ets\n-enable-toplevel-obfuscation\n-enable-property-obfuscation\n-enable-export-obfuscation\n-enable-filename-obfuscation\n-compact\n",
    );

    let output = fixture.run_with_dependencies(&obfuscator, vec![kept, dependency], dependencies);
    let sources = output
        .files
        .iter()
        .map(|file| fs_err::read_to_string(&file.output_path).expect("read obfuscation output"))
        .collect::<Vec<_>>();

    assert!(sources
        .iter()
        .any(|source| source.contains("DependencyApi")));
    assert!(sources
        .iter()
        .any(|source| source.contains("stableProperty")));
    assert!(sources
        .iter()
        .all(|source| !source.contains("LocalBinding")));
    assert!(output.files.iter().all(|file| {
        !matches!(
            file.output_path.file_stem().and_then(|name| name.to_str()),
            Some("Kept" | "Dependency")
        )
    }));
}

#[test]
fn generated_resource_table_is_an_implicit_keep_source() {
    let fixture = NativeArkguardFixture::new();
    let resource_table = fixture.source_file(
        "entry/build/generated/r/ResourceTable.ts",
        "export const ResourceTable = { stableResource: 1 };",
    );
    let obfuscator = fixture.obfuscator(
        "-enable-toplevel-obfuscation\n-enable-property-obfuscation\n-enable-export-obfuscation\n-enable-filename-obfuscation\n-compact\n",
    );

    let output = fixture.run(&obfuscator, vec![resource_table]);
    let code = fs_err::read_to_string(&output.files[0].output_path).expect("read resource table");

    assert!(code.contains("ResourceTable"), "{code}");
    assert!(code.contains("stableResource"), "{code}");
    assert_ne!(
        output.files[0]
            .output_path
            .file_stem()
            .and_then(|name| name.to_str()),
        Some("ResourceTable")
    );
}

#[test]
fn wildcard_keep_names_apply_to_identifiers_properties_and_file_names() {
    let fixture = NativeArkguardFixture::new();
    let file = fixture.source_file(
        "entry/src/main/ets/PublicIndex.ets",
        "const PublicEntry = { stableApi: 1, privateApi: 2 }; PublicEntry.stableApi; PublicEntry.privateApi;",
    );
    let obfuscator = fixture.obfuscator(
        "-keep-global-name\nPublic*\n-keep-property-name\nstable*\n-keep-file-name\nPublic*\n-enable-toplevel-obfuscation\n-enable-property-obfuscation\n-enable-export-obfuscation\n-enable-filename-obfuscation\n-compact\n",
    );

    let output = fixture.run(&obfuscator, vec![file]);
    let code =
        fs_err::read_to_string(&output.files[0].output_path).expect("read obfuscated source");

    assert!(code.contains("PublicEntry"));
    assert!(code.contains("stableApi"));
    assert!(!code.contains("privateApi"));
    assert_eq!(
        output.files[0]
            .output_path
            .file_name()
            .and_then(|name| name.to_str()),
        Some("PublicIndex.js")
    );
}

#[test]
fn incremental_name_cache_prunes_deleted_sources_and_outputs() {
    let fixture = NativeArkguardFixture::new();
    let first = fixture.source_file("entry/src/main/ets/First.ets", "export const first = 1;");
    let removed = fixture.source_file(
        "entry/src/main/ets/Removed.ets",
        "export const removed = 2;",
    );
    let rules = "-enable-toplevel-obfuscation\n-enable-export-obfuscation\n-enable-filename-obfuscation\n-compact\n";
    let first_output = fixture.run(&fixture.obfuscator(rules), vec![first, removed]);
    let removed_output = first_output
        .files
        .iter()
        .find(|file| {
            fs_err::read_to_string(&file.output_path).is_ok_and(|source| source.contains("=2"))
        })
        .map(|file| file.output_path.clone())
        .expect("find removed source output");

    let surviving = fixture.source_file("entry/src/main/ets/First.ets", "export const first = 3;");
    let second_output = fixture.run(&fixture.obfuscator(rules), vec![surviving]);
    let cache: serde_json::Value = serde_json::from_slice(
        &fs_err::read(&second_output.name_cache_path).expect("read updated name cache"),
    )
    .expect("parse updated name cache");

    assert!(!removed_output.exists());
    assert!(cache.get("entry/src/main/ets/Removed.ets").is_none());
    assert!(cache.get("entry/src/main/ets/First.ets").is_some());
}

#[test]
fn incremental_name_cache_reuses_identifier_property_and_file_names() {
    let fixture = NativeArkguardFixture::new();
    let rules = "-enable-toplevel-obfuscation\n-enable-property-obfuscation\n-enable-string-property-obfuscation\n-enable-export-obfuscation\n-enable-filename-obfuscation\n-compact\n";
    let first = fixture.source_file(
        "entry/src/main/ets/Index.ets",
        "const StableBinding = { stableProperty: 1 }; export function stableFunction() { return StableBinding.stableProperty; }",
    );
    let first_output = fixture.run(&fixture.obfuscator(rules), vec![first]);
    let first_cache: serde_json::Value = serde_json::from_slice(
        &fs_err::read(&first_output.name_cache_path).expect("read first name cache"),
    )
    .expect("parse first name cache");
    let first_identifier = first_cache["entry/src/main/ets/Index.ets"]["IdentifierCache"]
        ["#StableBinding"]
        .as_str()
        .expect("first stable identifier")
        .to_string();
    let first_property = first_cache["PropertyCache"]["stableProperty"]
        .as_str()
        .expect("first stable property")
        .to_string();
    let first_file = first_cache["FileNameCache"]["Index"]
        .as_str()
        .expect("first stable filename")
        .to_string();

    let second = fixture.source_file(
        "entry/src/main/ets/Index.ets",
        "const AddedBinding = { addedProperty: 0 }; const StableBinding = { stableProperty: 2 }; export function stableFunction() { return AddedBinding.addedProperty + StableBinding.stableProperty; }",
    );
    let second_output = fixture.run(&fixture.obfuscator(rules), vec![second]);
    let second_cache: serde_json::Value = serde_json::from_slice(
        &fs_err::read(&second_output.name_cache_path).expect("read second name cache"),
    )
    .expect("parse second name cache");
    let second_identifier = second_cache["entry/src/main/ets/Index.ets"]["IdentifierCache"]
        ["#StableBinding"]
        .as_str()
        .expect("second stable identifier");

    assert_eq!(second_identifier, first_identifier);
    assert_eq!(
        second_cache["PropertyCache"]["stableProperty"],
        first_property
    );
    assert_eq!(second_cache["FileNameCache"]["Index"], first_file);
    assert_ne!(
        second_cache["PropertyCache"]["addedProperty"],
        second_cache["PropertyCache"]["stableProperty"]
    );
}

#[test]
fn explicit_print_name_cache_does_not_create_its_parent_directory() {
    let fixture = NativeArkguardFixture::new();
    let file = fixture.source_file(
        "entry/src/main/ets/Index.ets",
        "export const stableName = 1;",
    );
    let obfuscator = fixture.obfuscator(
        "-print-namecache\nmissing/output/nameCache.json\n-enable-toplevel-obfuscation\n",
    );

    let error = obfuscator
        .obfuscate_module(ObfuscationModuleInput {
            files: vec![file],
            inventory_sources: Vec::new(),
            module_dependencies: BTreeMap::new(),
            preserved_source_maps: BTreeMap::new(),
            source_maps_path: Some(fixture.compile_cache.join("sourceMaps.map")),
            source_map_mirror_paths: Vec::new(),
        })
        .expect_err("missing explicit name-cache parent must fail");

    assert!(
        matches!(error, ObfuscationError::Io(error) if error.kind() == std::io::ErrorKind::NotFound)
    );
}

#[test]
fn apply_name_cache_accepts_arkguard_scope_keys_with_line_ranges() {
    let fixture = NativeArkguardFixture::new();
    fs_err::write(
        fixture.temporary.path().join("history.json"),
        serde_json::to_vec_pretty(&serde_json::json!({
            "entry/src/main/ets/Index.ets": {
                "IdentifierCache": {
                    "#PublicEntry:1:1": "cachedTop",
                    "run#localValue:2:2": "cachedLocal"
                },
                "MemberMethodCache": {},
                "obfName": "entry/src/main/ets/Index.ets"
            },
            "PropertyCache": {}
        }))
        .expect("serialize history cache"),
    )
    .expect("write history cache");
    let file = fixture.source_file(
        "entry/src/main/ets/Index.ets",
        "const PublicEntry = 1; function run() { const localValue = PublicEntry; return localValue; }",
    );
    let obfuscator = fixture.obfuscator(
        "-apply-namecache\nhistory.json\n-enable-toplevel-obfuscation\n-enable-export-obfuscation\n-compact\n",
    );

    let output = fixture.run(&obfuscator, vec![file]);
    let code =
        fs_err::read_to_string(&output.files[0].output_path).expect("read obfuscated source");

    assert!(code.contains("cachedTop"), "{code}");
    assert!(code.contains("cachedLocal"), "{code}");
}

#[test]
fn identifier_cache_uses_arkguard_scope_locations_instead_of_oxc_scope_ids() {
    let fixture = NativeArkguardFixture::new();
    let file = fixture.source_file(
        "entry/src/main/ets/Index.ets",
        r#"
            function OutFunction(longArgument) {
                const inFunction = longArgument;
                { const nested = inFunction; }
                const arrow = (arrowParam) => arrowParam + inFunction;
                return arrow(1);
            }
            class Student {
                constructor(name) { const ctorLocal = name; }
                method(methodParam) { return methodParam; }
            }
        "#,
    );
    let output = fixture.run(
        &fixture.obfuscator("-enable-toplevel-obfuscation\n-enable-export-obfuscation\n-compact\n"),
        vec![file],
    );
    let cache: serde_json::Value = serde_json::from_slice(
        &fs_err::read(output.name_cache_path).expect("read ArkGuard name cache"),
    )
    .expect("parse ArkGuard name cache");
    let identifiers = cache["entry/src/main/ets/Index.ets"]["IdentifierCache"]
        .as_object()
        .expect("identifier cache");

    for key in [
        "#OutFunction:2:7",
        "OutFunction#longArgument",
        "OutFunction#inFunction",
        "OutFunction#$0#nested",
        "OutFunction#arrow:5:5",
        "OutFunction#arrow#arrowParam",
        "#Student",
        "Student#$0#name",
        "Student#$0#ctorLocal",
        "Student#method#methodParam",
    ] {
        assert!(
            identifiers.contains_key(key),
            "missing {key}: {identifiers:#?}"
        );
    }
    assert!(
        identifiers.keys().all(|key| !key.starts_with("ScopeId(")),
        "OXC allocation IDs must not escape into ArkGuard caches: {identifiers:#?}"
    );
}

#[test]
fn print_kept_names_writes_arkguard_reasons_whitelist_and_incremental_cache() {
    let fixture = NativeArkguardFixture::new();
    let first = fixture.source_file(
        "entry/src/main/ets/First.ets",
        "const preservedTop = { preservedProperty: 1 }; export { preservedTop };",
    );
    let rules = "-enable-property-obfuscation\n-enable-toplevel-obfuscation\n-enable-export-obfuscation\n-keep-global-name\npreservedTop\n-keep-property-name\npreservedProperty\n-print-kept-names\nreports/kept.json\n-compact\n";
    fixture.run(&fixture.obfuscator(rules), vec![first]);

    let default_path = fixture.cache.join("keptNames.json");
    let first_report: serde_json::Value = serde_json::from_slice(
        &fs_err::read(&default_path).expect("read default kept-name report"),
    )
    .expect("parse default kept-name report");
    assert_eq!(
        first_report["keptNames"]["property"]["preservedProperty"],
        serde_json::json!(["conf"])
    );
    assert_eq!(
        first_report["keptNames"]["entry/src/main/ets/First.ets"]["#preservedTop"],
        serde_json::json!(["conf"])
    );
    assert_eq!(
        first_report["keptReasons"]["objectProp"],
        "same as the object property"
    );
    assert_eq!(
        fs_err::read(fixture.temporary.path().join("reports/kept.json"))
            .expect("read explicit kept-name report"),
        fs_err::read(&default_path).expect("read default kept-name report for comparison")
    );
    let whitelist: serde_json::Value = serde_json::from_slice(
        &fs_err::read(fixture.cache.join("whitelist.json")).expect("read whitelist report"),
    )
    .expect("parse whitelist report");
    assert!(whitelist["conf"]
        .as_array()
        .expect("configured whitelist")
        .contains(&serde_json::json!("preservedTop")));

    let second = fixture.source_file(
        "entry/src/main/ets/Second.ets",
        "const preservedTop = { preservedProperty: 2 }; export { preservedTop };",
    );
    fixture.run(&fixture.obfuscator(rules), vec![second]);
    let second_report: serde_json::Value = serde_json::from_slice(
        &fs_err::read(default_path).expect("read incremental kept-name report"),
    )
    .expect("parse incremental kept-name report");
    assert!(second_report["keptNames"]["entry/src/main/ets/First.ets"].is_object());
    assert!(second_report["keptNames"]["entry/src/main/ets/Second.ets"].is_object());

    let changed_second = fixture.source_file(
        "entry/src/main/ets/Second.ets",
        "const changedTop = { changedProperty: 3 }; export { changedTop };",
    );
    fixture.run(&fixture.obfuscator(rules), vec![changed_second]);
    let changed_report: serde_json::Value = serde_json::from_slice(
        &fs_err::read(fixture.cache.join("keptNames.json")).expect("read changed kept-name report"),
    )
    .expect("parse changed kept-name report");
    assert_eq!(
        changed_report["keptNames"]["entry/src/main/ets/Second.ets"],
        serde_json::json!({})
    );
}

#[test]
fn keep_in_source_directives_reserve_declarations_and_members() {
    let fixture = NativeArkguardFixture::new();
    let file = fixture.source_file(
        "entry/src/main/ets/Index.ets",
        r#"
            // @KeepSymbol
            class StableService {
                // @KeepAsConsumer
                stableMethod() { return 1; }
            }
            const ChangedBinding = { changedProperty: 2 };
        "#,
    );
    let obfuscator = fixture.obfuscator(
        "-use-keep-in-source\n-enable-toplevel-obfuscation\n-enable-property-obfuscation\n-enable-export-obfuscation\n-compact\n",
    );

    let output = fixture.run(&obfuscator, vec![file]);
    let code =
        fs_err::read_to_string(&output.files[0].output_path).expect("read obfuscated source");

    assert!(code.contains("StableService"));
    assert!(code.contains("stableMethod"));
    assert!(!code.contains("ChangedBinding"));
    assert!(!code.contains("changedProperty"));
    assert!(!code.contains("KeepSymbol"));
    assert!(!code.contains("KeepAsConsumer"));
}
