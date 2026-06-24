# TEST-009: arkdown-abc (ArkCompiler Bytecode Compiler)

**Crate**: `arkdown-abc`
**Reference**: `https://github.com/openharmony/arkcompiler_ets_frontend`

---

## Test File Layout

```
crates/arkdown-abc/src/
├── tests/
│   ├── binary_format.rs      ← .abc file header and structure
│   ├── ir_builder.rs         ← JS AST → Panda IR lowering
│   ├── string_table.rs       ← string interning and deduplication
│   ├── merge_abc.rs          ← multi-record merging
│   ├── cache.rs              ← incremental compilation cache
│   └── roundtrip.rs          ← compile JS → .abc → verify record structure
```

---

## binary_format.rs

### TC-001: empty module produces valid .abc header

- **Input**: minimal JS `(function(){})()`
- **Expected**: output bytes start with correct Panda magic bytes
- **Note**: verify magic against `arkcompiler_runtime_core` format spec

### TC-002: .abc header contains correct file_size

- **Expected**: `header.file_size` equals actual byte length of output

### TC-003: string table entries are deduplicated

- **Input**: JS with `"hello"` used three times
- **Expected**: `StringTable` contains `"hello"` once; three references point to same entry

### TC-004: class index present for each module record

- **Input**: two modules A and B (merge-abc mode)
- **Expected**: `ClassIndex` contains two entries

---

## ir_builder.rs

### TC-005: `var x = 42` → ldai + sta opcodes

- **Input**: `(function() { var x = 42; })()`
- **Expected IR**: contains `ldai 42` followed by `sta v0` (or equivalent register)

### TC-006: `var s = "hello"` → lda.str opcode

- **Input**: `(function() { var s = "hello"; })()`
- **Expected IR**: contains `lda.str <string_id_for_hello>`

### TC-007: function call → call opcode

- **Input**: `(function() { foo(); })()`
- **Expected IR**: contains `call.short` or `call` opcode

### TC-008: `globalThis["x_hash"]` read → ldglobalvar opcode

- **Input**: `(function() { var m = globalThis["commons_abc123"]; })()`
- **Expected IR**: contains `ldglobalvar` with key `"commons_abc123"`

### TC-009: `return` statement → return opcode

- **Input**: function with `return value;`
- **Expected IR**: ends with `return` or `returnundefined`

---

## string_table.rs

### TC-010: identical strings interned to same ID

- **Setup**: two uses of `"foo"` in one module
- **Expected**: `string_table.id("foo")` called twice returns same u32

### TC-011: distinct strings get distinct IDs

- **Expected**: `string_table.id("foo") != string_table.id("bar")`

---

## merge_abc.rs

### TC-012: two modules merged into one .abc with two records

- **Input**: `AbcModule { record_name: "A", source: ... }` and `AbcModule { record_name: "B", ... }`
- **Expected**: output `.abc` file has `class_index.len() == 2`

### TC-013: record names accessible in merged .abc

- **Expected**: after loading, records can be looked up by name `"A"` and `"B"`

### TC-014: string table shared across records (no duplication)

- **Input**: both A and B use `"shared_string"`
- **Expected**: `"shared_string"` appears once in the string table

---

## cache.rs

### TC-015: unchanged function → cache hit, no re-emission

- **Setup**: compile module, store cache; recompile same source
- **Expected**: cache hit logged; IR builder not invoked for cached functions

### TC-016: changed function → cache miss, re-emitted

- **Setup**: compile module, modify one function, recompile
- **Expected**: modified function recompiled; unchanged functions use cache

### TC-017: cache invalidated when api_version changes

- **Setup**: cache from version 12 build; recompile with version 11
- **Expected**: full recompilation (cache miss)

---

## roundtrip.rs

### TC-018: hello-world IIFE compiles to parseable .abc

- **Input**: `(function() { "use strict"; var x = 1 + 2; })()`
- **Expected**: no errors; output file parses as valid Panda binary
- **Verify**: header magic, class count ≥ 1, no invalid offsets

### TC-019: globalThis registry chunk compiles correctly

- **Input**: IIFE with `globalThis['EntryAbility_abc123'] = { default: exports.default }`
- **Expected**: compiles without error; `ldglobalvar` / store opcodes present
