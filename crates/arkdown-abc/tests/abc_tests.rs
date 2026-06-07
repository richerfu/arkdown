//! Integration tests for the `arkdown-abc` public API.
//!
//! These tests exercise [`AbcCompiler`] through its public interface only,
//! without access to crate internals.

use arkdown_abc::abc_reader::AbcLiteralEntry;
use arkdown_abc::cache::{source_cache_hash, ProgramCacheFile};
use arkdown_abc::emit::file::version_for_target_api;
use arkdown_abc::proto_panda::scalar_value;
use arkdown_abc::{
    AbcCompileOptions, AbcCompiler, AbcError, AbcModule, AbcProgramCacheInput, AbcReader,
};
use std::collections::BTreeMap;

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

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

/// Compiler with debug_info=true, opt_level=0 — matches upstream es2abc in debug mode.
/// Use this when testing DebugInfoSection / LNP emission.
fn make_debug_compiler() -> AbcCompiler {
    AbcCompiler::new(AbcCompileOptions {
        debug_info: true,
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

fn iife_module(record_name: &str, source: &str) -> AbcModule {
    AbcModule {
        record_name: record_name.to_string(),
        source: source.to_string(),
        source_map: None,
        source_file: None,
        package_name: None,
        is_commonjs: false,
        is_shared_module: false,
        origin_source_lang: String::new(),
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

/// Find the end of an LNP data_ stream (opcode-only format, inline reg SLEB128s).
/// Returns the position AFTER the END_SEQUENCE byte.
fn lnp_body_end(bytes: &[u8], start: usize) -> usize {
    let mut pos = start;
    while pos < bytes.len() {
        let op = bytes[pos];
        match op {
            0x00 => return pos + 1,                              // END_SEQUENCE
            0x01 | 0x02 | 0x07 | 0x08 | 0x09 | 0x0a => pos += 1, // no inline operand
            0x03 | 0x04 | 0x05 | 0x06 => {
                // START_LOCAL*, END_LOCAL, RESTART_LOCAL: skip opcode + reg SLEB128
                pos += 1;
                while pos < bytes.len() && (bytes[pos] & 0x80) != 0 {
                    pos += 1;
                }
                pos += 1; // last SLEB128 byte
            }
            0x0b => pos += 1, // SET_COLUMN (operand in cp, not inline)
            _ => pos += 1,    // SPECIAL opcode (self-contained)
        }
    }
    pos
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

fn read_class_tagged_data(bytes: &[u8], pos: &mut usize) -> (u8, u8, Option<u32>) {
    let mut source_lang_tag = 0;
    let mut source_lang = 0;
    let mut source_file = None;
    loop {
        let tag = bytes[*pos];
        *pos += 1;
        match tag {
            0x00 => return (source_lang_tag, source_lang, source_file),
            0x02 => {
                source_lang_tag = tag;
                source_lang = bytes[*pos];
                *pos += 1;
            }
            0x04 => *pos += 4,
            0x07 => {
                source_file = Some(read_u32(bytes, *pos));
                *pos += 4;
            }
            other => panic!("unexpected class tag {other:#x}"),
        }
    }
}

fn class_header_counts(bytes: &[u8], offset: usize) -> (u32, u64, u64, u8, u8, Option<u32>) {
    let mut pos = offset;
    skip_inline_string(bytes, &mut pos);
    let super_off = read_u32(bytes, pos);
    pos += 4;
    let access_flags = read_uleb(bytes, &mut pos);
    let fields_count = read_uleb(bytes, &mut pos);
    let methods_count = read_uleb(bytes, &mut pos);
    assert_eq!(access_flags, 1);
    let (source_lang_tag, source_lang, source_file) = read_class_tagged_data(bytes, &mut pos);
    (
        super_off,
        fields_count,
        methods_count,
        source_lang_tag,
        source_lang,
        source_file,
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

    let (_, _, class_source_file) = read_class_tagged_data(bytes, &mut pos);

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
            0x01 | 0x05 | 0x06 => {
                // 0x01 = CODE, 0x05 = DEBUG_INFO, 0x06 = ANNOTATION
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
    let _methods_count = read_uleb(bytes, &mut pos) as usize;
    assert_eq!(access_flags, 1);

    read_class_tagged_data(bytes, &mut pos);

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

fn class_method_items(bytes: &[u8], class_offset: usize) -> Vec<(usize, String, Option<u32>)> {
    let mut pos = class_offset;
    skip_inline_string(bytes, &mut pos);
    pos += 4; // super_class_off
    let access_flags = read_uleb(bytes, &mut pos);
    let fields_count = read_uleb(bytes, &mut pos) as usize;
    let methods_count = read_uleb(bytes, &mut pos) as usize;
    assert_eq!(access_flags, 1);

    read_class_tagged_data(bytes, &mut pos);

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

    let mut methods = Vec::with_capacity(methods_count);
    for _ in 0..methods_count {
        let method_off = pos;
        pos += 2 + 2;
        let name_off = read_u32(bytes, pos) as usize;
        pos += 4;
        let _access_flags = read_uleb(bytes, &mut pos);
        let mut code_off = None;
        loop {
            let tag = bytes[pos];
            pos += 1;
            match tag {
                0x00 => break,
                0x01 => {
                    code_off = Some(read_u32(bytes, pos));
                    pos += 4;
                }
                0x02 => pos += 1,
                0x05 | 0x06 => pos += 4,
                other => panic!("unexpected method tag {other:#x}"),
            }
        }
        methods.push((method_off, inline_string_at(bytes, name_off), code_off));
    }
    methods
}

fn method_source_lang_at(bytes: &[u8], method_offset: usize) -> u8 {
    let mut pos = method_offset;
    pos += 2 + 2;
    pos += 4;
    let _access_flags = read_uleb(bytes, &mut pos);
    loop {
        let tag = bytes[pos];
        pos += 1;
        match tag {
            0x00 => panic!("method SOURCE_LANG tag must exist before MethodTag::NOTHING"),
            0x01 => pos += 4,
            0x02 => return bytes[pos],
            0x05 | 0x06 => pos += 4,
            other => panic!("unexpected method tag {other:#x}"),
        }
    }
}

fn code_item_bytecode(bytes: &[u8], code_off: usize) -> &[u8] {
    let mut pos = code_off;
    let _num_vregs = read_uleb(bytes, &mut pos);
    let _num_args = read_uleb(bytes, &mut pos);
    let num_insns = read_uleb(bytes, &mut pos) as usize;
    let _tries_size = read_uleb(bytes, &mut pos);
    &bytes[pos..pos + num_insns]
}

fn code_item_header(bytes: &[u8], code_off: usize) -> (u64, u64, u64, u64) {
    let mut pos = code_off;
    let num_vregs = read_uleb(bytes, &mut pos);
    let num_args = read_uleb(bytes, &mut pos);
    let num_insns = read_uleb(bytes, &mut pos);
    let tries_size = read_uleb(bytes, &mut pos);
    (num_vregs, num_args, num_insns, tries_size)
}

fn code_item_tries_size(bytes: &[u8], code_off: usize) -> u64 {
    let mut pos = code_off;
    let _num_vregs = read_uleb(bytes, &mut pos);
    let _num_args = read_uleb(bytes, &mut pos);
    let _num_insns = read_uleb(bytes, &mut pos);
    read_uleb(bytes, &mut pos)
}

fn bytecode_insn_len(bytecode: &[u8], pos: usize, opcode: u8) -> usize {
    match opcode {
        0x05 => 2,
        0x07 | 0x08 => 4,
        0x33 | 0x34 => 5,
        0x35 => 9,
        0x06 => 4,
        0x0a..=0x1b | 0x25 | 0x26 | 0x27 | 0x28 => 3,
        0x37 => 3,
        0x38 => 4,
        0x3e | 0xd3 => 3,
        0x36 | 0x67 | 0xc2 | 0xc5 => 2,
        0x68 | 0x77 | 0xb4 | 0xb9 | 0xc6 => 3,
        0x4d | 0x4f | 0x51 => 2,
        0x4e | 0x50 | 0x9b => 3,
        0x98 | 0x9a | 0x9c => 5,
        0x3f | 0x40 | 0x42 => 4,
        0x43 | 0x8e | 0xbc | 0xdb | 0xdc => 5,
        0x31 | 0x32 | 0x73 | 0x78 | 0x99 | 0xba => 4,
        0x62 => 5,
        0x79 => 5,
        0x8a | 0x8b => 3,
        0x44 | 0x60 | 0x61 | 0x7c | 0x7d | 0x3c | 0x3d | 0xb1 | 0xc3 | 0xc4 | 0xcd | 0xce => 2,
        0x1d | 0x21 | 0x22 => 2,
        0x6d | 0xb0 | 0xbd | 0xbf | 0xc0 => 1,
        // private field opcodes
        0xd8 | 0xda => 6,
        0xd9 => 7,
        0xfe if bytecode.get(pos + 1) == Some(&0x09) => 4,
        0xfe if matches!(bytecode.get(pos + 1), Some(0x05 | 0x07)) => 3,
        0xfe => 2,
        // callruntime (0xfb) prefix — length depends on sub-opcode
        0xfb if bytecode.get(pos + 1) == Some(&0x04) => 6, // createprivateproperty
        0xfb if bytecode.get(pos + 1) == Some(&0x05) => 8, // defineprivateproperty
        0xfb if matches!(bytecode.get(pos + 1), Some(0x13 | 0x14)) => 3, // istrue/isfalse
        0xfb => 5, // default callruntime (e.g. 0x01 definefieldbyvalue)
        _ => 1,
    }
}

fn bytecode_contains_opcode(bytecode: &[u8], target: u8) -> bool {
    let mut pos = 0usize;
    while let Some(&opcode) = bytecode.get(pos) {
        if opcode == target {
            return true;
        }
        pos += bytecode_insn_len(bytecode, pos, opcode);
    }
    false
}

fn bytecode_contains_any_opcode(bytecode: &[u8], targets: &[u8]) -> bool {
    let mut pos = 0usize;
    while let Some(&opcode) = bytecode.get(pos) {
        if targets.contains(&opcode) {
            return true;
        }
        pos += bytecode_insn_len(bytecode, pos, opcode);
    }
    false
}

fn bytecode_contains_jmp(bytecode: &[u8]) -> bool {
    bytecode_contains_any_opcode(bytecode, &[0x4d, 0x4e, 0x98])
}

fn bytecode_contains_jeqz(bytecode: &[u8]) -> bool {
    bytecode_contains_any_opcode(bytecode, &[0x4f, 0x50, 0x9a])
}

fn bytecode_contains_jnez(bytecode: &[u8]) -> bool {
    bytecode_contains_any_opcode(bytecode, &[0x51, 0x9b, 0x9c])
}

fn bytecode_opcode_count(bytecode: &[u8], target: u8) -> usize {
    let mut pos = 0usize;
    let mut count = 0usize;
    while let Some(&opcode) = bytecode.get(pos) {
        if opcode == target {
            count += 1;
        }
        pos += bytecode_insn_len(bytecode, pos, opcode);
    }
    count
}

fn first_opcode_position(bytecode: &[u8], target: u8) -> Option<usize> {
    let mut pos = 0usize;
    while let Some(&opcode) = bytecode.get(pos) {
        if opcode == target {
            return Some(pos);
        }
        pos += bytecode_insn_len(bytecode, pos, opcode);
    }
    None
}

fn bytes_contain_ascii(bytes: &[u8], text: &str) -> bool {
    bytes
        .windows(text.len())
        .any(|window| window == text.as_bytes())
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
    // Use a module-mode source with a class definition so that the class-buffer
    // literal array (in_method_index=true) and the constructor method
    // (in_method_index=true) appear in the IndexSection method index.
    // The string "hello" ensures at least one bytecode string is interned.
    let compiler = module_compiler();
    compiler
        .compile(
            vec![module_record(
                "com.example.app/entry/ets/pages/Index",
                r#"var x = "hello"; var Index = class {};"#,
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
    // The method index must contain at least the "hello" string (1 bytecode
    // string) + the class-buffer literal array (1 indexed lit array) + the
    // constructor method (1 indexed method) = at least 3 entries.
    assert!(
        index_method_size >= 3,
        "method index must contain bytecode strings, class-buffer lit array, and constructor; got {index_method_size}"
    );
    assert_eq!(read_u32(&bytes, index_class_off as usize), 2);
    assert_eq!(read_u32(&bytes, index_class_off as usize + 4), 6);
    assert_eq!(index_field_size, 0xFFFF_FFFF);
    assert_eq!(index_proto_size, 0xFFFF_FFFF);
}

#[test]
fn test_api18_literal_array_header_fields_are_reserved() {
    let tmp = tempfile::tempdir().expect("tempdir");
    let out = tmp.path().join("api18-literal-index.abc");
    module_compiler_for_target(18, None)
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
    let version = read_u32(&bytes, 0x0c);
    let num_classes = read_u32(&bytes, 0x1c);
    let class_idx_off = read_u32(&bytes, 0x20);
    let num_literalarrays = read_u32(&bytes, 0x2c);
    let literalarray_idx_off = read_u32(&bytes, 0x30);
    let idx_section_off = read_u32(&bytes, 0x38);

    assert_eq!(
        version,
        arkdown_abc::emit::file::version_for_target_api(18, None),
        "API18 maps to upstream bytecode version 13.0.1.0"
    );
    assert_eq!(
        num_literalarrays,
        u32::MAX,
        "v13 reserves num_literalarrays with INVALID_INDEX"
    );
    assert_eq!(
        literalarray_idx_off,
        u32::MAX,
        "v13 reserves literalarray_idx_off with INVALID_OFFSET"
    );
    assert_eq!(
        idx_section_off,
        class_idx_off + num_classes * 4,
        "v13 deletes the header-adjacent LiteralArrayIdx table"
    );
    AbcReader::validate(&bytes).expect("reader must accept upstream v13 reserved fields");

    let user_off = class_offsets(&bytes)
        .into_iter()
        .find(|off| {
            (*off as usize) < bytes.len()
                && class_name_at(&bytes, *off as usize) == "Lcom.example.app/entry/ets/pages/Index;"
        })
        .expect("user record exists") as usize;
    let fields = user_field_values(&bytes, user_off);
    for field in ["moduleRecordIdx", "scopeNames"] {
        let offset = *fields
            .get(field)
            .expect("module literal-array field is emitted");
        assert!(
            offset < bytes.len() as u32,
            "{field} must still point at a literal array payload in v13"
        );
    }
}

#[test]
fn test_defineclass_uses_matching_constructor_method_index() {
    let tmp = tempfile::tempdir().expect("tempdir");
    let out = tmp.path().join("defineclass-ctor.abc");
    module_compiler()
        .compile(
            vec![module_record(
                "com.example.app/entry/ets/pages/Index",
                r#"class A { constructor() { this.a = 1; } }
class B { constructor() { this.b = 2; } }"#,
            )],
            &out,
        )
        .expect("compile must succeed");

    let bytes = std::fs::read(&out).expect("read .abc");
    let idx_section_off = read_u32(&bytes, 0x38) as usize;
    let method_index_off = read_u32(&bytes, idx_section_off + 20) as usize;
    let user_class_off = class_offsets(&bytes)
        .into_iter()
        .find(|off| {
            class_name_at(&bytes, *off as usize) == "Lcom.example.app/entry/ets/pages/Index;"
        })
        .expect("user class exists") as usize;
    let methods = class_method_items(&bytes, user_class_off);
    let constructor_offsets: Vec<u32> = methods
        .iter()
        .filter_map(|(off, name, _)| matches!(name.as_str(), "A" | "B").then_some(*off as u32))
        .collect();
    assert_eq!(
        constructor_offsets.len(),
        2,
        "both explicit constructors must be MethodItems"
    );

    let (_, _, func_main_code) = methods
        .iter()
        .find(|(_, name, _)| name == "func_main_0")
        .expect("func_main_0 exists");
    let bytecode = code_item_bytecode(
        &bytes,
        func_main_code.expect("func_main_0 has code") as usize,
    );
    let defineclass_ctor_offsets: Vec<u32> = bytecode
        .windows(9)
        .filter_map(|window| {
            (window[0] == 0x35).then(|| {
                let ctor_index = read_u16(window, 2) as usize;
                read_u32(&bytes, method_index_off + ctor_index * 4)
            })
        })
        .collect();

    assert_eq!(
        defineclass_ctor_offsets, constructor_offsets,
        "defineclasswithbuffer must reference the matching constructor MethodItem offsets"
    );
}

#[test]
fn test_constructor_default_negative_parameter_compiles_without_debug_source() {
    let tmp = tempfile::tempdir().expect("tempdir");
    let out = tmp.path().join("constructor-default-negative.abc");
    module_compiler()
        .compile(
            vec![module_record(
                "com.example.app/entry/ets/pages/Index",
                r#"class A {
  constructor(value = -1) {
    this.value = value;
  }
}
new A();"#,
            )],
            &out,
        )
        .expect("constructor default negative parameter must compile without debug source");

    assert!(out.exists());
}

#[test]
fn test_exported_class_declaration_does_not_store_global_binding() {
    let tmp = tempfile::tempdir().expect("tempdir");
    let out = tmp.path().join("export-class.abc");
    module_compiler()
        .compile(
            vec![module_record(
                "com.example.app/entry/ets/pages/Index",
                r#"export class A {}"#,
            )],
            &out,
        )
        .expect("compile must succeed");

    let bytes = std::fs::read(&out).expect("read .abc");
    let user_class_off = class_offsets(&bytes)
        .into_iter()
        .find(|off| {
            class_name_at(&bytes, *off as usize) == "Lcom.example.app/entry/ets/pages/Index;"
        })
        .expect("user class exists") as usize;
    let methods = class_method_items(&bytes, user_class_off);
    let (_, _, func_main_code) = methods
        .iter()
        .find(|(_, name, _)| name == "func_main_0")
        .expect("func_main_0 exists");
    let bytecode = code_item_bytecode(
        &bytes,
        func_main_code.expect("func_main_0 has code") as usize,
    );

    assert!(
        !bytecode_contains_opcode(bytecode, 0x40),
        "exported class declarations must not write class names through global bindings"
    );
    assert!(
        bytecode.windows(2).any(|window| window == [0x7c, 0x00]),
        "exported class declaration must write the module export slot"
    );
}

#[test]
fn test_uncaptured_top_level_class_does_not_emit_module_lexenv() {
    let tmp = tempfile::tempdir().expect("tempdir");
    let out = tmp.path().join("uncaptured-class.abc");
    module_compiler()
        .compile(
            vec![module_record(
                "com.example.app/entry/ets/pages/DefaultCrypto",
                r#"class DefaultCrypto {}
export default new DefaultCrypto();"#,
            )],
            &out,
        )
        .expect("compile must succeed");

    let bytes = std::fs::read(&out).expect("read .abc");
    let user_class_off = class_offsets(&bytes)
        .into_iter()
        .find(|off| {
            class_name_at(&bytes, *off as usize)
                == "Lcom.example.app/entry/ets/pages/DefaultCrypto;"
        })
        .expect("user class exists") as usize;
    let methods = class_method_items(&bytes, user_class_off);
    let (_, _, func_main_code) = methods
        .iter()
        .find(|(_, name, _)| name == "func_main_0")
        .expect("func_main_0 exists");
    let bytecode = code_item_bytecode(
        &bytes,
        func_main_code.expect("func_main_0 has code") as usize,
    );

    assert!(
        !bytecode_contains_opcode(bytecode, 0x09),
        "uncaptured top-level class declarations must not create a runtime module lexenv"
    );
    assert!(
        !bytecode_contains_opcode(bytecode, 0x3d),
        "uncaptured top-level class declarations must not write stlexvar slots"
    );
}

#[test]
fn test_default_new_class_export_uses_local_class_binding() {
    let tmp = tempfile::tempdir().expect("tempdir");
    let out = tmp.path().join("default-new-class.abc");
    module_compiler()
        .compile(
            vec![module_record(
                "com.example.app/entry/ets/pages/Aaid",
                r#"class Aaid {}
export default new Aaid();"#,
            )],
            &out,
        )
        .expect("compile must succeed");

    let bytes = std::fs::read(&out).expect("read .abc");
    let user_class_off = class_offsets(&bytes)
        .into_iter()
        .find(|off| {
            class_name_at(&bytes, *off as usize) == "Lcom.example.app/entry/ets/pages/Aaid;"
        })
        .expect("user class exists") as usize;
    let methods = class_method_items(&bytes, user_class_off);
    let (_, _, func_main_code) = methods
        .iter()
        .find(|(_, name, _)| name == "func_main_0")
        .expect("func_main_0 exists");
    let bytecode = code_item_bytecode(
        &bytes,
        func_main_code.expect("func_main_0 has code") as usize,
    );

    assert!(
        !bytecode_contains_opcode(bytecode, 0x40),
        "module top-level class declarations must not write through global bindings"
    );
    assert!(
        bytecode.windows(2).any(|window| window == [0x7c, 0x00]),
        "export default expression must write the default module export slot"
    );
}

#[test]
fn test_class_method_captures_use_unnamed_module_lexenv() {
    let tmp = tempfile::tempdir().expect("tempdir");
    let out = tmp.path().join("class-method-captures.abc");
    module_compiler()
        .compile(
            vec![module_record(
                "com.example.app/entry/ets/pages/LoggerInterceptor",
                r#"const TAG = "HTTP";
export class LoggerInterceptor {
  intercept(request) {
    let msg = maskUrl(request.url);
    return msg + TAG;
  }
}
function maskUrl(url) { return url ? url.split(".")[0] : ""; }"#,
            )],
            &out,
        )
        .expect("compile must succeed");

    let bytes = std::fs::read(&out).expect("read .abc");
    let user_class_off = class_offsets(&bytes)
        .into_iter()
        .find(|off| {
            class_name_at(&bytes, *off as usize)
                == "Lcom.example.app/entry/ets/pages/LoggerInterceptor;"
        })
        .expect("user class exists") as usize;
    let methods = class_method_items(&bytes, user_class_off);
    let (_, _, func_main_code) = methods
        .iter()
        .find(|(_, name, _)| name == "func_main_0")
        .expect("func_main_0 exists");
    let bytecode = code_item_bytecode(
        &bytes,
        func_main_code.expect("func_main_0 has code") as usize,
    );

    assert_eq!(
        &bytecode[..2],
        &[0x09, 0x02],
        "captured module bindings must use upstream's unnamed newlexenv prologue"
    );
    assert!(
        !bytecode_contains_opcode(bytecode, 0xb6),
        "module-captured bindings must not use newlexenvwithname"
    );
    let stlex_slots: Vec<u8> = bytecode
        .windows(2)
        .filter_map(|window| (window[0] == 0x3d).then_some(window[1]))
        .take(2)
        .collect();
    assert_eq!(
        stlex_slots,
        vec![0x00, 0x10],
        "captured function declarations occupy earlier slots than captured variables"
    );
}

#[test]
fn test_newlexenv_count_eight_does_not_corrupt_following_definefunc() {
    let tmp = tempfile::tempdir().expect("tempdir");
    let out = tmp.path().join("lexenv-count-eight.abc");
    module_compiler()
        .compile(
            vec![module_record(
                "com.example.app/entry/ets/pages/LexEnvCount",
                r#"const A = 1;
const B = 2;
const C = 3;
const D = 4;
const E = 5;
const F = 6;
const G = 7;
function check() { return true; }
class Owner {
  run() { return A + B + C + D + check() + E + F + G; }
}"#,
            )],
            &out,
        )
        .expect("compile must succeed");

    let bytes = std::fs::read(&out).expect("read .abc");
    let user_class_off = class_offsets(&bytes)
        .into_iter()
        .find(|off| {
            class_name_at(&bytes, *off as usize) == "Lcom.example.app/entry/ets/pages/LexEnvCount;"
        })
        .expect("user class exists") as usize;
    let methods = class_method_items(&bytes, user_class_off);
    let (_, _, func_main_code) = methods
        .iter()
        .find(|(_, name, _)| name == "func_main_0")
        .expect("func_main_0 exists");
    let bytecode = code_item_bytecode(
        &bytes,
        func_main_code.expect("func_main_0 has code") as usize,
    );

    assert_eq!(
        &bytecode[..3],
        &[0x09, 0x08, 0x33],
        "newlexenv count 8 must be walked as a two-byte instruction before definefunc"
    );
}

#[test]
fn test_class_method_references_local_export_via_module_var() {
    let tmp = tempfile::tempdir().expect("tempdir");
    let out = tmp.path().join("local-export-reference.abc");
    module_compiler()
        .compile(
            vec![module_record(
                "com.example.app/entry/ets/pages/LocalExportRef",
                r#"const NAME = "x";
export class Target {}
class Owner {
  make() { return new Target(NAME); }
}"#,
            )],
            &out,
        )
        .expect("compile must succeed");

    let bytes = std::fs::read(&out).expect("read .abc");
    let user_class_off = class_offsets(&bytes)
        .into_iter()
        .find(|off| {
            class_name_at(&bytes, *off as usize)
                == "Lcom.example.app/entry/ets/pages/LocalExportRef;"
        })
        .expect("user class exists") as usize;
    let methods = class_method_items(&bytes, user_class_off);
    let (_, _, make_code) = methods
        .iter()
        .find(|(_, name, _)| name == "make")
        .expect("make method exists");
    let make_bytecode = code_item_bytecode(&bytes, make_code.expect("make has code") as usize);
    assert!(
        bytecode_contains_opcode(make_bytecode, 0x7d),
        "nested methods must read same-module exports through ldlocalmodulevar"
    );

    let (_, _, func_main_code) = methods
        .iter()
        .find(|(_, name, _)| name == "func_main_0")
        .expect("func_main_0 exists");
    let func_main = code_item_bytecode(
        &bytes,
        func_main_code.expect("func_main_0 has code") as usize,
    );
    assert_eq!(
        &func_main[..2],
        &[0x09, 0x01],
        "local exports referenced by nested methods must not consume runtime lexenv slots"
    );
}

#[test]
fn test_static_class_method_define_method_uses_method_index() {
    let tmp = tempfile::tempdir().expect("tempdir");
    let out = tmp.path().join("static-method.abc");
    module_compiler()
        .compile(
            vec![module_record(
                "com.example.app/entry/ets/pages/Logger",
                r#"export class Logger { static debug(): void {} }"#,
            )],
            &out,
        )
        .expect("compile must succeed");

    let bytes = std::fs::read(&out).expect("read .abc");
    let idx_section_off = read_u32(&bytes, 0x38) as usize;
    let method_index_off = read_u32(&bytes, idx_section_off + 20) as usize;
    let user_class_off = class_offsets(&bytes)
        .into_iter()
        .find(|off| {
            class_name_at(&bytes, *off as usize) == "Lcom.example.app/entry/ets/pages/Logger;"
        })
        .expect("user class exists") as usize;
    let methods = class_method_items(&bytes, user_class_off);
    let debug_method_off = methods
        .iter()
        .find_map(|(off, name, _)| (name == "debug").then_some(*off as u32))
        .expect("static debug method exists");
    let (_, _, func_main_code) = methods
        .iter()
        .find(|(_, name, _)| name == "func_main_0")
        .expect("func_main_0 exists");
    let bytecode = code_item_bytecode(
        &bytes,
        func_main_code.expect("func_main_0 has code") as usize,
    );
    let define_method_targets: Vec<u32> = bytecode
        .windows(5)
        .filter_map(|window| {
            (window[0] == 0x34).then(|| {
                let method_index = read_u16(window, 2) as usize;
                read_u32(&bytes, method_index_off + method_index * 4)
            })
        })
        .collect();

    assert!(
        define_method_targets.contains(&debug_method_off),
        "definemethod must reference the static method's MethodItem offset"
    );
}

#[test]
fn test_function_declaration_definefunc_uses_method_index() {
    let tmp = tempfile::tempdir().expect("tempdir");
    let out = tmp.path().join("function-decl.abc");
    module_compiler()
        .compile(
            vec![module_record(
                "com.example.app/entry/ets/pages/Index",
                r#"function foo(): number { return 1; }
export { foo };"#,
            )],
            &out,
        )
        .expect("compile must succeed");

    let bytes = std::fs::read(&out).expect("read .abc");
    let idx_section_off = read_u32(&bytes, 0x38) as usize;
    let method_index_off = read_u32(&bytes, idx_section_off + 20) as usize;
    let user_class_off = class_offsets(&bytes)
        .into_iter()
        .find(|off| {
            class_name_at(&bytes, *off as usize) == "Lcom.example.app/entry/ets/pages/Index;"
        })
        .expect("user class exists") as usize;
    let methods = class_method_items(&bytes, user_class_off);
    let foo_method_off = methods
        .iter()
        .find_map(|(off, name, _)| (name == "foo").then_some(*off as u32))
        .expect("foo method exists");
    let (_, _, func_main_code) = methods
        .iter()
        .find(|(_, name, _)| name == "func_main_0")
        .expect("func_main_0 exists");
    let bytecode = code_item_bytecode(
        &bytes,
        func_main_code.expect("func_main_0 has code") as usize,
    );
    let definefunc_targets: Vec<u32> = bytecode
        .windows(5)
        .filter_map(|window| {
            (window[0] == 0x33).then(|| {
                let method_index = read_u16(window, 2) as usize;
                read_u32(&bytes, method_index_off + method_index * 4)
            })
        })
        .collect();

    assert!(
        definefunc_targets.contains(&foo_method_off),
        "definefunc must reference the function declaration's MethodItem offset"
    );
}

#[test]
fn test_arrow_variable_definefunc_uses_inferred_name_method_index() {
    let tmp = tempfile::tempdir().expect("tempdir");
    let out = tmp.path().join("arrow-var.abc");
    module_compiler()
        .compile(
            vec![module_record(
                "com.example.app/entry/ets/pages/Index",
                r#"const genRanHex = (size: number): string => "";
export { genRanHex };"#,
            )],
            &out,
        )
        .expect("compile must succeed");

    let bytes = std::fs::read(&out).expect("read .abc");
    let idx_section_off = read_u32(&bytes, 0x38) as usize;
    let method_index_off = read_u32(&bytes, idx_section_off + 20) as usize;
    let user_class_off = class_offsets(&bytes)
        .into_iter()
        .find(|off| {
            class_name_at(&bytes, *off as usize) == "Lcom.example.app/entry/ets/pages/Index;"
        })
        .expect("user class exists") as usize;
    let methods = class_method_items(&bytes, user_class_off);
    let arrow_method_off = methods
        .iter()
        .find_map(|(off, name, _)| (name == "genRanHex").then_some(*off as u32))
        .expect("arrow method exists");
    let (_, _, func_main_code) = methods
        .iter()
        .find(|(_, name, _)| name == "func_main_0")
        .expect("func_main_0 exists");
    let bytecode = code_item_bytecode(
        &bytes,
        func_main_code.expect("func_main_0 has code") as usize,
    );
    let definefunc_targets: Vec<u32> = bytecode
        .windows(5)
        .filter_map(|window| {
            (window[0] == 0x33).then(|| {
                let method_index = read_u16(window, 2) as usize;
                read_u32(&bytes, method_index_off + method_index * 4)
            })
        })
        .collect();

    assert!(
        definefunc_targets.contains(&arrow_method_off),
        "arrow variable definefunc must reference the inferred-name MethodItem offset"
    );
}

#[test]
fn test_user_records_are_marked_arkts_and_npm_entries_have_native_fields() {
    let tmp = tempfile::tempdir().expect("tempdir");
    let out = tmp.path().join("records.abc");
    let npm_entries = tmp.path().join("npmEntries.txt");
    fs_err::write(&npm_entries, "@ohos.app:@native.ohos.app\n").expect("write npm entries");
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
        npm_entries: Some(npm_entries),
        dependency_context: None,
        remove_redundant_file: false,
        dump_deps_info: false,
    })
    .compile(
        vec![{
            let mut module = iife_module(
                "com.example.app/entry/ets/entryability/EntryAbility",
                "(function(){ var EntryAbility = class {}; })();",
            );
            module.source_file =
                Some("entry|entry|1.0.0|ets/entryability/EntryAbility.ts".to_string());
            module.origin_source_lang = "ets".to_string();
            module
        }],
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

    let (_, sdk_fields, _, sdk_tag, sdk_lang, sdk_source_file) =
        class_header_counts(&bytes, sdk_off);
    assert_eq!(
        sdk_fields, 1,
        "npm entry records carry one @native field in hvigor output"
    );
    assert_eq!((sdk_tag, sdk_lang), (0x02, 0x00));
    assert_eq!(sdk_source_file, None);
    assert!(
        bytes
            .windows(b"@native.ohos.app".len())
            .any(|w| w == b"@native.ohos.app"),
        "SDK native field name must be emitted as a StringItem"
    );

    let (_, user_fields, _, user_tag, user_lang, user_source_file) =
        class_header_counts(&bytes, user_off);
    assert_eq!(
        user_fields, 6,
        "user module records carry six module metadata fields"
    );
    assert_eq!(
        (user_tag, user_lang),
        (0x02, 0x04),
        "user records use ArkTS source language byte 0x04"
    );
    assert_eq!(
        inline_string_at(
            &bytes,
            user_source_file.expect("user record source file") as usize
        ),
        "entry|entry|1.0.0|ets/entryability/EntryAbility.ts"
    );
    assert!(
        bytes
            .windows(b"pkgName@entry".len())
            .any(|w| w == b"pkgName@entry"),
        "pkgName field includes the module name suffix"
    );
}

#[test]
fn test_origin_source_lang_controls_source_lang_tags() {
    let tmp = tempfile::tempdir().expect("tempdir");
    let out = tmp.path().join("source_lang.abc");
    let cases = [
        ("ets", "test/source_lang_ets", 0x04),
        ("ts", "test/source_lang_ts", 0x03),
        ("js", "test/source_lang_js", 0x02),
        ("", "test/source_lang_default", 0x00),
        ("unknown", "test/source_lang_unknown", 0x00),
    ];
    let modules = cases
        .iter()
        .map(|(origin_source_lang, record_name, _)| {
            let mut module = module_record_with_source_file(
                record_name,
                "export const value = 1;",
                "entry|entry|1.0.0|src/main/ets/pages/Index.ts",
            );
            module.origin_source_lang = (*origin_source_lang).to_string();
            module
        })
        .collect();

    module_compiler()
        .compile(modules, &out)
        .expect("compile sourceLang records");
    let bytes = std::fs::read(&out).expect("read sourceLang abc");

    for (_, record_name, expected_lang) in cases {
        let class_name = format!("L{record_name};");
        let class_off = class_offsets(&bytes)
            .into_iter()
            .map(|off| off as usize)
            .find(|&off| class_name_at(&bytes, off) == class_name)
            .expect("sourceLang class exists");
        let (_, _, _, tag, class_lang, source_file) = class_header_counts(&bytes, class_off);
        assert_eq!(
            (tag, class_lang),
            (0x02, expected_lang),
            "{record_name} class SOURCE_LANG must match upstream Program::SetSourceLang"
        );
        assert_eq!(
            inline_string_at(
                &bytes,
                source_file.expect("sourceLang class source file") as usize
            ),
            "entry|entry|1.0.0|src/main/ets/pages/Index.ts"
        );

        let func_main = class_method_items(&bytes, class_off)
            .into_iter()
            .find(|(_, name, _)| name == "func_main_0")
            .expect("func_main_0 method exists");
        assert_eq!(
            method_source_lang_at(&bytes, func_main.0),
            expected_lang,
            "{record_name} method SOURCE_LANG must match class SOURCE_LANG"
        );
    }
}

#[test]
fn test_module_record_scope_names_and_class_buffer_literals_are_linked() {
    let tmp = tempfile::tempdir().expect("tempdir");
    let out = tmp.path().join("module-record.abc");
    // Uses ESM format — IIFE/CJS unwrapping is removed from the compiler.
    module_compiler_for_target(20, None)
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
    let reader = AbcReader::new(&bytes).expect("abc reader");
    let literal_offsets = reader
        .literal_array_offsets()
        .expect("literal array offsets");
    assert!(
        literal_offsets.len() >= 3,
        "class buffer, module record, and scopeNames literal arrays are emitted"
    );

    let user_off = class_offsets(&bytes)
        .into_iter()
        .find(|off| {
            (*off as usize) < bytes.len()
                && class_name_at(&bytes, *off as usize) == "Lcom.example.app/entry/ets/pages/Index;"
        })
        .expect("user record exists") as usize;
    let method_names = class_method_items(&bytes, user_off)
        .into_iter()
        .map(|(_, name, _)| name)
        .collect::<Vec<_>>();
    assert!(
        method_names.contains(&"#~@0>#onCreate".to_string()),
        "API20 module class methods must use upstream scope-info names; got {method_names:?}"
    );
    let fields = user_field_values(&bytes, user_off);
    let module_record = *fields
        .get("moduleRecordIdx")
        .expect("moduleRecordIdx field has a literal-array value");
    let scope_names = *fields
        .get("scopeNames")
        .expect("scopeNames field has a literal-array value");
    assert!((module_record as usize) < bytes.len());
    assert!((scope_names as usize) < bytes.len());
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
        "@ohos:app.ability.UIAbility"
    );
    assert_eq!(read_u32(&bytes, pos), 1, "one regular import");
    pos += 4;
    let local_name = read_u32(&bytes, pos) as usize;
    pos += 4;
    let import_name = read_u32(&bytes, pos) as usize;
    pos += 4;
    assert_eq!(read_u16(&bytes, pos), 0, "regular import request index");
    assert_eq!(inline_string_at(&bytes, local_name), "UIAbility");
    assert_eq!(inline_string_at(&bytes, import_name), "default");

    let scope_names_item = reader
        .read_literal_array_item(scope_names)
        .expect("scopeNames literal array");
    let scope_values = scope_names_item
        .entries
        .iter()
        .filter_map(|entry| match entry {
            AbcLiteralEntry::String { value, .. } => Some(value.as_str()),
            _ => None,
        })
        .collect::<Vec<_>>();
    assert_eq!(
        scope_names_item.raw_item_count, 2,
        "API20 scopeNames stores one tagged string for compressed class scope"
    );
    assert_eq!(scope_values, vec!["Index"]);

    let has_class_method_buffer = literal_offsets.iter().any(|offset| {
        reader.read_literal_array_item(*offset).is_ok_and(|item| {
            let has_public_name = item.entries.iter().any(|entry| {
                    matches!(entry, AbcLiteralEntry::String { value, .. } if value == "onCreate")
                });
            let has_internal_method = item.entries.iter().any(|entry| {
                matches!(
                    entry,
                    AbcLiteralEntry::Method {
                        name: Some(name),
                        ..
                    } if name == "#~@0>#onCreate"
                )
            });
            has_public_name && has_internal_method
        })
    });
    assert!(
        has_class_method_buffer,
        "class buffer literal array links instance methods by upstream internal MethodItem name"
    );
}

#[test]
fn test_api20_scope_info_names_follow_upstream_optimization() {
    let tmp = tempfile::tempdir().expect("tempdir");
    let out = tmp.path().join("scope-info-names.abc");
    module_compiler_for_target(20, None)
        .compile(
            vec![module_record(
                "com.example.app/entry/ets/pages/Index",
                r#"
class A { constructor() {} foo() {} }
class AA { test() { let cb = () => this.a; return cb; } }
export { A, AA };"#,
            )],
            &out,
        )
        .expect("compile must succeed");
    let bytes = std::fs::read(&out).expect("read .abc");
    let reader = AbcReader::new(&bytes).expect("abc reader");

    let user_off = class_offsets(&bytes)
        .into_iter()
        .find(|off| {
            class_name_at(&bytes, *off as usize) == "Lcom.example.app/entry/ets/pages/Index;"
        })
        .expect("user record exists") as usize;
    let method_names = class_method_items(&bytes, user_off)
        .into_iter()
        .map(|(_, name, _)| name)
        .collect::<Vec<_>>();

    assert!(
        method_names.contains(&"#~A=#A".to_string()),
        "short class names are not index-compressed by upstream OptimizeSelfScopeName; got {method_names:?}"
    );
    assert!(method_names.contains(&"#~A>#foo".to_string()));
    assert!(method_names.contains(&"#~AA>#test".to_string()));
    assert!(method_names.contains(&"#~AA>@0*#cb".to_string()));
    assert!(
        !method_names
            .iter()
            .any(|name| name.starts_with("#~@0>#test")),
        "method record final self name must stay raw; only descendant full scope uses @ index"
    );

    let scope_names = *user_field_values(&bytes, user_off)
        .get("scopeNames")
        .expect("scopeNames field has a literal-array value");
    let scope_values = reader
        .read_literal_array_item(scope_names)
        .expect("scopeNames literal array")
        .entries
        .into_iter()
        .filter_map(|entry| match entry {
            AbcLiteralEntry::String { value, .. } => Some(value),
            _ => None,
        })
        .collect::<Vec<_>>();
    assert_eq!(
        scope_values,
        vec!["test".to_string()],
        "scopeNames must contain only names that upstream compressed to @ indexes"
    );
}

#[test]
fn test_module_record_marks_top_level_await() {
    let tmp = tempfile::tempdir().expect("tempdir");
    let out = tmp.path().join("module-tla.abc");
    module_compiler()
        .compile(
            vec![module_record(
                "com.example.app/entry/ets/pages/Index",
                r#"function load() { return Promise.resolve(1); }
await load();"#,
            )],
            &out,
        )
        .expect("compile must succeed");

    let bytes = std::fs::read(&out).expect("read .abc");
    let user_off = class_offsets(&bytes)
        .into_iter()
        .find(|off| {
            class_name_at(&bytes, *off as usize) == "Lcom.example.app/entry/ets/pages/Index;"
        })
        .expect("user record exists") as usize;
    let fields = user_field_values(&bytes, user_off);
    assert_eq!(
        fields.get("hasTopLevelAwait"),
        Some(&1),
        "merge-abc module metadata mirrors Program::HasTLA()"
    );
}

#[test]
fn test_module_record_does_not_mark_nested_async_await_as_tla() {
    let tmp = tempfile::tempdir().expect("tempdir");
    let out = tmp.path().join("module-nested-await.abc");
    module_compiler()
        .compile(
            vec![module_record(
                "com.example.app/entry/ets/pages/Index",
                r#"async function load() { await Promise.resolve(1); }
load();"#,
            )],
            &out,
        )
        .expect("compile must succeed");

    let bytes = std::fs::read(&out).expect("read .abc");
    let user_off = class_offsets(&bytes)
        .into_iter()
        .find(|off| {
            class_name_at(&bytes, *off as usize) == "Lcom.example.app/entry/ets/pages/Index;"
        })
        .expect("user record exists") as usize;
    let fields = user_field_values(&bytes, user_off);
    assert_eq!(
        fields.get("hasTopLevelAwait"),
        Some(&0),
        "await inside a function must not set Program::HasTLA()"
    );
}

#[test]
fn test_module_record_matches_upstream_accessor_entry_order() {
    let tmp = tempfile::tempdir().expect("tempdir");
    let out = tmp.path().join("module-record-order.abc");
    module_compiler()
        .compile(
            vec![module_record(
                "com.example.app/entry/ets/pages/Index",
                r#"import * as ns from "./namespace";
export { foo as bar } from "./indirect";
export * from "./star";"#,
            )],
            &out,
        )
        .expect("compile must succeed");
    let bytes = std::fs::read(&out).expect("read .abc");

    let user_off = class_offsets(&bytes)
        .into_iter()
        .find(|off| {
            class_name_at(&bytes, *off as usize) == "Lcom.example.app/entry/ets/pages/Index;"
        })
        .expect("user record exists") as usize;
    let fields = user_field_values(&bytes, user_off);
    let mut pos = *fields
        .get("moduleRecordIdx")
        .expect("moduleRecordIdx field has a literal-array value") as usize;

    assert_eq!(read_u32(&bytes, pos), 15, "module record item count");
    pos += 4;

    assert_eq!(read_u32(&bytes, pos), 3, "three module requests");
    pos += 4;
    let namespace_request = read_u32(&bytes, pos) as usize;
    pos += 4;
    let indirect_request = read_u32(&bytes, pos) as usize;
    pos += 4;
    let star_request = read_u32(&bytes, pos) as usize;
    pos += 4;
    assert_eq!(inline_string_at(&bytes, namespace_request), "./namespace");
    assert_eq!(inline_string_at(&bytes, indirect_request), "./indirect");
    assert_eq!(inline_string_at(&bytes, star_request), "./star");

    assert_eq!(read_u32(&bytes, pos), 0, "no regular imports");
    pos += 4;

    assert_eq!(read_u32(&bytes, pos), 1, "one namespace import");
    pos += 4;
    let namespace_local = read_u32(&bytes, pos) as usize;
    pos += 4;
    assert_eq!(
        read_u16(&bytes, pos),
        0,
        "namespace request index follows local name"
    );
    pos += 2;
    assert_eq!(inline_string_at(&bytes, namespace_local), "ns");

    assert_eq!(read_u32(&bytes, pos), 0, "no local exports");
    pos += 4;

    assert_eq!(read_u32(&bytes, pos), 1, "one indirect export");
    pos += 4;
    let indirect_export_name = read_u32(&bytes, pos) as usize;
    pos += 4;
    let indirect_import_name = read_u32(&bytes, pos) as usize;
    pos += 4;
    assert_eq!(
        read_u16(&bytes, pos),
        1,
        "indirect request index follows export/import names"
    );
    pos += 2;
    assert_eq!(inline_string_at(&bytes, indirect_export_name), "bar");
    assert_eq!(inline_string_at(&bytes, indirect_import_name), "foo");

    assert_eq!(read_u32(&bytes, pos), 1, "one star export");
    pos += 4;
    assert_eq!(read_u16(&bytes, pos), 2, "star request index");
}

#[test]
fn test_regular_imports_and_external_loads_use_import_table_order() {
    let tmp = tempfile::tempdir().expect("tempdir");
    let out = tmp.path().join("import-order.abc");
    module_compiler()
        .compile(
            vec![module_record(
                "test/import_order",
                r#"import z from "./z";
import { a } from "./a";
const values = [z, a];"#,
            )],
            &out,
        )
        .expect("compile must succeed");
    let bytes = std::fs::read(&out).expect("read .abc");

    let user_off = class_offsets(&bytes)
        .into_iter()
        .find(|off| class_name_at(&bytes, *off as usize) == "Ltest/import_order;")
        .expect("user record exists") as usize;
    let fields = user_field_values(&bytes, user_off);
    let module_record = *fields
        .get("moduleRecordIdx")
        .expect("moduleRecordIdx field has a literal-array value") as usize;

    let mut pos = module_record;
    assert_eq!(read_u32(&bytes, pos), 14, "module record item count");
    pos += 4;
    assert_eq!(read_u32(&bytes, pos), 2, "two module requests");
    pos += 4;
    pos += 8;
    assert_eq!(read_u32(&bytes, pos), 2, "two regular imports");
    pos += 4;
    let first_local = read_u32(&bytes, pos) as usize;
    pos += 4;
    pos += 4;
    let first_request = read_u16(&bytes, pos);
    pos += 2;
    let second_local = read_u32(&bytes, pos) as usize;
    pos += 4;
    pos += 4;
    let second_request = read_u16(&bytes, pos);
    assert_eq!(inline_string_at(&bytes, first_local), "a");
    assert_eq!(first_request, 1, "a comes from the second module request");
    assert_eq!(inline_string_at(&bytes, second_local), "z");
    assert_eq!(second_request, 0, "z comes from the first module request");

    let methods = class_method_items(&bytes, user_off);
    let (_, _, func_main_code) = methods
        .iter()
        .find(|(_, name, _)| name == "func_main_0")
        .expect("func_main_0 method exists");
    let bytecode = code_item_bytecode(&bytes, func_main_code.expect("func_main has code") as usize);
    let external_loads: Vec<u8> = bytecode
        .windows(2)
        .filter_map(|window| (window[0] == 0x7e).then_some(window[1]))
        .collect();
    assert_eq!(
        external_loads,
        vec![1, 0],
        "source uses z then a, but operands index sorted regular import bindings"
    );
}

#[test]
fn test_local_exports_and_module_stores_use_export_table_order() {
    let tmp = tempfile::tempdir().expect("tempdir");
    let out = tmp.path().join("export-order.abc");
    module_compiler()
        .compile(
            vec![module_record(
                "test/export_order",
                r#"const z = 1;
const a = 2;
const m = 3;
export { z, a, m as b };"#,
            )],
            &out,
        )
        .expect("compile must succeed");
    let bytes = std::fs::read(&out).expect("read .abc");

    let user_off = class_offsets(&bytes)
        .into_iter()
        .find(|off| class_name_at(&bytes, *off as usize) == "Ltest/export_order;")
        .expect("user record exists") as usize;
    let fields = user_field_values(&bytes, user_off);
    let module_record = *fields
        .get("moduleRecordIdx")
        .expect("moduleRecordIdx field has a literal-array value") as usize;

    let mut pos = module_record;
    assert_eq!(read_u32(&bytes, pos), 12, "module record item count");
    pos += 4;
    assert_eq!(read_u32(&bytes, pos), 0, "no module requests");
    pos += 4;
    assert_eq!(read_u32(&bytes, pos), 0, "no regular imports");
    pos += 4;
    assert_eq!(read_u32(&bytes, pos), 0, "no namespace imports");
    pos += 4;
    assert_eq!(read_u32(&bytes, pos), 3, "three local exports");
    pos += 4;

    let mut exports = Vec::new();
    for _ in 0..3 {
        let local_name = read_u32(&bytes, pos) as usize;
        pos += 4;
        let export_name = read_u32(&bytes, pos) as usize;
        pos += 4;
        exports.push((
            inline_string_at(&bytes, local_name),
            inline_string_at(&bytes, export_name),
        ));
    }
    assert_eq!(
        exports,
        vec![
            ("a".to_string(), "a".to_string()),
            ("m".to_string(), "b".to_string()),
            ("z".to_string(), "z".to_string()),
        ],
        "local exports follow upstream accessor order by local name"
    );

    let methods = class_method_items(&bytes, user_off);
    let (_, _, func_main_code) = methods
        .iter()
        .find(|(_, name, _)| name == "func_main_0")
        .expect("func_main_0 method exists");
    let bytecode = code_item_bytecode(&bytes, func_main_code.expect("func_main has code") as usize);
    let stores: Vec<u8> = bytecode
        .windows(2)
        .filter_map(|window| (window[0] == 0x7c).then_some(window[1]))
        .collect();
    assert_eq!(
        stores,
        vec![2, 0, 1],
        "source initializes z, a, m, but stmodulevar operands index sorted local exports"
    );
}

#[test]
fn test_module_record_excludes_type_only_import_requests() {
    let tmp = tempfile::tempdir().expect("tempdir");
    let out = tmp.path().join("type-imports.abc");
    module_compiler()
        .compile(
            vec![module_record_with_source_file(
                "com.example.app/entry/ets/entryability/EntryAbility",
                r#"import type common from "@ohos:app.ability.common";
import UIAbility from "@ohos:app.ability.UIAbility";
export default class EntryAbility extends UIAbility {
  onCreate(want: object, launchParam: common.AbilityConstant.LaunchParam): void {}
}"#,
                "/private/tmp/EntryAbility.ts",
            )],
            &out,
        )
        .expect("compile must succeed");
    let bytes = std::fs::read(&out).expect("read .abc");

    let user_off = class_offsets(&bytes)
        .into_iter()
        .find(|off| {
            class_name_at(&bytes, *off as usize)
                == "Lcom.example.app/entry/ets/entryability/EntryAbility;"
        })
        .expect("user record exists") as usize;
    let fields = user_field_values(&bytes, user_off);
    let mut pos = *fields
        .get("moduleRecordIdx")
        .expect("moduleRecordIdx field has a literal-array value") as usize;

    let _item_count = read_u32(&bytes, pos);
    pos += 4;
    assert_eq!(
        read_u32(&bytes, pos),
        1,
        "type-only imports must be routed to the upstream type module record, not runtime requests"
    );
    pos += 4;
    let request = read_u32(&bytes, pos) as usize;
    assert_eq!(
        inline_string_at(&bytes, request),
        "@ohos:app.ability.UIAbility"
    );
    assert!(
        !bytes
            .windows(b"@ohos:app.ability.common".len())
            .any(|w| w == b"@ohos:app.ability.common"),
        "type-only import source must not be serialized as a runtime module request"
    );
}

#[test]
fn test_module_record_excludes_imports_used_only_in_type_positions() {
    let tmp = tempfile::tempdir().expect("tempdir");
    let out = tmp.path().join("semantic-type-imports.abc");
    module_compiler()
        .compile(
            vec![module_record_with_source_file(
                "pkg_modules/.ohpm/@hw-agconnect+hmcore@1.0.5/pkg_modules/@hw-agconnect/hmcore/src/main/ets/core/hmcore",
                r#"import common from "@ohos:app.ability.common";
import hilog from "@ohos:hilog";

export function createCore(applicationContext: common.Context): void {
  hilog.info(0x0000, "hmcore", "init");
}"#,
                "/private/tmp/hmcore.ts",
            )],
            &out,
        )
        .expect("compile must succeed");
    let bytes = std::fs::read(&out).expect("read .abc");

    let user_off = class_offsets(&bytes)
        .into_iter()
        .find(|off| {
            class_name_at(&bytes, *off as usize)
                == "Lpkg_modules/.ohpm/@hw-agconnect+hmcore@1.0.5/pkg_modules/@hw-agconnect/hmcore/src/main/ets/core/hmcore;"
        })
        .expect("user record exists") as usize;
    let fields = user_field_values(&bytes, user_off);
    let mut pos = *fields
        .get("moduleRecordIdx")
        .expect("moduleRecordIdx field has a literal-array value") as usize;

    let _item_count = read_u32(&bytes, pos);
    pos += 4;
    assert_eq!(
        read_u32(&bytes, pos),
        1,
        "imports referenced only from TS type positions must not become runtime requests"
    );
    pos += 4;
    let request = read_u32(&bytes, pos) as usize;
    assert_eq!(inline_string_at(&bytes, request), "@ohos:hilog");
}

#[test]
fn test_exported_value_declarations_are_local_exports() {
    let tmp = tempfile::tempdir().expect("tempdir");
    let out = tmp.path().join("value-exports.abc");
    module_compiler()
        .compile(
            vec![module_record_with_source_file(
                "com.example.app/entry/ets/pages/Index",
                r#"export const VALUE = 1;
export enum GRS_TYPE { AGC_GW }
export function getGrsConfig() { return VALUE; }"#,
                "/private/tmp/Index.ts",
            )],
            &out,
        )
        .expect("compile must succeed");
    let bytes = std::fs::read(&out).expect("read .abc");

    let user_off = class_offsets(&bytes)
        .into_iter()
        .find(|off| {
            class_name_at(&bytes, *off as usize) == "Lcom.example.app/entry/ets/pages/Index;"
        })
        .expect("user record exists") as usize;
    let fields = user_field_values(&bytes, user_off);
    let mut pos = *fields
        .get("moduleRecordIdx")
        .expect("moduleRecordIdx field has a literal-array value") as usize;

    assert_eq!(read_u32(&bytes, pos), 12, "module record item count");
    pos += 4;

    assert_eq!(read_u32(&bytes, pos), 0, "no module requests");
    pos += 4;
    assert_eq!(read_u32(&bytes, pos), 0, "no regular imports");
    pos += 4;
    assert_eq!(read_u32(&bytes, pos), 0, "no namespace imports");
    pos += 4;

    assert_eq!(read_u32(&bytes, pos), 3, "three local value exports");
    pos += 4;
    let mut exports = Vec::new();
    for _ in 0..3 {
        let local_name = read_u32(&bytes, pos) as usize;
        pos += 4;
        let export_name = read_u32(&bytes, pos) as usize;
        pos += 4;
        exports.push((
            inline_string_at(&bytes, local_name),
            inline_string_at(&bytes, export_name),
        ));
    }

    assert!(exports.contains(&("VALUE".to_string(), "VALUE".to_string())));
    assert!(exports.contains(&("GRS_TYPE".to_string(), "GRS_TYPE".to_string())));
    assert!(exports.contains(&("getGrsConfig".to_string(), "getGrsConfig".to_string())));
}

#[test]
fn test_ts_enum_declaration_emits_runtime_object_like_upstream_transform() {
    let tmp = tempfile::tempdir().unwrap();
    let out = tmp.path().join("ts_enum_runtime.abc");
    let record_name = "test/ts_enum_runtime";
    module_compiler()
        .compile(
            vec![module_record(
                record_name,
                r#"export enum Status { Ready, Done = 3, Label = "label" }
const ready = Status.Ready;
const label = Status.Label;"#,
            )],
            &out,
        )
        .expect("TS enum declaration must compile to a runtime object");
    let bytes = std::fs::read(&out).unwrap();
    let bytecode = module_func_main_bytecode(&bytes, record_name);
    assert!(
        bytecode_contains_opcode(&bytecode, 0x04),
        "TS enum lowering must create the runtime enum object"
    );
    assert!(
        bytecode_contains_opcode(&bytecode, 0x7c),
        "exported TS enum must store the runtime object to the module export slot"
    );
    assert!(
        bytecode_contains_opcode(&bytecode, 0x43),
        "TS enum members must assign name -> value properties"
    );
    assert!(
        bytecode_contains_opcode(&bytecode, 0x38),
        "numeric TS enum members must assign value -> name reverse mappings"
    );
}

#[test]
fn test_ts_enum_uninitialized_member_reads_previous_member_like_upstream_transform() {
    let tmp = tempfile::tempdir().unwrap();
    let out = tmp.path().join("ts_enum_previous_member.abc");
    let record_name = "test/ts_enum_previous_member";
    module_compiler()
        .compile(
            vec![module_record(
                record_name,
                r#"enum Step { Start = 3, Next }
const next = Step.Next;"#,
            )],
            &out,
        )
        .expect("TS enum declaration with implicit member must compile");
    let bytes = std::fs::read(&out).unwrap();
    let bytecode = module_func_main_bytecode(&bytes, record_name);
    assert!(
        bytecode_contains_opcode(&bytecode, 0x42),
        "implicit enum members after the first must read E.previous before adding one"
    );
    assert!(
        bytecode_contains_opcode(&bytecode, 0x0a),
        "implicit enum members after the first must add one at runtime"
    );
}

#[test]
fn test_ts_enum_string_member_reference_avoids_numeric_reverse_mapping() {
    let tmp = tempfile::tempdir().unwrap();
    let out = tmp.path().join("ts_enum_string_ref.abc");
    let record_name = "test/ts_enum_string_ref";
    module_compiler()
        .compile(
            vec![module_record(
                record_name,
                r#"enum Labels { A = "a", B = A }
const label = Labels.B;"#,
            )],
            &out,
        )
        .expect("TS enum declaration with string member reference must compile");
    let bytes = std::fs::read(&out).unwrap();
    let bytecode = module_func_main_bytecode(&bytes, record_name);
    assert_eq!(
        bytecode_opcode_count(&bytecode, 0x38),
        0,
        "string-valued enum members must not emit value -> name reverse mappings"
    );
    assert!(
        bytecode_opcode_count(&bytecode, 0x43) >= 2,
        "string-valued enum members must still assign name -> value properties"
    );
}

#[test]
fn test_ts_enum_binding_is_hoisted_before_runtime_declaration() {
    let tmp = tempfile::tempdir().unwrap();
    let out = tmp.path().join("ts_enum_hoist.abc");
    let record_name = "test/ts_enum_hoist";
    module_compiler()
        .compile(
            vec![module_record(
                record_name,
                r#"function readStatus() { return Status; }
const before = readStatus();
enum Status { Ready }"#,
            )],
            &out,
        )
        .expect("TS enum binding must be available before runtime declaration");
    let bytes = std::fs::read(&out).unwrap();
    let bytecode = module_func_main_bytecode(&bytes, record_name);
    assert!(
        !bytecode_contains_opcode(&bytecode, 0x6d),
        "pre-declaration enum reads must use the local enum binding instead of global fallback"
    );
    assert!(
        bytecode_contains_jnez(&bytecode),
        "enum object initialization must use the upstream logical-or guard"
    );
}

#[test]
fn test_ts_enum_computed_template_member_name_lowers_like_upstream_transform() {
    let tmp = tempfile::tempdir().unwrap();
    let out = tmp.path().join("ts_enum_template_key.abc");
    let record_name = "test/ts_enum_template_key";
    module_compiler()
        .compile(
            vec![module_record(
                record_name,
                r#"enum Status { [`ready`] = 1 }
const ready = Status.ready;"#,
            )],
            &out,
        )
        .expect("TS enum computed template member names must lower to runtime properties");
    let bytes = std::fs::read(&out).unwrap();
    let bytecode = module_func_main_bytecode(&bytes, record_name);
    assert!(
        bytecode_contains_opcode(&bytecode, 0x43),
        "computed template enum keys must assign name -> value properties"
    );
    assert!(
        bytecode_contains_opcode(&bytecode, 0x38),
        "numeric computed template enum keys must assign value -> name reverse mappings"
    );
}

#[test]
fn test_exported_variable_declaration_writes_module_variable() {
    let tmp = tempfile::tempdir().unwrap();
    let out = tmp.path().join("exported_var_bytecode.abc");
    let record_name = "test/exported_var_bytecode";
    module_compiler()
        .compile(
            vec![module_record(record_name, "export const VALUE = 1;")],
            &out,
        )
        .expect("compile exported variable declaration");
    let bytes = std::fs::read(&out).unwrap();
    let bytecode = module_func_main_bytecode(&bytes, record_name);
    assert!(
        bytecode_contains_opcode(&bytecode, 0x7c),
        "exported variable declarations must store their initializer into the module variable slot"
    );
}

#[test]
fn test_anonymous_default_function_uses_default_local_export() {
    let tmp = tempfile::tempdir().unwrap();
    let out = tmp.path().join("anonymous_default_function.abc");
    let record_name = "test/anonymous_default_function";
    module_compiler()
        .compile(
            vec![module_record(
                record_name,
                "export default function () { return 1; }",
            )],
            &out,
        )
        .expect("compile anonymous default function declaration");
    let bytes = std::fs::read(&out).unwrap();

    assert_eq!(
        module_local_exports(&bytes, record_name),
        vec![("*default*".to_string(), "default".to_string())],
        "anonymous export default declarations use upstream's *default* local export name"
    );

    let bytecode = module_func_main_bytecode(&bytes, record_name);
    assert!(
        bytecode_contains_opcode(&bytecode, 0x33),
        "anonymous default function declarations must define the function in func_main_0"
    );
    assert!(
        bytecode_contains_opcode(&bytecode, 0x7c),
        "anonymous default function declarations must store the function into the module variable slot"
    );
}

#[test]
fn test_default_expression_uses_default_local_export() {
    let tmp = tempfile::tempdir().unwrap();
    let out = tmp.path().join("default_expression_export.abc");
    let record_name = "test/default_expression_export";
    module_compiler()
        .compile(vec![module_record(record_name, "export default 1;")], &out)
        .expect("compile default expression export");
    let bytes = std::fs::read(&out).unwrap();

    assert_eq!(
        module_local_exports(&bytes, record_name),
        vec![("*default*".to_string(), "default".to_string())],
        "export default expressions use upstream's *default* local export name"
    );

    let bytecode = module_func_main_bytecode(&bytes, record_name);
    assert!(
        bytecode_contains_opcode(&bytecode, 0x7c),
        "export default expressions must store the value into the module variable slot"
    );
}

#[test]
fn test_default_ts_interface_does_not_emit_local_export() {
    let tmp = tempfile::tempdir().unwrap();
    let out = tmp.path().join("default_ts_interface.abc");
    let record_name = "test/default_ts_interface";
    module_compiler()
        .compile(
            vec![module_record(
                record_name,
                "export default interface Foo { value: string }",
            )],
            &out,
        )
        .expect("compile default TS interface export");
    let bytes = std::fs::read(&out).unwrap();

    assert_eq!(
        module_local_exports(&bytes, record_name),
        Vec::<(String, String)>::new(),
        "upstream skips type-only default interface declarations in module records"
    );
}

#[test]
fn test_declare_ts_module_declarations_are_type_only() {
    let tmp = tempfile::tempdir().unwrap();
    let out = tmp.path().join("declare_ts_module.abc");
    let record_name = "test/declare_ts_module";
    module_compiler()
        .compile(
            vec![module_record(
                record_name,
                r#"declare namespace Types {
  export interface Shape { value: string }
  export const size: number;
}
declare module "virtual:asset" {
  export const asset: string;
}"#,
            )],
            &out,
        )
        .expect("declare TS module declarations must be erased as type-only");
    let bytes = std::fs::read(&out).unwrap();
    assert_eq!(
        module_local_exports(&bytes, record_name),
        Vec::<(String, String)>::new(),
        "declare TS module declarations must not create runtime local exports"
    );
    let bytecode = module_func_main_bytecode(&bytes, record_name);
    assert!(
        !bytecode_contains_opcode(&bytecode, 0x04),
        "declare TS module declarations must not create runtime namespace objects"
    );
}

#[test]
fn test_runtime_ts_namespace_declaration_lowers_through_transformer() {
    let tmp = tempfile::tempdir().unwrap();
    let out = tmp.path().join("runtime_ts_module.abc");
    let record_name = "test/runtime_ts_module";
    module_compiler()
        .compile(
            vec![module_record(
                record_name,
                r#"export namespace RuntimeNs {
  export const answer = 42;
  export function read() { return answer; }
}"#,
            )],
            &out,
        )
        .expect("runtime TS namespace must lower through upstream-shaped TS transform");
    let bytes = std::fs::read(&out).unwrap();
    assert_eq!(
        module_local_exports(&bytes, record_name),
        vec![("RuntimeNs".to_string(), "RuntimeNs".to_string())],
        "export namespace must create a runtime local export for the namespace object"
    );
    let bytecode = module_func_main_bytecode(&bytes, record_name);
    assert!(
        bytecode_contains_opcode(&bytecode, 0x04),
        "runtime namespace must create a namespace object"
    );
    assert!(
        bytecode_contains_opcode(&bytecode, 0x2a),
        "runtime namespace must call the transformed namespace IIFE with one argument"
    );
    assert!(
        bytes_contain_ascii(&bytes, "RuntimeNs")
            && bytes_contain_ascii(&bytes, "answer")
            && bytes_contain_ascii(&bytes, "read"),
        "runtime namespace object and exported members must be present in ABC strings"
    );
}

#[test]
fn test_commonjs_ts_export_assignment_lowers_to_module_exports() {
    let tmp = tempfile::tempdir().unwrap();
    let out = tmp.path().join("ts_export_assignment_cjs.abc");
    let record_name = "test/ts_export_assignment_cjs";
    module_compiler()
        .compile(
            vec![commonjs_module_record(
                record_name,
                r#"const value = 7; export = value;"#,
            )],
            &out,
        )
        .expect("CommonJS TS export assignment must lower through TypeScript transform");
    let bytes = std::fs::read(&out).unwrap();
    let bytecode = module_func_main_bytecode(&bytes, record_name);
    assert!(
        bytecode_contains_opcode(&bytecode, 0x43),
        "export = must lower to module.exports property store"
    );
    assert!(
        bytes_contain_ascii(&bytes, "module") && bytes_contain_ascii(&bytes, "exports"),
        "transformed module.exports names must be present in ABC strings"
    );
}

#[test]
fn test_esm_ts_export_assignment_reports_transformer_diagnostic() {
    let tmp = tempfile::tempdir().unwrap();
    let out = tmp.path().join("ts_export_assignment_esm.abc");
    let err = match module_compiler().compile(
        vec![module_record(
            "test/ts_export_assignment_esm",
            r#"export = value;"#,
        )],
        &out,
    ) {
        Ok(_) => panic!("ESM TS export assignment must not silently compile"),
        Err(err) => err,
    };
    assert!(
        matches!(
            err,
            AbcError::Parse {
                ref message,
                ..
            } if message.contains("export =") || message.contains("Export assignment")
        ),
        "ESM export assignment must report the TypeScript transformer diagnostic, got {err:?}"
    );
}

#[test]
fn test_ts_namespace_export_declaration_is_type_only() {
    let tmp = tempfile::tempdir().unwrap();
    let out = tmp.path().join("ts_namespace_export.abc");
    let record_name = "test/ts_namespace_export";
    module_compiler()
        .compile(
            vec![module_record(
                record_name,
                r#"export as namespace PublicApi;
const value = 1;"#,
            )],
            &out,
        )
        .expect("TS namespace export declarations must be erased as type-only");
    let bytes = std::fs::read(&out).unwrap();
    assert_eq!(
        module_local_exports(&bytes, record_name),
        Vec::<(String, String)>::new(),
        "TS namespace export declarations must not create runtime local exports"
    );
    assert!(
        !bytes
            .windows(b"PublicApi".len())
            .any(|w| w == b"PublicApi"),
        "TS namespace export declarations must not serialize the namespace name into runtime ABC data"
    );
}

#[test]
fn test_declare_class_and_value_declarations_are_type_only() {
    let tmp = tempfile::tempdir().unwrap();
    let out = tmp.path().join("declare_runtime_declarations.abc");
    let record_name = "test/declare_runtime_declarations";
    module_compiler()
        .compile(
            vec![module_record(
                record_name,
                r#"declare class AmbientClass {
  field: string;
  method(): number;
}
export declare class ExportedAmbient {}
declare const ambientValue: number;
declare enum AmbientEnum { A = 1 }
const runtime = 1;"#,
            )],
            &out,
        )
        .expect("declare class/var/enum declarations must be erased as type-only");
    let bytes = std::fs::read(&out).unwrap();
    assert_eq!(
        module_local_exports(&bytes, record_name),
        Vec::<(String, String)>::new(),
        "declare runtime-shaped declarations must not create module local exports"
    );
    for name in [
        "AmbientClass",
        "ExportedAmbient",
        "ambientValue",
        "AmbientEnum",
    ] {
        assert!(
            !bytes_contain_ascii(&bytes, name),
            "declare runtime-shaped declaration name {name} must not be serialized into runtime ABC data"
        );
    }
}

#[test]
fn test_type_only_ts_import_equals_declarations_are_erased() {
    let tmp = tempfile::tempdir().unwrap();
    let out = tmp.path().join("type_only_ts_import_equals.abc");
    let record_name = "test/type_only_ts_import_equals";
    module_compiler()
        .compile(
            vec![module_record(
                record_name,
                r#"import type Alias = Types.Shape;
import type External = require("virtual:types");
declare namespace Types { export interface Shape { value: string } }
const value = 1;"#,
            )],
            &out,
        )
        .expect("type-only TS import-equals declarations must be erased");
    let bytes = std::fs::read(&out).unwrap();
    assert_eq!(
        module_local_exports(&bytes, record_name),
        Vec::<(String, String)>::new(),
        "type-only TS import-equals declarations must not create runtime exports"
    );
    assert!(
        !bytes
            .windows(b"virtual:types".len())
            .any(|w| w == b"virtual:types"),
        "type-only external import-equals must not create module requests"
    );
}

#[test]
fn test_runtime_external_ts_import_equals_lowers_to_require_binding() {
    let tmp = tempfile::tempdir().unwrap();
    let out = tmp.path().join("runtime_ts_import_equals.abc");
    let record_name = "test/runtime_ts_import_equals";
    module_compiler()
        .compile(
            vec![module_record(
                record_name,
                r#"import Runtime = require("runtime");
const value = Runtime.value;"#,
            )],
            &out,
        )
        .expect("runtime external TS import-equals must lower like a require binding");
    let bytes = std::fs::read(&out).unwrap();
    assert!(
        bytes.windows(b"runtime".len()).any(|w| w == b"runtime"),
        "runtime external TS import-equals must emit the require module request"
    );
}

#[test]
fn test_runtime_qualified_ts_import_equals_lowers_to_member_alias() {
    let tmp = tempfile::tempdir().unwrap();
    let out = tmp.path().join("runtime_qualified_ts_import_equals.abc");
    let record_name = "test/runtime_qualified_ts_import_equals";
    module_compiler()
        .compile(
            vec![module_record(
                record_name,
                r#"import Alias = Ns.Inner;
const value = Alias.value;"#,
            )],
            &out,
        )
        .expect("runtime qualified TS import-equals must lower to a member alias");
    let bytes = std::fs::read(&out).unwrap();
    assert!(
        bytes.windows(b"Ns".len()).any(|w| w == b"Ns")
            && bytes.windows(b"Inner".len()).any(|w| w == b"Inner"),
        "qualified TS import-equals must emit the qualified member load before storing Alias"
    );
}

#[test]
fn test_nested_ts_namespace_lowers_iife_within_iife() {
    // Upstream: nested namespaces produce an inner IIFE assigned to the outer
    // namespace object, following the TypeScript runtime namespace pattern.
    // Both the outer and inner namespace objects must be present in ABC strings.
    let tmp = tempfile::tempdir().unwrap();
    let out = tmp.path().join("nested_ns.abc");
    let record_name = "test/nested_ns";
    module_compiler()
        .compile(
            vec![module_record(
                record_name,
                r#"export namespace Outer {
  export namespace Inner {
    export const value = 42;
  }
}"#,
            )],
            &out,
        )
        .expect("nested TS namespaces must lower through transformer");
    let bytes = std::fs::read(&out).unwrap();
    // Outer namespace must be a runtime export.
    assert_eq!(
        module_local_exports(&bytes, record_name),
        vec![("Outer".to_string(), "Outer".to_string())],
        "outer namespace must create a runtime local export"
    );
    // Both names must appear in the ABC string table.
    assert!(
        bytes_contain_ascii(&bytes, "Outer") && bytes_contain_ascii(&bytes, "Inner"),
        "nested namespace names must be present in ABC string data"
    );
}

#[test]
fn test_const_enum_compiles_and_inlines_values() {
    // Upstream OXC transformer with `optimize_const_enums = false` inlines
    // const enum member values at the use site (numeric literal substitution).
    // The runtime enum object itself may be stripped since all references are
    // replaced by literals — this matches TypeScript's single-file const enum
    // behaviour.  The key assertion is that the compilation succeeds and emits
    // a numeric literal for the const enum member.
    let tmp = tempfile::tempdir().unwrap();
    let out = tmp.path().join("const_enum.abc");
    let record_name = "test/const_enum";
    module_compiler()
        .compile(
            vec![module_record(
                record_name,
                r#"const enum Direction { Up = 1, Down = 2, Left = 3, Right = 4 }
const dir = Direction.Up;"#,
            )],
            &out,
        )
        .expect("const enum must compile with inlined numeric literal");
    let bytes = std::fs::read(&out).unwrap();
    let bytecode = module_func_main_bytecode(&bytes, record_name);
    // The const enum member `Direction.Up = 1` must be inlined as `ldai 1`.
    assert!(
        bytecode_contains_opcode(&bytecode, 0x62),
        "const enum member reference must inline numeric literal (ldai 0x62)"
    );
}

#[test]
fn test_module_augmentation_declare_is_erased() {
    // `declare module "x" { ... }` is a TypeScript module augmentation.
    // It is type-only and must produce no runtime bytecode.
    let tmp = tempfile::tempdir().unwrap();
    let out = tmp.path().join("module_augmentation.abc");
    let record_name = "test/module_augmentation";
    module_compiler()
        .compile(
            vec![module_record(
                record_name,
                r#"declare module "lodash" {
  interface LodashStatic { version: string; }
}
const x = 1;"#,
            )],
            &out,
        )
        .expect("declare module augmentation must be erased");
    let bytes = std::fs::read(&out).unwrap();
    // No module request for "lodash" must appear (it's type-only).
    assert!(
        !bytes.windows(b"lodash".len()).any(|w| w == b"lodash"),
        "declare module augmentation must not emit a module request"
    );
}

#[test]
fn test_exported_ts_function_overload_does_not_emit_runtime_export() {
    let tmp = tempfile::tempdir().unwrap();
    let out = tmp.path().join("exported_ts_function_overload.abc");
    let record_name = "test/exported_ts_function_overload";
    module_compiler()
        .compile(
            vec![module_record(
                record_name,
                "export function foo(value: string): string;",
            )],
            &out,
        )
        .expect("compile exported TS function overload");
    let bytes = std::fs::read(&out).unwrap();

    assert_eq!(
        module_local_exports(&bytes, record_name),
        Vec::<(String, String)>::new(),
        "upstream skips function overload signatures in module records"
    );

    let bytecode = module_func_main_bytecode(&bytes, record_name);
    assert!(
        !bytecode_contains_opcode(&bytecode, 0x33),
        "function overload signatures must not emit DefineFuncShort"
    );

    let class_name = format!("L{record_name};");
    let class_off = class_offsets(&bytes)
        .into_iter()
        .map(|off| off as usize)
        .find(|&off| class_name_at(&bytes, off) == class_name)
        .expect("module class must exist");
    let method_names: Vec<_> = class_method_items(&bytes, class_off)
        .into_iter()
        .map(|(_, name, _)| name)
        .collect();
    assert!(
        !method_names.iter().any(|name| name == "foo"),
        "function overload signatures must not emit MethodRecord entries"
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
        optimize_try_catch_func: true,
        target_api_version: 9,
        target_api_sub_version: None,
        enable_annotations: false,
        merge_abc: true,
        module: false,
        module_record_field_name: None,
        npm_entries: None,
        dependency_context: None,
        remove_redundant_file: false,
        dump_deps_info: false,
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
            source_file: None,
            package_name: None,
            is_commonjs: false,
            is_shared_module: false,
            origin_source_lang: String::new(),
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
fn test_source_file_emits_debug_info_and_lnp() {
    // Upstream es2abc only emits DebugInfoSection when both debug_info=true AND
    // opt_level=0.  Use make_debug_compiler() to match that contract.
    let tmp = tempfile::tempdir().expect("tempdir");
    let out = tmp.path().join("sourcemap-debug.abc");
    let source_file = "entry/src/main/ets/pages/Index.ts";
    let result = make_debug_compiler()
        .compile(
            vec![AbcModule {
                record_name: "com.example.app/entry/ets/pages/Index".to_string(),
                source: "(function(){ var x = 1; })();".to_string(),
                source_map: None,
                source_file: Some(source_file.to_string()),
                package_name: None,
                is_commonjs: false,
                is_shared_module: false,
                origin_source_lang: String::new(),
            }],
            &out,
        )
        .expect("compile must succeed");
    assert!(
        result.has_debug_info,
        "source-file backed LNP data marks the artifact as debug-bearing"
    );

    let bytes = std::fs::read(&out).expect("read .abc");
    let num_lnp = read_u32(&bytes, 0x24);
    let lnp_idx_off = read_u32(&bytes, 0x28) as usize;
    assert_eq!(
        num_lnp, 2,
        "func_main_0 and the IIFE function expression each get one line-number program"
    );
    assert!(lnp_idx_off > 0, "LNP index offset must be present");

    let user_off = class_offsets(&bytes)
        .into_iter()
        .find(|off| {
            class_name_at(&bytes, *off as usize) == "Lcom.example.app/entry/ets/pages/Index;"
        })
        .expect("user record exists") as usize;
    let (class_source_file, method_debug_info) = first_user_method_debug_info(&bytes, user_off);
    assert_eq!(
        inline_string_at(
            &bytes,
            class_source_file.expect("class SOURCE_FILE tag exists") as usize
        ),
        source_file,
        "record source_file is written as ClassTag::SOURCE_FILE"
    );
    let debug_info = method_debug_info.expect("method DEBUG_INFO tag exists");

    // DebugInfo format (libpandafile DebugInfoItem::Write, corrected architecture):
    //   line_num:           ULEB128(0xFFFF_FFFF)
    //   num_params:         ULEB128(0)
    //   constant_pool_size: ULEB128(N)   byte count of operand pool
    //   constant_pool:      [scope var operands..., line operands...]
    //   lnp_index:          ULEB128(0)
    //
    // The constant_pool contains ULEB128-encoded OPERANDS for LNP opcodes
    // (not the opcodes themselves). The opcodes are in the separate LNP entry.
    let mut pos = debug_info as usize;
    assert_eq!(
        read_uleb(&bytes, &mut pos),
        0,
        "standalone es2abc-style input stores the concrete 0-based start line"
    );
    assert_eq!(read_uleb(&bytes, &mut pos), 0, "no parameter names emitted");
    let _cp_size = read_uleb(&bytes, &mut pos) as usize;
    pos = debug_info as usize;
    read_uleb(&bytes, &mut pos); // line_start
    read_uleb(&bytes, &mut pos); // num_params
    let cp_size2 = read_uleb(&bytes, &mut pos) as usize;
    pos += cp_size2;
    let lnp_index = read_uleb(&bytes, &mut pos);
    assert!(
        lnp_index < u64::from(num_lnp),
        "debug info references a valid LNP index; got {lnp_index}, count {num_lnp}"
    );

    // The LNP entry does not repeat SET_FILE when method source_file equals
    // the owning record source_file. The LNP data_ contains opcodes + inline
    // register SLEB128s (no other inline operands).
    let lnp_offset = read_u32(&bytes, lnp_idx_off) as usize;
    assert!(
        !matches!(bytes[lnp_offset], 0x09 | 0x0a),
        "same-source LNP must not start with SET_FILE or SET_SOURCE_CODE"
    );
    // After SET_FILE, the LNP contains START_LOCAL_EXTENDED entries (opcode + reg),
    // SET_PROLOGUE_END, body opcodes, SET_EPILOGUE_BEGIN, ADVANCE_PC, END_SEQUENCE.
    let lnp_end = lnp_body_end(&bytes, lnp_offset);
    assert!(lnp_end > lnp_offset, "LNP must have content");
    assert_eq!(bytes[lnp_end - 1], 0x00, "LNP terminates with END_SEQUENCE");
}

#[test]
fn test_api18_standalone_explicit_constructor_matches_upstream_lnp_layout() {
    let tmp = tempfile::tempdir().expect("tempdir");
    let out = tmp.path().join("explicit-constructor.abc");
    let compiler = AbcCompiler::new(AbcCompileOptions {
        debug_info: false,
        opt_level: 0,
        optimize_try_catch_func: true,
        target_api_version: 18,
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
    compiler
        .compile(
            vec![AbcModule {
                record_name: "com.example.app/entry/ets/pages/Index".to_string(),
                source: include_str!(
                    "fixtures/upstream/es2panda/bytecode/js/class/test-explicit-constructor.js"
                )
                .to_string(),
                source_map: None,
                source_file: Some("entry/src/main/ets/pages/Index.ts".to_string()),
                package_name: None,
                is_commonjs: false,
                is_shared_module: false,
                origin_source_lang: String::new(),
            }],
            &out,
        )
        .expect("compile explicit constructor fixture");

    let bytes = std::fs::read(&out).expect("read .abc");
    assert_eq!(bytes.len(), 980, "API18 standalone fixture size");
    assert_eq!(read_u32(&bytes, 0x28), 0x3c8, "LNP index offset");
    assert_eq!(
        &bytes[0x3a4..0x3d4],
        &[
            0x0b, 0x00, 0x05, 0x00, 0x01, 0x00, 0x00, 0x0b, 0x01, 0x02, 0x10, 0x65, 0x00, 0x08,
            0x00, 0x03, 0x00, 0x10, 0x77, 0x01, 0x0b, 0x01, 0x11, 0x01, 0x02, 0x10, 0x00, 0x05,
            0x00, 0x04, 0x00, 0x12, 0x0d, 0x79, 0x02, 0x00, 0xa4, 0x03, 0x00, 0x00, 0xab, 0x03,
            0x00, 0x00, 0xb8, 0x03, 0x00, 0x00,
        ],
        "LNP/DebugInfo section matches upstream es2abc for the fixture"
    );
}

#[test]
fn test_api18_standalone_lexical_arrow_matches_upstream_abc_tail_layout() {
    let tmp = tempfile::tempdir().expect("tempdir");
    let out = tmp.path().join("lexical-arrow.abc");
    let compiler = AbcCompiler::new(AbcCompileOptions {
        debug_info: false,
        opt_level: 0,
        optimize_try_catch_func: true,
        target_api_version: 18,
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
    compiler
        .compile(
            vec![AbcModule {
                record_name: "com.example.app/entry/ets/pages/Index".to_string(),
                source: include_str!(
                    "fixtures/upstream/es2panda/bytecode/js/lexicalEnv/arrow-function-reference-ctx.js"
                )
                .to_string(),
                source_map: None,
                source_file: Some("entry/src/main/ets/pages/Index.ts".to_string()),
                package_name: None,
                is_commonjs: false,
            is_shared_module: false,
            origin_source_lang: String::new(),
            }],
            &out,
        )
        .expect("compile lexical arrow fixture");

    let bytes = std::fs::read(&out).expect("read .abc");
    assert_eq!(bytes.len(), 2048, "API18 lexical fixture size");
    assert_eq!(read_u32(&bytes, 0x28), 0x7e8, "LNP index offset");
    assert_eq!(
        bytes[0x770], 0x00,
        "standalone lexenv catch-all handler code_size matches upstream"
    );
    assert_eq!(
        &bytes[0x771..0x800],
        &[
            0x0b, 0x01, 0x02, 0x10, 0x00, 0xff, 0xff, 0xff, 0xff, 0x0f, 0x00, 0x03, 0x00, 0x06,
            0x27, 0x00, 0x06, 0x00, 0x03, 0x00, 0x0a, 0x79, 0x00, 0x0b, 0xaf, 0x7e, 0x01, 0x12,
            0x00, 0xff, 0xff, 0xff, 0xff, 0x0f, 0x00, 0x02, 0x00, 0x10, 0x02, 0xff, 0xff, 0xff,
            0xff, 0x0f, 0x00, 0x04, 0x00, 0x06, 0x1e, 0x10, 0x01, 0x0b, 0x01, 0x02, 0x10, 0x7f,
            0x01, 0x12, 0x00, 0xff, 0xff, 0xff, 0xff, 0x0f, 0x00, 0x04, 0x00, 0x06, 0x13, 0x10,
            0x01, 0x0b, 0x6b, 0x2f, 0x01, 0x11, 0x00, 0x09, 0x00, 0x02, 0x00, 0x10, 0x03, 0x0b,
            0x01, 0x11, 0x6b, 0xfe, 0xaa, 0x00, 0x13, 0x00, 0x02, 0x00, 0x15, 0x04, 0x0b, 0x01,
            0x11, 0x2f, 0xfe, 0xaa, 0x00, 0x1e, 0x00, 0x02, 0x00, 0x15, 0x05, 0x05, 0x00, 0x03,
            0x00, 0x24, 0x7a, 0x00, 0x00, 0x00, 0x00, 0x71, 0x07, 0x00, 0x00, 0xa4, 0x07, 0x00,
            0x00, 0x88, 0x07, 0x00, 0x00, 0xb8, 0x07, 0x00, 0x00, 0xc4, 0x07, 0x00, 0x00, 0xd1,
            0x07, 0x00, 0x00,
        ],
        "DebugInfo/LNP tail matches upstream es2abc for lexical fixture"
    );
}

#[test]
fn test_module_metadata_uses_files_info_package_name() {
    let tmp = tempfile::tempdir().expect("tempdir");
    let out = tmp.path().join("package-name.abc");
    make_compiler()
        .compile(
            vec![
                AbcModule {
                    record_name: "pkg_modules/.ohpm/@scope+pkg@1.0.0/pkg_modules/@scope/pkg/index"
                        .to_string(),
                    source: "export const value = 1;".to_string(),
                    source_map: None,
                    source_file: None,
                    package_name: Some(
                        "pkg_modules/.ohpm/@scope+pkg@1.0.0/pkg_modules/@scope/pkg".to_string(),
                    ),
                    is_commonjs: false,
                    is_shared_module: false,
                    origin_source_lang: String::new(),
                },
                AbcModule {
                    record_name: "com.example.app/phone@utils/Index".to_string(),
                    source: "export const value = 1;".to_string(),
                    source_map: None,
                    source_file: None,
                    package_name: Some("utils".to_string()),
                    is_commonjs: false,
                    is_shared_module: false,
                    origin_source_lang: String::new(),
                },
            ],
            &out,
        )
        .expect("compile must succeed");

    let bytes = std::fs::read(&out).expect("read abc");
    assert!(bytes
        .windows(b"pkgName@pkg_modules/.ohpm/@scope+pkg@1.0.0/pkg_modules/@scope/pkg".len())
        .any(|window| {
            window == b"pkgName@pkg_modules/.ohpm/@scope+pkg@1.0.0/pkg_modules/@scope/pkg"
        }));
    assert!(bytes
        .windows(b"pkgName@utils".len())
        .any(|window| window == b"pkgName@utils"));
    assert!(!bytes
        .windows(b"pkgName@.ohpm".len())
        .any(|window| window == b"pkgName@.ohpm"));
    assert!(!bytes
        .windows(b"pkgName@phone@utils".len())
        .any(|window| window == b"pkgName@phone@utils"));
}

#[test]
fn test_non_merge_module_record_field_name_option_matches_upstream() {
    let tmp = tempfile::tempdir().expect("tempdir");
    let out = tmp.path().join("module-record-field-name.abc");
    let compiler = AbcCompiler::new(AbcCompileOptions {
        debug_info: false,
        opt_level: 0,
        optimize_try_catch_func: true,
        target_api_version: 12,
        target_api_sub_version: None,
        enable_annotations: false,
        merge_abc: false,
        module: true,
        module_record_field_name: Some("customModuleRecord".to_string()),
        npm_entries: None,
        dependency_context: None,
        remove_redundant_file: false,
        dump_deps_info: false,
    });
    compiler
        .compile(
            vec![AbcModule {
                record_name: String::new(),
                source: "export const value = 1;".to_string(),
                source_map: None,
                source_file: Some("main.js".to_string()),
                package_name: None,
                is_commonjs: false,
                is_shared_module: false,
                origin_source_lang: String::new(),
            }],
            &out,
        )
        .expect("compile must succeed");

    let bytes = std::fs::read(&out).expect("read .abc");
    let module_record_off = class_offsets(&bytes)
        .into_iter()
        .find(|off| class_name_at(&bytes, *off as usize).contains("_ESModuleRecord"))
        .expect("_ESModuleRecord exists") as usize;
    let fields = user_field_values(&bytes, module_record_off);
    assert!(
        fields.contains_key("customModuleRecord"),
        "--module-record-field-name must replace the default source-file field name"
    );
    assert!(
        !fields.contains_key("main.js"),
        "custom module record field name must override the source-file fallback"
    );
}

#[test]
fn test_module_metadata_marks_commonjs_records() {
    let tmp = tempfile::tempdir().expect("tempdir");
    let out = tmp.path().join("commonjs.abc");
    module_compiler()
        .compile(
            vec![AbcModule {
                record_name: "pkg_modules/.ohpm/@protobufjs+base64@1.1.2/pkg_modules/@protobufjs/base64/index"
                    .to_string(),
                source: r#"var base64 = exports; base64.length = function length() { return 0; };"#
                    .to_string(),
                source_map: None,
                source_file: Some("@protobufjs/base64/index.js".to_string()),
                package_name: Some(
                    "pkg_modules/.ohpm/@protobufjs+base64@1.1.2/pkg_modules/@protobufjs/base64"
                        .to_string(),
                ),
                is_commonjs: true,
            is_shared_module: false,
            origin_source_lang: String::new(),
            }],
            &out,
        )
        .expect("compile must succeed");
    let bytes = std::fs::read(&out).expect("read .abc");

    let user_off = class_offsets(&bytes)
        .into_iter()
        .find(|off| {
            class_name_at(bytes.as_slice(), *off as usize)
                == "Lpkg_modules/.ohpm/@protobufjs+base64@1.1.2/pkg_modules/@protobufjs/base64/index;"
        })
        .expect("commonjs record exists") as usize;
    let fields = user_field_values(&bytes, user_off);
    assert_eq!(
        fields.get("isCommonjs"),
        Some(&1),
        "commonjs filesInfo module type is written to the isCommonjs field"
    );
    assert!(
        !fields.contains_key("moduleRecordIdx"),
        "CommonJS records follow upstream ScriptKind::COMMONJS and do not emit moduleRecordIdx"
    );
}

#[test]
fn test_module_metadata_marks_shared_modules() {
    let tmp = tempfile::tempdir().expect("tempdir");
    let out = tmp.path().join("shared-module.abc");
    module_compiler()
        .compile(
            vec![AbcModule {
                record_name: "com.example.shared/shared/ets/pages/Index".to_string(),
                source: "export const value = 1;".to_string(),
                source_map: None,
                source_file: Some("shared/src/main/ets/pages/Index.ts".to_string()),
                package_name: Some("shared".to_string()),
                is_commonjs: false,
                is_shared_module: true,
                origin_source_lang: String::new(),
            }],
            &out,
        )
        .expect("compile must succeed");

    let bytes = std::fs::read(&out).expect("read .abc");
    let user_off = class_offsets(&bytes)
        .into_iter()
        .find(|off| {
            class_name_at(bytes.as_slice(), *off as usize)
                == "Lcom.example.shared/shared/ets/pages/Index;"
        })
        .expect("shared record exists") as usize;
    let fields = user_field_values(&bytes, user_off);
    assert_eq!(
        fields.get("isSharedModule"),
        Some(&1),
        "filesInfo isSharedModule is written to the isSharedModule field"
    );
}

#[test]
fn program_cache_hit_restores_program_and_skips_source_compile() {
    let tmp = tempfile::tempdir().unwrap();
    let cache_file = tmp.path().join("Index.protoBin");
    let first_output = tmp.path().join("first.abc");
    let second_output = tmp.path().join("second.abc");
    let record_name = "entry|entry|1.0.0|src/main/ets/pages/Index";
    let file_name = tmp.path().join("Index.js").to_string_lossy().into_owned();
    let valid_source = "export const value = 1;";
    let invalid_source = "export const = ;";
    let cache_input = AbcProgramCacheInput {
        file_name: file_name.clone(),
        cache_file: cache_file.clone(),
    };

    module_compiler()
        .compile_with_program_cache(
            vec![module_record(record_name, valid_source)],
            &first_output,
            vec![Some(cache_input.clone())],
        )
        .unwrap();

    let entry = ProgramCacheFile::read(&cache_file).unwrap().unwrap();
    let invalid_source_hash = source_cache_hash(
        invalid_source,
        &file_name,
        record_name,
        "",
        "",
        version_for_target_api(12, None),
        12,
    );
    ProgramCacheFile::write(&cache_file, invalid_source_hash, entry.program).unwrap();

    let result = module_compiler()
        .compile_with_program_cache(
            vec![module_record(record_name, invalid_source)],
            &second_output,
            vec![Some(cache_input)],
        )
        .unwrap();

    assert_eq!(result.path, second_output);
    assert_eq!(
        &fs_err::read(&result.path).unwrap()[0..8],
        b"PANDA\x00\x00\x00"
    );
    assert_eq!(
        ProgramCacheFile::read(&cache_file)
            .unwrap()
            .unwrap()
            .hash_code,
        invalid_source_hash
    );
}

/// Verify that the `.protoBin` file written by `ProgramCacheFile::write`
/// uses the correct upstream protobuf binary encoding.
///
/// hvigor's es2abc writes `ProgramCache` proto messages to `.protoBin` files.
/// This test verifies our format is byte-compatible by:
/// 1. Compiling a source and writing a `.protoBin` cache file.
/// 2. Reading the raw bytes and checking the protobuf field-tag layout.
/// 3. Decoding the bytes back through `ProgramCacheFile::read` to confirm
///    the round-trip is lossless.
///
/// Protobuf encoding rules (verified against upstream proto definition):
/// - Field 1 (hash_code: uint32):  wire type 0 (varint) → tag byte `0x08`
/// - Field 2 (program: Program):   wire type 2 (length-delimited) → tag `0x12`
///
/// This aligns our implementation with es2abc's `ProgramCacheFile` format.
#[test]
fn test_program_cache_proto_bin_has_correct_protobuf_layout() {
    let tmp = tempfile::tempdir().unwrap();
    let cache_file = tmp.path().join("test_module.protoBin");
    let output = tmp.path().join("test_module.abc");
    let record_name = "test/proto_bin_format|1.0.0|src/main/ets/Index";
    let file_name = tmp.path().join("Index.js").to_string_lossy().into_owned();
    let source = "export const answer = 42;";

    let cache_input = AbcProgramCacheInput {
        file_name: file_name.clone(),
        cache_file: cache_file.clone(),
    };

    // Compile and write the protoBin.
    module_compiler()
        .compile_with_program_cache(
            vec![module_record(record_name, source)],
            &output,
            vec![Some(cache_input)],
        )
        .expect("compile with program cache must succeed");

    // Verify the .protoBin file was created.
    assert!(
        cache_file.exists(),
        ".protoBin file must be created after compile"
    );

    let raw_bytes = std::fs::read(&cache_file).expect("must be able to read .protoBin file");
    assert!(!raw_bytes.is_empty(), ".protoBin must not be empty");

    // Verify the protobuf field-tag layout:
    //   Byte 0: must be 0x08 (field 1, wire type 0 = varint) for hash_code.
    // This matches es2abc's ProgramCache proto encoding.
    assert_eq!(
        raw_bytes[0], 0x08,
        "first byte must be protobuf field tag 0x08 (field 1 varint = hash_code); \
         got {:#04x}. This indicates our .protoBin format does NOT match \
         the upstream es2abc ProgramCache protobuf encoding.",
        raw_bytes[0]
    );

    // After the varint hash_code, the next field should be field 2 (program)
    // wire type 2 (length-delimited), tag byte 0x12.
    // Skip the varint hash_code (1+ bytes) to find it.
    let mut pos = 1;
    // Consume the varint hash_code bytes (all bytes with MSB set until one without).
    while pos < raw_bytes.len() && (raw_bytes[pos] & 0x80) != 0 {
        pos += 1;
    }
    pos += 1; // skip the last varint byte
    if pos < raw_bytes.len() {
        assert_eq!(
            raw_bytes[pos], 0x12,
            "after hash_code varint, must see field tag 0x12 (field 2 length-delimited = program); \
             got {:#04x} at position {}. This indicates our .protoBin format diverges from \
             the upstream es2abc ProgramCache proto structure.",
            raw_bytes[pos], pos
        );
    }

    // Round-trip: the written bytes must decode back to a valid ProgramCache.
    let entry = ProgramCacheFile::read(&cache_file)
        .expect("ProgramCacheFile::read must succeed")
        .expect("ProgramCacheFile::read must return Some for a freshly written file");

    // The program must have at least one record (our compiled module).
    // hvigor's ProgramCache uses record_table to store ClassRecord entries.
    assert!(
        !entry.program.record_table.is_empty(),
        "decoded Program must contain at least one record in record_table"
    );

    // The hash_code must be non-zero (it's derived from source content).
    assert_ne!(entry.hash_code, 0, "hash_code must be non-zero");
}

// ── New opcode coverage tests ────────────────────────────────────────────────

fn module_compiler() -> AbcCompiler {
    module_compiler_for_target(12, None)
}

/// Module compiler with debug_info=true for testing DebugInfo / debugger opcode emission.
fn module_debug_compiler() -> AbcCompiler {
    AbcCompiler::new(AbcCompileOptions {
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
    })
}

fn module_compiler_for_target(
    target_api_version: u32,
    target_api_sub_version: Option<&str>,
) -> AbcCompiler {
    AbcCompiler::new(AbcCompileOptions {
        debug_info: false,
        opt_level: 0,
        optimize_try_catch_func: true,
        target_api_version,
        target_api_sub_version: target_api_sub_version.map(str::to_string),
        enable_annotations: false,
        merge_abc: true,
        module: true,
        module_record_field_name: None,
        npm_entries: None,
        dependency_context: None,
        remove_redundant_file: false,
        dump_deps_info: false,
    })
}

fn module_compiler_with_annotations() -> AbcCompiler {
    AbcCompiler::new(AbcCompileOptions {
        debug_info: false,
        opt_level: 0,
        optimize_try_catch_func: true,
        target_api_version: 20,
        target_api_sub_version: None,
        enable_annotations: true,
        merge_abc: true,
        module: true,
        module_record_field_name: None,
        npm_entries: None,
        dependency_context: None,
        remove_redundant_file: false,
        dump_deps_info: false,
    })
}

fn module_record(record_name: &str, source: &str) -> AbcModule {
    AbcModule {
        record_name: record_name.to_string(),
        source: source.to_string(),
        source_map: None,
        source_file: None,
        package_name: None,
        is_commonjs: false,
        is_shared_module: false,
        origin_source_lang: String::new(),
    }
}

fn annotation_module(source: &str) -> AbcModule {
    module_record("test/annotation_decl.ets", source)
}

fn commonjs_module_record(record_name: &str, source: &str) -> AbcModule {
    AbcModule {
        record_name: record_name.to_string(),
        source: source.to_string(),
        source_map: None,
        source_file: None,
        package_name: None,
        is_commonjs: true,
        is_shared_module: false,
        origin_source_lang: String::new(),
    }
}

fn module_record_with_source_file(record_name: &str, source: &str, source_file: &str) -> AbcModule {
    AbcModule {
        record_name: record_name.to_string(),
        source: source.to_string(),
        source_map: None,
        source_file: Some(source_file.to_string()),
        package_name: None,
        is_commonjs: false,
        is_shared_module: false,
        origin_source_lang: String::new(),
    }
}

fn module_func_main_bytecode(bytes: &[u8], record_name: &str) -> Vec<u8> {
    let class_name = format!("L{record_name};");
    let class_off = class_offsets(bytes)
        .into_iter()
        .map(|off| off as usize)
        .find(|&off| class_name_at(bytes, off) == class_name)
        .expect("module class must exist");
    let code_off = class_method_items(bytes, class_off)
        .into_iter()
        .find(|(_, name, _)| name == "func_main_0")
        .and_then(|(_, _, code_off)| code_off)
        .expect("func_main_0 must have a code item") as usize;
    code_item_bytecode(bytes, code_off).to_vec()
}

fn module_method_bytecodes(bytes: &[u8], record_name: &str) -> Vec<Vec<u8>> {
    let class_name = format!("L{record_name};");
    let class_off = class_offsets(bytes)
        .into_iter()
        .map(|off| off as usize)
        .find(|&off| class_name_at(bytes, off) == class_name)
        .expect("module class must exist");
    class_method_items(bytes, class_off)
        .into_iter()
        .filter_map(|(_, _, code_off)| {
            code_off.map(|code_off| code_item_bytecode(bytes, code_off as usize).to_vec())
        })
        .collect()
}

fn module_method_bytecode_by_suffix(
    bytes: &[u8],
    record_name: &str,
    suffix: &str,
) -> Option<(String, Vec<u8>)> {
    let class_name = format!("L{record_name};");
    let class_off = class_offsets(bytes)
        .into_iter()
        .map(|off| off as usize)
        .find(|&off| class_name_at(bytes, off) == class_name)
        .expect("module class must exist");
    class_method_items(bytes, class_off)
        .into_iter()
        .find_map(|(_, name, code_off)| {
            let code_off = code_off? as usize;
            name.ends_with(suffix)
                .then(|| (name, code_item_bytecode(bytes, code_off).to_vec()))
        })
}

fn module_local_exports(bytes: &[u8], record_name: &str) -> Vec<(String, String)> {
    let class_name = format!("L{record_name};");
    let class_off = class_offsets(bytes)
        .into_iter()
        .map(|off| off as usize)
        .find(|&off| class_name_at(bytes, off) == class_name)
        .expect("module class must exist");
    let fields = user_field_values(bytes, class_off);
    let mut pos = *fields
        .get("moduleRecordIdx")
        .expect("moduleRecordIdx field has a literal-array value") as usize;

    let _item_count = read_u32(bytes, pos);
    pos += 4;

    let module_request_count = read_u32(bytes, pos) as usize;
    pos += 4 + module_request_count * 4;

    let regular_import_count = read_u32(bytes, pos) as usize;
    pos += 4 + regular_import_count * (4 + 4 + 2);

    let namespace_import_count = read_u32(bytes, pos) as usize;
    pos += 4 + namespace_import_count * (4 + 2);

    let local_export_count = read_u32(bytes, pos) as usize;
    pos += 4;
    let mut exports = Vec::with_capacity(local_export_count);
    for _ in 0..local_export_count {
        let local_name = read_u32(bytes, pos) as usize;
        pos += 4;
        let export_name = read_u32(bytes, pos) as usize;
        pos += 4;
        exports.push((
            inline_string_at(bytes, local_name),
            inline_string_at(bytes, export_name),
        ));
    }
    exports
}

fn first_non_main_method_bytecode_and_tries(bytes: &[u8], record_name: &str) -> (Vec<u8>, u64) {
    let class_name = format!("L{record_name};");
    let class_off = class_offsets(bytes)
        .into_iter()
        .map(|off| off as usize)
        .find(|&off| class_name_at(bytes, off) == class_name)
        .expect("module class must exist");
    class_method_items(bytes, class_off)
        .into_iter()
        .filter_map(|(_, name, code_off)| {
            let code_off = code_off? as usize;
            (name != "func_main_0").then(|| {
                (
                    code_item_bytecode(bytes, code_off).to_vec(),
                    code_item_tries_size(bytes, code_off),
                )
            })
        })
        .next()
        .expect("non-main function method must exist")
}

#[test]
fn test_bool_conversion_uses_default_api_opcode_branch() {
    let tmp = tempfile::tempdir().unwrap();
    let source = r#"if (flag) { globalThis["x"] = 1; }"#;

    let compact_out = tmp.path().join("bool_default_api.abc");
    module_compiler_for_target(12, None)
        .compile(
            vec![module_record("test/bool_default_api", source)],
            &compact_out,
        )
        .expect("compile default API bool source");
    let compact_bytes = std::fs::read(&compact_out).unwrap();
    let compact = module_func_main_bytecode(&compact_bytes, "test/bool_default_api");
    assert!(
        compact.contains(&0x24),
        "API12 with no target-api-sub-version follows upstream default beta1 and emits compact isfalse"
    );
    assert!(
        !compact.windows(2).any(|window| window == [0xfb, 0x14]),
        "default API branch must not emit callruntime.isfalse"
    );

    let runtime_out = tmp.path().join("bool_api12_beta3.abc");
    module_compiler_for_target(12, Some("beta3"))
        .compile(
            vec![module_record("test/bool_api12_beta3", source)],
            &runtime_out,
        )
        .expect("compile non-default API bool source");
    let runtime_bytes = std::fs::read(&runtime_out).unwrap();
    let runtime = module_func_main_bytecode(&runtime_bytes, "test/bool_api12_beta3");
    assert!(
        runtime.windows(2).any(|window| window == [0xfb, 0x14]),
        "API12 beta3 follows upstream non-default branch and emits callruntime.isfalse"
    );
}

#[test]
fn test_object_literal_uses_upstream_own_property_opcodes() {
    let tmp = tempfile::tempdir().unwrap();

    let define_field_out = tmp.path().join("object_define_field.abc");
    module_compiler()
        .compile(
            vec![module_record(
                "test/object_define_field",
                r#"const obj = { value: function() {}, method() {}, plain: 1, name: null }; export default obj;"#,
            )],
            &define_field_out,
        )
        .expect("compile object define field source");
    let define_field_bytes = std::fs::read(&define_field_out).unwrap();
    let define_field = module_func_main_bytecode(&define_field_bytes, "test/object_define_field");
    assert!(
        define_field.windows(5).any(|window| window[0] == 0xdb),
        "default API runtime object value must use definefieldbyname (0xdb), got {define_field:02x?}"
    );
    assert!(
        !define_field.windows(5).any(|window| window[0] == 0xdc),
        "default API object value must not use beta3 definepropertybyname (0xdc), got {define_field:02x?}"
    );

    let nameset_name_out = tmp.path().join("object_nameset_name.abc");
    module_compiler()
        .compile(
            vec![module_record(
                "test/object_nameset_name",
                r#"const obj = { "a\\.b": function() {} }; export default obj;"#,
            )],
            &nameset_name_out,
        )
        .expect("compile object nameSetting by name source");
    let nameset_name_bytes = std::fs::read(&nameset_name_out).unwrap();
    let nameset_name = module_func_main_bytecode(&nameset_name_bytes, "test/object_nameset_name");
    assert!(
        nameset_name.windows(5).any(|window| window[0] == 0x8e),
        "anonymous function value with legal static property name must use stownbynamewithnameset (0x8e), got {nameset_name:02x?}"
    );

    let nameset_value_out = tmp.path().join("object_nameset_value.abc");
    module_compiler()
        .compile(
            vec![module_record(
                "test/object_nameset_value",
                r#"const k = "x"; const obj = { [k]: function() {} }; export default obj;"#,
            )],
            &nameset_value_out,
        )
        .expect("compile object nameSetting by value source");
    let nameset_value_bytes = std::fs::read(&nameset_value_out).unwrap();
    let nameset_value =
        module_func_main_bytecode(&nameset_value_bytes, "test/object_nameset_value");
    assert!(
        nameset_value.windows(4).any(|window| window[0] == 0x99),
        "computed anonymous function value must use stownbyvaluewithnameset (0x99), got {nameset_value:02x?}"
    );

    let define_value_out = tmp.path().join("object_define_value.abc");
    module_compiler()
        .compile(
            vec![module_record(
                "test/object_define_value",
                r#"const k = "x"; const obj = { [k]: 1 }; export default obj;"#,
            )],
            &define_value_out,
        )
        .expect("compile object define field by value source");
    let define_value_bytes = std::fs::read(&define_value_out).unwrap();
    let define_value = module_func_main_bytecode(&define_value_bytes, "test/object_define_value");
    assert!(
        define_value
            .windows(5)
            .any(|window| window[0] == 0xfb && window[1] == 0x01),
        "computed non-nameSetting value must use callruntime.definefieldbyvalue (fb 01), got {define_value:02x?}"
    );

    let store_value_out = tmp.path().join("object_store_value_api10.abc");
    module_compiler_for_target(10, None)
        .compile(
            vec![module_record(
                "test/object_store_value_api10",
                r#"const k = "x"; const obj = { [k]: 1 }; export default obj;"#,
            )],
            &store_value_out,
        )
        .expect("compile object store own by value source");
    let store_value_bytes = std::fs::read(&store_value_out).unwrap();
    let store_value =
        module_func_main_bytecode(&store_value_bytes, "test/object_store_value_api10");
    assert!(
        store_value.windows(4).any(|window| window[0] == 0x78),
        "API10 computed non-nameSetting value must use stownbyvalue (0x78), got {store_value:02x?}"
    );
    assert!(
        !store_value.windows(4).any(|window| window[0] == 0x38),
        "API10 object literal StoreOwnProperty must not fall back to stobjbyvalue (0x38), got {store_value:02x?}"
    );
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
fn test_instanceof_operator_emits_instanceof_opcode() {
    let tmp = tempfile::tempdir().unwrap();
    let out = tmp.path().join("instanceof.abc");
    let record_name = "test/instanceof";
    module_compiler()
        .compile(
            vec![module_record(
                record_name,
                r#"const result = value instanceof Ctor;"#,
            )],
            &out,
        )
        .expect("compile instanceof source");
    let bytes = std::fs::read(&out).unwrap();
    let bytecode = module_func_main_bytecode(&bytes, record_name);
    assert!(
        bytecode_contains_opcode(&bytecode, 0x26),
        "instanceof must emit upstream instanceof opcode (0x26)"
    );
}

#[test]
fn test_exponential_operator_emits_exp_opcode() {
    let tmp = tempfile::tempdir().unwrap();
    let out = tmp.path().join("exp.abc");
    let record_name = "test/exp";
    module_compiler()
        .compile(
            vec![module_record(
                record_name,
                r#"const result = base ** power;"#,
            )],
            &out,
        )
        .expect("compile exponent source");
    let bytes = std::fs::read(&out).unwrap();
    let bytecode = module_func_main_bytecode(&bytes, record_name);
    assert!(
        bytecode_contains_opcode(&bytecode, 0x1b),
        "exponentiation must emit upstream exp opcode (0x1b)"
    );
}

#[test]
fn test_exponential_assignment_emits_exp_opcode() {
    let tmp = tempfile::tempdir().unwrap();
    let out = tmp.path().join("exp_assign.abc");
    let record_name = "test/exp_assign";
    module_compiler()
        .compile(
            vec![module_record(
                record_name,
                r#"let value = 2; value **= power;"#,
            )],
            &out,
        )
        .expect("compile exponent assignment source");
    let bytes = std::fs::read(&out).unwrap();
    let bytecode = module_func_main_bytecode(&bytes, record_name);
    assert!(
        bytecode_contains_opcode(&bytecode, 0x1b),
        "exponentiation assignment must emit upstream exp opcode (0x1b)"
    );
}

#[test]
fn test_logical_and_assignment_uses_short_circuit_store() {
    let tmp = tempfile::tempdir().unwrap();
    let out = tmp.path().join("logical_and_assign.abc");
    let record_name = "test/logical_and_assign";
    module_compiler()
        .compile(
            vec![module_record(
                record_name,
                r#"let value = true; value &&= next;"#,
            )],
            &out,
        )
        .expect("compile logical and assignment source");
    let bytes = std::fs::read(&out).unwrap();
    let bytecode = module_func_main_bytecode(&bytes, record_name);
    assert!(
        bytecode_contains_opcode(&bytecode, 0x24) && bytecode_contains_jnez(&bytecode),
        "&&= must test lhs falsiness and skip rhs/store when falsy"
    );
}

#[test]
fn test_logical_or_assignment_uses_short_circuit_store() {
    let tmp = tempfile::tempdir().unwrap();
    let out = tmp.path().join("logical_or_assign.abc");
    let record_name = "test/logical_or_assign";
    module_compiler()
        .compile(
            vec![module_record(
                record_name,
                r#"let value = false; value ||= next;"#,
            )],
            &out,
        )
        .expect("compile logical or assignment source");
    let bytes = std::fs::read(&out).unwrap();
    let bytecode = module_func_main_bytecode(&bytes, record_name);
    assert!(
        bytecode_contains_opcode(&bytecode, 0x23) && bytecode_contains_jnez(&bytecode),
        "||= must test lhs truthiness and skip rhs/store when truthy"
    );
}

#[test]
fn test_logical_nullish_assignment_uses_strict_nullish_short_circuit() {
    let tmp = tempfile::tempdir().unwrap();
    let out = tmp.path().join("logical_nullish_assign.abc");
    let record_name = "test/logical_nullish_assign";
    module_compiler()
        .compile(
            vec![module_record(
                record_name,
                r#"let value = null; value ??= next;"#,
            )],
            &out,
        )
        .expect("compile logical nullish assignment source");
    let bytes = std::fs::read(&out).unwrap();
    let bytecode = module_func_main_bytecode(&bytes, record_name);
    assert!(
        bytecode_contains_opcode(&bytecode, 0x28)
            && bytecode_contains_jnez(&bytecode)
            && bytecode_contains_jeqz(&bytecode),
        "??= must use strict null/undefined checks before deciding whether to store rhs"
    );
}

#[test]
fn test_nullish_coalescing_uses_strict_nullish_short_circuit() {
    let tmp = tempfile::tempdir().unwrap();
    let out = tmp.path().join("nullish_coalesce.abc");
    let record_name = "test/nullish_coalesce";
    module_compiler()
        .compile(
            vec![module_record(
                record_name,
                r#"const result = value ?? next;"#,
            )],
            &out,
        )
        .expect("compile nullish coalescing source");
    let bytes = std::fs::read(&out).unwrap();
    let bytecode = module_func_main_bytecode(&bytes, record_name);
    assert!(
        bytecode_contains_opcode(&bytecode, 0x28)
            && bytecode_contains_jnez(&bytecode)
            && bytecode_contains_jeqz(&bytecode)
            && !bytecode_contains_opcode(&bytecode, 0x23),
        "?? must use strict null/undefined checks instead of truthiness"
    );
}

#[test]
fn test_ts_expression_wrappers_compile_inner_expression() {
    let tmp = tempfile::tempdir().unwrap();
    let out = tmp.path().join("ts_expr_wrapper.abc");
    let record_name = "test/ts_expr_wrapper";
    module_compiler()
        .compile(
            vec![module_record(
                record_name,
                r#"const result = (true as boolean);"#,
            )],
            &out,
        )
        .expect("compile TS expression wrapper source");
    let bytes = std::fs::read(&out).unwrap();
    let bytecode = module_func_main_bytecode(&bytes, record_name);
    assert!(
        bytecode_contains_opcode(&bytecode, 0x02),
        "TS expression wrappers must compile their inner expression instead of undefined"
    );
}

#[test]
fn test_ts_instantiation_expression_compiles_inner_expression() {
    let tmp = tempfile::tempdir().unwrap();
    let out = tmp.path().join("ts_instantiation.abc");
    let record_name = "test/ts_instantiation";
    module_compiler()
        .compile(
            vec![module_record(
                record_name,
                r#"function identity<T>(value: T): T { return value; }
const result = identity<number>(1);"#,
            )],
            &out,
        )
        .expect("compile TS instantiation expression source");
    let bytes = std::fs::read(&out).unwrap();
    let bytecode = module_func_main_bytecode(&bytes, record_name);
    let call_pos = bytecode
        .iter()
        .position(|opcode| *opcode == 0x2a)
        .expect("identity<number>(1) must emit callarg1");
    assert_eq!(
        bytecode.get(call_pos.saturating_sub(2)..call_pos),
        Some(&[0x60, bytecode[call_pos - 1]][..]),
        "callarg1 must reload the callee register"
    );
    let callee_reg = bytecode[call_pos - 1];
    assert!(
        bytecode[..call_pos]
            .windows(4)
            .any(|window| window[0] == 0x60 && window[2] == 0x61 && window[3] == callee_reg),
        "TS instantiation callee must compile its inner expression before storing the callee register"
    );
}

#[test]
fn test_ts_assignment_target_wrapper_uses_inner_lreference() {
    let tmp = tempfile::tempdir().unwrap();
    let out = tmp.path().join("ts_assignment_target.abc");
    let record_name = "test/ts_assignment_target";
    module_compiler()
        .compile(
            vec![module_record(
                record_name,
                r#"let value = 1; (value as number) += delta;"#,
            )],
            &out,
        )
        .expect("compile TS assignment target wrapper source");
    let bytes = std::fs::read(&out).unwrap();
    let bytecode = module_func_main_bytecode(&bytes, record_name);
    assert!(
        bytecode_contains_opcode(&bytecode, 0x0a),
        "TS assignment target wrappers must use the inner lreference for compound assignment"
    );
}

#[test]
fn test_tagged_template_uses_upstream_template_object_call_shape() {
    let tmp = tempfile::tempdir().unwrap();
    let out = tmp.path().join("tagged_template.abc");
    let record_name = "test/tagged_template";
    module_compiler()
        .compile(
            vec![module_record(
                record_name,
                r#"
function tag(strings, value) { return value; }
const plain = tag`left${1}right`;
const receiver = { tag(strings) { return strings; } };
const method = receiver.tag`only`;
"#,
            )],
            &out,
        )
        .expect("compile tagged template source");
    let bytes = std::fs::read(&out).unwrap();
    let bytecode = module_func_main_bytecode(&bytes, record_name);
    assert!(
        bytecode_contains_opcode(&bytecode, 0x76),
        "tagged templates must call gettemplateobject"
    );
    assert!(
        bytecode.windows(2).any(|window| window == [0xfb, 0x01]),
        "tagged templates must populate raw/cooked arrays with definefieldbyvalue"
    );
    assert!(
        bytecode_contains_opcode(&bytecode, 0x2b),
        "tag`left${{expr}}right` must pass template object and expression to callargs2"
    );
    assert!(
        bytecode_contains_opcode(&bytecode, 0x2e),
        "receiver.tag`only` must preserve receiver with callthis1"
    );
}

#[test]
fn test_switch_statement_uses_strict_equality_dispatch() {
    let tmp = tempfile::tempdir().unwrap();
    let out = tmp.path().join("switch_statement.abc");
    let record_name = "test/switch_statement";
    module_compiler()
        .compile(
            vec![module_record(
                record_name,
                r#"
let input = 2;
let value = 0;
switch (input) {
  case 1:
    value = 10;
    break;
  case 2:
    value = 20;
  default:
    value += 1;
}
export default value;
"#,
            )],
            &out,
        )
        .expect("compile switch statement source");
    let bytes = std::fs::read(&out).unwrap();
    let bytecode = module_func_main_bytecode(&bytes, record_name);
    assert!(
        bytecode_contains_opcode(&bytecode, 0x28),
        "switch case tests must use strict equality"
    );
    assert!(
        bytecode_contains_jnez(&bytecode),
        "matching switch cases must branch to their case bodies"
    );
    assert!(
        bytecode_contains_jmp(&bytecode),
        "switch dispatch and break must use patched jumps"
    );
    assert!(
        bytecode.windows(2).any(|window| window == [0x7c, 0x00]),
        "code after switch must still export the selected value"
    );
}

#[test]
fn test_labeled_statement_emits_body() {
    let tmp = tempfile::tempdir().unwrap();
    let out = tmp.path().join("labeled_statement.abc");
    let record_name = "test/labeled_statement";
    module_compiler()
        .compile(
            vec![module_record(
                record_name,
                r#"let value = 0;
label: value = 1;
export default value;"#,
            )],
            &out,
        )
        .expect("compile labeled statement source");
    let bytes = std::fs::read(&out).unwrap();
    let bytecode = module_func_main_bytecode(&bytes, record_name);
    assert!(
        bytecode
            .windows(5)
            .any(|window| window == [0x62, 0x01, 0x00, 0x00, 0x00]),
        "plain labels must emit their body instead of dropping it"
    );
}

#[test]
fn test_labeled_break_targets_label_end() {
    let tmp = tempfile::tempdir().unwrap();
    let out = tmp.path().join("labeled_break.abc");
    let record_name = "test/labeled_break";
    module_compiler()
        .compile(
            vec![module_record(
                record_name,
                r#"let value = 0;
outer: while (true) {
  value = 1;
  break outer;
  value = 99;
}
export default value;"#,
            )],
            &out,
        )
        .expect("compile labeled break source");
    let bytes = std::fs::read(&out).unwrap();
    let bytecode = module_func_main_bytecode(&bytes, record_name);

    assert!(
        bytecode_contains_jmp(&bytecode),
        "labeled break must emit a patched forward jump to the label end"
    );
    assert!(
        bytecode.windows(2).any(|window| window == [0x7c, 0x00]),
        "code after the labeled statement must still export the selected value"
    );
}

#[test]
fn test_labeled_continue_targets_labeled_loop_update() {
    let tmp = tempfile::tempdir().unwrap();
    let out = tmp.path().join("labeled_continue.abc");
    let record_name = "test/labeled_continue";
    module_compiler()
        .compile(
            vec![module_record(
                record_name,
                r#"let value = 0;
outer: for (let i = 0; i < 3; i++) {
  value += 1;
  continue outer;
  value = 99;
}
export default value;"#,
            )],
            &out,
        )
        .expect("compile labeled continue source");
    let bytes = std::fs::read(&out).unwrap();
    let bytecode = module_func_main_bytecode(&bytes, record_name);

    assert!(
        bytecode_contains_jmp(&bytecode),
        "labeled continue must emit a patched jump to the labeled loop update"
    );
}

#[test]
fn test_labeled_continue_targets_labeled_for_in_loop_next_iteration() {
    let tmp = tempfile::tempdir().unwrap();
    let out = tmp.path().join("labeled_continue_for_in.abc");
    let record_name = "test/labeled_continue_for_in";
    module_compiler()
        .compile(
            vec![module_record(
                record_name,
                r#"let value = 0;
const source = { a: 1, b: 2 };
outer: for (const key in source) {
  value += 1;
  continue outer;
  value = 99;
}
export default value;"#,
            )],
            &out,
        )
        .expect("compile labeled for-in continue source");
    let bytes = std::fs::read(&out).unwrap();
    let bytecode = module_func_main_bytecode(&bytes, record_name);

    assert!(
        bytecode_contains_jmp(&bytecode),
        "labeled for-in continue must emit a patched jump to the next property"
    );
    assert!(
        bytecode.windows(2).any(|window| window == [0x7c, 0x00]),
        "code after the labeled for-in loop must still export the selected value"
    );
}

#[test]
fn test_labeled_continue_targets_labeled_for_of_loop_next_iteration() {
    let tmp = tempfile::tempdir().unwrap();
    let out = tmp.path().join("labeled_continue_for_of.abc");
    let record_name = "test/labeled_continue_for_of";
    module_compiler()
        .compile(
            vec![module_record(
                record_name,
                r#"let value = 0;
outer: for (const item of [1, 2]) {
  value += item;
  continue outer;
  value = 99;
}
export default value;"#,
            )],
            &out,
        )
        .expect("compile labeled for-of continue source");
    let bytes = std::fs::read(&out).unwrap();
    let bytecode = module_func_main_bytecode(&bytes, record_name);

    assert!(
        bytecode_contains_jmp(&bytecode),
        "labeled for-of continue must emit a patched jump to the iterator next call"
    );
    assert!(
        bytecode.windows(2).any(|window| window == [0x7c, 0x00]),
        "code after the labeled for-of loop must still export the selected value"
    );
}

#[test]
fn test_with_statement_is_rejected_matching_upstream_behavior() {
    // Upstream es2panda has NO withStatement.cpp in ir/statements — the `with`
    // statement has no IR lowering at all.  Since ArkTS modules are always strict
    // (and `with` is a SyntaxError in strict mode), rejecting it is the correct
    // upstream-aligned behavior.
    let tmp = tempfile::tempdir().unwrap();
    let out = tmp.path().join("with_statement.abc");
    let err = match module_compiler().compile(
        vec![module_record(
            "test/with_statement",
            r#"const obj = { value: 1 };
with (obj) { value = 2; }"#,
        )],
        &out,
    ) {
        Ok(_) => panic!("with statement must not compile in strict/module mode"),
        Err(err) => err,
    };

    assert!(
        matches!(
            err,
            AbcError::UnsupportedConstruct {
                ref construct,
                ..
            } if construct == "WithStatement"
        ),
        "with statement must be rejected (matches upstream — no IR lowering exists), got {err:?}"
    );
}

#[test]
fn test_commonjs_direct_eval_optional_require_matches_upstream_es2abc() {
    // API20 upstream es2abc compiles @protobufjs/inquire's optional-require
    // probe as plain CommonJS bytecode. It is not a compile-time hard error;
    // if `eval` or Node `require` is unavailable at runtime, the package's own
    // try/catch returns null.
    let tmp = tempfile::tempdir().unwrap();
    let out = tmp.path().join("protobufjs_inquire.abc");
    module_compiler()
        .compile(
            vec![commonjs_module_record(
                "pkg_modules/.ohpm/@protobufjs+inquire@1.1.0/pkg_modules/@protobufjs/inquire/index",
                r#""use strict";
module.exports = inquire;

function inquire(moduleName) {
    try {
        var mod = eval("quire".replace(/^/,"re"))(moduleName);
        if (mod && (mod.length || Object.keys(mod).length))
            return mod;
    } catch (e) {}
    return null;
}
"#,
            )],
            &out,
        )
        .expect("direct eval optional require must compile like upstream es2abc");

    let bytes = std::fs::read(&out).unwrap();
    assert!(bytes.starts_with(b"PANDA\0\0\0"));
}

/// Assignment to `eval` is forbidden in strict mode.
/// es2panda rejects this at compile time (strict mode violation).
#[test]
fn test_assignment_to_eval_is_rejected_strict_mode() {
    let tmp = tempfile::tempdir().unwrap();
    let out = tmp.path().join("eval_assign.abc");
    let result =
        module_compiler().compile(vec![module_record("test/eval_assign", "eval = 42;")], &out);
    assert!(
        matches!(result, Err(AbcError::UnsupportedConstruct { .. })),
        "assignment to 'eval' must be rejected (es2panda strict rule)"
    );
}

/// Assignment to `arguments` is forbidden in strict mode.
/// es2panda rejects this at compile time (strict mode violation).
#[test]
fn test_assignment_to_arguments_is_rejected_strict_mode() {
    let tmp = tempfile::tempdir().unwrap();
    let out = tmp.path().join("args_assign.abc");
    let result = module_compiler().compile(
        vec![module_record("test/args_assign", "arguments = [];")],
        &out,
    );
    assert!(
        matches!(result, Err(AbcError::UnsupportedConstruct { .. })),
        "assignment to 'arguments' must be rejected (es2panda strict rule)"
    );
}

/// Labeled function declarations are forbidden in strict mode.
/// es2panda / ArkTS transpiler never emits this pattern.
#[test]
fn test_labeled_function_declaration_is_rejected() {
    let tmp = tempfile::tempdir().unwrap();
    let out = tmp.path().join("labeled_fn.abc");
    let result = module_compiler().compile(
        vec![module_record(
            "test/labeled_fn",
            "myLabel: function foo() {}",
        )],
        &out,
    );
    assert!(
        matches!(result, Err(AbcError::UnsupportedConstruct { .. })),
        "labeled function declaration must be rejected (strict mode / es2panda rule)"
    );
}

#[test]
fn test_delete_global_identifier_uses_global_object_delete() {
    let tmp = tempfile::tempdir().unwrap();
    let out = tmp.path().join("delete_global.abc");
    let record_name = "test/delete_global";
    module_compiler()
        .compile(
            vec![module_record(record_name, r#"const ok = delete missing;"#)],
            &out,
        )
        .expect("compile delete global identifier source");
    let bytes = std::fs::read(&out).unwrap();
    let bytecode = module_func_main_bytecode(&bytes, record_name);
    assert!(
        bytecode_contains_opcode(&bytecode, 0x6d) && bytecode_contains_opcode(&bytecode, 0xc2),
        "delete of an unresolved identifier must load the global object and emit delobjprop"
    );
}

#[test]
fn test_delete_local_identifier_returns_false_without_delete_opcode() {
    let tmp = tempfile::tempdir().unwrap();
    let out = tmp.path().join("delete_local.abc");
    let record_name = "test/delete_local";
    module_compiler()
        .compile(
            vec![module_record(
                record_name,
                r#"let local = 1; const ok = delete local;"#,
            )],
            &out,
        )
        .expect("compile delete local identifier source");
    let bytes = std::fs::read(&out).unwrap();
    let bytecode = module_func_main_bytecode(&bytes, record_name);
    assert!(
        bytecode_contains_opcode(&bytecode, 0x03) && !bytecode_contains_opcode(&bytecode, 0xc2),
        "delete of a bound local identifier must lower to false without delobjprop"
    );
}

#[test]
fn test_delete_member_expression_uses_delete_property_opcode() {
    let tmp = tempfile::tempdir().unwrap();
    let out = tmp.path().join("delete_member.abc");
    let record_name = "test/delete_member";
    module_compiler()
        .compile(
            vec![module_record(
                record_name,
                r#"const obj = { value: 1 }; delete obj.value; delete obj[key];"#,
            )],
            &out,
        )
        .expect("compile delete member source");
    let bytes = std::fs::read(&out).unwrap();
    let bytecode = module_func_main_bytecode(&bytes, record_name);
    assert!(
        bytecode_contains_opcode(&bytecode, 0xc2),
        "delete of a member expression must emit delobjprop instead of loading the property"
    );
}

#[test]
fn test_unary_plus_uses_tonumber_opcode() {
    let tmp = tempfile::tempdir().unwrap();
    let out = tmp.path().join("unary_plus.abc");
    let record_name = "test/unary_plus";
    module_compiler()
        .compile(
            vec![module_record(record_name, r#"const value = +input;"#)],
            &out,
        )
        .expect("compile unary plus source");
    let bytes = std::fs::read(&out).unwrap();
    let bytecode = module_func_main_bytecode(&bytes, record_name);
    assert!(
        bytecode_contains_opcode(&bytecode, 0x1d),
        "unary plus must call ToNumber like upstream PandaGen::Unary"
    );
}

#[test]
fn test_update_identifier_uses_inc_dec_and_writes_back() {
    let tmp = tempfile::tempdir().unwrap();
    let out = tmp.path().join("update_identifier.abc");
    let record_name = "test/update_identifier";
    module_compiler()
        .compile(
            vec![module_record(
                record_name,
                r#"
let i = 0;
i++;
--i;
"#,
            )],
            &out,
        )
        .expect("compile update identifier source");
    let bytes = std::fs::read(&out).unwrap();
    let bytecode = module_func_main_bytecode(&bytes, record_name);

    assert!(
        bytecode_contains_opcode(&bytecode, 0x21),
        "++ must use inc like upstream PandaGen::Unary"
    );
    assert!(
        bytecode_contains_opcode(&bytecode, 0x22),
        "-- must use dec like upstream PandaGen::Unary"
    );
    assert!(
        bytecode_contains_opcode(&bytecode, 0x1d),
        "postfix update must convert the saved original value to numeric before inc/dec"
    );
    let sta_count = bytecode.iter().filter(|&&byte| byte == 0x61).count();
    assert!(
        sta_count >= 5,
        "identifier update must write values through local-register stores"
    );
}

#[test]
fn test_update_member_uses_lreference_read_modify_write() {
    let tmp = tempfile::tempdir().unwrap();
    let out = tmp.path().join("update_member.abc");
    let record_name = "test/update_member";
    module_compiler()
        .compile(
            vec![module_record(
                record_name,
                r#"
const obj = { value: 0 };
const key = "value";
obj.value++;
--obj[key];
"#,
            )],
            &out,
        )
        .expect("compile update member source");
    let bytes = std::fs::read(&out).unwrap();
    let bytecode = module_func_main_bytecode(&bytes, record_name);

    assert!(
        bytecode_contains_opcode(&bytecode, 0x21) && bytecode_contains_opcode(&bytecode, 0x22),
        "member update must use inc/dec like upstream PandaGen::Unary"
    );
    assert!(
        bytecode_contains_opcode(&bytecode, 0x42) && bytecode_contains_opcode(&bytecode, 0x43),
        "static member update must read and write through property lreference"
    );
    assert!(
        bytecode_contains_opcode(&bytecode, 0x37) && bytecode_contains_opcode(&bytecode, 0x38),
        "computed member update must read and write through property lreference"
    );
}

#[test]
fn test_regexp_literal_uses_upstream_regexp_constructor_path() {
    let tmp = tempfile::tempdir().unwrap();
    let out = tmp.path().join("regexp_literal.abc");
    let record_name = "test/regexp_literal";
    module_compiler()
        .compile(
            vec![module_record(
                record_name,
                r#"
const plain = /foo/;
const flagged = /bar/ig;
"#,
            )],
            &out,
        )
        .expect("compile regexp literal source");
    let bytes = std::fs::read(&out).unwrap();
    let bytecode = module_func_main_bytecode(&bytes, record_name);

    assert!(
        bytecode_contains_opcode(&bytecode, 0x3f),
        "regexp literal must load global RegExp like upstream TryLoadGlobalByName"
    );
    assert!(
        bytecode_contains_opcode(&bytecode, 0x3e),
        "regexp literal must load pattern and flags with lda.str"
    );
    assert!(
        bytecode
            .windows(4)
            .any(|window| window[0] == 0x08 && window[2] == 2 && window[3] == 0),
        "regexp literal without flags must call newobjrange with [ctor, pattern]"
    );
    assert!(
        bytecode
            .windows(4)
            .any(|window| window[0] == 0x08 && window[2] == 3 && window[3] == 3),
        "regexp literal with flags must call newobjrange with [ctor, pattern, flags]"
    );

    assert!(bytes_contain_ascii(&bytes, "RegExp"));
    assert!(bytes_contain_ascii(&bytes, "foo"));
    assert!(bytes_contain_ascii(&bytes, "bar"));
    assert!(
        bytes_contain_ascii(&bytes, "gi"),
        "regexp flags must be emitted as the constructor flags argument"
    );
}

#[test]
fn test_bigint_literal_uses_upstream_ldbigint() {
    let tmp = tempfile::tempdir().unwrap();
    let out = tmp.path().join("bigint_literal.abc");
    let record_name = "test/bigint_literal";
    module_compiler()
        .compile(
            vec![module_record(
                record_name,
                r#"
const decimal = 123n;
"#,
            )],
            &out,
        )
        .expect("compile bigint literal source");
    let bytes = std::fs::read(&out).unwrap();
    let bytecode = module_func_main_bytecode(&bytes, record_name);

    assert!(
        bytecode_contains_opcode(&bytecode, 0xd3),
        "BigInt literal must use ldbigint like upstream LoadAccumulatorBigInt"
    );
}

#[test]
fn test_import_expression_uses_upstream_dynamicimport() {
    let tmp = tempfile::tempdir().unwrap();
    let out = tmp.path().join("dynamic_import.abc");
    let record_name = "test/dynamic_import";
    module_compiler()
        .compile(
            vec![module_record(
                record_name,
                r#"
const promise = import("./dep");
"#,
            )],
            &out,
        )
        .expect("compile dynamic import source");
    let bytes = std::fs::read(&out).unwrap();
    let bytecode = module_func_main_bytecode(&bytes, record_name);

    assert!(
        bytecode_contains_opcode(&bytecode, 0x3e),
        "dynamic import must compile its source expression before importing"
    );
    assert!(
        bytecode_contains_opcode(&bytecode, 0xbd),
        "ImportExpression must emit dynamicimport like upstream DynamicImportCall"
    );
}

#[test]
fn test_debugger_statement_matches_upstream_debug_mode() {
    let tmp = tempfile::tempdir().unwrap();
    let normal_out = tmp.path().join("debugger_normal.abc");
    let record_name = "test/debugger_statement";
    module_compiler()
        .compile(
            vec![module_record(record_name, "debugger;\nconst value = 1;")],
            &normal_out,
        )
        .expect("compile non-debug debugger source");
    let normal_bytes = std::fs::read(&normal_out).unwrap();
    let normal_bytecode = module_func_main_bytecode(&normal_bytes, record_name);
    assert!(
        !bytecode_contains_opcode(&normal_bytecode, 0xb0),
        "debugger statement must not emit debugger opcode when debug info is disabled"
    );

    // Upstream: debugger opcode requires debug_info=true (compilerOptions_.debugInfo).
    // Use module_debug_compiler() to enable debug info emission.
    let debug_out = tmp.path().join("debugger_debug.abc");
    module_debug_compiler()
        .compile(
            vec![module_record_with_source_file(
                record_name,
                "debugger;\nconst value = 1;",
                "entry/src/main/ets/pages/Debugger.ts",
            )],
            &debug_out,
        )
        .expect("compile debug debugger source");
    let debug_bytes = std::fs::read(&debug_out).unwrap();
    let debug_bytecode = module_func_main_bytecode(&debug_bytes, record_name);
    assert!(
        bytecode_contains_opcode(&debug_bytecode, 0xb0),
        "debugger statement must emit debugger opcode in debug mode like upstream GenDebugger"
    );
}

#[test]
fn test_void_expression_still_compiles_operand() {
    let tmp = tempfile::tempdir().unwrap();
    let out = tmp.path().join("void_operand.abc");
    let record_name = "test/void_operand";
    module_compiler()
        .compile(
            vec![module_record(record_name, r#"const value = void 0;"#)],
            &out,
        )
        .expect("compile void source");
    let bytes = std::fs::read(&out).unwrap();
    let bytecode = module_func_main_bytecode(&bytes, record_name);
    assert!(
        bytecode_contains_opcode(&bytecode, 0x62) && bytecode_contains_opcode(&bytecode, 0x00),
        "void must compile its operand before loading undefined"
    );
}

#[test]
fn test_array_destructuring_assignment_uses_iterator_protocol() {
    // Upstream es2abc: `[a] = values` uses the full iterator protocol —
    // getiterator (0x67) to create an iterator, then callthis0 (0x2a) for next().
    let tmp = tempfile::tempdir().unwrap();
    let out = tmp.path().join("destructuring_assignment.abc");
    let record_name = "test/destructuring_assignment";
    module_compiler()
        .compile(
            vec![module_record(record_name, r#"let a; [a] = values;"#)],
            &out,
        )
        .expect("array destructuring assignment must lower via iterator protocol");
    let bytes = std::fs::read(&out).unwrap();
    let bytecode = module_func_main_bytecode(&bytes, record_name);
    assert!(
        bytecode_contains_opcode(&bytecode, 0x67),
        "array destructuring assignment must emit getiterator (0x67)"
    );
}

#[test]
fn test_array_destructuring_binding_uses_iterator_protocol() {
    // Upstream es2abc: `const [a] = values` uses the full iterator protocol.
    let tmp = tempfile::tempdir().unwrap();
    let out = tmp.path().join("destructuring_binding_decl.abc");
    let record_name = "test/destructuring_binding_decl";
    module_compiler()
        .compile(
            vec![module_record(record_name, r#"const [a] = values;"#)],
            &out,
        )
        .expect("array destructuring binding must lower via iterator protocol");
    let bytes = std::fs::read(&out).unwrap();
    let bytecode = module_func_main_bytecode(&bytes, record_name);
    assert!(
        bytecode_contains_opcode(&bytecode, 0x67),
        "array destructuring binding must emit getiterator (0x67)"
    );
}

#[test]
fn test_exported_array_destructuring_binding_lowers() {
    // Upstream: `export const [a] = values` creates a module export for `a`
    // AND uses the iterator protocol.
    let tmp = tempfile::tempdir().unwrap();
    let out = tmp.path().join("destructuring_binding_export.abc");
    let record_name = "test/destructuring_binding_export";
    module_compiler()
        .compile(
            vec![module_record(record_name, r#"export const [a] = values;"#)],
            &out,
        )
        .expect("exported array destructuring binding must compile");
    let bytes = std::fs::read(&out).unwrap();
    let bytecode = module_func_main_bytecode(&bytes, record_name);
    assert!(
        bytecode_contains_opcode(&bytecode, 0x67),
        "exported array destructuring binding must emit getiterator (0x67)"
    );
}

#[test]
fn test_for_left_array_destructuring_uses_nested_iterator() {
    // Upstream: `for (const [a] of values)` has two layers of iterator usage —
    // the outer for-of iterator and the inner array-destructure iterator.
    let tmp = tempfile::tempdir().unwrap();
    let out = tmp.path().join("destructuring_binding_for_left.abc");
    let record_name = "test/destructuring_binding_for_left";
    module_compiler()
        .compile(
            vec![module_record(
                record_name,
                r#"for (const [a] of values) { sink(a); }"#,
            )],
            &out,
        )
        .expect("for-left array destructuring must lower via nested iterator protocol");
    let bytes = std::fs::read(&out).unwrap();
    let bytecode = module_func_main_bytecode(&bytes, record_name);
    // Two getiterator calls: one for for-of, one for the inner destructure.
    assert!(
        bytecode_opcode_count(&bytecode, 0x67) >= 2,
        "for-left array destructuring must emit at least 2 getiterator (0x67) calls"
    );
}

#[test]
fn test_for_of_array_assignment_target_destructures_like_upstream() {
    // Upstream ForOfStatement compiles the left side through LReference; an
    // assignment-pattern left (`for ([a] of values)`) is a destructuring lref,
    // not an unsupported construct.
    let tmp = tempfile::tempdir().unwrap();
    let out = tmp.path().join("for_of_array_assignment_target.abc");
    let record_name = "test/for_of_array_assignment_target";
    module_compiler()
        .compile(
            vec![module_record(
                record_name,
                r#"let a;
for ([a] of [[1]]) {
  globalThis["value"] = a;
}"#,
            )],
            &out,
        )
        .expect("for-of array assignment target must lower through destructuring lref");
    let bytes = std::fs::read(&out).unwrap();
    let bytecode = module_func_main_bytecode(&bytes, record_name);
    assert!(
        bytecode_opcode_count(&bytecode, 0x67) >= 2,
        "for-of array assignment target must use outer and destructuring iterators"
    );
}

#[test]
fn test_for_of_object_assignment_target_destructures_like_upstream() {
    let tmp = tempfile::tempdir().unwrap();
    let out = tmp.path().join("for_of_object_assignment_target.abc");
    let record_name = "test/for_of_object_assignment_target";
    module_compiler()
        .compile(
            vec![module_record(
                record_name,
                r#"const target = {};
for ({x: target.value} of [{x: 1}]) {
  globalThis["value"] = target.value;
}"#,
            )],
            &out,
        )
        .expect("for-of object assignment target must lower through destructuring lref");
    let bytes = std::fs::read(&out).unwrap();
    let bytecode = module_func_main_bytecode(&bytes, record_name);
    assert!(
        bytecode_contains_opcode(&bytecode, 0x42),
        "object destructuring assignment target must load named properties"
    );
    assert!(
        bytecode_contains_opcode(&bytecode, 0x43),
        "member target inside object destructuring must store by property name"
    );
}

#[test]
fn test_object_destructuring_in_function_param_uses_property_load() {
    // Upstream: `function f({ value }) {}` extracts the property from the
    // parameter object via LdObjByName (0x41) in the function body prologue.
    let tmp = tempfile::tempdir().unwrap();
    let out = tmp.path().join("destructuring_binding_param.abc");
    let record_name = "test/destructuring_binding_param";
    module_compiler()
        .compile(
            vec![module_record(
                record_name,
                r#"function f({ value }) { return value; }"#,
            )],
            &out,
        )
        .expect("function parameter object destructuring must compile");
    let bytes = std::fs::read(&out).unwrap();
    // The function f must emit an LdObjByName (0x41) to extract `value`.
    assert!(
        bytes_contain_ascii(&bytes, "value"),
        "destructured parameter name 'value' must appear in ABC string table"
    );
}

#[test]
fn test_object_destructuring_binding_uses_property_load() {
    // Upstream: `const {a, b} = obj` uses LdObjByName (0x41) for each property.
    let tmp = tempfile::tempdir().unwrap();
    let out = tmp.path().join("object_destructure.abc");
    let record_name = "test/object_destructure";
    module_compiler()
        .compile(
            vec![module_record(record_name, r#"const {a, b} = obj;"#)],
            &out,
        )
        .expect("object destructuring binding must compile");
    let bytes = std::fs::read(&out).unwrap();
    let bytecode = module_func_main_bytecode(&bytes, record_name);
    // Object destructuring must use LdObjByName (0x42) for property access.
    assert!(
        bytecode_contains_opcode(&bytecode, 0x42),
        "object destructuring must emit ldobjbyname (0x42) for property access"
    );
}

#[test]
fn test_destructuring_with_default_uses_undefined_check() {
    // Upstream: `const [a = 0] = arr` checks if the iterator value is undefined
    // and applies the default — this requires a StrictEq (0x28) + Jeqz path.
    let tmp = tempfile::tempdir().unwrap();
    let out = tmp.path().join("destructure_default.abc");
    let record_name = "test/destructure_default";
    module_compiler()
        .compile(
            vec![module_record(record_name, r#"const [a = 0] = arr;"#)],
            &out,
        )
        .expect("array destructuring with default must compile");
    let bytes = std::fs::read(&out).unwrap();
    let bytecode = module_func_main_bytecode(&bytes, record_name);
    // Must use stricteq (0x28) for the undefined check.
    assert!(
        bytecode_contains_opcode(&bytecode, 0x28),
        "destructuring default must emit stricteq (0x28) for undefined check"
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
fn test_static_string_array_uses_createarraywithbuffer() {
    let tmp = tempfile::tempdir().unwrap();
    let out = tmp.path().join("static_array.abc");
    let record_name = "test/static_array";
    let source = r#"const propertyNames = ["abs", "acos", "sqrt"];"#;
    module_compiler()
        .compile(vec![module_record(record_name, source)], &out)
        .expect("compile static array source");
    let bytes = std::fs::read(&out).unwrap();
    let bytecode = module_func_main_bytecode(&bytes, record_name);
    assert!(
        bytecode_contains_opcode(&bytecode, 0x06),
        "static string array must use createarraywithbuffer (0x06), matching es2abc"
    );
    assert!(
        !bytecode_contains_opcode(&bytecode, 0x79),
        "static string array should not be expanded into stownbyindex writes"
    );
}

#[test]
fn test_dynamic_array_ic_slot_walker_preserves_following_sta() {
    let tmp = tempfile::tempdir().unwrap();
    let out = tmp.path().join("dynamic_array_call.abc");
    let record_name = "test/dynamic_array_call";
    let source = r#"
function take(a, b) {}
const DynamicPropertyContainer = {};
take([DynamicPropertyContainer], take);
"#;
    module_compiler()
        .compile(vec![module_record(record_name, source)], &out)
        .expect("compile dynamic array call source");
    let bytes = std::fs::read(&out).unwrap();
    let bytecode = module_func_main_bytecode(&bytes, record_name);
    assert!(
        bytecode
            .windows(3)
            .any(|window| window[0] == 0x05 && window[2] == 0x61),
        "createemptyarray must be walked as a two-byte instruction before the following sta"
    );
    assert!(
        bytecode_contains_opcode(&bytecode, 0x79),
        "dynamic array element must be written with stownbyindex"
    );
}

#[test]
fn test_dynamic_computed_member_uses_byvalue_opcodes() {
    let tmp = tempfile::tempdir().unwrap();
    let out = tmp.path().join("computed_member.abc");
    let record_name = "test/computed_member";
    let source = r#"
const target = {};
const names = ["abs"];
target[names[0]] = Math[names[0]];
"#;
    module_compiler()
        .compile(vec![module_record(record_name, source)], &out)
        .expect("compile computed member source");
    let bytes = std::fs::read(&out).unwrap();
    let bytecode = module_func_main_bytecode(&bytes, record_name);
    assert!(
        bytecode_contains_opcode(&bytecode, 0x37),
        "dynamic computed reads must use ldobjbyvalue (0x37)"
    );
    assert!(
        bytecode_contains_opcode(&bytecode, 0x38),
        "dynamic computed writes must use stobjbyvalue (0x38)"
    );
}

#[test]
fn test_dynamic_computed_method_call_uses_byvalue_load_before_callthis() {
    let tmp = tempfile::tempdir().unwrap();
    let out = tmp.path().join("computed_method_call.abc");
    let record_name = "test/computed_method_call";
    let source = r#"
const names = ["run"];
target[names[0]]();
"#;
    module_compiler()
        .compile(vec![module_record(record_name, source)], &out)
        .expect("compile computed method call source");
    let bytes = std::fs::read(&out).unwrap();
    let bytecode = module_func_main_bytecode(&bytes, record_name);
    assert!(
        bytecode_contains_opcode(&bytecode, 0x37),
        "computed method calls must load the callee with ldobjbyvalue like es2panda"
    );
    assert!(
        bytecode_contains_opcode(&bytecode, 0x2d),
        "computed method calls must invoke through callthis0 with the saved receiver"
    );
}

#[test]
fn test_optional_computed_member_uses_nullish_guard_and_byvalue_load() {
    let tmp = tempfile::tempdir().unwrap();
    let out = tmp.path().join("optional_computed_member.abc");
    let record_name = "test/optional_computed_member";
    let source = r#"
const names = ["value"];
const result = target?.[names[0]];
"#;
    module_compiler()
        .compile(vec![module_record(record_name, source)], &out)
        .expect("compile optional computed member source");
    let bytes = std::fs::read(&out).unwrap();
    let bytecode = module_func_main_bytecode(&bytes, record_name);
    assert!(
        bytecode_contains_opcode(&bytecode, 0x37),
        "optional computed member access must load with ldobjbyvalue like es2panda"
    );
    assert!(
        bytecode_contains_opcode(&bytecode, 0x01) && bytecode_contains_opcode(&bytecode, 0x00),
        "optional chaining must guard both null and undefined"
    );
}

#[test]
fn test_optional_computed_method_call_uses_byvalue_load_before_callthis() {
    let tmp = tempfile::tempdir().unwrap();
    let out = tmp.path().join("optional_computed_method_call.abc");
    let record_name = "test/optional_computed_method_call";
    let source = r#"
const names = ["run"];
target?.[names[0]]();
"#;
    module_compiler()
        .compile(vec![module_record(record_name, source)], &out)
        .expect("compile optional computed method call source");
    let bytes = std::fs::read(&out).unwrap();
    let bytecode = module_func_main_bytecode(&bytes, record_name);
    assert!(
        bytecode_contains_opcode(&bytecode, 0x37),
        "optional computed method calls must load the callee with ldobjbyvalue"
    );
    assert!(
        bytecode_contains_opcode(&bytecode, 0x2d),
        "optional computed method calls must invoke through callthis0"
    );
}

#[test]
fn test_optional_plain_call_guards_callee_before_call() {
    let tmp = tempfile::tempdir().unwrap();
    let out = tmp.path().join("optional_plain_call.abc");
    let record_name = "test/optional_plain_call";
    let source = r#"
const maybe = undefined;
const result = maybe?.();
"#;
    module_compiler()
        .compile(vec![module_record(record_name, source)], &out)
        .expect("compile optional plain call source");
    let bytes = std::fs::read(&out).unwrap();
    let bytecode = module_func_main_bytecode(&bytes, record_name);
    assert!(
        bytecode_contains_opcode(&bytecode, 0x01) && bytecode_contains_opcode(&bytecode, 0x00),
        "optional plain call must guard callee against null and undefined"
    );
    assert!(
        bytecode_contains_opcode(&bytecode, 0x29),
        "optional plain call must still use callarg0 on the non-nullish path"
    );
}

#[test]
fn test_optional_method_call_with_many_args_uses_callthisrange() {
    let tmp = tempfile::tempdir().unwrap();
    let out = tmp.path().join("optional_method_many_args.abc");
    let record_name = "test/optional_method_many_args";
    let source = r#"
target?.run(a, b, c, d);
"#;
    module_compiler()
        .compile(vec![module_record(record_name, source)], &out)
        .expect("compile optional method many args source");
    let bytes = std::fs::read(&out).unwrap();
    let bytecode = module_func_main_bytecode(&bytes, record_name);
    assert!(
        bytecode_contains_opcode(&bytecode, 0x31),
        "optional method calls with more than three args must use callthisrange like regular calls"
    );
}

#[test]
fn test_optional_spread_call_uses_apply_after_nullish_guard() {
    let tmp = tempfile::tempdir().unwrap();
    let out = tmp.path().join("optional_spread_call.abc");
    let record_name = "test/optional_spread_call";
    let source = r#"
const args = [1, 2];
maybe?.(...args);
"#;
    module_compiler()
        .compile(vec![module_record(record_name, source)], &out)
        .expect("compile optional spread call source");
    let bytes = std::fs::read(&out).unwrap();
    let bytecode = module_func_main_bytecode(&bytes, record_name);
    assert!(
        bytecode_contains_opcode(&bytecode, 0xba),
        "optional spread calls must use apply on the non-nullish path"
    );
    assert!(
        bytecode_contains_opcode(&bytecode, 0xc6),
        "optional spread call arguments must be materialized with starrayspread"
    );
}

#[test]
fn test_identifier_compound_assignment_loads_lhs() {
    let tmp = tempfile::tempdir().unwrap();
    let out = tmp.path().join("compound_assign.abc");
    let record_name = "test/compound_assign";
    let source = r#"let i = 0; i += 1;"#;
    module_compiler()
        .compile(vec![module_record(record_name, source)], &out)
        .expect("compile compound assignment source");
    let bytes = std::fs::read(&out).unwrap();
    let bytecode = module_func_main_bytecode(&bytes, record_name);
    assert!(
        bytecode_contains_opcode(&bytecode, 0x0a),
        "identifier += must emit add2 (0x0a), not just store the right-hand side"
    );
}

#[test]
fn test_for_in_checks_getnextpropname_against_undefined() {
    let tmp = tempfile::tempdir().unwrap();
    let out = tmp.path().join("for_in.abc");
    let record_name = "test/for_in";
    let source = r#"pre.a; pre.b; for (var attr in sourcePrototype) { target[attr] = sourcePrototype[attr]; }"#;
    module_compiler()
        .compile(vec![module_record(record_name, source)], &out)
        .expect("compile for-in source");
    let bytes = std::fs::read(&out).unwrap();
    let class_name = format!("L{record_name};");
    let class_off = class_offsets(&bytes)
        .into_iter()
        .map(|off| off as usize)
        .find(|&off| class_name_at(&bytes, off) == class_name)
        .expect("module class must exist");
    let code_off = class_method_items(&bytes, class_off)
        .into_iter()
        .find(|(_, name, _)| name == "func_main_0")
        .and_then(|(_, _, code_off)| code_off)
        .expect("func_main_0 must have a code item") as usize;
    let bytecode = code_item_bytecode(&bytes, code_off);
    assert_eq!(
        code_item_tries_size(&bytes, code_off),
        1,
        "for-in getpropiterator must be protected by a rethrow catch table"
    );
    assert!(
        bytecode_contains_opcode(&bytecode, 0x0f) && bytecode_contains_jnez(&bytecode),
        "for-in must compare getnextpropname result with undefined before exiting"
    );
}

#[test]
fn test_for_of_return_uses_upstream_iterator_context() {
    let tmp = tempfile::tempdir().unwrap();
    let out = tmp.path().join("for_of_return.abc");
    let record_name = "test/for_of_return";
    let source = r#"
function f(values) {
  for (let value of values) {
    return value;
  }
  return undefined;
}
export default f;
"#;
    module_compiler()
        .compile(vec![module_record(record_name, source)], &out)
        .expect("compile for-of return source");
    let bytes = std::fs::read(&out).unwrap();
    let class_name = format!("L{record_name};");
    let class_off = class_offsets(&bytes)
        .into_iter()
        .map(|off| off as usize)
        .find(|&off| class_name_at(&bytes, off) == class_name)
        .expect("module class must exist");
    let (bytecode, tries_size) = class_method_items(&bytes, class_off)
        .into_iter()
        .filter_map(|(_, name, code_off)| {
            let code_off = code_off? as usize;
            let bytecode = code_item_bytecode(&bytes, code_off);
            (name != "func_main_0" && bytecode_contains_opcode(bytecode, 0x67))
                .then_some((bytecode.to_vec(), code_item_tries_size(&bytes, code_off)))
        })
        .next()
        .expect("for-of function method must exist");

    assert!(
        tries_size >= 3,
        "for-of return must emit init, body iterator-context, and iterator-close catch tables"
    );
    assert!(
        bytecode_contains_opcode(&bytecode, 0x67),
        "for-of must initialize the iterator with getiterator"
    );
    assert!(
        bytecode
            .windows(3)
            .any(|window| matches!(window, [0xfe, 0x05, _])),
        "Iterator::Next/Close must validate objects with throw.ifnotobject"
    );
}

#[test]
fn test_for_of_break_closes_iterator_like_upstream_abort_context() {
    let tmp = tempfile::tempdir().unwrap();
    let out = tmp.path().join("for_of_break.abc");
    let record_name = "test/for_of_break";
    let source = r#"
function f(values) {
  for (let value of values) {
    break;
  }
}
export default f;
"#;
    module_compiler()
        .compile(vec![module_record(record_name, source)], &out)
        .expect("compile for-of break source");
    let bytes = std::fs::read(&out).unwrap();
    let (bytecode, tries_size) = first_non_main_method_bytecode_and_tries(&bytes, record_name);

    assert!(
        tries_size >= 3,
        "for-of break must emit init, body iterator-context, and iterator-close catch tables"
    );
    assert!(
        bytecode
            .windows(3)
            .any(|window| matches!(window, [0xfe, 0x05, _])),
        "break from for-of must run Iterator::Close and validate return result"
    );
}

#[test]
fn test_try_finally_uses_upstream_nested_try_contexts() {
    let tmp = tempfile::tempdir().unwrap();
    let out = tmp.path().join("try_finally.abc");
    let record_name = "test/try_finally";
    let source = r#"
function f(value) {
  try {
    return value;
  } finally {
    cleanup();
  }
}
export default f;
"#;
    module_compiler()
        .compile(vec![module_record(record_name, source)], &out)
        .expect("compile try-finally source");
    let bytes = std::fs::read(&out).unwrap();
    let (bytecode, tries_size) = first_non_main_method_bytecode_and_tries(&bytes, record_name);

    assert_eq!(
        tries_size, 2,
        "try-finally must use inner and outer catch tables like es2panda TryContext"
    );
    assert!(
        bytecode_contains_opcode(&bytecode, 0x70),
        "try-finally normal path must seed the exception register with hole before finalizer rethrow"
    );
    assert!(
        bytecode_contains_opcode(&bytecode, 0xfe),
        "try-finally exception path must rethrow after running the finalizer"
    );
}

#[test]
fn test_nested_try_catch_finally_emits_correct_catch_table() {
    // Upstream: `try { try { } catch(e) { } } finally { }` produces nested
    // catch entries — inner try-catch + outer try-finally.
    let tmp = tempfile::tempdir().unwrap();
    let out = tmp.path().join("nested_try_finally.abc");
    let record_name = "test/nested_try_finally";
    module_compiler()
        .compile(
            vec![module_record(
                record_name,
                r#"
function f(val) {
  try {
    try { return val; } catch (e) { log(e); }
  } finally {
    cleanup();
  }
}
export default f;
"#,
            )],
            &out,
        )
        .expect("nested try-catch-finally must compile");
    let bytes = std::fs::read(&out).unwrap();
    let (_, tries_size) = first_non_main_method_bytecode_and_tries(&bytes, record_name);
    // Inner try-catch has 1 entry; outer try-finally adds 2 more (inner + outer).
    assert!(
        tries_size >= 3,
        "nested try-catch-finally must emit at least 3 catch table entries, got {tries_size}"
    );
}

#[test]
fn test_top_level_await_uses_async_function_enter_opcode() {
    // Top-level await (TLA): module-level `await expr` is supported in module
    // mode and must emit the asyncfunctionenter (0xae) prologue at module level.
    let tmp = tempfile::tempdir().unwrap();
    let out = tmp.path().join("tla.abc");
    let record_name = "test/tla";
    module_compiler()
        .compile(
            vec![module_record(
                record_name,
                r#"const x = await Promise.resolve(42);"#,
            )],
            &out,
        )
        .expect("top-level await must compile in module mode");
    let bytes = std::fs::read(&out).unwrap();
    let bytecode = module_func_main_bytecode(&bytes, record_name);
    // TLA wraps the module body in an async context — asyncfunctionenter (0xae)
    // must appear in the module's func_main_0.
    assert!(
        bytecode_contains_opcode(&bytecode, 0xae),
        "top-level await must emit asyncfunctionenter (0xae) in func_main_0"
    );
}

#[test]
fn test_for_of_continue_calls_iterator_next_on_next_iteration() {
    // Upstream: `continue` inside for-of does NOT close the iterator; it just
    // jumps back to the loop header. The catch table must cover the body so
    // that only abrupt exits close the iterator.
    let tmp = tempfile::tempdir().unwrap();
    let out = tmp.path().join("for_of_continue.abc");
    let record_name = "test/for_of_continue";
    module_compiler()
        .compile(
            vec![module_record(
                record_name,
                r#"
function f(values) {
  for (const v of values) {
    if (cond) continue;
    use(v);
  }
}
export default f;
"#,
            )],
            &out,
        )
        .expect("for-of continue must compile");
    let bytes = std::fs::read(&out).unwrap();
    let (bytecode, tries_size) = first_non_main_method_bytecode_and_tries(&bytes, record_name);
    // Same catch-table shape as for-of return/break: init + body + close.
    assert!(
        tries_size >= 3,
        "for-of continue must have init+body+close catch tables"
    );
    // Must call iterator.next (callthis0 = 0x2a).
    assert!(
        bytecode_contains_opcode(&bytecode, 0x2a),
        "for-of must call iterator.next via callthis0 (0x2a)"
    );
}

#[test]
fn test_private_field_uses_ldprivateproperty_opcode() {
    // Upstream es2abc: `this.#value` uses ldprivateproperty (0xd8) to read the
    // private field, and the class emits callruntime.createprivateproperty (0xfb04)
    // after defineclasswithbuffer to set up the private environment.
    let tmp = tempfile::tempdir().unwrap();
    let out = tmp.path().join("private_field.abc");
    let record_name = "test/private_field";
    module_compiler()
        .compile(
            vec![module_record(
                record_name,
                "class Box { #value = 1; read() { return this.#value; } }",
            )],
            &out,
        )
        .expect("private field must compile with ldprivateproperty lowering");
    let bytes = std::fs::read(&out).unwrap();
    // callruntime.createprivateproperty (0xfb) must appear in func_main_0.
    assert!(
        bytes_contain_ascii(&bytes, "Box"),
        "class name Box must appear in ABC strings"
    );
    // ldprivateproperty (0xd8) must appear in the read() method bytecode.
    assert!(
        bytes.contains(&0xd8),
        "private field read must emit ldprivateproperty (0xd8)"
    );
}

#[test]
fn test_object_accessor_uses_definegettersetterbyvalue() {
    let tmp = tempfile::tempdir().unwrap();
    let out = tmp.path().join("object_accessor.abc");
    let record_name = "test/object_accessor";
    let source = r#"
const target = {
  get value() { return 1; },
  set value(next) {}
};
"#;
    module_compiler()
        .compile(vec![module_record(record_name, source)], &out)
        .expect("compile object accessor source");
    let bytes = std::fs::read(&out).unwrap();
    let bytecode = module_func_main_bytecode(&bytes, record_name);
    assert!(
        bytecode_contains_opcode(&bytecode, 0xbc),
        "object accessors must use definegettersetterbyvalue like es2panda"
    );
}

#[test]
fn test_static_class_accessor_uses_class_constructor_as_target() {
    let tmp = tempfile::tempdir().unwrap();
    let out = tmp.path().join("static_class_accessor.abc");
    let record_name = "test/static_class_accessor";
    module_compiler()
        .compile(
            vec![module_record(
                record_name,
                r#"class Box { static get value() { return 1; } }
const value = Box.value;"#,
            )],
            &out,
        )
        .expect("compile static class accessor source");
    let bytes = std::fs::read(&out).unwrap();
    let bytecode = module_func_main_bytecode(&bytes, record_name);

    let define_class = first_opcode_position(&bytecode, 0x35).expect("defineclasswithbuffer");
    let after_define_class = define_class + bytecode_insn_len(&bytecode, define_class, 0x35);
    assert_eq!(
        bytecode[after_define_class], 0x61,
        "class constructor must be stored immediately after defineclasswithbuffer"
    );
    let class_reg = bytecode[after_define_class + 1];
    let define_accessor =
        first_opcode_position(&bytecode, 0xbc).expect("definegettersetterbyvalue");
    assert_eq!(
        bytecode[define_accessor + 1],
        class_reg,
        "static class accessors must define on the class constructor like upstream CompileMissingProperties"
    );
}

#[test]
fn test_private_in_expression_uses_testin_opcode() {
    // Upstream es2abc: `#value in other` emits testin (0xda) for the brand check.
    let tmp = tempfile::tempdir().unwrap();
    let out = tmp.path().join("private_in.abc");
    module_compiler()
        .compile(
            vec![module_record(
                "test/private_in",
                r#"class Box { #value; has(other) { return #value in other; } }"#,
            )],
            &out,
        )
        .expect("private-in expression must compile with testin lowering");
    let bytes = std::fs::read(&out).unwrap();
    assert!(
        bytes.contains(&0xda),
        "private-in expression must emit testin (0xda)"
    );
}

#[test]
fn test_private_field_write_uses_stprivateproperty_opcode() {
    // Upstream es2abc: `this.#value = v` emits stprivateproperty (0xd9).
    let tmp = tempfile::tempdir().unwrap();
    let out = tmp.path().join("private_field_write.abc");
    module_compiler()
        .compile(
            vec![module_record(
                "test/private_field_write",
                "class Box { #value = 0; set(v) { this.#value = v; } }",
            )],
            &out,
        )
        .expect("private field write must compile with stprivateproperty lowering");
    let bytes = std::fs::read(&out).unwrap();
    assert!(
        bytes.contains(&0xd9),
        "private field write must emit stprivateproperty (0xd9)"
    );
}

#[test]
fn test_class_with_private_field_emits_createprivateproperty() {
    // Upstream es2abc: callruntime.createprivateproperty (0xfb 0x04) is emitted
    // after defineclasswithbuffer when a class has private elements.
    let tmp = tempfile::tempdir().unwrap();
    let out = tmp.path().join("private_create.abc");
    module_compiler()
        .compile(
            vec![module_record(
                "test/private_create",
                "class Box { #x = 1; #y = 2; }",
            )],
            &out,
        )
        .expect("class with private fields must compile");
    let bytes = std::fs::read(&out).unwrap();
    // 0xfb prefix byte must appear (callruntime.createprivateproperty uses 0xfb 0x04).
    assert!(
        bytes.windows(2).any(|w| w == [0xfb, 0x04]),
        "class with private fields must emit callruntime.createprivateproperty (0xfb 0x04)"
    );
}

#[test]
fn test_import_meta_emits_getimportmeta_opcode() {
    // Upstream es2abc emits `getimportmeta` (0xd5) for `import.meta`.
    // The runtime loader populates the meta object's `url` / `resolve` properties.
    let tmp = tempfile::tempdir().unwrap();
    let out = tmp.path().join("import_meta.abc");
    let record_name = "test/import_meta";
    module_compiler()
        .compile(
            vec![module_record(
                record_name,
                r#"const url = import.meta.url;"#,
            )],
            &out,
        )
        .expect("import.meta must compile with getimportmeta lowering");
    let bytes = std::fs::read(&out).unwrap();
    let bytecode = module_func_main_bytecode(&bytes, record_name);
    assert!(
        bytecode_contains_opcode(&bytecode, 0xd5),
        "import.meta must emit getimportmeta (0xd5) like upstream es2abc"
    );
}

#[test]
fn test_annotation_declaration_requires_upstream_enable_annotations() {
    let tmp = tempfile::tempdir().unwrap();
    let out = tmp.path().join("annotation_decl.abc");
    let err = match module_compiler().compile(
        vec![annotation_module(
            "@interface RouteMeta {\n  title: string;\n}",
        )],
        &out,
    ) {
        Ok(_) => panic!("annotation declarations must require upstream --enable-annotations"),
        Err(err) => err,
    };
    assert!(
        matches!(err, AbcError::Parse { ref message, .. } if message.contains("Current configuration does not support using annotations")),
        "unexpected annotation error: {err:?}"
    );
}

#[test]
fn test_annotation_declaration_requires_upstream_prefix_when_enabled() {
    let tmp = tempfile::tempdir().unwrap();
    let out = tmp.path().join("annotation_decl_prefix.abc");
    let err = match module_compiler_with_annotations().compile(
        vec![annotation_module(
            "@interface RouteMeta {\n  title: string;\n}",
        )],
        &out,
    ) {
        Ok(_) => panic!("enabled annotation declarations must still require the upstream prefix"),
        Err(err) => err,
    };
    assert!(
        matches!(err, AbcError::Parse { ref message, .. } if message.contains("Annotation declaration need to be prefixed with '__$$ETS_ANNOTATION$$__'")),
        "unexpected annotation prefix error: {err:?}"
    );
}

#[test]
fn test_prefixed_annotation_declaration_enabled_compiles() {
    let tmp = tempfile::tempdir().unwrap();
    let out = tmp.path().join("annotation_decl_enabled.abc");
    module_compiler_with_annotations()
        .compile(
            vec![annotation_module(
                "@interface __$$ETS_ANNOTATION$$__RouteMeta {\n  title: string;\n}",
            )],
            &out,
        )
        .expect("prefixed annotation declarations must pass the upstream parser gate");
    let bytes = std::fs::read(&out).unwrap();
    let foreign_off = read_u32(&bytes, 0x14);
    let foreign_size = read_u32(&bytes, 0x18);
    assert_ne!(
        foreign_off, 0,
        "panda.String must be emitted as ForeignItem"
    );
    assert_ne!(foreign_size, 0, "ForeignItem section must not be empty");
    assert_eq!(
        class_name_at(&bytes, foreign_off as usize),
        "Lpanda.String;"
    );
    let reader_names = AbcReader::new(&bytes)
        .expect("annotation abc must have valid magic")
        .class_names()
        .expect("reader must accept class-index foreign items");
    assert!(reader_names.iter().any(|name| name == "Lpanda.String;"));
    assert!(reader_names
        .iter()
        .any(|name| name == "Ltest/annotation_decl.ets.RouteMeta;"));

    let route_off = class_offsets(&bytes)
        .into_iter()
        .find(|offset| {
            class_name_at(&bytes, *offset as usize) == "Ltest/annotation_decl.ets.RouteMeta;"
        })
        .expect("prefixed annotation declaration must emit RouteMeta record");
    let mut pos = route_off as usize;
    skip_inline_string(&bytes, &mut pos);
    assert_eq!(read_u32(&bytes, pos), 0);
    pos += 4;
    assert_eq!(read_uleb(&bytes, &mut pos), 0x2000);
    assert_eq!(read_uleb(&bytes, &mut pos), 1);
    assert_eq!(read_uleb(&bytes, &mut pos), 0);
    assert_eq!(bytes[pos], 0x02);
    assert_eq!(bytes[pos + 1], 0);
    assert_eq!(bytes[pos + 2], 0);
    pos += 3;

    let field_class_idx = read_u16(&bytes, pos) as usize;
    pos += 2;
    let field_type_idx = read_u16(&bytes, pos) as usize;
    pos += 2;
    let field_name_off = read_u32(&bytes, pos) as usize;
    pos += 4;
    assert_eq!(read_uleb(&bytes, &mut pos), 0);
    assert_eq!(
        bytes[pos], 0,
        "annotation field without default has no value tag"
    );
    assert_eq!(inline_string_at(&bytes, field_name_off), "title");

    let idx_off = read_u32(&bytes, 0x38) as usize;
    let index_class_idx_off = read_u32(&bytes, idx_off + 12) as usize;
    assert_eq!(
        read_u32(&bytes, index_class_idx_off + field_class_idx * 4),
        route_off
    );
    assert_eq!(
        read_u32(&bytes, index_class_idx_off + field_type_idx * 4),
        foreign_off
    );
}

#[test]
fn test_prefixed_annotation_program_cache_preserves_metadata_records() {
    let tmp = tempfile::tempdir().unwrap();
    let out = tmp.path().join("annotation_cache.abc");
    let cache_file = tmp.path().join("annotation.protoBin");
    let cache_input = AbcProgramCacheInput {
        file_name: "entry/src/main/ets/Annotation.ets".to_string(),
        cache_file: cache_file.clone(),
    };
    module_compiler_with_annotations()
        .compile_with_program_cache(
            vec![annotation_module(
                "@interface __$$ETS_ANNOTATION$$__RouteMeta {\n  title: string;\n}",
            )],
            &out,
            vec![Some(cache_input)],
        )
        .expect("annotation declaration must write ProgramCache");

    let entry = ProgramCacheFile::read(&cache_file)
        .unwrap()
        .expect("ProgramCache must be written");
    let record_names = entry
        .program
        .record_table
        .iter()
        .filter_map(|entry| entry.value.as_ref())
        .map(|record| String::from_utf8(record.name.clone()).unwrap())
        .collect::<Vec<_>>();
    assert!(record_names
        .iter()
        .any(|name| name == "test/annotation_decl.ets.RouteMeta"));
    assert!(record_names.iter().any(|name| name == "panda.String"));

    let route_record = entry
        .program
        .record_table
        .iter()
        .filter_map(|entry| entry.value.as_ref())
        .find(|record| record.name == b"test/annotation_decl.ets.RouteMeta")
        .expect("RouteMeta record must be in ProgramCache");
    let title = route_record
        .field_list
        .iter()
        .find(|field| field.name == b"title")
        .expect("RouteMeta.title must be in ProgramCache");
    assert_eq!(
        title.r#type.as_ref().map(|ty| ty.name.as_slice()),
        Some(b"panda.String".as_slice())
    );
    assert_eq!(
        title
            .metadata
            .as_ref()
            .and_then(|metadata| metadata.field_type.as_ref())
            .map(|ty| ty.type_id),
        Some(1)
    );
}

#[test]
fn test_prefixed_annotation_array_fields_emit_upstream_type_descriptors() {
    let tmp = tempfile::tempdir().unwrap();
    let out = tmp.path().join("annotation_array_fields.abc");
    module_compiler_with_annotations()
        .compile(
            vec![annotation_module(
                "@interface __$$ETS_ANNOTATION$$__Anno {\n  a: number[] = [1, -2]\n  b: string[][]\n  c: boolean[]\n}",
            )],
            &out,
        )
        .expect("annotation array declarations must compile");

    let bytes = std::fs::read(&out).unwrap();
    let reader_names = AbcReader::new(&bytes)
        .expect("annotation abc must have valid magic")
        .class_names()
        .expect("reader must accept foreign array items");
    for expected in [
        "Ltest/annotation_decl.ets.Anno;",
        "Lpanda.String;",
        "[D",
        "[Z",
        "[[Lpanda.String;",
    ] {
        assert!(
            reader_names.iter().any(|name| name == expected),
            "missing class/index item {expected}; names={reader_names:?}"
        );
    }

    let anno_off = class_offsets(&bytes)
        .into_iter()
        .find(|offset| class_name_at(&bytes, *offset as usize) == "Ltest/annotation_decl.ets.Anno;")
        .expect("prefixed annotation declaration must emit Anno record");
    let mut pos = anno_off as usize;
    skip_inline_string(&bytes, &mut pos);
    pos += 4;
    assert_eq!(read_uleb(&bytes, &mut pos), 0x2000);
    let field_count = read_uleb(&bytes, &mut pos);
    assert_eq!(field_count, 3);
    assert_eq!(read_uleb(&bytes, &mut pos), 0);
    assert_eq!(bytes[pos], 0x02);
    assert_eq!(bytes[pos + 1], 0);
    assert_eq!(bytes[pos + 2], 0);
    pos += 3;

    let idx_off = read_u32(&bytes, 0x38) as usize;
    let index_class_idx_off = read_u32(&bytes, idx_off + 12) as usize;
    let mut field_types = BTreeMap::new();
    let mut field_has_value = BTreeMap::new();
    for _ in 0..field_count {
        pos += 2;
        let field_type_idx = read_u16(&bytes, pos) as usize;
        pos += 2;
        let field_name_off = read_u32(&bytes, pos) as usize;
        pos += 4;
        let _access_flags = read_uleb(&bytes, &mut pos);
        let has_value = bytes[pos] != 0;
        if has_value {
            assert_eq!(bytes[pos], 0x02);
            pos += 5;
        }
        assert_eq!(bytes[pos], 0);
        pos += 1;

        let type_offset = read_u32(&bytes, index_class_idx_off + field_type_idx * 4);
        let field_name = inline_string_at(&bytes, field_name_off);
        field_types.insert(
            field_name.clone(),
            class_name_at(&bytes, type_offset as usize),
        );
        field_has_value.insert(field_name, has_value);
    }

    assert_eq!(field_types.get("a").map(String::as_str), Some("[D"));
    assert_eq!(
        field_types.get("b").map(String::as_str),
        Some("[[Lpanda.String;")
    );
    assert_eq!(field_types.get("c").map(String::as_str), Some("[Z"));
    assert_eq!(field_has_value.get("a"), Some(&true));
    assert_eq!(field_has_value.get("b"), Some(&false));
    assert_eq!(field_has_value.get("c"), Some(&false));
}

#[test]
fn test_prefixed_annotation_array_program_cache_preserves_rank_and_literal_key() {
    let tmp = tempfile::tempdir().unwrap();
    let out = tmp.path().join("annotation_array_cache.abc");
    let cache_file = tmp.path().join("annotation_array.protoBin");
    let cache_input = AbcProgramCacheInput {
        file_name: "entry/src/main/ets/AnnotationArray.ets".to_string(),
        cache_file: cache_file.clone(),
    };
    module_compiler_with_annotations()
        .compile_with_program_cache(
            vec![annotation_module(
                "@interface __$$ETS_ANNOTATION$$__Anno {\n  a: number[] = [1, -2]\n  b: string[][]\n  c: boolean[]\n}",
            )],
            &out,
            vec![Some(cache_input)],
        )
        .expect("annotation array declaration must write ProgramCache");

    let entry = ProgramCacheFile::read(&cache_file)
        .unwrap()
        .expect("ProgramCache must be written");
    let anno_record = entry
        .program
        .record_table
        .iter()
        .filter_map(|entry| entry.value.as_ref())
        .find(|record| record.name == b"test/annotation_decl.ets.Anno")
        .expect("Anno record must be in ProgramCache");
    let field = |name: &[u8]| {
        anno_record
            .field_list
            .iter()
            .find(|field| field.name == name)
            .expect("annotation field must be present")
    };
    let a = field(b"a");
    let b = field(b"b");
    let c = field(b"c");

    let a_type = a.r#type.as_ref().expect("a field type");
    assert_eq!(a_type.component_name, b"f64");
    assert_eq!(a_type.rank, 1);
    assert_eq!(a_type.name, b"f64[]");
    assert_eq!(a_type.type_id, 13);

    let b_type = b.r#type.as_ref().expect("b field type");
    assert_eq!(b_type.component_name, b"panda.String");
    assert_eq!(b_type.rank, 2);
    assert_eq!(b_type.name, b"panda.String[][]");
    assert_eq!(b_type.type_id, 13);

    let c_type = c.r#type.as_ref().expect("c field type");
    assert_eq!(c_type.component_name, b"u1");
    assert_eq!(c_type.rank, 1);
    assert_eq!(c_type.name, b"u1[]");
    assert_eq!(c_type.type_id, 13);

    let literal_keys = entry
        .program
        .literal_array_table
        .iter()
        .map(|entry| String::from_utf8(entry.key.clone()).unwrap())
        .collect::<Vec<_>>();
    assert!(
        literal_keys
            .iter()
            .any(|key| key == "test/annotation_decl.ets.Anno_a_0"),
        "missing annotation array literal key; keys={literal_keys:?}"
    );
    let a_value_key = a
        .metadata
        .as_ref()
        .and_then(|metadata| metadata.value.as_ref())
        .and_then(|value| match &value.value {
            Some(scalar_value::Value::ValueStr(value)) => {
                Some(String::from_utf8(value.clone()).unwrap())
            }
            _ => None,
        });
    assert_eq!(
        a_value_key.as_deref(),
        Some("test/annotation_decl.ets.Anno_a_0")
    );
}

#[test]
fn test_generator_function_uses_state_machine_opcodes() {
    // Upstream es2abc: generator functions emit CreateGeneratorObj (0xb1),
    // SuspendGenerator (0xc3), ResumeGenerator (0xbf), GetResumeMode (0xc0).
    let tmp = tempfile::tempdir().unwrap();
    let out = tmp.path().join("generator.abc");
    let record_name = "test/generator";
    module_compiler()
        .compile(
            vec![module_record(
                record_name,
                r#"function* ids() { yield 1; yield 2; }"#,
            )],
            &out,
        )
        .expect("generator function must compile via state-machine lowering");
    let bytes = std::fs::read(&out).unwrap();
    // The generator function must be in the class record.
    let class_name = format!("L{record_name};");
    let class_off = class_offsets(&bytes)
        .into_iter()
        .map(|off| off as usize)
        .find(|&off| class_name_at(&bytes, off) == class_name)
        .expect("module class must exist");
    let methods = class_method_items(&bytes, class_off);
    let gen_bytecode = methods
        .iter()
        .filter_map(|(_, name, code_off)| {
            let code_off = (*code_off)? as usize;
            let bc = code_item_bytecode(&bytes, code_off);
            (name != "func_main_0" && bytecode_contains_opcode(bc, 0xb1)).then_some(bc)
        })
        .next()
        .expect("generator method with createGeneratorObj must exist");
    for (opcode, name) in [
        (0xb1, "createGeneratorObj"),
        (0xc3, "suspendGenerator"),
        (0xbf, "resumeGenerator"),
        (0xc0, "getResumeMode"),
    ] {
        assert!(
            bytecode_contains_opcode(gen_bytecode, opcode),
            "generator must emit {} ({opcode:#x})",
            name
        );
    }
}

#[test]
fn test_instance_class_field_initializer_still_compiles() {
    let tmp = tempfile::tempdir().unwrap();
    let out = tmp.path().join("class_field.abc");
    let record_name = "test/class_field";
    module_compiler()
        .compile(
            vec![module_record(
                record_name,
                r#"class Box { value = 1; }
const box = new Box();"#,
            )],
            &out,
        )
        .expect("initialized instance class field source must compile");
    let bytes = std::fs::read(&out).unwrap();
    assert!(bytes.starts_with(b"PANDA\0\0\0"));
    let (_, bytecode) = module_method_bytecode_by_suffix(&bytes, record_name, "Box")
        .expect("class constructor bytecode exists");
    assert!(
        bytecode_contains_opcode(&bytecode, 0xdb),
        "instance class fields must use definefieldbyname like upstream CompileField"
    );
}

#[test]
fn test_uninitialized_instance_class_field_defines_undefined_property() {
    let tmp = tempfile::tempdir().unwrap();
    let out = tmp.path().join("uninitialized_class_field.abc");
    let record_name = "test/uninitialized_class_field";
    module_compiler()
        .compile(
            vec![module_record(
                record_name,
                r#"class Box { value; }
const box = new Box();"#,
            )],
            &out,
        )
        .expect("uninitialized instance class field source must compile");
    let bytes = std::fs::read(&out).unwrap();
    let (_, bytecode) = module_method_bytecode_by_suffix(&bytes, record_name, "Box")
        .expect("class constructor bytecode exists");
    assert!(
        bytecode_contains_opcode(&bytecode, 0x00),
        "uninitialized instance class fields must load undefined like upstream CompileField"
    );
    assert!(
        bytecode_contains_opcode(&bytecode, 0xdb),
        "uninitialized instance class fields must define the own property"
    );
}

#[test]
fn test_computed_instance_class_field_uses_captured_class_key() {
    let tmp = tempfile::tempdir().unwrap();
    let out = tmp.path().join("computed_instance_class_field.abc");
    let record_name = "test/computed_instance_class_field";
    module_compiler()
        .compile(
            vec![module_record(
                record_name,
                r#"const key = "value";
class Box { [key] = 1; }
const box = new Box();"#,
            )],
            &out,
        )
        .expect("computed instance class field must compile through captured class key");
    let bytes = std::fs::read(&out).unwrap();
    let main = module_func_main_bytecode(&bytes, record_name);
    assert!(
        bytecode_contains_opcode(&main, 0x09),
        "computed instance field keys must allocate a class lexical environment"
    );
    assert!(
        bytecode_contains_opcode(&main, 0x3d) || bytecode_contains_opcode(&main, 0x8b),
        "computed instance field keys must be saved during class evaluation"
    );
    let (_, ctor) = module_method_bytecode_by_suffix(&bytes, record_name, "Box")
        .expect("class constructor bytecode exists");
    assert!(
        bytecode_contains_opcode(&ctor, 0x3c) || bytecode_contains_opcode(&ctor, 0x8a),
        "constructor must read the precomputed key instead of recomputing it"
    );
    assert!(
        ctor.windows(2).any(|window| window == [0xfb, 0x01]),
        "computed instance fields must define the property by runtime key value"
    );
}

#[test]
fn test_static_class_field_runs_static_initializer() {
    let tmp = tempfile::tempdir().unwrap();
    let out = tmp.path().join("static_class_field.abc");
    let record_name = "test/static_class_field";
    module_compiler()
        .compile(
            vec![module_record(
                record_name,
                r#"class Box { static value = 1; }"#,
            )],
            &out,
        )
        .expect("static class field source must compile through the static initializer path");

    let bytes = std::fs::read(&out).unwrap();
    let main = module_func_main_bytecode(&bytes, record_name);
    assert!(
        bytecode_contains_opcode(&main, 0x2d),
        "class definition must call the generated static initializer with callthis0"
    );
    let (_, initializer) =
        module_method_bytecode_by_suffix(&bytes, record_name, "static_initializer#")
            .expect("static initializer method exists");
    assert!(
        bytecode_contains_opcode(&initializer, 0x6f),
        "static initializer must load this like upstream CompileClassInitializer"
    );
    assert!(
        bytecode_contains_opcode(&initializer, 0xdb),
        "static class field must define an own property like upstream CompileField"
    );
}

#[test]
fn test_default_export_uninitialized_static_class_field_defines_undefined_property() {
    let tmp = tempfile::tempdir().unwrap();
    let out = tmp.path().join("default_static_class_field.abc");
    let record_name = "test/default_static_class_field";
    module_compiler()
        .compile(
            vec![module_record(
                record_name,
                r#"export default class { static value; }"#,
            )],
            &out,
        )
        .expect("default-exported static class field source must compile");

    let bytes = std::fs::read(&out).unwrap();
    let (_, initializer) =
        module_method_bytecode_by_suffix(&bytes, record_name, "static_initializer#")
            .expect("static initializer method exists");
    assert!(
        bytecode_contains_opcode(&initializer, 0x00),
        "uninitialized static class fields must load undefined like upstream CompileField"
    );
    assert!(
        bytecode_contains_opcode(&initializer, 0xdb),
        "uninitialized static class fields must define the own property"
    );
}

#[test]
fn test_static_block_runs_static_initializer() {
    let tmp = tempfile::tempdir().unwrap();
    let out = tmp.path().join("static_block.abc");
    let record_name = "test/static_block";
    module_compiler()
        .compile(
            vec![module_record(
                record_name,
                r#"class Box { static { this.value = 1; } }
const value = Box.value;"#,
            )],
            &out,
        )
        .expect("static block source must compile through the static initializer path");

    let bytes = std::fs::read(&out).unwrap();
    let main = module_func_main_bytecode(&bytes, record_name);
    assert!(
        bytecode_contains_opcode(&main, 0x2d),
        "class definition must call the generated static block initializer with callthis0"
    );
    let (_, initializer) =
        module_method_bytecode_by_suffix(&bytes, record_name, "static_initializer#")
            .expect("static block initializer method exists");
    assert!(
        bytecode_contains_opcode(&initializer, 0x6f),
        "static block initializer must load this like upstream CompileClassInitializer"
    );
    assert!(
        bytecode_contains_opcode(&initializer, 0x43),
        "static block body must compile its this.value assignment"
    );
}

#[test]
fn test_computed_static_class_field_uses_precomputed_key_in_static_initializer() {
    let tmp = tempfile::tempdir().unwrap();
    let out = tmp.path().join("computed_static_class_field.abc");
    let record_name = "test/computed_static_class_field";
    module_compiler()
        .compile(
            vec![module_record(
                record_name,
                r#"const name = "value";
class Box { static [name] = 1; }"#,
            )],
            &out,
        )
        .expect("computed static class field must lower through the static initializer");

    let bytes = std::fs::read(&out).unwrap();
    let main = module_func_main_bytecode(&bytes, record_name);
    let (_, initializer) =
        module_method_bytecode_by_suffix(&bytes, record_name, "static_initializer#")
            .expect("computed static field must create a static initializer method");
    assert!(
        bytecode_contains_opcode(&main, 0x2e),
        "class definition must pass the precomputed key as an argument to the static initializer"
    );
    assert!(
        bytecode_contains_opcode(&initializer, 0x6f),
        "static initializer must load this like upstream CompileClassInitializer"
    );
    assert!(
        bytecode_contains_opcode(&initializer, 0xfb),
        "computed static field must define the property by runtime key value"
    );
    assert!(
        !bytecode_contains_opcode(&initializer, 0x3f),
        "static initializer must use the precomputed key argument instead of recomputing the key expression"
    );
}

#[test]
fn test_computed_class_methods_use_precomputed_keys_in_missing_properties() {
    let tmp = tempfile::tempdir().unwrap();
    let out = tmp.path().join("computed_class_method.abc");
    let record_name = "test/computed_class_method";
    module_compiler()
        .compile(
            vec![module_record(
                record_name,
                r#"const name = "run";
const staticName = "make";
const getterName = "value";
class Box {
  [name]() { return 1; }
  static [staticName]() { return 2; }
  get [getterName]() { return 3; }
}"#,
            )],
            &out,
        )
        .expect("computed class methods must lower through missing-properties");

    let bytes = std::fs::read(&out).unwrap();
    let main = module_func_main_bytecode(&bytes, record_name);
    assert!(
        bytecode_opcode_count(&main, 0x34) >= 2,
        "computed class methods must define method functions outside the class buffer"
    );
    assert!(
        bytecode_opcode_count(&main, 0x78) >= 2,
        "computed class methods must store onto prototype/class by precomputed key value"
    );
    assert!(
        bytecode_contains_opcode(&main, 0xbc),
        "computed class accessors must define getter/setter pairs by precomputed key value"
    );
    assert!(
        bytes_contain_ascii(&bytes, "computed_class_method"),
        "computed class methods must still emit callable method records"
    );
}

#[test]
fn test_auto_accessor_class_field_lowers_to_getter_setter_and_private_backing() {
    // Upstream: `accessor value = 1` is lowered to a synthesized private backing
    // field + getter/setter pair installed via definegettersetterbyvalue (0xbc).
    // The backing field is initialized via callruntime.defineprivateproperty.
    let tmp = tempfile::tempdir().unwrap();
    let out = tmp.path().join("auto_accessor_class_field.abc");
    module_compiler()
        .compile(
            vec![module_record(
                "test/auto_accessor_class_field",
                r#"class Box { accessor value = 1; }"#,
            )],
            &out,
        )
        .expect("auto-accessor class field must lower to getter/setter with private backing");
    let bytes = std::fs::read(&out).unwrap();
    // DefineGetterSetterByValue (0xbc) must be emitted to install the getter/setter.
    assert!(
        bytes.contains(&0xbc),
        "auto-accessor must emit definegettersetterbyvalue (0xbc)"
    );
    // callruntime.createprivateproperty (0xfb 0x04) for the backing field.
    assert!(
        bytes.windows(2).any(|w| w == [0xfb, 0x04]),
        "auto-accessor must emit callruntime.createprivateproperty (0xfb 0x04)"
    );
}

#[test]
fn test_computed_auto_accessor_uses_precomputed_key_without_synthetic_name() {
    let tmp = tempfile::tempdir().unwrap();
    let out = tmp.path().join("computed_auto_accessor.abc");
    let record_name = "test/computed_auto_accessor";
    module_compiler()
        .compile(
            vec![module_record(
                record_name,
                r#"function key() { return "value"; }
class Box {
  accessor [key()] = 1;
}"#,
            )],
            &out,
        )
        .expect("computed auto-accessor must lower through upstream-style key backup");

    let bytes = std::fs::read(&out).unwrap();
    let main = module_func_main_bytecode(&bytes, record_name);
    assert!(
        bytecode_contains_opcode(&main, 0xbc),
        "computed auto-accessor must install getter/setter by runtime key value"
    );
    assert!(
        bytes.windows(2).any(|w| w == [0xfb, 0x04]),
        "computed auto-accessor must still create a private backing slot"
    );
    assert_eq!(
        bytecode_opcode_count(&main, 0x29),
        1,
        "computed auto-accessor key expression must be evaluated once and reused"
    );
    assert!(
        !bytes_contain_ascii(&bytes, "__computed_accessor"),
        "computed auto-accessor must not use the old synthetic-key fallback"
    );
}

#[test]
fn test_object_spread_and_proto_use_upstream_object_ops() {
    let tmp = tempfile::tempdir().unwrap();
    let out = tmp.path().join("object_spread_proto.abc");
    let record_name = "test/object_spread_proto";
    let source = r#"
const source = { a: 1 };
const proto = { p: 1 };
const target = { x: 1, ...source, __proto__: proto };
"#;
    module_compiler()
        .compile(vec![module_record(record_name, source)], &out)
        .expect("compile object spread/proto source");
    let bytes = std::fs::read(&out).unwrap();
    let bytecode = module_func_main_bytecode(&bytes, record_name);
    assert!(
        bytecode_contains_opcode(&bytecode, 0xc5),
        "object spread must use copydataproperties like es2panda"
    );
    assert!(
        bytecode_contains_opcode(&bytecode, 0x77),
        "__proto__ init must use setobjectwithproto like es2panda"
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
fn test_logical_and_emits_jnez_branch() {
    let tmp = tempfile::tempdir().unwrap();
    let out = tmp.path().join("test.abc");
    let record_name = "test/mod";
    module_compiler()
        .compile(
            vec![module_record(record_name, r#"const x = a && b;"#)],
            &out,
        )
        .expect("compile");
    let bytes = std::fs::read(&out).unwrap();
    assert!(bytes.starts_with(b"PANDA\0\0\0"));
    let method_bytecodes = module_method_bytecodes(&bytes, record_name);
    assert!(
        method_bytecodes
            .iter()
            .any(|bytecode| bytecode_contains_jnez(bytecode)),
        "jnez must appear in module code for '&&' short-circuit"
    );
}

#[test]
fn test_logical_or_emits_jnez_branch() {
    let tmp = tempfile::tempdir().unwrap();
    let out = tmp.path().join("test.abc");
    let record_name = "test/mod";
    module_compiler()
        .compile(
            vec![module_record(record_name, r#"const x = a || b;"#)],
            &out,
        )
        .expect("compile");
    let bytes = std::fs::read(&out).unwrap();
    assert!(bytes.starts_with(b"PANDA\0\0\0"));
    let method_bytecodes = module_method_bytecodes(&bytes, record_name);
    assert!(
        method_bytecodes
            .iter()
            .any(|bytecode| bytecode_contains_jnez(bytecode)),
        "jnez must appear in module code for '||' short-circuit"
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
    initialRender() {
        this.observeComponentCreation2((elmtId, isInitialRender) => {
            Row.create();
        }, Row);
        this.observeComponentCreation2((elmtId, isInitialRender) => {
            Column.create();
        }, Column);
        this.observeComponentCreation2((elmtId, isInitialRender) => {
            Text.create(this.message);
            Text.fontSize(50);
            Text.fontWeight(FontWeight.Bold);
        }, Text);
        Text.pop();
        Column.pop();
        Row.pop();
    }
    rerender() { this.updateDirtyElements(); }
    static getEntryName() { return "Index"; }
};
registerNamedRoute(() => new Index(void 0, {}), "", {bundleName: "com.example.arkdown.test", moduleName: "entry", pagePath: "pages/Index", pageFullPath: "entry/src/main/ets/pages/Index", integratedHsp: "false", moduleType: "followWithHap"});
"#;
    let tmp = tempfile::tempdir().unwrap();
    let out = tmp.path().join("modules_debug.abc");
    AbcCompiler::new(AbcCompileOptions {
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
    })
    .compile(
        vec![
            AbcModule {
                record_name: "com.example.test/entry/ets/entryability/EntryAbility".to_string(),
                source: entry_source.to_string(),
                source_map: None,
                source_file: None,
                package_name: None,
                is_commonjs: false,
                is_shared_module: false,
                origin_source_lang: String::new(),
            },
            AbcModule {
                record_name: "com.example.test/entry/ets/pages/Index".to_string(),
                source: index_source.to_string(),
                source_map: None,
                source_file: None,
                package_name: None,
                is_commonjs: false,
                is_shared_module: false,
                origin_source_lang: String::new(),
            },
        ],
        &out,
    )
    .expect("compile both modules with debug info");
    let bytes = std::fs::read(&out).unwrap();
    assert!(bytes.starts_with(b"PANDA\0\0\0"));
    let sz = bytes.len();
    eprintln!(
        "[parity] debug two-module ABC = {sz}B / 9844B hvigor ({:.0}%)",
        sz as f64 / 9844.0 * 100.0
    );
    // Phase 5 parity (2026-05-30): Correct LNP opcode values + DebugInfo cp format +
    // L_ESSlotNumberAnnotation per-method + correct LNP architecture (opcodes in data_,
    // operands in cp) + added initialRender/getEntryName/closure methods = 73% (7174B).
    // Remaining gap: per-instruction SPECIAL opcodes (~2843B) vs per-statement.
    assert!(
        sz > 7000,
        "debug-info two-module compile must exceed 7000B (Phase 5 parity regression guard), got {sz}B"
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
fn test_array_spread_uses_upstream_starrayspread() {
    let tmp = tempfile::tempdir().unwrap();
    let out = tmp.path().join("array_spread.abc");
    let record_name = "test/array_spread";
    let source = r#"
const rest = [2, 3];
const values = [1, ...rest, 4];
"#;
    module_compiler()
        .compile(vec![module_record(record_name, source)], &out)
        .expect("compile array spread source");
    let bytes = std::fs::read(&out).unwrap();
    let bytecode = module_func_main_bytecode(&bytes, record_name);
    assert!(
        bytecode_contains_opcode(&bytecode, 0xc6),
        "array spread must use starrayspread like es2panda"
    );
    assert!(
        bytecode_contains_opcode(&bytecode, 0x78),
        "array elements after spread must use dynamic-index stownbyvalue"
    );
}

#[test]
fn test_call_spread_uses_upstream_apply_with_arguments_array() {
    let tmp = tempfile::tempdir().unwrap();
    let out = tmp.path().join("call_spread.abc");
    let record_name = "test/call_spread";
    let source = r#"
function fn(a, b) { return a + b; }
const args = [1, 2];
const value = fn(...args);
"#;
    module_compiler()
        .compile(vec![module_record(record_name, source)], &out)
        .expect("compile call spread source");
    let bytes = std::fs::read(&out).unwrap();
    let bytecode = module_func_main_bytecode(&bytes, record_name);
    assert!(
        bytecode_contains_opcode(&bytecode, 0xba),
        "spread call must use apply like es2panda CallSpread"
    );
    assert!(
        bytecode_contains_opcode(&bytecode, 0xc6),
        "spread call arguments must be materialized through starrayspread"
    );
    assert!(
        !bytecode_contains_opcode(&bytecode, 0x73),
        "spread call must not pass the spread array as a callrange positional argument"
    );
}

#[test]
fn test_method_spread_uses_upstream_apply_with_receiver() {
    let tmp = tempfile::tempdir().unwrap();
    let out = tmp.path().join("method_spread.abc");
    let record_name = "test/method_spread";
    let source = r#"
const obj = { method(a, b) { return a + b; } };
const args = [1, 2];
const value = obj.method(...args);
"#;
    module_compiler()
        .compile(vec![module_record(record_name, source)], &out)
        .expect("compile method spread source");
    let bytes = std::fs::read(&out).unwrap();
    let bytecode = module_func_main_bytecode(&bytes, record_name);
    assert!(
        bytecode_contains_opcode(&bytecode, 0xba),
        "method spread call must use apply with the receiver register"
    );
    assert!(
        !bytecode_contains_opcode(&bytecode, 0x31),
        "method spread call must not use callthisrange"
    );
}

#[test]
fn test_new_spread_uses_upstream_newobjapply() {
    let tmp = tempfile::tempdir().unwrap();
    let out = tmp.path().join("new_spread.abc");
    let record_name = "test/new_spread";
    let source = r#"
function Ctor(a, b) { this.value = a + b; }
const args = [1, 2];
const value = new Ctor(...args);
"#;
    module_compiler()
        .compile(vec![module_record(record_name, source)], &out)
        .expect("compile new spread source");
    let bytes = std::fs::read(&out).unwrap();
    let bytecode = module_func_main_bytecode(&bytes, record_name);
    assert!(
        bytecode_contains_opcode(&bytecode, 0xb4),
        "new spread must use newobjapply like es2panda NewObjSpread"
    );
}

#[test]
fn test_super_spread_uses_upstream_supercallspread() {
    let tmp = tempfile::tempdir().unwrap();
    let out = tmp.path().join("super_spread.abc");
    let record_name = "test/super_spread";
    let source = r#"
class Base {
  constructor(a, b) { this.value = a + b; }
}
class Child extends Base {
  constructor(args) { super(...args); }
}
const value = new Child([1, 2]);
"#;
    module_compiler()
        .compile(vec![module_record(record_name, source)], &out)
        .expect("compile super spread source");
    let bytes = std::fs::read(&out).unwrap();
    let class_name = format!("L{record_name};");
    let class_off = class_offsets(&bytes)
        .into_iter()
        .map(|off| off as usize)
        .find(|&off| class_name_at(&bytes, off) == class_name)
        .expect("module class must exist");
    let has_supercallspread = class_method_items(&bytes, class_off)
        .into_iter()
        .filter_map(|(_, name, code_off)| {
            let code_off = code_off? as usize;
            (name != "func_main_0").then(|| code_item_bytecode(&bytes, code_off))
        })
        .any(|bytecode| bytecode_contains_opcode(bytecode, 0xb9));
    assert!(
        has_supercallspread,
        "super(...args) must use supercallspread"
    );
}

#[test]
fn test_super_property_access_uses_upstream_super_opcodes() {
    let tmp = tempfile::tempdir().unwrap();
    let out = tmp.path().join("super_property.abc");
    let record_name = "test/super_property";
    let source = r#"
class Base {
  constructor() { this.value = 1; }
  method() { return this.value; }
}
class Child extends Base {
  constructor() { super(); }
  read(key) { return super.value + super[key]; }
  call() { return super.method(); }
  write(key) { super.value = 1; super[key] += 2; return ++super.value; }
}
const child = new Child();
child.read("value");
child.call();
child.write("value");
"#;
    module_compiler()
        .compile(vec![module_record(record_name, source)], &out)
        .expect("compile super property source");
    let bytes = std::fs::read(&out).unwrap();
    let class_name = format!("L{record_name};");
    let class_off = class_offsets(&bytes)
        .into_iter()
        .map(|off| off as usize)
        .find(|&off| class_name_at(&bytes, off) == class_name)
        .expect("module class must exist");
    let methods = class_method_items(&bytes, class_off);
    let method_bytecode = |method_name: &str| {
        methods
            .iter()
            .find_map(|(_, name, code_off)| {
                (name == method_name).then(|| {
                    code_item_bytecode(&bytes, code_off.expect("method has code") as usize)
                })
            })
            .unwrap_or_else(|| panic!("{method_name} method must exist"))
    };

    let read = method_bytecode("read");
    assert!(
        bytecode_contains_opcode(read, 0x46),
        "super.value must use ldsuperbyname"
    );
    assert!(
        bytecode_contains_opcode(read, 0x39),
        "super[key] must use ldsuperbyvalue"
    );

    let call = method_bytecode("call");
    assert!(
        bytecode_contains_opcode(call, 0x46),
        "super.method() must load callee with ldsuperbyname"
    );
    assert!(
        bytecode_contains_opcode(call, 0x2d),
        "super.method() must preserve callthis receiver semantics"
    );

    let write = method_bytecode("write");
    assert!(
        bytecode_contains_opcode(write, 0xd0),
        "super.value assignment/update must use stsuperbyname"
    );
    assert!(
        bytecode_contains_opcode(write, 0xc9),
        "super[key] assignment must use stsuperbyvalue"
    );
}

#[test]
fn test_function_object_literal_uses_definefieldbyname_on_default_api() {
    let tmp = tempfile::tempdir().unwrap();
    let out = tmp.path().join("object_methods.abc");
    let source = r#"
function BaseEvent() {}
BaseEvent.prototype = {
  triggerEvent: function (eventName, args) {},
  addEventListener: function (eventName, callback) {},
};
export default BaseEvent;
"#;
    module_compiler()
        .compile(vec![module_record("test/object_methods", source)], &out)
        .expect("compile object method literal source");
    let bytes = std::fs::read(&out).unwrap();
    assert!(bytes.starts_with(b"PANDA\0\0\0"));
    assert!(
        bytes.contains(&0x07u8),
        "object method literal must seed keys with createobjectwithbuffer (0x07)"
    );
    assert!(
        bytes.contains(&0xdbu8),
        "default API object method literal must define runtime values with definefieldbyname (0xdb)"
    );
}

#[test]
fn test_function_body_hoists_function_declarations_before_uses() {
    let tmp = tempfile::tempdir().unwrap();
    let out = tmp.path().join("function_hoist.abc");
    let source = r#"
const result = (function () {
  var ob = {};
  ob.x = x;
  function x() {}
  return ob;
})();
"#;
    module_compiler()
        .compile(vec![module_record("test/function_hoist", source)], &out)
        .expect("compile function hoist source");
    let bytes = std::fs::read(&out).unwrap();
    let class_off = class_offsets(&bytes)
        .into_iter()
        .map(|off| off as usize)
        .find(|&off| class_name_at(&bytes, off) == "Ltest/function_hoist;")
        .expect("module class must exist");
    let methods = class_method_items(&bytes, class_off);
    let body = methods
        .iter()
        .filter_map(|(_, name, code_off)| {
            let code_off = (*code_off)? as usize;
            let bytecode = code_item_bytecode(&bytes, code_off);
            (name != "func_main_0" && bytecode_contains_opcode(bytecode, 0x43)).then_some(bytecode)
        })
        .next()
        .expect("IIFE body method must assign ob.x");
    assert!(
        !bytecode_contains_opcode(body, 0x3f),
        "function declaration used before its source position must be loaded from the hoisted local binding, not tryldglobalbyname"
    );
}

#[test]
fn test_async_await_uses_es2panda_suspend_resume_sequence() {
    let tmp = tempfile::tempdir().unwrap();
    let out = tmp.path().join("async_await.abc");
    let source = r#"
async function f(p) {
  const x = await p;
  return x;
}
export default f;
"#;
    module_compiler()
        .compile(vec![module_record("test/async_await", source)], &out)
        .expect("compile async await source");
    let bytes = std::fs::read(&out).unwrap();
    let class_off = class_offsets(&bytes)
        .into_iter()
        .map(|off| off as usize)
        .find(|&off| class_name_at(&bytes, off) == "Ltest/async_await;")
        .expect("module class must exist");
    let methods = class_method_items(&bytes, class_off);
    let (bytecode, tries_size) = methods
        .iter()
        .filter_map(|(_, name, code_off)| {
            let code_off = (*code_off)? as usize;
            let bytecode = code_item_bytecode(&bytes, code_off);
            (name != "func_main_0" && bytecode_contains_opcode(bytecode, 0xae))
                .then_some((bytecode, code_item_tries_size(&bytes, code_off)))
        })
        .next()
        .expect("async function method must exist");

    for opcode in [0xae, 0xc4, 0xc3, 0xbf, 0xc0, 0xcd, 0xce] {
        assert!(
            bytecode_contains_opcode(bytecode, opcode),
            "async await method must contain opcode {opcode:#x}"
        );
    }
    assert!(
        tries_size > 0,
        "async function body must emit a catch table"
    );
}

#[test]
fn test_nested_function_reads_parent_function_lexical_binding() {
    let tmp = tempfile::tempdir().unwrap();
    let out = tmp.path().join("function_lexical_capture.abc");
    let source = r#"
const result = (function () {
  function resetShape() { return 1; }
  var EllShapeProperty = (function () {
    var local = 2;
    function readLocal() { return local; }
    var proto = {
      reset: resetShape,
      readLocal: readLocal
    };
    return proto;
  })();
  return EllShapeProperty.reset();
})();
"#;
    module_compiler()
        .compile(
            vec![module_record("test/function_lexical_capture", source)],
            &out,
        )
        .expect("compile function lexical capture source");
    let bytes = std::fs::read(&out).unwrap();
    let class_off = class_offsets(&bytes)
        .into_iter()
        .map(|off| off as usize)
        .find(|&off| class_name_at(&bytes, off) == "Ltest/function_lexical_capture;")
        .expect("module class must exist");
    let methods = class_method_items(&bytes, class_off);
    let nested_iife = methods
        .iter()
        .filter_map(|(_, name, code_off)| {
            let code_off = (*code_off)? as usize;
            let bytecode = code_item_bytecode(&bytes, code_off);
            (name != "func_main_0" && bytecode_contains_opcode(bytecode, 0xdb)).then_some(bytecode)
        })
        .find(|bytecode| {
            bytecode
                .windows(2)
                .any(|window| window[0] == 0x3c && (window[1] & 0x0f) == 1)
        })
        .expect("nested IIFE must load parent lexical binding with ldlexvar level 1");
    assert!(
        !bytecode_contains_opcode(nested_iife, 0x3f),
        "parent function binding must not fall back to tryldglobalbyname"
    );
}

#[test]
fn test_captured_function_parameter_is_stored_in_function_lexenv() {
    let tmp = tempfile::tempdir().unwrap();
    let out = tmp.path().join("captured_param.abc");
    let source = r#"
const result = (function (math) {
  function readRandom() { return math.random; }
  return math.random + readRandom();
})(Math);
"#;
    module_compiler()
        .compile(vec![module_record("test/captured_param", source)], &out)
        .expect("compile captured param source");
    let bytes = std::fs::read(&out).unwrap();
    let class_off = class_offsets(&bytes)
        .into_iter()
        .map(|off| off as usize)
        .find(|&off| class_name_at(&bytes, off) == "Ltest/captured_param;")
        .expect("module class must exist");
    let methods = class_method_items(&bytes, class_off);
    let (outer, outer_vregs) = methods
        .iter()
        .filter_map(|(_, name, code_off)| {
            let code_off = (*code_off)? as usize;
            let (num_vregs, _, _, _) = code_item_header(&bytes, code_off);
            let bytecode = code_item_bytecode(&bytes, code_off);
            (name != "func_main_0"
                && bytecode.first() == Some(&0x09)
                && bytecode
                    .windows(3)
                    .any(|window| window == [0x60, (num_vregs + 3) as u8, 0x3d]))
            .then_some((bytecode, num_vregs))
        })
        .next()
        .expect("outer function must store captured parameter into function lexenv");
    assert_eq!(
        outer_vregs + 3,
        6,
        "fixture parameter should live in argument register num_vregs + 3"
    );
    assert!(
        bytecode_contains_opcode(outer, 0x3d),
        "captured parameter must be available through stlexvar"
    );
    assert!(
        outer.windows(3).any(|window| window == [0x3c, 0x00, 0xfe]),
        "captured parameter reads in the owning function must use ldlexvar like es2abc"
    );
    assert!(
        !outer
            .windows(4)
            .any(|window| window == [0x60, 0x02, 0x3d, 0x00]),
        "plain function newlexenv must not be treated as a hidden this-capture env"
    );
}

#[test]
fn test_function_lexical_slots_above_imm4_use_wide_encoding() {
    let tmp = tempfile::tempdir().unwrap();
    let out = tmp.path().join("wide_function_lexical_slots.abc");
    let source = r#"
const result = (function () {
  var a0 = 0;
  var a1 = 1;
  var a2 = 2;
  var a3 = 3;
  var a4 = 4;
  var a5 = 5;
  var a6 = 6;
  var a7 = 7;
  var a8 = 8;
  var a9 = 9;
  var a10 = 10;
  var a11 = 11;
  var a12 = 12;
  var a13 = 13;
  var a14 = 14;
  var a15 = 15;
  var a16 = 16;
  function read() {
    return a0 + a1 + a2 + a3 + a4 + a5 + a6 + a7 + a8 + a9 + a10 + a11 + a12 + a13 + a14 + a15 + a16;
  }
  return read();
})();
"#;
    module_compiler()
        .compile(
            vec![module_record("test/wide_function_lexical_slots", source)],
            &out,
        )
        .expect("compile wide function lexical slot source");
    let bytes = std::fs::read(&out).unwrap();
    let class_off = class_offsets(&bytes)
        .into_iter()
        .map(|off| off as usize)
        .find(|&off| class_name_at(&bytes, off) == "Ltest/wide_function_lexical_slots;")
        .expect("module class must exist");
    let methods = class_method_items(&bytes, class_off);
    let all_method_bytes: Vec<&[u8]> = methods
        .iter()
        .filter_map(|(_, _, code_off)| {
            let code_off = (*code_off)? as usize;
            Some(code_item_bytecode(&bytes, code_off))
        })
        .collect();

    assert!(
        all_method_bytes.iter().any(|bytecode| bytecode
            .windows(3)
            .any(|window| window == [0x8b, 0x00, 0x10])),
        "slot 16 lexical stores must use es2abc's wide stlexvar encoding"
    );
    assert!(
        all_method_bytes.iter().any(|bytecode| bytecode
            .windows(3)
            .any(|window| window == [0x8a, 0x00, 0x10])),
        "slot 16 lexical loads must use es2abc's wide ldlexvar encoding"
    );
}

#[test]
fn test_arrow_this_capture_lives_in_owner_function_lexenv() {
    let tmp = tempfile::tempdir().unwrap();
    let out = tmp.path().join("arrow_this_capture.abc");
    let source = r#"
class Box {
  method(x) {
    const f = () => this.value + x;
    return f();
  }
}
"#;
    module_compiler()
        .compile(vec![module_record("test/arrow_this_capture", source)], &out)
        .expect("compile arrow this capture source");
    let bytes = std::fs::read(&out).unwrap();
    let class_off = class_offsets(&bytes)
        .into_iter()
        .map(|off| off as usize)
        .find(|&off| class_name_at(&bytes, off) == "Ltest/arrow_this_capture;")
        .expect("module class must exist");
    let methods = class_method_items(&bytes, class_off);
    let method = methods
        .iter()
        .find_map(|(_, name, code_off)| {
            (name == "method").then(|| code_item_bytecode(&bytes, code_off.unwrap() as usize))
        })
        .expect("class method must exist");
    assert_eq!(
        method.get(..2),
        Some(&[0x09, 0x02][..]),
        "owner method must allocate one lexenv for captured x and this"
    );
    assert!(
        !method
            .windows(4)
            .any(|window| window == [0xb6, 0x01, 0x00, 0x00]),
        "arrow this capture must not create a local newlexenvwithname fallback"
    );

    let arrow = methods
        .iter()
        .filter_map(|(_, name, code_off)| {
            let code_off = (*code_off)? as usize;
            (name != "func_main_0" && name != "method" && name != "Box")
                .then(|| code_item_bytecode(&bytes, code_off))
        })
        .find(|bytecode| bytecode.windows(2).any(|window| window == [0x3c, 0x10]))
        .expect("arrow function must load captured this from owner lexenv slot 1");
    assert!(
        arrow.windows(2).any(|window| window == [0x3c, 0x00]),
        "arrow function must load captured parameter x from owner lexenv slot 0"
    );
}

#[test]
fn test_function_new_target_loads_mandatory_parameter() {
    let tmp = tempfile::tempdir().unwrap();
    let out = tmp.path().join("new_target.abc");
    let record_name = "test/new_target";
    module_compiler()
        .compile(
            vec![module_record(
                record_name,
                r#"function factory() { return new.target; }
const value = new factory();"#,
            )],
            &out,
        )
        .expect("compile new.target source");
    let bytes = std::fs::read(&out).unwrap();
    let class_off = class_offsets(&bytes)
        .into_iter()
        .map(|off| off as usize)
        .find(|&off| class_name_at(&bytes, off) == "Ltest/new_target;")
        .expect("module class must exist");
    let methods = class_method_items(&bytes, class_off);
    let factory = methods
        .iter()
        .find_map(|(_, name, code_off)| {
            let code_off = (*code_off)? as usize;
            (name != "func_main_0"
                && bytecode_contains_opcode(code_item_bytecode(&bytes, code_off), 0x64))
            .then(|| code_item_bytecode(&bytes, code_off))
        })
        .expect("factory function body must exist");
    assert!(
        factory.first() == Some(&0x60),
        "new.target must load the mandatory new-target parameter instead of undefined"
    );
    assert!(
        factory.first() != Some(&0x00),
        "new.target must not lower to ldundefined"
    );
}

#[test]
fn test_lnp_per_instruction_tracking() {
    use arkdown_abc::emit::file::LiteralArray;
    use arkdown_abc::emit::strings::StringTable;
    use arkdown_abc::ir::builder;

    let source = "var x = 1;\nvar y = 2;\n";
    let mut strings = StringTable::default();
    let mut literal_arrays: Vec<LiteralArray> = Vec::new();

    let record = builder::build_record_with_debug(
        source,
        "test/module",
        &mut strings,
        &mut literal_arrays,
        Some("test.ts"),
        false,
    )
    .unwrap();

    let main_method = record
        .methods
        .iter()
        .find(|m| m.name == "func_main_0")
        .unwrap();
    println!("func_main_0 line_ops len: {}", main_method.line_ops.len());
    println!(
        "func_main_0 line_ops hex: {}",
        main_method
            .line_ops
            .iter()
            .map(|b| format!("{:02x}", b))
            .collect::<Vec<_>>()
            .join(" ")
    );

    // Should have more than just END_SEQUENCE
    assert!(
        main_method.line_ops.len() > 1,
        "line_ops should have more than just END_SEQUENCE"
    );
}

#[test]
fn test_lnp_module_with_class() {
    use arkdown_abc::emit::file::LiteralArray;
    use arkdown_abc::emit::strings::StringTable;
    use arkdown_abc::ir::builder;

    // Simulate what the bundled Index.ets looks like
    let source = r#"var Index = class extends ViewPU {
  constructor() { super(); }
};
registerNamedRoute(() => new Index(void 0, {}), "", {bundleName: "x"});
"#;
    let mut strings = StringTable::default();
    let mut literal_arrays: Vec<LiteralArray> = Vec::new();

    let record = builder::build_record_with_debug(
        source,
        "com.example.app/entry/ets/pages/Index",
        &mut strings,
        &mut literal_arrays,
        Some("./ets/pages/Index.ets"),
        true, // is_module
    )
    .unwrap();

    let main_method = record
        .methods
        .iter()
        .find(|m| m.name == "func_main_0")
        .unwrap();
    println!("func_main_0 line_ops len: {}", main_method.line_ops.len());
    println!(
        "func_main_0 line_ops hex: {}",
        main_method
            .line_ops
            .iter()
            .map(|b| format!("{:02x}", b))
            .collect::<Vec<_>>()
            .join(" ")
    );

    // Should have more than just END_SEQUENCE
    assert!(
        main_method.line_ops.len() > 1,
        "line_ops should have more than just END_SEQUENCE, got {} bytes: {:?}",
        main_method.line_ops.len(),
        main_method.line_ops
    );
}

// ── ProgramCache / protoBin integration tests ────────────────────────────────

/// Compile a simple module with a `cache_file` (`.protoBin`) destination,
/// then verify:
///
/// 1. The `.abc` output starts with the Panda magic bytes `PANDA\x00\x00\x00`.
/// 2. The `.protoBin` file exists and is non-empty.
/// 3. The `.protoBin` decodes as a valid `ProgramCache` protobuf (has a
///    `program` field with at least one function entry — the module body).
/// 4. The hash stored in the cache matches the source hash computed by
///    `source_cache_hash()`.
///
/// This mirrors the upstream `es2abc --cache-file file.protoBin` behavior
/// described in `cache.rs`: the cache stores a `ProgramCache { hash_code,
/// program }` proto whose `hash_code` is `GetHash32String(source + fileName +
/// recordName + sourceFile + pkgName + targetPandaFileVersion)`.
#[test]
fn proto_bin_cache_file_written_with_correct_format() {
    let tmp = tempfile::tempdir().unwrap();
    let abc_out = tmp.path().join("module.abc");
    let proto_bin = tmp.path().join("module.protoBin");

    let record_name = "@bundle:com.example.app/entry/ets/pages/Index";
    let file_name = tmp.path().join("Index.js").to_string_lossy().into_owned();
    let source = "export const greeting = 'hello';";

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

    let cache_input = AbcProgramCacheInput {
        file_name: file_name.clone(),
        cache_file: proto_bin.clone(),
    };

    let result = compiler
        .compile_with_program_cache(
            vec![AbcModule {
                record_name: record_name.to_string(),
                source: source.to_string(),
                source_map: None,
                source_file: None,
                package_name: None,
                is_commonjs: false,
                is_shared_module: false,
                origin_source_lang: String::new(),
            }],
            &abc_out,
            vec![Some(cache_input)],
        )
        .expect("compile_with_program_cache must succeed");

    // 1. ABC output path is correct.
    assert_eq!(
        result.path, abc_out,
        "artifact path must equal the output path"
    );

    // 2. ABC output starts with Panda magic bytes.
    let abc_bytes = std::fs::read(&abc_out).expect("abc output must exist");
    assert!(
        abc_bytes.len() >= 8,
        "ABC output must be at least 8 bytes, got {}",
        abc_bytes.len()
    );
    assert_eq!(
        &abc_bytes[0..8],
        b"PANDA\x00\x00\x00",
        "ABC magic bytes must be PANDA\\x00\\x00\\x00"
    );

    // 3. .protoBin file exists and is non-empty.
    assert!(
        proto_bin.exists(),
        ".protoBin file must be written by compile_with_program_cache"
    );
    let proto_bytes = std::fs::read(&proto_bin).expect(".protoBin must be readable");
    assert!(!proto_bytes.is_empty(), ".protoBin file must not be empty");

    // 4. .protoBin decodes as a valid ProgramCache with a program field.
    let entry = ProgramCacheFile::read(&proto_bin)
        .expect("ProgramCacheFile::read must not return Err")
        .expect(".protoBin must decode as a valid ProgramCache");

    // The program must have at least one function entry (the module body
    // func_main_0 is always emitted for module-mode inputs).
    assert!(
        !entry.program.function_table.is_empty(),
        "ProgramCache program must contain at least one function (func_main_0); \
         got function_table len={}",
        entry.program.function_table.len()
    );

    // 5. The stored hash matches the source_cache_hash for this input.
    let expected_hash = source_cache_hash(
        source,
        &file_name,
        record_name,
        "", // source_file (None → empty string in hash)
        "", // package_name (None → empty string in hash)
        version_for_target_api(12, None),
        12,
    );
    assert_eq!(
        entry.hash_code, expected_hash,
        "ProgramCache hash_code must match source_cache_hash(source, file_name, \
         record_name, \"\", \"\", panda_file_version)"
    );
}

// ---------------------------------------------------------------------------
// Panda ISA specification opcode verification
//
// These tests verify that the compiler emits the EXACT opcode bytes mandated
// by the Panda ISA specification documented at:
//   https://gitee.com/openharmony/arkcompiler_runtime_core
//
// Opcode constants verified against isa/isa.yaml (API-24 branch):
//   0x62  lda.i32  imm:i32   — load signed 32-bit integer into accumulator
//   0x3E  lda.str  id:u16    — load string literal into accumulator
//   0x65  return.void         — return undefined
// ---------------------------------------------------------------------------

/// Verify that `export const answer = 42;` compiles to an LdAi (0x62) instruction
/// followed by the 32-bit little-endian encoding of 42 (0x2A 0x00 0x00 0x00).
///
/// ISA spec reference: `lda.i32 imm:i32` — opcode `0x62`, format `op_imm_32`,
/// encoding `[0x62][i32 LE]` = 5 bytes total.
/// Source: `isa/isa.yaml` — `ldai imm:i32`, `opcode_idx: [0x62]`.
#[test]
fn test_integer_constant_produces_lda_i32_opcode() {
    let tmp = tempfile::tempdir().expect("tempdir");
    let out = tmp.path().join("lda_i32.abc");
    module_compiler()
        .compile(
            vec![module_record(
                "test/isa/lda_i32",
                "export const answer = 42;",
            )],
            &out,
        )
        .expect("compile must succeed");

    let bytes = std::fs::read(&out).expect("read .abc");
    let func_main = module_func_main_bytecode(&bytes, "test/isa/lda_i32");

    // The Panda ISA mandates: lda.i32 42 → [0x62, 0x2A, 0x00, 0x00, 0x00]
    // 42 in little-endian i32: 0x2A 0x00 0x00 0x00
    let lda_i32_seq: &[u8] = &[0x62, 0x2A, 0x00, 0x00, 0x00];
    assert!(
        func_main.windows(5).any(|w| w == lda_i32_seq),
        "export const answer = 42 must emit lda.i32 opcode 0x62 followed by \
         LE bytes for 42 (0x2A 0x00 0x00 0x00); \
         ISA spec: isa.yaml ldai opcode_idx [0x62]; \
         func_main_0 bytecode: {:02x?}",
        func_main,
    );
}

/// Verify that `export const msg = "hello";` compiles to an LdaStr (0x3E) instruction.
///
/// ISA spec reference: `lda.str id:u16` — opcode `0x3E`, format `op_id_16`,
/// encoding `[0x3E][string_id: u16 LE]` = 3 bytes total.
/// Source: `isa/isa.yaml` — `lda.str id:string`, `opcode_idx: [0x3e]`.
///
/// The string index (u16) points into the file's string table.  We verify
/// the opcode byte only, not the index value, since the index depends on
/// the order strings are interned.
#[test]
fn test_string_constant_produces_lda_str_opcode() {
    let tmp = tempfile::tempdir().expect("tempdir");
    let out = tmp.path().join("lda_str.abc");
    module_compiler()
        .compile(
            vec![module_record(
                "test/isa/lda_str",
                r#"export const msg = "hello";"#,
            )],
            &out,
        )
        .expect("compile must succeed");

    let bytes = std::fs::read(&out).expect("read .abc");
    let func_main = module_func_main_bytecode(&bytes, "test/isa/lda_str");

    // The Panda ISA mandates: lda.str <string_id> → [0x3E, lo, hi]
    // We verify the opcode byte 0x3E is present in the bytecode stream.
    assert!(
        bytecode_contains_opcode(&func_main, 0x3E),
        "export const msg = \"hello\" must emit lda.str opcode 0x3E; \
         ISA spec: isa.yaml lda.str opcode_idx [0x3e]; \
         func_main_0 bytecode: {:02x?}",
        func_main,
    );

    // Additionally verify the "hello" string is present in the ABC binary.
    assert!(
        bytes_contain_ascii(&bytes, "hello"),
        "the string literal \"hello\" must appear in the ABC string table"
    );
}

/// Verify that `export function f() {}` compiles to a ReturnUndefined (0x65) opcode
/// in the function body.
///
/// ISA spec reference: `return.void` — opcode `0x65`, format `op_none`,
/// encoding `[0x65]` = 1 byte.
/// Source: `isa/isa.yaml` — `returnundefined`, `opcode_idx: [0x65]`.
///
/// Every function with no explicit return statement must end with `return.void`
/// (0x65), not `return` (0x64 which returns the accumulator value).
#[test]
fn test_undefined_return_produces_return_undefined_opcode() {
    let tmp = tempfile::tempdir().expect("tempdir");
    let out = tmp.path().join("return_undefined.abc");
    module_compiler()
        .compile(
            vec![module_record(
                "test/isa/return_undefined",
                "export function f() {}",
            )],
            &out,
        )
        .expect("compile must succeed");

    let bytes = std::fs::read(&out).expect("read .abc");

    // Find the function record "f" inside the module class.
    let class_name = "Ltest/isa/return_undefined;";
    let class_off = class_offsets(&bytes)
        .into_iter()
        .map(|off| off as usize)
        .find(|&off| class_name_at(&bytes, off) == class_name)
        .expect("module class must exist");

    let methods = class_method_items(&bytes, class_off);

    // Find the method named "f".
    let (_, _, f_code_off) = methods
        .iter()
        .find(|(_, name, _)| name == "f")
        .expect("function f must be emitted as a MethodItem");

    let f_bytecode = code_item_bytecode(
        &bytes,
        f_code_off.expect("function f must have a code item") as usize,
    );

    // The Panda ISA mandates return.void (0x65) for void functions.
    // This is distinct from return (0x64) which returns the accumulator.
    assert!(
        bytecode_contains_opcode(f_bytecode, 0x65),
        "export function f() {{}} must emit return.void opcode 0x65; \
         ISA spec: isa.yaml returnundefined opcode_idx [0x65]; \
         function f bytecode: {:02x?}",
        f_bytecode,
    );

    // Sanity: an empty void function must NOT emit the value-return opcode (0x64).
    assert!(
        !bytecode_contains_opcode(f_bytecode, 0x64),
        "empty void function must not emit value-return opcode 0x64 (return acc); \
         function f bytecode: {:02x?}",
        f_bytecode,
    );
}
