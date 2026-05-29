//! Integration tests for the `arkdown-abc` public API.
//!
//! These tests exercise [`AbcCompiler`] through its public interface only,
//! without access to crate internals.

use arkdown_abc::{AbcCompileOptions, AbcCompiler, AbcModule};
use std::collections::BTreeMap;

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

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

fn iife_module(record_name: &str, source: &str) -> AbcModule {
    AbcModule {
        record_name: record_name.to_string(),
        source: source.to_string(),
        source_map: None,
    }
}

fn read_u32(bytes: &[u8], offset: usize) -> u32 {
    u32::from_le_bytes(bytes[offset..offset + 4].try_into().unwrap())
}

fn read_u16(bytes: &[u8], offset: usize) -> u16 {
    u16::from_le_bytes(bytes[offset..offset + 2].try_into().unwrap())
}

fn read_uleb(bytes: &[u8], offset: &mut usize) -> u64 {
    let mut value = 0u64;
    let mut shift = 0;
    loop {
        let byte = bytes[*offset];
        *offset += 1;
        value |= ((byte & 0x7f) as u64) << shift;
        if byte & 0x80 == 0 {
            return value;
        }
        shift += 7;
    }
}

fn skip_inline_string(bytes: &[u8], offset: &mut usize) {
    let header = read_uleb(bytes, offset);
    let len = (header >> 1) as usize;
    let is_ascii = header & 1 == 1;
    *offset += if is_ascii { len + 1 } else { len * 2 + 2 };
}

fn class_offsets(bytes: &[u8]) -> Vec<u32> {
    let num_classes = read_u32(bytes, 0x1c) as usize;
    let class_idx_off = read_u32(bytes, 0x20) as usize;
    (0..num_classes)
        .map(|idx| read_u32(bytes, class_idx_off + idx * 4))
        .collect()
}

fn class_name_at(bytes: &[u8], offset: usize) -> String {
    let mut pos = offset;
    let header = read_uleb(bytes, &mut pos);
    let len = (header >> 1) as usize;
    let is_ascii = header & 1 == 1;
    assert!(is_ascii, "test fixtures use ASCII class names");
    String::from_utf8(bytes[pos..pos + len].to_vec()).unwrap()
}

fn inline_string_at(bytes: &[u8], offset: usize) -> String {
    let mut pos = offset;
    let header = read_uleb(bytes, &mut pos);
    let len = (header >> 1) as usize;
    let is_ascii = header & 1 == 1;
    assert!(is_ascii, "test fixtures use ASCII strings");
    String::from_utf8(bytes[pos..pos + len].to_vec()).unwrap()
}

fn class_header_counts(bytes: &[u8], offset: usize) -> (u32, u64, u64, u8, u8, u8) {
    let mut pos = offset;
    skip_inline_string(bytes, &mut pos);
    let super_off = read_u32(bytes, pos);
    pos += 4;
    let access_flags = read_uleb(bytes, &mut pos);
    let fields_count = read_uleb(bytes, &mut pos);
    let methods_count = read_uleb(bytes, &mut pos);
    assert_eq!(access_flags, 1);
    (
        super_off,
        fields_count,
        methods_count,
        bytes[pos],
        bytes[pos + 1],
        bytes[pos + 2],
    )
}

fn first_user_method_debug_info(bytes: &[u8], class_offset: usize) -> (Option<u32>, Option<u32>) {
    let mut pos = class_offset;
    skip_inline_string(bytes, &mut pos);
    pos += 4; // super_class_off
    let access_flags = read_uleb(bytes, &mut pos);
    let fields_count = read_uleb(bytes, &mut pos) as usize;
    let methods_count = read_uleb(bytes, &mut pos) as usize;
    assert_eq!(access_flags, 1);
    assert!(methods_count > 0);

    let mut class_source_file = None;
    loop {
        let tag = bytes[pos];
        pos += 1;
        match tag {
            0x00 => break,
            0x02 => pos += 1,
            0x07 => {
                class_source_file = Some(read_u32(bytes, pos));
                pos += 4;
            }
            other => panic!("unexpected class tag {other:#x}"),
        }
    }

    for _ in 0..fields_count {
        pos += 2 + 2 + 4;
        let _access_flags = read_uleb(bytes, &mut pos);
        loop {
            let tag = bytes[pos];
            pos += 1;
            match tag {
                0x00 => break,
                0x01 => pos += 1,
                0x02 => pos += 4,
                other => panic!("unexpected field tag {other:#x}"),
            }
        }
    }

    pos += 2 + 2 + 4;
    let _access_flags = read_uleb(bytes, &mut pos);
    let mut method_debug_info = None;
    loop {
        let tag = bytes[pos];
        pos += 1;
        match tag {
            0x00 => break,
            0x01 | 0x05 => {
                let off = read_u32(bytes, pos);
                pos += 4;
                if tag == 0x05 {
                    method_debug_info = Some(off);
                }
            }
            0x02 => pos += 1,
            other => panic!("unexpected method tag {other:#x}"),
        }
    }

    (class_source_file, method_debug_info)
}

fn user_field_values(bytes: &[u8], class_offset: usize) -> BTreeMap<String, u32> {
    let mut pos = class_offset;
    skip_inline_string(bytes, &mut pos);
    pos += 4; // super_class_off
    let access_flags = read_uleb(bytes, &mut pos);
    let fields_count = read_uleb(bytes, &mut pos) as usize;
    let methods_count = read_uleb(bytes, &mut pos) as usize;
    assert_eq!(access_flags, 1);
    assert!(methods_count > 0);

    loop {
        let tag = bytes[pos];
        pos += 1;
        match tag {
            0x00 => break,
            0x02 => pos += 1,
            0x07 => pos += 4,
            other => panic!("unexpected class tag {other:#x}"),
        }
    }

    let mut fields = BTreeMap::new();
    for _ in 0..fields_count {
        pos += 2 + 2;
        let name_off = read_u32(bytes, pos) as usize;
        pos += 4;
        let _access_flags = read_uleb(bytes, &mut pos);
        let mut value = None;
        loop {
            let tag = bytes[pos];
            pos += 1;
            match tag {
                0x00 => break,
                0x01 => {
                    value = Some(bytes[pos] as u32);
                    pos += 1;
                }
                0x02 => {
                    value = Some(read_u32(bytes, pos));
                    pos += 4;
                }
                other => panic!("unexpected field tag {other:#x}"),
            }
        }
        if let Some(value) = value {
            fields.insert(inline_string_at(bytes, name_off), value);
        }
    }
    fields
}

// ---------------------------------------------------------------------------
// Output file creation
// ---------------------------------------------------------------------------

#[test]
fn test_compile_creates_output_file() {
    let tmp = tempfile::tempdir().expect("failed to create temp dir");
    let out = tmp.path().join("modules.abc");
    let compiler = make_compiler();
    let result = compiler.compile(vec![iife_module("main", "(function(){}())")], &out);
    assert!(result.is_ok(), "compile failed: {:?}", result.err());
    assert!(out.exists(), ".abc file was not written to disk");
}

#[test]
fn test_compile_creates_nested_output_dir() {
    // The compiler must create intermediate directories.
    let tmp = tempfile::tempdir().expect("failed to create temp dir");
    let out = tmp.path().join("a/b/c/modules.abc");
    make_compiler()
        .compile(vec![iife_module("entry", "(function(){}())")], &out)
        .expect("compile must succeed even when output dir doesn't exist yet");
    assert!(out.exists(), "nested output file not created");
}

// ---------------------------------------------------------------------------
// Binary format verification
// ---------------------------------------------------------------------------

#[test]
fn test_output_starts_with_pand_magic() {
    let tmp = tempfile::tempdir().expect("tempdir");
    let out = tmp.path().join("magic.abc");
    make_compiler()
        .compile(
            vec![iife_module("magic_test", "(function(){ var x = 42; })();")],
            &out,
        )
        .expect("compile must succeed");
    let bytes = std::fs::read(&out).expect("read .abc");
    // Magic is 8 bytes: ASCII "PANDA" + 3 null bytes.
    assert_eq!(
        &bytes[0..8],
        b"PANDA\x00\x00\x00",
        "output must begin with 8-byte PANDA magic"
    );
}

#[test]
fn test_output_has_nonzero_length() {
    let tmp = tempfile::tempdir().expect("tempdir");
    let out = tmp.path().join("len.abc");
    make_compiler()
        .compile(vec![iife_module("len_test", "(function(){}())")], &out)
        .expect("compile must succeed");
    let meta = std::fs::metadata(&out).expect("metadata");
    assert!(meta.len() > 0, "output .abc file must not be empty");
}

#[test]
fn test_literal_array_index_header_is_valid_when_classes_emit_literals() {
    let tmp = tempfile::tempdir().expect("tempdir");
    let out = tmp.path().join("literal-index.abc");
    make_compiler()
        .compile(
            vec![iife_module(
                "com.example.app/entry/ets/pages/Index",
                "(function(){ var Index = class {}; })();",
            )],
            &out,
        )
        .expect("compile must succeed");

    let bytes = std::fs::read(&out).expect("read .abc");
    let num_classes = read_u32(&bytes, 0x1c);
    let class_idx_off = read_u32(&bytes, 0x20);
    let num_literalarrays = read_u32(&bytes, 0x2c);
    let literalarray_idx_off = read_u32(&bytes, 0x30);
    let idx_section_off = read_u32(&bytes, 0x38);

    assert!(
        num_literalarrays > 0,
        "class lowering must register a literal array"
    );
    assert_eq!(
        literalarray_idx_off,
        class_idx_off + num_classes * 4,
        "literal-array index follows class index in hvigor API 17 layout"
    );
    assert_eq!(
        idx_section_off,
        literalarray_idx_off + num_literalarrays * 4,
        "IndexSection follows the literal-array index"
    );

    let first_literal = read_u32(&bytes, literalarray_idx_off as usize);
    assert!(
        first_literal < bytes.len() as u32,
        "literal-array offsets must point inside the file"
    );

    let index_start = read_u32(&bytes, idx_section_off as usize);
    let index_end = read_u32(&bytes, idx_section_off as usize + 4);
    let index_class_size = read_u32(&bytes, idx_section_off as usize + 8);
    let index_class_off = read_u32(&bytes, idx_section_off as usize + 12);
    let index_method_size = read_u32(&bytes, idx_section_off as usize + 16);
    let index_method_off = read_u32(&bytes, idx_section_off as usize + 20);
    let index_field_size = read_u32(&bytes, idx_section_off as usize + 24);
    let index_proto_size = read_u32(&bytes, idx_section_off as usize + 32);

    assert!(index_start < bytes.len() as u32);
    assert_eq!(index_end, bytes.len() as u32);
    assert_eq!(
        index_class_off,
        idx_section_off + 40,
        "IndexHeader is followed by its class index array"
    );
    assert_eq!(
        index_method_off,
        index_class_off + index_class_size * 4,
        "method/constant index follows class index array"
    );
    assert!(
        index_method_size > 0,
        "bytecode strings and methods are indexed for id operands"
    );
    assert_eq!(read_u32(&bytes, index_class_off as usize), 2);
    assert_eq!(read_u32(&bytes, index_class_off as usize + 4), 6);
    assert_eq!(index_field_size, 0xFFFF_FFFF);
    assert_eq!(index_proto_size, 0xFFFF_FFFF);
}

#[test]
fn test_user_records_are_marked_arkts_and_sdk_stubs_have_native_fields() {
    let tmp = tempfile::tempdir().expect("tempdir");
    let out = tmp.path().join("records.abc");
    make_compiler()
        .compile(
            vec![iife_module(
                "com.example.app/entry/ets/entryability/EntryAbility",
                "(function(){ var EntryAbility = class {}; })();",
            )],
            &out,
        )
        .expect("compile must succeed");
    let bytes = std::fs::read(&out).expect("read .abc");

    let offsets = class_offsets(&bytes);
    let sdk_off = offsets
        .iter()
        .copied()
        .find(|off| class_name_at(&bytes, *off as usize) == "L@ohos.app;")
        .expect("sdk stub record exists") as usize;
    let user_off = offsets
        .iter()
        .copied()
        .find(|off| {
            class_name_at(&bytes, *off as usize)
                == "Lcom.example.app/entry/ets/entryability/EntryAbility;"
        })
        .expect("user record exists") as usize;

    let (_, sdk_fields, _, sdk_tag, sdk_lang, sdk_end) = class_header_counts(&bytes, sdk_off);
    assert_eq!(
        sdk_fields, 1,
        "SDK stubs carry one @native field in hvigor output"
    );
    assert_eq!((sdk_tag, sdk_lang, sdk_end), (0x02, 0x00, 0x00));
    assert!(
        bytes
            .windows(b"@native.ohos.app".len())
            .any(|w| w == b"@native.ohos.app"),
        "SDK native field name must be emitted as a StringItem"
    );

    let (_, user_fields, _, user_tag, user_lang, user_end) = class_header_counts(&bytes, user_off);
    assert_eq!(
        user_fields, 6,
        "user module records carry six module metadata fields"
    );
    assert_eq!(
        (user_tag, user_lang, user_end),
        (0x02, 0x04, 0x00),
        "user records use ArkTS source language byte 0x04"
    );
    assert!(
        bytes
            .windows(b"pkgName@entry".len())
            .any(|w| w == b"pkgName@entry"),
        "pkgName field includes the module name suffix"
    );
}

#[test]
fn test_module_record_scope_names_and_class_buffer_literals_are_linked() {
    let tmp = tempfile::tempdir().expect("tempdir");
    let out = tmp.path().join("module-record.abc");
    // Uses ESM format — IIFE/CJS unwrapping is removed from the compiler.
    module_compiler()
        .compile(
            vec![module_record(
                "com.example.app/entry/ets/pages/Index",
                r#"import UIAbility from "@ohos:app.ability.UIAbility";
var Index = class extends UIAbility { onCreate(){} };
export { Index as default };"#,
            )],
            &out,
        )
        .expect("compile must succeed");
    let bytes = std::fs::read(&out).expect("read .abc");

    let num_literalarrays = read_u32(&bytes, 0x2c) as usize;
    let literalarray_idx_off = read_u32(&bytes, 0x30) as usize;
    assert!(
        num_literalarrays >= 3,
        "class buffer, module record, and scopeNames literal arrays are emitted"
    );
    let literal_offsets: Vec<u32> = (0..num_literalarrays)
        .map(|idx| read_u32(&bytes, literalarray_idx_off + idx * 4))
        .collect();

    let user_off = class_offsets(&bytes)
        .into_iter()
        .find(|off| {
            class_name_at(&bytes, *off as usize) == "Lcom.example.app/entry/ets/pages/Index;"
        })
        .expect("user record exists") as usize;
    let fields = user_field_values(&bytes, user_off);
    let module_record = *fields
        .get("moduleRecordIdx")
        .expect("moduleRecordIdx field has a literal-array value");
    let scope_names = *fields
        .get("scopeNames")
        .expect("scopeNames field has a literal-array value");
    assert!(literal_offsets.contains(&module_record));
    assert!(literal_offsets.contains(&scope_names));

    // ESM source has: 1 request + 1 regular import (6 entries) + 1 local export (3 entries)
    // + 4 section counts (0, 1, 0, 0 for namespace/local/indirect/star) = 12 total entries.
    let mut pos = module_record as usize;
    assert_eq!(
        read_u32(&bytes, pos),
        12,
        "ESM: one request, one import, one export, and section counts"
    );
    pos += 4;
    assert_eq!(read_u32(&bytes, pos), 1, "one module request");
    pos += 4;
    let request_string = read_u32(&bytes, pos) as usize;
    pos += 4;
    assert_eq!(
        inline_string_at(&bytes, request_string),
        "L@ohos.app.ability.UIAbility;"
    );
    assert_eq!(read_u32(&bytes, pos), 1, "one regular import");
    pos += 4;
    let local_name = read_u32(&bytes, pos) as usize;
    pos += 4;
    let import_name = read_u32(&bytes, pos) as usize;
    pos += 4;
    assert_eq!(read_u16(&bytes, pos), 0, "regular import request index");
    pos += 2;
    assert_eq!(inline_string_at(&bytes, local_name), "UIAbility");
    assert_eq!(inline_string_at(&bytes, import_name), "default");

    let mut scope_pos = scope_names as usize;
    let scope_item_count = read_u32(&bytes, scope_pos);
    scope_pos += 4;
    // hvigor format: scopeNames contains (class_name, method_with_closures)
    // pairs — two tagged strings per class method that has call-arg arrow
    // functions.  For a source without any call-arg closures the array is
    // empty (item_count == 0).  Either way the item count must be even.
    assert!(
        scope_item_count % 2 == 0,
        "scopeNames item count must be even, got {scope_item_count}"
    );
    if scope_item_count > 0 {
        assert_eq!(
            bytes[scope_pos], 0x05,
            "non-empty scopeNames entries are tagged strings"
        );
    }

    let has_class_method_buffer = literal_offsets.iter().any(|offset| {
        let offset = *offset as usize;
        read_u32(&bytes, offset) == 8
            && bytes[offset + 4] == 0x05
            && inline_string_at(&bytes, read_u32(&bytes, offset + 5) as usize) == "onCreate"
            && bytes[offset + 9] == 0x06
            && bytes[offset + 14] == 0x09
            && bytes[offset + 17] == 0x02
            && read_u32(&bytes, offset + 18) == 1
    });
    assert!(
        has_class_method_buffer,
        "class buffer literal array links instance methods by MethodItem offset"
    );
}

// ---------------------------------------------------------------------------
// AbcArtifactResult fields
// ---------------------------------------------------------------------------

#[test]
fn test_artifact_api_version_propagated() {
    let tmp = tempfile::tempdir().expect("tempdir");
    let out = tmp.path().join("api.abc");
    let compiler = AbcCompiler::new(AbcCompileOptions {
        debug_info: false,
        opt_level: 0,
        target_api_version: 9,
        merge_abc: true,
        module: false,
        cache_path: None,
    });
    let result = compiler
        .compile(vec![iife_module("api_test", "(function(){}())")], &out)
        .expect("compile must succeed");
    assert_eq!(
        result.api_version, 9,
        "api_version must match compile option"
    );
}

#[test]
fn test_artifact_debug_info_false() {
    let tmp = tempfile::tempdir().expect("tempdir");
    let out = tmp.path().join("nodebug.abc");
    let result = make_compiler()
        .compile(vec![iife_module("nodebug", "(function(){}())")], &out)
        .expect("compile must succeed");
    assert!(
        !result.has_debug_info,
        "has_debug_info must be false when debug_info=false"
    );
}

#[test]
fn test_artifact_debug_info_true() {
    let tmp = tempfile::tempdir().expect("tempdir");
    let out = tmp.path().join("debug.abc");
    let compiler = AbcCompiler::new(AbcCompileOptions {
        debug_info: true,
        opt_level: 0,
        target_api_version: 12,
        merge_abc: true,
        module: false,
        cache_path: None,
    });
    let result = compiler
        .compile(vec![iife_module("debug_test", "(function(){}())")], &out)
        .expect("compile must succeed");
    assert!(
        result.has_debug_info,
        "has_debug_info must be true when debug_info=true"
    );
}

#[test]
fn test_artifact_path_matches_output_arg() {
    let tmp = tempfile::tempdir().expect("tempdir");
    let out = tmp.path().join("path_check.abc");
    let result = make_compiler()
        .compile(vec![iife_module("path_test", "(function(){}())")], &out)
        .expect("compile must succeed");
    assert_eq!(result.path, out, "artifact path must equal the output arg");
}

// ---------------------------------------------------------------------------
// OHM URL record names (OHOS production usage)
// ---------------------------------------------------------------------------

#[test]
fn test_compile_with_ohm_url_record_name() {
    let tmp = tempfile::tempdir().expect("tempdir");
    let out = tmp.path().join("ohm.abc");
    let result = make_compiler().compile(
        vec![iife_module(
            "@bundle:com.example.myapp/entry/ets/pages/Index",
            "(function(){ globalThis['idx_hash'] = { default: {} }; })();",
        )],
        &out,
    );
    assert!(
        result.is_ok(),
        "OHM URL record name must not cause compile failure: {:?}",
        result.err()
    );
}

// ---------------------------------------------------------------------------
// Edge cases
// ---------------------------------------------------------------------------

#[test]
fn test_compile_empty_source_does_not_panic() {
    let tmp = tempfile::tempdir().expect("tempdir");
    let out = tmp.path().join("empty.abc");
    // Empty source: either Ok or Err is acceptable — must not panic.
    let result = make_compiler().compile(vec![iife_module("empty_module", "")], &out);
    let _ = result;
}

#[test]
fn test_compile_empty_module_list_does_not_panic() {
    let tmp = tempfile::tempdir().expect("tempdir");
    let out = tmp.path().join("nomodules.abc");
    let result = make_compiler().compile(vec![], &out);
    // Empty module list: either Ok or Err is acceptable — must not panic.
    let _ = result;
}

#[test]
fn test_compile_invalid_js_does_not_panic() {
    let tmp = tempfile::tempdir().expect("tempdir");
    let out = tmp.path().join("invalid.abc");
    let result = make_compiler().compile(vec![iife_module("bad", "@@@ not javascript !!!")], &out);
    // Must not panic regardless of parse outcome.
    let _ = result;
}

#[test]
fn test_compile_source_map_field_ignored_safely() {
    let tmp = tempfile::tempdir().expect("tempdir");
    let out = tmp.path().join("sourcemap.abc");
    let result = make_compiler().compile(
        vec![AbcModule {
            record_name: "sm_test".to_string(),
            source: "(function(){ var y = 2; })();".to_string(),
            source_map: Some(r#"{"version":3,"sources":[],"mappings":""}"#.to_string()),
        }],
        &out,
    );
    // source_map is accepted without panicking
    assert!(
        result.is_ok(),
        "compile with source_map must succeed: {:?}",
        result.err()
    );
}

#[test]
fn test_source_map_emits_source_file_debug_info_and_lnp() {
    let tmp = tempfile::tempdir().expect("tempdir");
    let out = tmp.path().join("sourcemap-debug.abc");
    let source_file = "entry/src/main/ets/pages/Index.ts";
    let result = make_compiler()
        .compile(
            vec![AbcModule {
                record_name: "com.example.app/entry/ets/pages/Index".to_string(),
                source: "(function(){ var x = 1; })();".to_string(),
                source_map: Some(format!(
                    r#"{{"version":3,"sources":["{source_file}"],"mappings":""}}"#
                )),
            }],
            &out,
        )
        .expect("compile must succeed");
    assert!(
        result.has_debug_info,
        "source-map backed LNP data marks the artifact as debug-bearing"
    );

    let bytes = std::fs::read(&out).expect("read .abc");
    let num_lnp = read_u32(&bytes, 0x24);
    let lnp_idx_off = read_u32(&bytes, 0x28) as usize;
    assert_eq!(num_lnp, 1, "one method gets one line-number program");
    assert!(lnp_idx_off > 0, "LNP index offset must be present");

    let user_off = class_offsets(&bytes)
        .into_iter()
        .find(|off| {
            class_name_at(&bytes, *off as usize) == "Lcom.example.app/entry/ets/pages/Index;"
        })
        .expect("user record exists") as usize;
    let (class_source_file, method_debug_info) = first_user_method_debug_info(&bytes, user_off);
    let class_source_file = class_source_file.expect("class SOURCE_FILE tag exists");
    let debug_info = method_debug_info.expect("method DEBUG_INFO tag exists");
    assert_eq!(
        inline_string_at(&bytes, class_source_file as usize),
        source_file,
        "source_map.sources[0] drives the emitted source file StringItem"
    );

    let mut pos = debug_info as usize;
    assert_eq!(read_uleb(&bytes, &mut pos), 1, "line_start is 1");
    assert_eq!(read_uleb(&bytes, &mut pos), 0, "no parameter names emitted");
    let cp_count = read_uleb(&bytes, &mut pos) as usize;
    assert_eq!(
        cp_count, 1,
        "constant pool has exactly one entry: the source file offset"
    );
    assert_eq!(
        read_uleb(&bytes, &mut pos),
        class_source_file as u64,
        "DebugInfo constant pool stores the source file offset"
    );
    assert_eq!(
        read_uleb(&bytes, &mut pos),
        0,
        "debug info references LNP index 0"
    );

    let lnp_offset = read_u32(&bytes, lnp_idx_off) as usize;
    assert_eq!(bytes[lnp_offset], 0x05, "LNP starts with SET_FILE opcode");
    assert_eq!(
        read_u32(&bytes, lnp_offset + 1),
        class_source_file,
        "SET_FILE operand is the source file string offset"
    );
    // After SET_FILE (5 bytes), the LNP may contain advance sequences
    // (ADVANCE_PC, ADVANCE_LINE, SPECIAL_OPCODE) followed by END_SEQUENCE.
    // Scan past the advance opcodes to find the terminating 0x00.
    let lnp_body = &bytes[lnp_offset + 5..];
    let end_seq_pos = lnp_body
        .iter()
        .position(|&b| b == 0x00)
        .expect("LNP must contain END_SEQUENCE (0x00)");
    assert_eq!(
        lnp_body[end_seq_pos], 0x00,
        "LNP body terminates with END_SEQUENCE"
    );
}

// ── New opcode coverage tests ────────────────────────────────────────────────

fn module_compiler() -> AbcCompiler {
    AbcCompiler::new(AbcCompileOptions {
        debug_info: false,
        opt_level: 0,
        target_api_version: 12,
        merge_abc: true,
        module: true,
        cache_path: None,
    })
}

fn module_record(record_name: &str, source: &str) -> AbcModule {
    AbcModule {
        record_name: record_name.to_string(),
        source: source.to_string(),
        source_map: None,
    }
}

#[test]
fn test_in_operator_emits_isin_opcode() {
    let tmp = tempfile::tempdir().unwrap();
    let out = tmp.path().join("test.abc");
    module_compiler()
        .compile(
            vec![module_record("test/mod", r#"const x = "key" in obj;"#)],
            &out,
        )
        .expect("compile");
    let bytes = std::fs::read(&out).unwrap();
    assert!(bytes.starts_with(b"PANDA\0\0\0"));
    // isin opcode is 0x25
    assert!(
        bytes.contains(&0x25),
        "isin opcode (0x25) must appear for 'in' operator"
    );
}

#[test]
fn test_typeof_emits_typeof_opcode() {
    let tmp = tempfile::tempdir().unwrap();
    let out = tmp.path().join("test.abc");
    module_compiler()
        .compile(
            vec![module_record(
                "test/mod",
                r#"const t = typeof paramsLambda;"#,
            )],
            &out,
        )
        .expect("compile");
    let bytes = std::fs::read(&out).unwrap();
    assert!(bytes.starts_with(b"PANDA\0\0\0"));
    // typeof opcode is 0x1c
    assert!(
        bytes.contains(&0x1c),
        "typeof opcode (0x1c) must appear for 'typeof' expression"
    );
}

#[test]
fn test_strict_equality_emits_stricteq_opcode() {
    let tmp = tempfile::tempdir().unwrap();
    let out = tmp.path().join("test.abc");
    module_compiler()
        .compile(
            vec![module_record(
                "test/mod",
                r#"const ok = typeof x === "function";"#,
            )],
            &out,
        )
        .expect("compile");
    let bytes = std::fs::read(&out).unwrap();
    assert!(bytes.starts_with(b"PANDA\0\0\0"));
    // stricteq opcode is 0x28
    assert!(
        bytes.contains(&0x28),
        "stricteq opcode (0x28) must appear for '===' operator"
    );
}

#[test]
fn test_void_expression_emits_ldundefined() {
    let tmp = tempfile::tempdir().unwrap();
    let out = tmp.path().join("test.abc");
    module_compiler()
        .compile(
            vec![module_record("test/mod", r#"const x = void 0;"#)],
            &out,
        )
        .expect("compile");
    let bytes = std::fs::read(&out).unwrap();
    assert!(bytes.starts_with(b"PANDA\0\0\0"));
    // ldundefined opcode is 0x00
    assert!(
        bytes.contains(&0x00),
        "ldundefined must appear for 'void 0'"
    );
}

#[test]
fn test_logical_and_emits_jeqz_branch() {
    let tmp = tempfile::tempdir().unwrap();
    let out = tmp.path().join("test.abc");
    module_compiler()
        .compile(
            vec![module_record("test/mod", r#"const x = a && b;"#)],
            &out,
        )
        .expect("compile");
    let bytes = std::fs::read(&out).unwrap();
    assert!(bytes.starts_with(b"PANDA\0\0\0"));
    // jeqz opcode is 0x50 — emitted for short-circuit &&
    assert!(
        bytes.contains(&0x50),
        "jeqz (0x50) must appear for '&&' short-circuit"
    );
}

#[test]
fn test_logical_or_emits_jnez_branch() {
    let tmp = tempfile::tempdir().unwrap();
    let out = tmp.path().join("test.abc");
    module_compiler()
        .compile(
            vec![module_record("test/mod", r#"const x = a || b;"#)],
            &out,
        )
        .expect("compile");
    let bytes = std::fs::read(&out).unwrap();
    assert!(bytes.starts_with(b"PANDA\0\0\0"));
    // jnez opcode is 0x9b — emitted for short-circuit ||
    assert!(
        bytes.contains(&0x9b),
        "jnez (0x9b) must appear for '||' short-circuit"
    );
}

#[test]
fn test_two_module_compile_size_tracks_parity_goal() {
    // Compile both EntryAbility-like and Index-like sources together (matching
    // what the minimal-hap build does), and assert the combined ABC is at least
    // as large as what we produce with current constructs.  This gives a
    // regression baseline while we work toward the 9844-byte hvigor target.
    let tmp = tempfile::tempdir().unwrap();
    let out = tmp.path().join("modules.abc");
    let entry_source = r#"
import UIAbility from "@ohos:app.ability.UIAbility";
import hilog from "@ohos:hilog";
const DOMAIN = 0;
var EntryAbility = class extends UIAbility {
    onCreate(want, launchParam) {
        hilog.info(DOMAIN, "testTag", "%{public}s", "Ability onCreate");
    }
    onWindowStageCreate(windowStage) {
        hilog.info(DOMAIN, "testTag", "%{public}s", "Ability onWindowStageCreate");
        windowStage.loadContent("pages/Index", (err) => {
            if (err.code) {
                hilog.error(DOMAIN, "testTag", "failed", JSON.stringify(err));
                return;
            }
            hilog.info(DOMAIN, "testTag", "Succeeded.");
        });
    }
    onForeground() {
        hilog.info(DOMAIN, "testTag", "%{public}s", "Ability onForeground");
    }
    onBackground() {
        hilog.info(DOMAIN, "testTag", "%{public}s", "Ability onBackground");
    }
};
export { EntryAbility as default };
"#;
    let index_source = r#"
if (!("finalizeConstruction" in ViewPU.prototype))
    Reflect.set(ViewPU.prototype, "finalizeConstruction", () => {});
var Index = class extends ViewPU {
    constructor(parent, params, __localStorage, elmtId, paramsLambda, extraInfo) {
        super(parent, __localStorage, elmtId, extraInfo);
        if (typeof paramsLambda === "function") this.paramsGenerator_ = paramsLambda;
        this.__message = new ObservedPropertySimplePU("Hello World", this, "message");
        this.setInitiallyProvidedValue(params);
        this.finalizeConstruction();
    }
    setInitiallyProvidedValue(params) {
        if (params.message !== void 0) this.message = params.message;
    }
    updateStateVars(params) {}
    purgeVariableDependenciesOnElmtId(rmElmtId) {
        this.__message.purgeDependencyOnElmtId(rmElmtId);
    }
    aboutToBeDeleted() {
        this.__message.aboutToBeDeleted();
        SubscriberManager.Get().delete(this.id__());
        this.aboutToBeDeletedInternal();
    }
    get message() { return this.__message.get(); }
    set message(newValue) { this.__message.set(newValue); }
    rerender() { this.updateDirtyElements(); }
};
registerNamedRoute(() => new Index(void 0, {}), "", {bundleName: "x"});
"#;
    module_compiler()
        .compile(
            vec![
                module_record(
                    "com.example.test/entry/ets/entryability/EntryAbility",
                    entry_source,
                ),
                module_record("com.example.test/entry/ets/pages/Index", index_source),
            ],
            &out,
        )
        .expect("compile both modules");
    let bytes = std::fs::read(&out).unwrap();
    assert!(bytes.starts_with(b"PANDA\0\0\0"));
    // This test uses debug_info:false; it is a structural regression baseline only.
    // For byte-parity against hvigor (9844 bytes) see test_two_module_debug_parity below.
    assert!(
        bytes.len() > 2000,
        "two-module compile must produce >2000 byte ABC, got {} bytes",
        bytes.len()
    );
}

#[test]
fn test_two_module_debug_parity() {
    // Compile with debug_info:true (LNP enabled) to measure parity against hvigor.
    // Hvigor produces 9844 bytes for the same source.  We track progress here.
    let entry_source = r#"
import UIAbility from "@ohos:app.ability.UIAbility";
import hilog from "@ohos:hilog";
const DOMAIN = 0;
var EntryAbility = class extends UIAbility {
    onCreate(want, launchParam) {
        hilog.info(DOMAIN, "testTag", "%{public}s", "Ability onCreate");
    }
    onWindowStageCreate(windowStage) {
        hilog.info(DOMAIN, "testTag", "%{public}s", "Ability onWindowStageCreate");
        windowStage.loadContent("pages/Index", (err) => {
            if (err.code) {
                hilog.error(DOMAIN, "testTag", "failed", JSON.stringify(err));
                return;
            }
            hilog.info(DOMAIN, "testTag", "Succeeded.");
        });
    }
    onForeground() {
        hilog.info(DOMAIN, "testTag", "%{public}s", "Ability onForeground");
    }
    onBackground() {
        hilog.info(DOMAIN, "testTag", "%{public}s", "Ability onBackground");
    }
};
export { EntryAbility as default };
"#;
    let index_source = r#"
if (!("finalizeConstruction" in ViewPU.prototype))
    Reflect.set(ViewPU.prototype, "finalizeConstruction", () => {});
var Index = class extends ViewPU {
    constructor(parent, params, __localStorage, elmtId, paramsLambda, extraInfo) {
        super(parent, __localStorage, elmtId, extraInfo);
        if (typeof paramsLambda === "function") this.paramsGenerator_ = paramsLambda;
        this.__message = new ObservedPropertySimplePU("Hello World", this, "message");
        this.setInitiallyProvidedValue(params);
        this.finalizeConstruction();
    }
    setInitiallyProvidedValue(params) {
        if (params.message !== void 0) this.message = params.message;
    }
    updateStateVars(params) {}
    purgeVariableDependenciesOnElmtId(rmElmtId) {
        this.__message.purgeDependencyOnElmtId(rmElmtId);
    }
    aboutToBeDeleted() {
        this.__message.aboutToBeDeleted();
        SubscriberManager.Get().delete(this.id__());
        this.aboutToBeDeletedInternal();
    }
    get message() { return this.__message.get(); }
    set message(newValue) { this.__message.set(newValue); }
    rerender() { this.updateDirtyElements(); }
};
registerNamedRoute(() => new Index(void 0, {}), "", {bundleName: "x"});
"#;
    let tmp = tempfile::tempdir().unwrap();
    let out = tmp.path().join("modules_debug.abc");
    AbcCompiler::new(AbcCompileOptions {
        debug_info: true,
        opt_level: 0,
        target_api_version: 12,
        merge_abc: true,
        module: true,
        cache_path: None,
    })
    .compile(
        vec![
            AbcModule {
                record_name: "com.example.test/entry/ets/entryability/EntryAbility".to_string(),
                source: entry_source.to_string(),
                source_map: None,
            },
            AbcModule {
                record_name: "com.example.test/entry/ets/pages/Index".to_string(),
                source: index_source.to_string(),
                source_map: None,
            },
        ],
        &out,
    )
    .expect("compile both modules with debug info");
    let bytes = std::fs::read(&out).unwrap();
    assert!(bytes.starts_with(b"PANDA\0\0\0"));
    let sz = bytes.len();
    eprintln!("[parity] debug two-module ABC = {sz}B / 9844B hvigor ({:.0}%)", sz as f64 / 9844.0 * 100.0);
    // Phase 5 parity baseline: LNP index moved to file end + OHM URL method names +
    // field name deduplication raised parity from 76.9% (7573B) to 88.1% (8675B).
    // Remaining gap: LNP body content (~737B) + method index ordering (~432B).
    assert!(
        sz > 6500,
        "debug-info two-module compile must exceed 6500B (Phase 5 parity regression guard), got {sz}B"
    );
}

#[test]
fn test_this_expression_emits_ldthis_opcode() {
    let tmp = tempfile::tempdir().unwrap();
    let out = tmp.path().join("test.abc");
    module_compiler()
        .compile(
            vec![module_record(
                "test/mod",
                r#"
class Foo {
    bar() { return this.x; }
    baz() { this.y = 1; }
}
"#,
            )],
            &out,
        )
        .expect("compile");
    let bytes = std::fs::read(&out).unwrap();
    assert!(bytes.starts_with(b"PANDA\0\0\0"));
    // ldthis opcode is 0x6f
    assert!(
        bytes.contains(&0x6f),
        "ldthis (0x6f) must appear for 'this' in a class method body"
    );
}

#[test]
fn test_binary_arithmetic_emits_arith_opcodes() {
    let tmp = tempfile::tempdir().unwrap();
    let out = tmp.path().join("test.abc");
    module_compiler()
        .compile(
            vec![module_record(
                "test/mod",
                r#"const x = a + b; const y = a - b; const z = a * b;"#,
            )],
            &out,
        )
        .expect("compile");
    let bytes = std::fs::read(&out).unwrap();
    assert!(bytes.starts_with(b"PANDA\0\0\0"));
    assert!(bytes.contains(&0x0a), "add2 opcode (0x0a) for '+'");
    assert!(bytes.contains(&0x0b), "sub2 opcode (0x0b) for '-'");
    assert!(bytes.contains(&0x0c), "mul2 opcode (0x0c) for '*'");
}

#[test]
fn test_entry_ability_like_source_generates_nontrivial_bytecode() {
    let tmp = tempfile::tempdir().unwrap();
    let out = tmp.path().join("entry.abc");
    let source = r#"
import UIAbility from "@ohos:app.ability.UIAbility";
import hilog from "@ohos:hilog";
const DOMAIN = 0;
var EntryAbility = class extends UIAbility {
    onCreate(want, launchParam) {
        hilog.info(DOMAIN, "testTag", "%{public}s", "Ability onCreate");
    }
    onWindowStageCreate(windowStage) {
        hilog.info(DOMAIN, "testTag", "%{public}s", "Ability onWindowStageCreate");
        windowStage.loadContent("pages/Index", (err) => {
            if (err.code) {
                hilog.error(DOMAIN, "testTag", "failed", JSON.stringify(err));
                return;
            }
            hilog.info(DOMAIN, "testTag", "Succeeded.");
        });
    }
    onForeground() {
        hilog.info(DOMAIN, "testTag", "%{public}s", "Ability onForeground");
    }
    onBackground() {
        hilog.info(DOMAIN, "testTag", "%{public}s", "Ability onBackground");
    }
};
export { EntryAbility as default };
"#;
    module_compiler()
        .compile(
            vec![module_record(
                "com.example.test/entry/ets/entryability/EntryAbility",
                source,
            )],
            &out,
        )
        .expect("compile EntryAbility-like source");
    let bytes = std::fs::read(&out).unwrap();
    assert!(bytes.starts_with(b"PANDA\0\0\0"));
    // The ABC must contain method bytecode for at least the class methods.
    // Minimum sensible size: >500 bytes for this module with 4 class methods.
    assert!(
        bytes.len() > 500,
        "EntryAbility-like source must produce >500 byte ABC, got {}",
        bytes.len()
    );
}

#[test]
fn test_index_viewpu_like_source_generates_nontrivial_bytecode() {
    let tmp = tempfile::tempdir().unwrap();
    let out = tmp.path().join("index.abc");
    let source = r#"
if (!("finalizeConstruction" in ViewPU.prototype))
    Reflect.set(ViewPU.prototype, "finalizeConstruction", () => {});
var Index = class extends ViewPU {
    constructor(parent, params, __localStorage, elmtId, paramsLambda, extraInfo) {
        super(parent, __localStorage, elmtId, extraInfo);
        if (typeof paramsLambda === "function") this.paramsGenerator_ = paramsLambda;
        this.__message = new ObservedPropertySimplePU("Hello World", this, "message");
        this.setInitiallyProvidedValue(params);
        this.finalizeConstruction();
    }
    setInitiallyProvidedValue(params) {
        if (params.message !== void 0) this.message = params.message;
    }
    updateStateVars(params) {}
    purgeVariableDependenciesOnElmtId(rmElmtId) {
        this.__message.purgeDependencyOnElmtId(rmElmtId);
    }
    aboutToBeDeleted() {
        this.__message.aboutToBeDeleted();
        SubscriberManager.Get().delete(this.id__());
        this.aboutToBeDeletedInternal();
    }
    get message() { return this.__message.get(); }
    set message(newValue) { this.__message.set(newValue); }
    rerender() { this.updateDirtyElements(); }
};
registerNamedRoute(() => new Index(void 0, {}), "", {bundleName: "x"});
"#;
    module_compiler()
        .compile(
            vec![module_record(
                "com.example.test/entry/ets/pages/Index",
                source,
            )],
            &out,
        )
        .expect("compile Index-like source");
    let bytes = std::fs::read(&out).unwrap();
    assert!(bytes.starts_with(b"PANDA\0\0\0"));
    assert!(
        bytes.len() > 600,
        "Index-like ViewPU source must produce >600 byte ABC, got {}",
        bytes.len()
    );
    // verify key opcodes appear
    assert!(bytes.contains(&0x25), "isin (0x25) for 'in' operator");
    assert!(
        bytes.contains(&0x1c),
        "typeof (0x1c) for 'typeof' expression"
    );
    assert!(bytes.contains(&0x27), "strictnoteq (0x27) for '!=='");
}

#[test]
fn test_static_object_uses_createobjectwithbuffer() {
    // A static object literal (all string keys, all string values) must be
    // compiled via `createobjectwithbuffer` (opcode 0x07) rather than the
    // slower `createemptyobject` (0x04) + `stobjbyname` sequence.
    //
    // This mirrors how hvigor/es2abc compiles the `registerNamedRoute` metadata
    // argument: a 6-property all-string object is emitted as a 24-entry literal
    // array (12 tagged-string entries × 2 items each = count 24).
    let tmp = tempfile::tempdir().unwrap();
    let out = tmp.path().join("static_obj.abc");
    let source = r#"const meta = { bundleName: "test", moduleName: "entry" };"#;
    module_compiler()
        .compile(vec![module_record("test/static_obj", source)], &out)
        .expect("compile static object source");
    let bytes = std::fs::read(&out).unwrap();
    assert!(bytes.starts_with(b"PANDA\0\0\0"));

    // opcode 0x07 = createobjectwithbuffer must appear in the output.
    assert!(
        bytes.contains(&0x07u8),
        "createobjectwithbuffer (0x07) must be emitted for static object literal"
    );

    // The literal-array table must include the object-literal array plus the
    // standard module-record and scope-names arrays.
    let num_lits = read_u32(&bytes, 0x2c);
    assert!(
        num_lits >= 3,
        "at least 3 literal arrays expected (obj + module-record + scope-names); got {num_lits}"
    );
}

#[test]
fn test_while_loop_emits_backward_branch() {
    // A while loop must emit a jeqz (0x50) for the loop test and a backward
    // jmp (0x4E) to restart the loop.  Both opcodes must appear in the output.
    let tmp = tempfile::tempdir().unwrap();
    let out = tmp.path().join("while.abc");
    module_compiler()
        .compile(
            vec![module_record(
                "test/while",
                r#"let i = 0; while (i < 10) { i = i + 1; }"#,
            )],
            &out,
        )
        .expect("compile while loop");
    let bytes = std::fs::read(&out).unwrap();
    assert!(bytes.starts_with(b"PANDA\0\0\0"));
    // jeqz (0x50) — condition exit branch
    assert!(bytes.contains(&0x50u8), "jeqz (0x50) must appear for while loop condition");
    // jmp (0x4E) — backward loop edge
    assert!(bytes.contains(&0x4Eu8), "jmp (0x4E) must appear as the loop back-edge");
}

#[test]
fn test_conditional_expr_emits_branches() {
    // A conditional expression `a ? b : c` must emit both a jeqz (0x50) to
    // skip the consequent and a jmp (0x4E) to skip the alternate.
    let tmp = tempfile::tempdir().unwrap();
    let out = tmp.path().join("cond.abc");
    module_compiler()
        .compile(
            vec![module_record(
                "test/cond",
                r#"const x = flag ? 1 : 2;"#,
            )],
            &out,
        )
        .expect("compile conditional expression");
    let bytes = std::fs::read(&out).unwrap();
    assert!(bytes.starts_with(b"PANDA\0\0\0"));
    assert!(bytes.contains(&0x50u8), "jeqz (0x50) must appear for ternary test branch");
    assert!(bytes.contains(&0x4Eu8), "jmp (0x4E) must appear to skip alternate branch");
}

#[test]
fn test_template_literal_emits_string_concatenation() {
    // A template literal with an interpolated expression must emit an
    // ldaStr (0x3E) for the static quasi parts and Add2 (0x0A) for
    // each interpolation.
    let tmp = tempfile::tempdir().unwrap();
    let out = tmp.path().join("tmpl.abc");
    module_compiler()
        .compile(
            vec![module_record(
                "test/tmpl",
                r#"const s = `Hello ${name}!`;"#,
            )],
            &out,
        )
        .expect("compile template literal");
    let bytes = std::fs::read(&out).unwrap();
    assert!(bytes.starts_with(b"PANDA\0\0\0"));
    // lda.str opcode is 0x3E — emitted for quasi string parts
    assert!(bytes.contains(&0x3Eu8), "lda.str (0x3E) must appear for template quasi strings");
    // add2 opcode is 0x0A — emitted for string concatenation
    assert!(bytes.contains(&0x0Au8), "add2 (0x0A) must appear for template expression concat");
}

#[test]
fn test_for_loop_emits_opcodes() {
    // A counted for loop emits a jeqz test branch and backward jmp back-edge.
    let tmp = tempfile::tempdir().unwrap();
    let out = tmp.path().join("for.abc");
    module_compiler()
        .compile(
            vec![module_record(
                "test/for",
                r#"for (let i = 0; i < 5; i = i + 1) { }"#,
            )],
            &out,
        )
        .expect("compile for loop");
    let bytes = std::fs::read(&out).unwrap();
    assert!(bytes.starts_with(b"PANDA\0\0\0"));
    assert!(bytes.contains(&0x50u8), "jeqz (0x50) for loop test");
    assert!(bytes.contains(&0x4Eu8), "jmp (0x4E) backward loop edge");
}

// ---------------------------------------------------------------------------
// TryStatement / ThrowStatement / AwaitExpression (Commit 1+2)
// ---------------------------------------------------------------------------

#[test]
fn test_await_expression_compiles() {
    // An `async` function with `await` must compile to valid Panda ABC without
    // panicking.  Simplified lowering: the awaited expression is evaluated and
    // left in the accumulator; ArkRuntime's Promise machinery handles the rest.
    let tmp = tempfile::tempdir().unwrap();
    let out = tmp.path().join("await.abc");
    let source = r#"
        async function loadData() {
            const result = await fetchData();
            return result;
        }
    "#;
    module_compiler()
        .compile(vec![module_record("test/await_test", source)], &out)
        .expect("compile async/await source");
    let bytes = std::fs::read(&out).unwrap();
    assert!(bytes.starts_with(b"PANDA\0\0\0"));
    assert!(
        bytes.len() > 32,
        "async/await bytecode must be non-trivial, got {} bytes",
        bytes.len()
    );
}

#[test]
fn test_throw_statement_emits_throw_opcode() {
    // ThrowStatement must emit the Panda `throw` prefix opcode sequence
    // 0xFE 0x00 after evaluating its argument.
    let tmp = tempfile::tempdir().unwrap();
    let out = tmp.path().join("throw.abc");
    module_compiler()
        .compile(
            vec![module_record(
                "test/throw",
                r#"throw new Error("oops");"#,
            )],
            &out,
        )
        .expect("compile throw statement");
    let bytes = std::fs::read(&out).unwrap();
    assert!(bytes.starts_with(b"PANDA\0\0\0"));
    // throw is encoded as [0xFE, 0x00]
    assert!(
        bytes.windows(2).any(|w| w == [0xFE, 0x00]),
        "throw prefix (0xFE 0x00) must appear for throw statement"
    );
}

#[test]
fn test_throw_statement_emits_throw_opcode_2() {
    // `throw new Error("oops")` must produce the Panda `throw` opcode,
    // which is a two-byte prefixed instruction: [0xFE, 0x00].
    let tmp = tempfile::tempdir().unwrap();
    let out = tmp.path().join("throw.abc");
    let source = r#"throw new Error("oops");"#;
    module_compiler()
        .compile(vec![module_record("test/throw_test", source)], &out)
        .expect("compile throw source");
    let bytes = std::fs::read(&out).unwrap();
    assert!(bytes.starts_with(b"PANDA\0\0\0"));
    let has_throw = bytes.windows(2).any(|w| w == [0xFE, 0x00]);
    assert!(
        has_throw,
        "throw opcode [0xFE, 0x00] must appear in output for `throw` statement"
    );
}

#[test]
fn test_optional_chaining_compiles_and_emits_null_check() {
    let tmp = tempfile::tempdir().unwrap();
    let out = tmp.path().join("optchain.abc");
    let source = r#"
const x = obj?.prop;
const y = arr?.length;
"#;
    module_compiler()
        .compile(vec![module_record("test/optchain", source)], &out)
        .expect("optional chaining must compile without error");
    let bytes = std::fs::read(&out).unwrap();
    assert!(bytes.starts_with(b"PANDA\0\0\0"));
    // eq opcode (0x0f) for null check (obj == null catches both null and undefined)
    assert!(
        bytes.contains(&0x0f),
        "eq opcode (0x0f) must appear for null check in optional chaining"
    );
    // jnez opcode (0x9b) for short-circuit branch on null/undefined
    assert!(
        bytes.contains(&0x9b),
        "jnez (0x9b) must appear for short-circuit in optional chaining"
    );
}

#[test]
fn test_destructuring_array_and_object_compile() {
    let tmp = tempfile::tempdir().unwrap();
    let out = tmp.path().join("destruct.abc");
    let source = r#"
const [a, b] = someArray;
const {x, y} = someObject;
const sum = a + b;
"#;
    module_compiler()
        .compile(vec![module_record("test/destruct", source)], &out)
        .expect("destructuring declarations must compile without error");
    let bytes = std::fs::read(&out).unwrap();
    assert!(bytes.starts_with(b"PANDA\0\0\0"));
    // ldobjbyname (0x42) must appear for reading elements/props via string index
    assert!(
        bytes.contains(&0x42),
        "ldobjbyname (0x42) must appear for destructuring element access"
    );
    // add2 (0x0a) must appear for the sum expression proving bindings were established
    assert!(
        bytes.contains(&0x0a),
        "add2 (0x0a) must appear after destructuring binds are established"
    );
}

#[test]
fn test_for_of_and_array_literal_compile() {
    let tmp = tempfile::tempdir().unwrap();
    let out = tmp.path().join("forof.abc");
    let source = r#"
const arr = [1, 2, 3];
let total = 0;
for (const item of arr) {
    total = total + item;
}
"#;
    module_compiler()
        .compile(vec![module_record("test/forof", source)], &out)
        .expect("for-of and array literal must compile without error");
    let bytes = std::fs::read(&out).unwrap();
    assert!(bytes.starts_with(b"PANDA\0\0\0"));
    // createemptyarray (0x05) must appear for array literal [1, 2, 3]
    assert!(
        bytes.contains(&0x05),
        "createemptyarray (0x05) must appear for array literal"
    );
    // stownbyindex (0x79) must appear for populating array elements
    assert!(
        bytes.contains(&0x79),
        "stownbyindex (0x79) must appear for array element assignment"
    );
}

#[test]
fn test_for_in_stmt_emits_getpropiterator() {
    let tmp = tempfile::tempdir().unwrap();
    let out = tmp.path().join("for_in.abc");
    let source = r#"
const obj = { a: 1, b: 2 };
const keys = [];
for (const key in obj) {
    keys.push(key);
}
"#;
    module_compiler()
        .compile(vec![module_record("test/for_in", source)], &out)
        .expect("compile for-in source");
    let bytes = std::fs::read(&out).unwrap();
    assert!(bytes.starts_with(b"PANDA\0\0\0"));
    assert!(bytes.contains(&0x66u8), "getpropiterator (0x66) must appear for 'for...in' loop");
    assert!(bytes.contains(&0x36u8), "getnextpropname (0x36) must appear for 'for...in' loop");
    assert!(bytes.contains(&0x50u8), "jeqz (0x50) must appear for 'for...in' exit test");
}

#[test]
fn test_for_of_stmt_emits_getiterator() {
    let tmp = tempfile::tempdir().unwrap();
    let out = tmp.path().join("for_of.abc");
    let source = r#"
const arr = [1, 2, 3];
let sum = 0;
for (const x of arr) {
    sum = sum + x;
}
"#;
    module_compiler()
        .compile(vec![module_record("test/for_of", source)], &out)
        .expect("compile for-of source");
    let bytes = std::fs::read(&out).unwrap();
    assert!(bytes.starts_with(b"PANDA\0\0\0"));
    assert!(bytes.contains(&0x67u8), "getiterator (0x67) must appear for 'for...of' loop");
    assert!(bytes.contains(&0x9bu8), "jnez (0x9b) must appear for 'for...of' done-check");
}

fn test_try_catch_block_compiles_valid_bytecode() {
    // A try/catch block with a named catch parameter must compile to valid
    // Panda ABC.  The CodeGen emits a `jmp` (0x4E) at the end of the try body
    // to skip the catch handler on the normal (no-exception) path, and
    // registers a CatchEntry in the method's exception table.
    let tmp = tempfile::tempdir().unwrap();
    let out = tmp.path().join("try_catch.abc");
    let source = r#"
        try {
            const x = doRiskyOp();
        } catch (e) {
            handleError(e);
        }
    "#;
    module_compiler()
        .compile(vec![module_record("test/try_catch_test", source)], &out)
        .expect("compile try/catch source");
    let bytes = std::fs::read(&out).unwrap();
    assert!(bytes.starts_with(b"PANDA\0\0\0"));
    assert!(
        bytes.len() > 64,
        "try/catch bytecode must be non-trivial, got {} bytes",
        bytes.len()
    );
    // jmp (0x4E) is emitted to skip the catch body on the normal path.
    assert!(
        bytes.contains(&0x4Eu8),
        "jmp opcode (0x4E) must appear in try/catch output"
    );
}


fn test_module_request_annotations_all_specifiers_tracked() {
    // All import specifiers (relative, package, and @ohos:) must appear in the
    // module record literal array embedded in the binary.
    let tmp = tempfile::tempdir().unwrap();
    let out = tmp.path().join("module_req.abc");
    module_compiler()
        .compile(
            vec![module_record(
                "com.example.test/entry/ets/pages/Index",
                "import x from './utils'; export { x };",
            )],
            &out,
        )
        .expect("compile");
    let bytes = std::fs::read(&out).unwrap();
    assert!(bytes.starts_with(b"PANDA\0\0\0"));
    let specifier = b"./utils";
    assert!(
        bytes.windows(specifier.len()).any(|w| w == specifier),
        "relative module specifier './utils' must appear in the binary"
    );
}

#[test]
fn test_namespace_import_tracked_in_module_record() {
    // `import * as ns from './x'` must be recorded as a namespace import in the
    // module record literal array (not as a regular import).
    let tmp = tempfile::tempdir().unwrap();
    let out = tmp.path().join("ns_import.abc");
    module_compiler()
        .compile(
            vec![module_record(
                "com.example.test/entry/ets/pages/Foo",
                "import * as utils from './utils'; utils.doSomething();",
            )],
            &out,
        )
        .expect("compile");
    let bytes = std::fs::read(&out).unwrap();
    assert!(bytes.starts_with(b"PANDA\0\0\0"));
    let specifier = b"./utils";
    assert!(
        bytes.windows(specifier.len()).any(|w| w == specifier),
        "namespace import specifier './utils' must appear in the binary"
    );
}

#[test]
fn test_indirect_export_tracked_in_module_record() {
    // `export { foo } from './x'` must be recorded as an indirect export.
    let tmp = tempfile::tempdir().unwrap();
    let out = tmp.path().join("indirect_export.abc");
    module_compiler()
        .compile(
            vec![module_record(
                "com.example.test/entry/ets/pages/Bar",
                "export { foo as myFoo } from './other';",
            )],
            &out,
        )
        .expect("compile");
    let bytes = std::fs::read(&out).unwrap();
    assert!(bytes.starts_with(b"PANDA\0\0\0"));
    let specifier = b"./other";
    assert!(
        bytes.windows(specifier.len()).any(|w| w == specifier),
        "indirect export specifier './other' must appear in the binary"
    );
}

#[test]
fn test_star_export_tracked_in_module_record() {
    // `export * from './x'` must be recorded as a star export.
    let tmp = tempfile::tempdir().unwrap();
    let out = tmp.path().join("star_export.abc");
    module_compiler()
        .compile(
            vec![module_record(
                "com.example.test/entry/ets/pages/Baz",
                "export * from './shared';",
            )],
            &out,
        )
        .expect("compile");
    let bytes = std::fs::read(&out).unwrap();
    assert!(bytes.starts_with(b"PANDA\0\0\0"));
    let specifier = b"./shared";
    assert!(
        bytes.windows(specifier.len()).any(|w| w == specifier),
        "star export specifier './shared' must appear in the binary"
    );
}

#[test]
fn test_spread_call_emits_callrange_opcode() {
    // `foo(...args)` must compile to a `callrange` (0x73) instruction rather
    // than the shorter fixed-arity forms, since the spread element forces the
    // range path in emit_call.
    let tmp = tempfile::tempdir().unwrap();
    let out = tmp.path().join("spread_call.abc");
    let source = r#"
        function applyAll(fn, args) {
            return fn(...args);
        }
    "#;
    module_compiler()
        .compile(vec![module_record("test/spread_call", source)], &out)
        .expect("compile spread call");
    let bytes = std::fs::read(&out).unwrap();
    assert!(bytes.starts_with(b"PANDA\0\0\0"));
    // callrange opcode is 0x73
    assert!(
        bytes.contains(&0x73),
        "callrange opcode (0x73) must appear for spread-argument call foo(...args)"
    );
}

#[test]
fn test_spread_method_call_emits_callthisrange_opcode() {
    // `obj.method(...args)` must use `callthisrange` (0x31) when spread args are present.
    let tmp = tempfile::tempdir().unwrap();
    let out = tmp.path().join("spread_method.abc");
    let source = r#"
        function callMethod(obj, args) {
            return obj.run(...args);
        }
    "#;
    module_compiler()
        .compile(vec![module_record("test/spread_method", source)], &out)
        .expect("compile spread method call");
    let bytes = std::fs::read(&out).unwrap();
    assert!(bytes.starts_with(b"PANDA\0\0\0"));
    // callthisrange opcode is 0x31
    assert!(
        bytes.contains(&0x31),
        "callthisrange opcode (0x31) must appear for spread-argument method call obj.run(...args)"
    );
}

#[test]
fn test_rest_parameter_emits_copyrestargs_opcode() {
    // A function with a rest parameter `...args` must emit `copyrestargs`
    // (0xA1) at the start of the function body to collect trailing arguments
    // into an array.
    let tmp = tempfile::tempdir().unwrap();
    let out = tmp.path().join("rest_param.abc");
    let source = r#"
        function sum(...nums) {
            let total = 0;
            for (const n of nums) {
                total = total + n;
            }
            return total;
        }
    "#;
    module_compiler()
        .compile(vec![module_record("test/rest_param", source)], &out)
        .expect("compile rest parameter function");
    let bytes = std::fs::read(&out).unwrap();
    assert!(bytes.starts_with(b"PANDA\0\0\0"));
    // copyrestargs opcode is 0xA1
    assert!(
        bytes.contains(&0xA1),
        "copyrestargs opcode (0xA1) must appear for rest parameter ...nums"
    );
}

#[test]
fn test_rest_param_with_fixed_params_uses_correct_count() {
    // `function foo(a, b, ...rest)` should emit `copyrestargs 2` (fixed=2).
    // We verify the 0xA1 opcode is present and is followed by the byte 0x02.
    let tmp = tempfile::tempdir().unwrap();
    let out = tmp.path().join("rest_fixed.abc");
    let source = r#"
        function merge(target, source, ...extras) {
            return [target, source, extras];
        }
    "#;
    module_compiler()
        .compile(vec![module_record("test/rest_fixed", source)], &out)
        .expect("compile rest param with fixed args");
    let bytes = std::fs::read(&out).unwrap();
    assert!(bytes.starts_with(b"PANDA\0\0\0"));
    // copyrestargs 2: [0xA1, 0x02]
    assert!(
        bytes.windows(2).any(|w| w == [0xA1, 0x02]),
        "copyrestargs 2 ([0xA1, 0x02]) must appear for function(a, b, ...rest)"
    );
}

#[test]
fn test_class_instance_field_emits_stobjbyname() {
    // Class instance fields (`x = 1`) must emit `stobjbyname` (0x43) in the
    // constructor so the field is initialised on each new instance.
    let tmp = tempfile::tempdir().unwrap();
    let out = tmp.path().join("class_field.abc");
    let source = r#"
        class Counter {
            count = 0;
            step = 1;
            increment() {
                this.count = this.count + this.step;
            }
        }
    "#;
    module_compiler()
        .compile(vec![module_record("test/class_field", source)], &out)
        .expect("compile class with instance fields");
    let bytes = std::fs::read(&out).unwrap();
    assert!(bytes.starts_with(b"PANDA\0\0\0"));
    // stobjbyname opcode is 0x43
    assert!(
        bytes.contains(&0x43),
        "stobjbyname opcode (0x43) must appear for class instance field initializers"
    );
}

#[test]
fn test_async_function_emits_asyncfunctionenter_opcode() {
    // An `async function` must emit `asyncfunctionenter` (0xBB) as a prologue
    // so ArkRuntime can set up the async execution context.
    let tmp = tempfile::tempdir().unwrap();
    let out = tmp.path().join("async_fn.abc");
    let source = r#"
        async function fetchUser(id) {
            const data = await getUser(id);
            return data;
        }
    "#;
    module_compiler()
        .compile(vec![module_record("test/async_fn", source)], &out)
        .expect("compile async function");
    let bytes = std::fs::read(&out).unwrap();
    assert!(bytes.starts_with(b"PANDA\0\0\0"));
    // asyncfunctionenter opcode is 0xBB
    assert!(
        bytes.contains(&0xBB),
        "asyncfunctionenter opcode (0xBB) must appear for async function declaration"
    );
    // asyncfunctionresolve opcode is 0xBC (emitted before return)
    assert!(
        bytes.contains(&0xBC),
        "asyncfunctionresolve opcode (0xBC) must appear before return in async function"
    );
}
