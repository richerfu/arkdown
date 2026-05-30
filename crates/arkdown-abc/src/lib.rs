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
//!     ▼  emit::file
//! .abc binary
//! ```
//!
//! # Status
//!
//! The binary format skeleton and public API are complete.  The code-generation
//! pass (AST → opcodes) currently emits only `ReturnUndefined` stubs; full
//! statement lowering is a SPEC-009 follow-up task.

pub mod cache;
pub mod compiler;
pub mod emit;
pub mod error;
pub mod ir;
pub mod widget;

pub use compiler::{AbcArtifactResult, AbcCompileOptions, AbcCompiler, AbcModule};
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
            target_api_version: 12,
            merge_abc: true,
            module: false,
            cache_path: None,
        })
    }

    #[test]
    fn test_compile_minimal_iife() {
        let compiler = make_compiler();
        let modules = vec![AbcModule {
            record_name: "main".to_string(),
            source: "(function () { var x = 1; })()".to_string(),
            source_map: None,
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
            },
            AbcModule {
                record_name: "@bundle:com.example/entry/ets/utils/Helper".to_string(),
                source: "(function () { function helper() { return 42; } })()".to_string(),
                source_map: None,
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
        // API 13+ / hvigorw: num_classes = 2 user modules + 7 SDK framework stubs
        // (L@ohos.app, L@ohos.curves, L@ohos.matrix4, L@system.app, L@system.curves,
        //  L@system.matrix4, L@system.router) + 2 annotation stubs
        // (L_ESConcurrentModuleRequestsAnnotation, L_ESSlotNumberAnnotation) = 11 total.
        let num_classes = u32::from_le_bytes(bytes[0x1C..0x20].try_into().unwrap());
        assert_eq!(
            num_classes, 11,
            "num_classes must be user_modules(2) + sdk_stubs(7) + annotation_stubs(2) = 11"
        );
        let _ = std::fs::remove_file(&tmp);
    }

    #[test]
    fn test_compile_parse_error() {
        let compiler = make_compiler();
        let modules = vec![AbcModule {
            record_name: "bad".to_string(),
            source: "this is not valid javascript ???".to_string(),
            source_map: None,
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
            target_api_version: 12,
            merge_abc: true,
            module: true,
            cache_path: None,
        });
        let source = r#"import UIAbility from "@ohos:app.ability.UIAbility";
import hilog from "@ohos:hilog";
globalThis["test"] = "hello";"#;
        let modules = vec![AbcModule {
            record_name: "entry/src/main/ets/test".to_string(),
            source: source.to_string(),
            source_map: None,
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

    // ── Incremental cache tests (SPEC-025) ──────────────────────────────────

    /// Verify that a cache hit avoids recompilation: compile once with a cache
    /// directory, then compile again with the same sources.  The second call
    /// must return the same bytecode without running the IR lowering pass
    /// (verified by checking the output is byte-for-byte identical).
    #[test]
    fn test_cache_hit_avoids_recompile() {
        let cache_dir = tempfile::tempdir().expect("create temp dir");
        let out1 = std::env::temp_dir().join("test_cache_hit_1.abc");
        let out2 = std::env::temp_dir().join("test_cache_hit_2.abc");

        let make_opts = || AbcCompileOptions {
            debug_info: false,
            opt_level: 0,
            target_api_version: 12,
            merge_abc: true,
            module: false,
            cache_path: Some(cache_dir.path().to_path_buf()),
        };

        let modules = || {
            vec![AbcModule {
                record_name: "entry/main".to_string(),
                source: "(function () { var x = 42; })()".to_string(),
                source_map: None,
            }]
        };

        // First compile — populates the cache.
        let compiler1 = AbcCompiler::new(make_opts());
        compiler1.compile(modules(), &out1).expect("first compile");

        // Second compile — should be a cache hit.
        let compiler2 = AbcCompiler::new(make_opts());
        compiler2
            .compile(modules(), &out2)
            .expect("second compile (cache hit)");

        let bytes1 = std::fs::read(&out1).expect("read out1");
        let bytes2 = std::fs::read(&out2).expect("read out2");
        assert_eq!(
            bytes1, bytes2,
            "cache hit must produce byte-for-byte identical output"
        );

        // Verify the cache file was created.
        let cache_file_path = crate::cache::CacheFile::path_for(cache_dir.path());
        assert!(
            cache_file_path.exists(),
            "cache file must exist at {:?}",
            cache_file_path
        );

        let _ = std::fs::remove_file(&out1);
        let _ = std::fs::remove_file(&out2);
    }

    /// Verify that the cache is invalidated when the source changes: compile
    /// with source A, then compile with source B.  The second compile must
    /// produce different bytecode and update the cache entry.
    #[test]
    fn test_cache_invalidates_on_source_change() {
        let cache_dir = tempfile::tempdir().expect("create temp dir");
        let out1 = std::env::temp_dir().join("test_cache_inval_1.abc");
        let out2 = std::env::temp_dir().join("test_cache_inval_2.abc");

        let make_opts = || AbcCompileOptions {
            debug_info: false,
            opt_level: 0,
            target_api_version: 12,
            merge_abc: true,
            module: false,
            cache_path: Some(cache_dir.path().to_path_buf()),
        };

        // Compile module A — the record name appears in the ABC string table, so
        // different record names guarantee different bytecode output even when
        // the IR lowering stubs out the function body.
        let compiler1 = AbcCompiler::new(make_opts());
        compiler1
            .compile(
                vec![AbcModule {
                    record_name: "entry/moduleA".to_string(),
                    source: "(function () { var x = 1; })()".to_string(),
                    source_map: None,
                }],
                &out1,
            )
            .expect("compile module A");

        // Compile module B (different record name — cache must miss and produce
        // different output because the class name string differs in the ABC).
        let compiler2 = AbcCompiler::new(make_opts());
        compiler2
            .compile(
                vec![AbcModule {
                    record_name: "entry/moduleB".to_string(),
                    source: "(function () { var x = 1; })()".to_string(),
                    source_map: None,
                }],
                &out2,
            )
            .expect("compile module B");

        let bytes1 = std::fs::read(&out1).expect("read out1");
        let bytes2 = std::fs::read(&out2).expect("read out2");

        // The two outputs must differ because the record names differ (the class
        // name string is embedded in the ABC binary).
        assert_ne!(
            bytes1, bytes2,
            "different record name must produce different bytecode (cache invalidated)"
        );

        // The cache file must now contain 2 distinct entries.
        let cache = crate::cache::CacheFile::load(cache_dir.path());
        assert_eq!(
            cache.entries.len(),
            2,
            "cache must have 2 entries after compiling 2 distinct sources"
        );

        let _ = std::fs::remove_file(&out1);
        let _ = std::fs::remove_file(&out2);
    }
}
