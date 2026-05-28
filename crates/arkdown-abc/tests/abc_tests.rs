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
    make_compiler()
        .compile(
            vec![iife_module(
                "com.example.app/entry/ets/pages/Index",
                r#"(function(){
                    var ability = require("@ohos:app.ability.UIAbility");
                    var Index = class { onCreate(){} };
                })();"#,
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

    let mut pos = module_record as usize;
    assert_eq!(
        read_u32(&bytes, pos),
        10,
        "one request, one regular import, and four empty section counts"
    );
    pos += 4;
    assert_eq!(read_u32(&bytes, pos), 1, "one module request");
    pos += 4;
    let request_string = read_u32(&bytes, pos) as usize;
    pos += 4;
    assert_eq!(
        inline_string_at(&bytes, request_string),
        "@ohos:app.ability.UIAbility"
    );
    assert_eq!(read_u32(&bytes, pos), 1, "one regular import");
    pos += 4;
    let local_name = read_u32(&bytes, pos) as usize;
    pos += 4;
    let import_name = read_u32(&bytes, pos) as usize;
    pos += 4;
    assert_eq!(read_u16(&bytes, pos), 0, "regular import request index");
    pos += 2;
    assert_eq!(inline_string_at(&bytes, local_name), "ability");
    assert_eq!(inline_string_at(&bytes, import_name), "default");
    for label in [
        "namespace imports",
        "local exports",
        "indirect exports",
        "star exports",
    ] {
        assert_eq!(read_u32(&bytes, pos), 0, "empty {label} section count");
        pos += 4;
    }

    let mut scope_pos = scope_names as usize;
    let scope_item_count = read_u32(&bytes, scope_pos);
    scope_pos += 4;
    assert!(scope_item_count >= 2 && scope_item_count % 2 == 0);
    assert_eq!(
        bytes[scope_pos], 0x05,
        "scopeNames entries are tagged strings"
    );

    let has_minimal_class_buffer = literal_offsets.iter().any(|offset| {
        let offset = *offset as usize;
        read_u32(&bytes, offset) == 2
            && bytes[offset + 4] == 0x02
            && read_u32(&bytes, offset + 5) == 0
    });
    assert!(
        has_minimal_class_buffer,
        "class buffer literal array carries tagged instancePropertyCount=0"
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
    let cp_len = read_uleb(&bytes, &mut pos) as usize;
    let cp_start = pos;
    let mut cp_pos = cp_start;
    assert_eq!(
        read_uleb(&bytes, &mut cp_pos),
        class_source_file as u64,
        "DebugInfo constant pool stores the source file offset"
    );
    assert_eq!(cp_pos, cp_start + cp_len);
    pos += cp_len;
    assert_eq!(
        read_uleb(&bytes, &mut pos),
        0,
        "debug info references LNP index 0"
    );

    let lnp_offset = read_u32(&bytes, lnp_idx_off) as usize;
    assert_eq!(
        &bytes[lnp_offset..lnp_offset + 2],
        &[0x09, 0x00],
        "minimal LNP sets source file then ends"
    );
}
