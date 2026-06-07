//! Pure-Rust Panda bytecode compiler for the OpenHarmony ArkRuntime.
//!
//! This crate replaces the `es2abc` SDK binary (SPEC-009). It accepts normal
//! JavaScript/TypeScript input files directly. When invoked through
//! `ts2abc --module` / `es2abc --module`, the source is parsed and lowered as
//! an ECMAScript module. IIFE-wrapped CommonJS remains supported as a legacy
//! non-module input shape.
//!
//! # Pipeline
//!
//! ```text
//! JS/TS source file
//!     │
//!     ▼  oxc_parser
//! oxc Program AST
//!     │
//!     ▼  ir::builder
//! ClassRecord + MethodRecord (IR)
//!     │
//!     ▼  optimizer (opt_level > 0)
//! MethodRecord bytecode + compacted register frame
//!     │
//!     ▼  emit::file
//! .abc binary
//! ```
//!
//! # Status
//!
//! The binary format skeleton and public API are complete. The code-generation
//! pass lowers supported JS/TS syntax directly to Panda bytecode. The release
//! optimizer path is being ported from upstream `bytecode_optimizer`: non-zero
//! opt levels run the Rust optimizer entry and currently include encoded
//! bytecode register-frame compaction.

mod abc2program;
pub mod abc_reader;
pub mod cache;
pub mod compiler;
mod dependency;
pub mod emit;
pub mod error;
pub mod ir;
pub(crate) mod isa;
pub(crate) mod names;
mod npm_entries;
mod optimizer;
mod program_cache;
pub mod proto_panda;
pub mod widget;

pub use abc_reader::{AbcClassInfo, AbcReadError, AbcReader, AbcSectionHashes, ParsedAbcSummary};
pub use compiler::{
    AbcArtifactResult, AbcCompileOptions, AbcCompiler, AbcInputFile, AbcModule,
    AbcProgramCacheInput,
};
pub use dependency::AbcDependencyContext;
pub use error::AbcError;
pub use widget::{
    collect_form_json_paths, need_widget_compile, ExtensionAbilityRef, WidgetCompileConfig,
    WidgetInput,
};

#[cfg(test)]
mod tests {
    use super::*;

    // ── Binary format sanity checks ─────────────────────────────────────────

    #[test]
    fn test_abc_header_magic() {
        let abc = crate::emit::file::AbcFile::new();
        let bytes = abc.serialize();
        // Magic is 8 bytes: ASCII "PANDA" + 3 null bytes.
        assert_eq!(
            &bytes[0..8],
            b"PANDA\x00\x00\x00",
            "ABC magic bytes must be PANDA\\x00\\x00\\x00 (8 bytes)"
        );
    }

    #[test]
    fn test_abc_header_version() {
        let abc = crate::emit::file::AbcFile::new();
        let bytes = abc.serialize();
        // With 8-byte magic: checksum is at 0x08, version is at 0x0C.
        let version = u32::from_le_bytes(bytes[0x0C..0x10].try_into().unwrap());
        assert_eq!(
            version,
            crate::emit::file::VERSION,
            "ABC version field must match VERSION constant"
        );
    }

    #[test]
    fn test_target_api_version_mapping() {
        assert_eq!(
            crate::emit::file::version_for_target_api(12, None),
            0x0002_000C
        );
        assert_eq!(
            crate::emit::file::version_for_target_api(17, None),
            0x0006_000C
        );
        assert_eq!(
            crate::emit::file::version_for_target_api(20, None),
            0x0001_000D
        );
    }

    #[test]
    fn test_abc_file_size_patched() {
        let abc = crate::emit::file::AbcFile::new();
        let bytes = abc.serialize();
        // With 8-byte magic: file_size is at 0x10.
        let file_size = u32::from_le_bytes(bytes[0x10..0x14].try_into().unwrap());
        assert_eq!(
            file_size as usize,
            bytes.len(),
            "file_size header field must equal the actual serialized length"
        );
    }

    // ── String table ────────────────────────────────────────────────────────

    #[test]
    fn test_string_table_intern_idempotent() {
        let mut st = crate::emit::strings::StringTable::default();
        let idx1 = st.intern("hello");
        let idx2 = st.intern("hello");
        assert_eq!(
            idx1, idx2,
            "interning the same string twice must return the same index"
        );
    }

    #[test]
    fn test_string_table_intern_distinct() {
        let mut st = crate::emit::strings::StringTable::default();
        let idx1 = st.intern("hello");
        let idx2 = st.intern("world");
        assert_ne!(idx1, idx2, "distinct strings must get distinct indices");
    }

    #[test]
    fn test_string_table_get() {
        let mut st = crate::emit::strings::StringTable::default();
        let idx = st.intern("arkdown");
        assert_eq!(st.get(idx), Some("arkdown"));
    }

    // ── Compiler integration ────────────────────────────────────────────────

    fn make_compiler() -> AbcCompiler {
        AbcCompiler::new(AbcCompileOptions {
            debug_info: false,
            opt_level: 0,
            optimize_try_catch_func: true,
            target_api_version: 12,
            target_api_sub_version: None,
            enable_annotations: false,
            merge_abc: true,
            module: false,
            module_record_field_name: None,
            npm_entries: None,
            dependency_context: None,
            remove_redundant_file: false,
            dump_deps_info: false,
        })
    }

    #[test]
    fn test_compile_minimal_iife() {
        let compiler = make_compiler();
        let modules = vec![AbcModule {
            record_name: "main".to_string(),
            source: "(function () { var x = 1; })()".to_string(),
            source_map: None,
            source_file: None,
            package_name: None,
            is_commonjs: false,
            is_shared_module: false,
            origin_source_lang: String::new(),
        }];
        let tmp = std::env::temp_dir().join("test_abc_minimal.abc");
        let result = compiler.compile(modules, &tmp);
        assert!(result.is_ok(), "compile must succeed: {:?}", result.err());
        let bytes = std::fs::read(&tmp).unwrap();
        assert_eq!(
            &bytes[0..8],
            b"PANDA\x00\x00\x00",
            "output must begin with 8-byte PANDA magic"
        );
        let _ = std::fs::remove_file(&tmp);
    }

    #[test]
    fn test_compile_multiple_modules() {
        let compiler = make_compiler();
        let modules = vec![
            AbcModule {
                record_name: "@bundle:com.example/entry/ets/pages/Index".to_string(),
                source: "(function () { var x = 1; })()".to_string(),
                source_map: None,
                source_file: None,
                package_name: None,
                is_commonjs: false,
                is_shared_module: false,
                origin_source_lang: String::new(),
            },
            AbcModule {
                record_name: "@bundle:com.example/entry/ets/utils/Helper".to_string(),
                source: "(function () { function helper() { return 42; } })()".to_string(),
                source_map: None,
                source_file: None,
                package_name: None,
                is_commonjs: false,
                is_shared_module: false,
                origin_source_lang: String::new(),
            },
        ];
        let tmp = std::env::temp_dir().join("test_abc_multi.abc");
        let result = compiler.compile(modules, &tmp);
        assert!(
            result.is_ok(),
            "multi-module compile must succeed: {:?}",
            result.err()
        );
        let bytes = std::fs::read(&tmp).unwrap();
        assert_eq!(&bytes[0..8], b"PANDA\x00\x00\x00");
        // Upstream es2abc only emits records supplied by source files plus
        // annotation stubs. SDK/native package records are supplied separately
        // through --npm-module-entry-list, not synthesized by the file emitter.
        let num_classes = u32::from_le_bytes(bytes[0x1C..0x20].try_into().unwrap());
        assert_eq!(
            num_classes, 4,
            "num_classes must be user_modules(2) + annotation_stubs(2) without npm entries"
        );
        let _ = std::fs::remove_file(&tmp);
    }

    #[test]
    fn test_non_merge_single_module_compiles() {
        let compiler = AbcCompiler::new(AbcCompileOptions {
            debug_info: false,
            opt_level: 0,
            optimize_try_catch_func: true,
            target_api_version: 12,
            target_api_sub_version: None,
            enable_annotations: false,
            merge_abc: false,
            module: true,
            module_record_field_name: None,
            npm_entries: None,
            dependency_context: None,
            remove_redundant_file: false,
            dump_deps_info: false,
        });
        let tmp = tempfile::NamedTempFile::new().unwrap();

        let result = compiler
            .compile(
                vec![AbcModule {
                    record_name: "nonmerge/main".to_string(),
                    source: "export const value = 1;".to_string(),
                    source_map: None,
                    source_file: Some("main.js".to_string()),
                    package_name: None,
                    is_commonjs: false,
                    is_shared_module: false,
                    origin_source_lang: String::new(),
                }],
                tmp.path(),
            )
            .expect("single-source non-merge compile must succeed");

        assert_eq!(result.path, tmp.path());
        let bytes = std::fs::read(tmp.path()).unwrap();
        assert_eq!(&bytes[0..8], b"PANDA\x00\x00\x00");
        let class_names = AbcReader::new(&bytes).unwrap().class_names().unwrap();
        assert!(
            class_names.iter().any(|name| name == "_ESModuleRecord"),
            "non-merge module output must emit upstream _ESModuleRecord, got {class_names:?}"
        );
        assert!(
            class_names.iter().any(|name| name == "_ESScopeNamesRecord"),
            "non-merge module output must emit upstream _ESScopeNamesRecord, got {class_names:?}"
        );
        assert!(
            !bytes
                .windows("moduleRecordIdx".len())
                .any(|window| window == b"moduleRecordIdx"),
            "non-merge module field name must be the source file, not moduleRecordIdx"
        );
    }

    #[test]
    fn test_non_merge_multiple_modules_rejected_for_single_output_api() {
        let compiler = AbcCompiler::new(AbcCompileOptions {
            debug_info: false,
            opt_level: 0,
            optimize_try_catch_func: true,
            target_api_version: 12,
            target_api_sub_version: None,
            enable_annotations: false,
            merge_abc: false,
            module: true,
            module_record_field_name: None,
            npm_entries: None,
            dependency_context: None,
            remove_redundant_file: false,
            dump_deps_info: false,
        });
        let tmp = tempfile::NamedTempFile::new().unwrap();

        let err = match compiler.compile(
            vec![
                AbcModule {
                    record_name: "nonmerge/a".to_string(),
                    source: "export const a = 1;".to_string(),
                    source_map: None,
                    source_file: Some("a.js".to_string()),
                    package_name: None,
                    is_commonjs: false,
                    is_shared_module: false,
                    origin_source_lang: String::new(),
                },
                AbcModule {
                    record_name: "nonmerge/b".to_string(),
                    source: "export const b = 2;".to_string(),
                    source_map: None,
                    source_file: Some("b.js".to_string()),
                    package_name: None,
                    is_commonjs: false,
                    is_shared_module: false,
                    origin_source_lang: String::new(),
                },
            ],
            tmp.path(),
        ) {
            Ok(_) => panic!("non-merge multi-source compile needs per-input outputs"),
            Err(err) => err,
        };

        assert!(
            matches!(err, AbcError::UnsupportedConstruct { ref construct, .. } if construct.contains("merge_abc=false")),
            "expected merge_abc=false unsupported construct, got {err:?}"
        );
    }

    #[test]
    fn test_non_merge_top_level_await_emits_upstream_metadata_record() {
        let tmp = tempfile::NamedTempFile::new().unwrap();
        AbcCompiler::new(AbcCompileOptions {
            debug_info: false,
            opt_level: 0,
            optimize_try_catch_func: true,
            target_api_version: 12,
            target_api_sub_version: None,
            enable_annotations: false,
            merge_abc: false,
            module: true,
            module_record_field_name: None,
            npm_entries: None,
            dependency_context: None,
            remove_redundant_file: false,
            dump_deps_info: false,
        })
        .compile(
            vec![AbcModule {
                record_name: String::new(),
                source: "await Promise.resolve();".to_string(),
                source_map: None,
                source_file: Some("tla.js".to_string()),
                package_name: None,
                is_commonjs: false,
                is_shared_module: false,
                origin_source_lang: String::new(),
            }],
            tmp.path(),
        )
        .expect("non-merge top-level await module must compile");

        let bytes = std::fs::read(tmp.path()).unwrap();
        let class_names = AbcReader::new(&bytes).unwrap().class_names().unwrap();
        assert!(
            class_names.iter().any(|name| name == "_HasTopLevelAwait"),
            "non-merge top-level await must emit _HasTopLevelAwait, got {class_names:?}"
        );
    }

    #[test]
    fn test_non_merge_shared_module_emits_upstream_metadata_record() {
        let tmp = tempfile::NamedTempFile::new().unwrap();
        AbcCompiler::new(AbcCompileOptions {
            debug_info: false,
            opt_level: 0,
            optimize_try_catch_func: true,
            target_api_version: 12,
            target_api_sub_version: None,
            enable_annotations: false,
            merge_abc: false,
            module: true,
            module_record_field_name: None,
            npm_entries: None,
            dependency_context: None,
            remove_redundant_file: false,
            dump_deps_info: false,
        })
        .compile(
            vec![AbcModule {
                record_name: String::new(),
                source: "export const value = 1;".to_string(),
                source_map: None,
                source_file: Some("shared.js".to_string()),
                package_name: None,
                is_commonjs: false,
                is_shared_module: true,
                origin_source_lang: String::new(),
            }],
            tmp.path(),
        )
        .expect("non-merge shared module must compile");

        let bytes = std::fs::read(tmp.path()).unwrap();
        let class_names = AbcReader::new(&bytes).unwrap().class_names().unwrap();
        assert!(
            class_names.iter().any(|name| name == "_SharedModuleRecord"),
            "non-merge shared module must emit _SharedModuleRecord, got {class_names:?}"
        );
    }

    #[test]
    fn test_non_merge_commonjs_emits_upstream_metadata_record() {
        let tmp = tempfile::NamedTempFile::new().unwrap();
        AbcCompiler::new(AbcCompileOptions {
            debug_info: false,
            opt_level: 0,
            optimize_try_catch_func: true,
            target_api_version: 12,
            target_api_sub_version: None,
            enable_annotations: false,
            merge_abc: false,
            module: false,
            module_record_field_name: None,
            npm_entries: None,
            dependency_context: None,
            remove_redundant_file: false,
            dump_deps_info: false,
        })
        .compile(
            vec![AbcModule {
                record_name: String::new(),
                source: "exports.value = 1;".to_string(),
                source_map: None,
                source_file: Some("commonjs.js".to_string()),
                package_name: None,
                is_commonjs: true,
                is_shared_module: false,
                origin_source_lang: String::new(),
            }],
            tmp.path(),
        )
        .expect("non-merge CommonJS source must compile");

        let bytes = std::fs::read(tmp.path()).unwrap();
        let class_names = AbcReader::new(&bytes).unwrap().class_names().unwrap();
        assert!(
            class_names.iter().any(|name| name == "_CommonJsRecord"),
            "non-merge CommonJS must emit _CommonJsRecord, got {class_names:?}"
        );
        assert!(
            !class_names.iter().any(|name| name == "_ESModuleRecord"),
            "non-merge CommonJS must not emit _ESModuleRecord, got {class_names:?}"
        );
    }

    #[test]
    fn test_compile_parse_error() {
        let compiler = make_compiler();
        let modules = vec![AbcModule {
            record_name: "bad".to_string(),
            source: "this is not valid javascript ???".to_string(),
            source_map: None,
            source_file: None,
            package_name: None,
            is_commonjs: false,
            is_shared_module: false,
            origin_source_lang: String::new(),
        }];
        let tmp = std::env::temp_dir().join("test_abc_parse_error.abc");
        let result = compiler.compile(modules, &tmp);
        // The parser may or may not surface a hard error for all invalid JS;
        // what matters is that the compiler does not panic.
        let _ = result; // result may be Ok or Err — both are acceptable
        let _ = std::fs::remove_file(&tmp);
    }

    #[test]
    fn test_artifact_result_fields() {
        let compiler = make_compiler();
        let modules = vec![AbcModule {
            record_name: "test_record".to_string(),
            source: "(function(){}())".to_string(),
            source_map: None,
            source_file: None,
            package_name: None,
            is_commonjs: false,
            is_shared_module: false,
            origin_source_lang: String::new(),
        }];
        let tmp = std::env::temp_dir().join("test_abc_result_fields.abc");
        let result = compiler.compile(modules, &tmp).unwrap();
        assert_eq!(result.api_version, 12);
        assert!(!result.has_debug_info);
        assert_eq!(result.path, tmp);
        let _ = std::fs::remove_file(&tmp);
    }

    /// Verify that `require("@ohos:xxx")` specifier strings appear as
    /// standalone `StringItem`s in the compiled `.abc` binary.
    ///
    /// ArkRuntime needs to find these specifiers by file offset when resolving
    /// module imports.  The minimum requirement is that the byte sequence
    /// `@ohos:app.ability.UIAbility\0` (and the hilog one) is present somewhere
    /// in the output file.
    #[test]
    fn test_module_requests_in_abc() {
        // Uses ESM imports — the bundler now always emits ESM, not IIFE+require.
        let compiler = AbcCompiler::new(AbcCompileOptions {
            debug_info: false,
            opt_level: 0,
            optimize_try_catch_func: true,
            target_api_version: 12,
            target_api_sub_version: None,
            enable_annotations: false,
            merge_abc: true,
            module: true,
            module_record_field_name: None,
            npm_entries: None,
            dependency_context: None,
            remove_redundant_file: false,
            dump_deps_info: false,
        });
        let source = r#"import UIAbility from "@ohos:app.ability.UIAbility";
import hilog from "@ohos:hilog";
globalThis["test"] = "hello";"#;
        let modules = vec![AbcModule {
            record_name: "entry/src/main/ets/test".to_string(),
            source: source.to_string(),
            source_map: None,
            source_file: None,
            package_name: None,
            is_commonjs: false,
            is_shared_module: false,
            origin_source_lang: String::new(),
        }];
        let tmp = std::env::temp_dir().join("test_abc_module_requests.abc");
        compiler.compile(modules, &tmp).unwrap();
        let bytes = std::fs::read(&tmp).unwrap();

        // hvigor keeps the original module request strings in the method index
        // and emits compact native records (for example L@ohos.app;) separately.
        let ohos_ui = b"@ohos:app.ability.UIAbility";
        let ohos_hilog = b"@ohos:hilog";

        let has_ui_ability = bytes.windows(ohos_ui.len()).any(|w| w == ohos_ui);
        let has_hilog = bytes.windows(ohos_hilog.len()).any(|w| w == ohos_hilog);

        assert!(
            has_ui_ability,
            "@ohos:app.ability.UIAbility must appear as a module request in the ABC file"
        );
        assert!(
            has_hilog,
            "@ohos:hilog must appear as a module request in the ABC file"
        );

        let _ = std::fs::remove_file(&tmp);
    }

    /// Verify that LNP density is per-instruction, not just per-statement.
    ///
    /// es2abc emits one LNP row per bytecode instruction.  With a 4-statement
    /// module body that compiles to many instructions, the `line_ops` byte count
    /// must exceed what per-statement tracking alone would produce.
    #[test]
    fn test_lnp_per_instruction_density() {
        use crate::emit::file::LiteralArray;
        use crate::emit::strings::StringTable;
        use crate::ir::builder::build_record_with_debug;

        // Four statements — each compiles to multiple bytecode instructions.
        let source = "const x = 1;\nconst y = 2;\nconst z = x + y;\nglobalThis[\"r\"] = z;\n";

        let mut strings = StringTable::default();
        let mut literal_arrays: Vec<LiteralArray> = Vec::new();
        let record = build_record_with_debug(
            source,
            "test/density",
            &mut strings,
            &mut literal_arrays,
            Some("test.ts"),
            true,
        )
        .expect("build_record_with_debug must succeed");

        let main = record
            .methods
            .iter()
            .find(|m| m.name == "func_main_0")
            .expect("func_main_0 must exist");

        // Per-statement tracking would yield at most 4 advance opcodes (one per
        // statement) + END_SEQUENCE = 5 bytes.  Per-instruction tracking emits
        // one SPECIAL_OPCODE byte per instruction, so line_ops.len() must be
        // significantly larger.  The source compiles to at least 8 instructions.
        assert!(
            main.line_ops.len() > 5,
            "per-instruction LNP must exceed 5 bytes; got {} bytes (bytecode {} bytes)",
            main.line_ops.len(),
            main.bytecode.len(),
        );
    }

    /// Verify that an ESM class compiles to the expected method records.
    ///
    /// The bundler now always emits clean ESM (no IIFE wrapper, no CJS interop
    /// helpers). The `var __xxx = ...` helper variables are no longer present in
    /// rolldown ESM output, so this test uses the modern ESM pattern.
    #[test]
    fn test_esm_class_compiles_method_records() {
        use crate::emit::file::LiteralArray;
        use crate::emit::strings::StringTable;
        use crate::ir::builder::build_record_with_debug;

        // ESM class (the shape rolldown now emits for EntryAbility.ets)
        let source = r#"import UIAbility from "@ohos:app.ability.UIAbility";
var EntryAbility = class extends UIAbility {
    onCreate(want, launchParam) {}
    onWindowStageCreate(windowStage) {}
};
export { EntryAbility as default };"#;

        let mut strings = StringTable::default();
        let mut literal_arrays: Vec<LiteralArray> = Vec::new();
        let record = build_record_with_debug(
            source,
            "com.example.app/entry/ets/entryability/EntryAbility",
            &mut strings,
            &mut literal_arrays,
            None,
            true, // module mode
        )
        .expect("build_record_with_debug failed");

        let method_names: Vec<&str> = record.methods.iter().map(|m| m.name.as_str()).collect();
        // func_main_0 + constructor + 2 class methods = 4 minimum
        assert!(
            method_names.contains(&"func_main_0"),
            "must have func_main_0; got: {:?}",
            method_names
        );
        assert!(
            method_names.iter().any(|n| n.contains("onCreate")),
            "must have onCreate method; got: {:?}",
            method_names
        );
        assert!(
            method_names
                .iter()
                .any(|n| n.contains("onWindowStageCreate")),
            "must have onWindowStageCreate; got: {:?}",
            method_names
        );
        // No CJS interop helpers in ESM output
        assert!(
            !method_names.iter().any(|n| n.starts_with("__")),
            "ESM output must not have __ CJS helpers; got: {:?}",
            method_names
        );
    }

    #[test]
    fn test_ast_function_scope_records_are_collected() {
        use crate::emit::file::LiteralArray;
        use crate::emit::strings::StringTable;
        use crate::ir::builder::build_record_with_debug;

        let source = r#"
function outer() {
  function nested() { return 1; }
  const local = function() { return nested(); };
  const arrow = () => local();
  return arrow();
}
const object = { method() { return outer(); } };
"#;

        let mut strings = StringTable::default();
        let mut literal_arrays: Vec<LiteralArray> = Vec::new();
        let record = build_record_with_debug(
            source,
            "pkg/test/functions",
            &mut strings,
            &mut literal_arrays,
            Some("functions.js"),
            true,
        )
        .expect("build_record_with_debug failed");

        let method_names: Vec<&str> = record.methods.iter().map(|m| m.name.as_str()).collect();
        for expected in ["func_main_0", "outer", "nested", "local", "method"] {
            assert!(
                method_names.contains(&expected),
                "missing {expected}; got {method_names:?}"
            );
        }
    }

    #[test]
    fn test_top_level_block_call_arg_arrows_are_collected() {
        use crate::emit::file::LiteralArray;
        use crate::emit::strings::StringTable;
        use crate::ir::builder::build_record_with_debug;

        let source = r#"
if (!("finalizeConstruction" in ViewPU.prototype)) {
    Reflect.set(ViewPU.prototype, "finalizeConstruction", () => {});
}
class Index extends ViewPU {}
registerNamedRoute(() => new Index(undefined, {}), "", {});
"#;

        let mut strings = StringTable::default();
        let mut literal_arrays: Vec<LiteralArray> = Vec::new();
        let record = build_record_with_debug(
            source,
            "com.example.app/entry/ets/pages/Index",
            &mut strings,
            &mut literal_arrays,
            Some("entry|entry|1.0.0|src/main/ets/pages/Index.ts"),
            true,
        )
        .expect("build_record_with_debug failed");

        let method_names: Vec<&str> = record.methods.iter().map(|m| m.name.as_str()).collect();
        let anonymous_arrows = method_names
            .iter()
            .filter(|name| name.starts_with('#') && name.ends_with('#') && !name.contains('*'))
            .count();
        assert!(
            anonymous_arrows >= 2,
            "top-level call-argument arrows must be collected through the upstream function queue; got: {:?}",
            method_names
        );
        assert!(
            !method_names.iter().any(|name| name.starts_with("#*#")),
            "top-level arrows must not use the removed custom #*# naming path; got: {:?}",
            method_names
        );
    }

    #[test]
    fn test_class_declaration_emits_missing_property_initializers() {
        use crate::emit::file::LiteralArray;
        use crate::emit::strings::StringTable;
        use crate::ir::builder::build_record_with_debug;

        let source = r#"
class Index extends ViewPU {
    initialRender() {}
    rerender() {}
    static getEntryName() { return "Index"; }
}
registerNamedRoute(() => new Index(undefined, {}), "", {});
"#;

        let mut strings = StringTable::default();
        let mut literal_arrays: Vec<LiteralArray> = Vec::new();
        let record = build_record_with_debug(
            source,
            "com.example.app/entry/ets/pages/Index",
            &mut strings,
            &mut literal_arrays,
            Some("entry|entry|1.0.0|src/main/ets/pages/Index.ts"),
            true,
        )
        .expect("build_record_with_debug failed");

        let main = record
            .methods
            .iter()
            .find(|m| m.name == "func_main_0")
            .expect("func_main_0 must exist");
        let stown_count = main.bytecode.iter().filter(|&&b| b == 0x7a).count();
        assert!(
            stown_count >= 3,
            "class declaration must install initialRender/rerender/getEntryName; bytecode={:02x?}",
            main.bytecode
        );
    }

    // ── Auto-accessor regression test ───────────────────────────────────────

    /// Regression: auto-accessor getter/setter was emitted once per class
    /// element (N duplicates) because the accessor loop was nested inside
    /// the method-definition loop.  This test compiles at the IR level and
    /// counts MethodRecords directly — not binary substrings — to verify
    /// exactly one getter and one setter is produced regardless of how many
    /// other class elements are present.
    #[test]
    fn test_auto_accessor_no_duplicate_records() {
        use crate::emit::strings::StringTable;
        use crate::ir::builder::build_record_with_debug_and_package_for_target;

        // Class has 4 elements: 1 auto-accessor + 3 regular methods.
        // Buggy code emitted N getter+setter pairs (N = number of elements);
        // correct code emits exactly 1 pair for the single auto-accessor.
        let source = r#"
class Counter {
    accessor count = 0;
    method1() { return 1; }
    method2() { return 2; }
    method3() { return 3; }
}
"#;
        let mut strings = StringTable::default();
        let mut literals = Vec::new();
        let record = build_record_with_debug_and_package_for_target(
            source,
            "test/AutoAccessor",
            &mut strings,
            &mut literals,
            None,  // source_file
            None,  // package_name
            false, // is_commonjs
            false, // is_module
            12,    // target_api_version
            None,  // sub_version
        )
        .expect("auto-accessor class must compile");

        // Count getter/setter MethodRecords by name pattern.
        let getters: Vec<_> = record
            .methods
            .iter()
            .filter(|m| m.name.contains("_get_count"))
            .collect();
        let setters: Vec<_> = record
            .methods
            .iter()
            .filter(|m| m.name.contains("_set_count"))
            .collect();

        assert_eq!(
            getters.len(),
            1,
            "expected exactly 1 auto-accessor getter record; found {}: {:?}",
            getters.len(),
            getters.iter().map(|m| &m.name).collect::<Vec<_>>(),
        );
        assert_eq!(
            setters.len(),
            1,
            "expected exactly 1 auto-accessor setter record; found {}: {:?}",
            setters.len(),
            setters.iter().map(|m| &m.name).collect::<Vec<_>>(),
        );
    }

    // ── opt_level semantics tests ───────────────────────────────────────────

    /// opt_level=0 with debug_info=true should emit debug information.
    /// Non-zero opt levels run the local Rust bytecode optimizer path and
    /// suppress debug payloads, matching upstream release output semantics.
    #[test]
    fn test_nonzero_opt_level_runs_optimizer_path() {
        let source = "export const x = 1;".to_string();
        let tmp_debug = tempfile::NamedTempFile::new().unwrap();

        // opt_level=0 + debug_info=true → has_debug_info must be true.
        let compiler_debug = AbcCompiler::new(AbcCompileOptions {
            debug_info: true,
            opt_level: 0,
            optimize_try_catch_func: true,
            target_api_version: 12,
            target_api_sub_version: None,
            enable_annotations: false,
            merge_abc: true,
            module: true,
            module_record_field_name: None,
            npm_entries: None,
            dependency_context: None,
            remove_redundant_file: false,
            dump_deps_info: false,
        });
        let result_debug = compiler_debug
            .compile(
                vec![AbcModule {
                    record_name: "test/debug".to_string(),
                    source: source.clone(),
                    source_map: None,
                    source_file: Some("test.js".to_string()),
                    package_name: None,
                    is_commonjs: false,
                    is_shared_module: false,
                    origin_source_lang: String::new(),
                }],
                tmp_debug.path(),
            )
            .expect("compile with debug_info=true, opt_level=0");
        assert!(
            result_debug.has_debug_info,
            "opt_level=0 + debug_info=true must produce debug info"
        );

        for opt_level in [1, 2] {
            let tmp_release = tempfile::NamedTempFile::new().unwrap();
            let compiler_release = AbcCompiler::new(AbcCompileOptions {
                debug_info: true,
                opt_level,
                optimize_try_catch_func: true,
                target_api_version: 12,
                target_api_sub_version: None,
                enable_annotations: false,
                merge_abc: true,
                module: true,
                module_record_field_name: None,
                npm_entries: None,
                dependency_context: None,
                remove_redundant_file: false,
                dump_deps_info: false,
            });
            let result_release = compiler_release
                .compile(
                    vec![AbcModule {
                        record_name: "test/debug".to_string(),
                        source: source.clone(),
                        source_map: None,
                        source_file: Some("test.js".to_string()),
                        package_name: None,
                        is_commonjs: false,
                        is_shared_module: false,
                        origin_source_lang: String::new(),
                    }],
                    tmp_release.path(),
                )
                .expect("compile with non-zero opt_level");
            assert!(
                !result_release.has_debug_info,
                "opt_level={opt_level} must suppress debug info"
            );
            assert!(
                tmp_release.path().metadata().unwrap().len() > 0,
                "opt_level={opt_level} must write ABC output"
            );
        }
    }
}
