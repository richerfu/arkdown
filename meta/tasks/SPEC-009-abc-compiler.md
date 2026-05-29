# SPEC-009: ArkCompiler Bytecode Compiler (Rust)

**Crate**: `arkdown-abc`
**Priority**: P0
**Blocked by**: SPEC-005, SPEC-006
**Blocks**: SPEC-012, SPEC-027

---

## Summary

Implement the ArkCompiler bytecode (`.abc`) compilation pipeline in pure Rust. This is a
Rust port of `es2abc` — the ETS/JS → bytecode compiler from OpenHarmony's
`arkcompiler_ets_frontend` project.

**Reference implementation**: `https://gitcode.com/openharmony/arkcompiler_ets_frontend`
**Bytecode format spec**: `https://gitcode.com/openharmony/arkcompiler_runtime_core`

The compiled Rust implementation replaces the SDK `es2abc` binary entirely.
No subprocess invocation. All compilation logic lives in this crate.

---

## Background: What es2abc Does

From reading `arkcompiler_ets_frontend`, `es2abc` (internally `es2panda`) performs:

1. **Parse** — JS/TS source → AST (using its own parser, not OXC)
2. **Check** — semantic analysis, type checking (for TS)
3. **Lower** — AST → IR (intermediate representation)
4. **Emit** — IR → Panda Bytecode (`.abc` binary format)

The input to `arkdown-abc` is the source file shape hvigor passes to `es2abc`, so:

- Steps 1–2 are simplified: the pinned `ohos-rs/oxc` fork parses JS/TS/ETS syntax before
  lowering. Stage-model builds pass module-mode input, matching upstream `--module`.
- Steps 3–4 are the core of what we implement

## TypeScript/ETS Normalization (Required Pre-Step)

Before lowering to Panda IR, TypeScript/ETS syntax must be normalized through the pinned
`ohos-rs/oxc` parser/codegen path. This must preserve module-mode semantics; upstream
`es2abc --module` accepts normal source files and does not require an IIFE wrapper.

```rust
// Normalize TS/ETS before bytecode generation.
use oxc_codegen::Codegen;
use oxc_parser::Parser;
use oxc_span::SourceType;

fn strip_types(source: &str) -> String {
    let source_type = SourceType::tsx().with_module(true);
    let allocator = Allocator::default();
    let ret = Parser::new(&allocator, source, source_type).parse();
    Codegen::new().build(&ret.program).code
}
```

This step produces source that the IR lowering stage can process without encountering
type-only constructs (`interface`, `: Type`, `as Type`, `<Type>` casts, etc.).

---

## Panda Bytecode Format

The `.abc` file format is defined in `arkcompiler_runtime_core`. Key structures:

```
.abc file layout:
  Header (magic: b"PANDA\x00\x00\x00" (8 bytes), version, file_size, ...)
  ClassIndex        ← table of class records
  MethodIndex       ← table of method records
  StringTable       ← interned strings
  LiteralArrayTable ← constant arrays
  CodeSection       ← bytecode instruction sequences
  DebugInfoSection  ← source positions (when --debug-info)
```

**Magic bytes**: The Panda `.abc` header starts with `b"PANDA\x00\x00\x00"` (8 bytes, null-padded
to 8). This is distinct from older references to `0x414243` — the correct magic for files produced
by the current ArkCompiler is the `PANDA` ASCII string.

Reference: `arkcompiler_runtime_core/libpandafile/file_format.md` (if present) or
the `.abc` format documentation in `runtime_core`.

---

## Key Implementation Components

### 1. JS IR Lowering

From the parsed JS/TS source:

- Map JS constructs to Panda IR opcodes
- `function` → method record
- `var`/`let`/`const` → register allocation
- `import`/`export` → module record metadata and module-variable opcodes
- `globalThis['x']` access → `ldglobalvar` opcode

### 2. Panda Instruction Set

Key opcodes (from `arkcompiler_runtime_core/isa/isa.yaml`, verified against real `modules.abc`):

```
# Constant loaders (opcode / byte length)
0x00  ldundefined             # load undefined (1 byte)
0x01  ldnull                  # load null (1 byte)
0x02  ldtrue / 0x03 ldfalse   # load boolean (1 byte)
0x62  ldai <i32>              # load integer immediate (5 bytes)
0x63  fldai <f64>             # load float immediate (9 bytes)
0x3E  lda.str <id:u16>        # load string (3 bytes)
0x6F  ldthis                  # load `this` receiver in method (1 byte)

# Register moves
0x60  lda <vreg:u8>           # load register → acc (2 bytes)
0x61  sta <vreg:u8>           # store acc → register (2 bytes)

# Arithmetic (format: op_imm_8_v_8 = opcode + slot + vreg = 3 bytes)
0x0A  add2   slot, vreg       # acc = acc + v[vreg]
0x0B  sub2   slot, vreg       # acc = acc - v[vreg]
0x0C  mul2   slot, vreg       # acc = acc * v[vreg]
0x0D  div2   slot, vreg       # acc = acc / v[vreg]
0x0E  mod2   slot, vreg       # acc = acc % v[vreg]

# Equality / comparison (op_imm_8_v_8, 3 bytes; acc = acc OP v[vreg])
0x0F  eq           slot, vreg  # ==
0x10  noteq        slot, vreg  # !=
0x11  less         slot, vreg  # <
0x12  lesseq       slot, vreg  # <=
0x13  greater      slot, vreg  # >
0x14  greatereq    slot, vreg  # >=
0x27  strictnoteq  slot, vreg  # !==
0x28  stricteq     slot, vreg  # ===
0x25  isin         slot, vreg  # acc (key) in v[vreg] (object)

# Bitwise binary (op_imm_8_v_8, 3 bytes)
0x15  shl2  / 0x16 shr2  / 0x17 ashr2
0x18  and2  / 0x19 or2   / 0x1A xor2

# Unary (op_imm_8 = opcode + slot = 2 bytes; acc = OP acc)
0x1C  typeof  slot   # acc = typeof acc (string)
0x1F  neg     slot   # acc = -acc
0x20  not     slot   # acc = ~acc (bitwise NOT)
0x21  inc     slot   # acc = acc + 1
0x22  dec     slot   # acc = acc - 1

# Calls
0x29–0x2C  callarg0–callarg3   slot [, v0..v2]
0x2D–0x30  callthis0–callthis3 slot, this [, v0..v2]
0x73  callrange   slot, argc, v_start
0x31  callthisrange slot, argc, this_vreg

# Object construction
0x04  createemptyobject                         # acc = {} (1 byte)
0x07  createobjectwithbuffer  slot, lit_id      # acc = object from literal array (4 bytes)
                                                # lit_id = 0-based index in LiteralArray table
                                                # literal array: alternating TaggedString key/value
                                                # entries; count = 2 × 2N for N properties

# Object access
0x42  ldobjbyname  slot, name_id         # acc = acc.name
0x43  stobjbyname  slot, name_id, obj    # obj.name = acc

# Class / function definition
0x33  definefunc  slot, method_id, length
0x35  defineclasswithbuffer slot, ctor_id, buf_id, proto_argc, proto_vreg

# Module variables
0x7E  ldexternalmodulevar index
0x7C  stmodulevar index
0x3C  ldlexvar level, slot
0x3D  stlexvar level, slot

# Branches
0x4E  jmp   offset:i16
0x50  jeqz  offset:i16  # branch if acc == 0 / falsy
0x9B  jnez  offset:i16  # branch if acc != 0 / truthy

# Exception
0xFE 0x00  throw
```

Full ISA in `arkcompiler_runtime_core/isa/`.

### 3. Module Record System (--merge-abc)

With `--merge-abc`, multiple JS files become named "records" in one `.abc`:

```
merged.abc
  └── record "EntryAbility"     ← from EntryAbility.js
  └── record "commons"          ← from commons.js
  └── record "vendors"          ← from vendors.js
```

Each record has its own class index and method index. The globalThis registry allows
cross-record references at runtime.

### 4. Line Number Programs (Debug Info)

When a source file path is available (via `source_map` or `debug_info: true`), each method
gets a `DebugInfoRecord` and a Panda **LineNumberProgram (LNP)**.

#### DebugInfoRecord binary layout (emitted in the DebugInfo section)

```
ULEB128(line_start)       — 1-based starting line for the method
ULEB128(param_names_count)
[param_names: ULEB128 string offsets, count times]
ULEB128(constant_pool_count)  — currently 1 (the source file offset)
ULEB128(source_file_string_offset)
ULEB128(lnp_index)           — 0-based index into the LNP section
```

#### LNP body format (emitted in the LNP section, one entry per method)

```
SET_FILE (0x05) + u32 LE source-file string offset
<advance opcodes — may be empty>
END_SEQUENCE (0x00)
```

#### LNP opcodes generated by `LineTracker`

| Byte      | Meaning                                | Operands       |
| --------- | -------------------------------------- | -------------- |
| 0x00      | END_SEQUENCE — terminates the LNP      | none           |
| 0x01      | ADVANCE_PC — advance PC register       | ULEB128(delta) |
| 0x02      | ADVANCE_LINE — advance line register   | SLEB128(delta) |
| 0x08–0xFF | SPECIAL_OPCODE — combined PC+line step | none           |

**SPECIAL_OPCODE** encoding: `op = (line_delta * 16 + pc_delta) + 8`.
Valid when `0 ≤ pc_delta ≤ 16` and `0 ≤ line_delta ≤ 15` and `op ≤ 255`.

#### Implementation notes

- `MethodRecord::line_ops` holds the pre-encoded LNP body (including END_SEQUENCE)
  produced by `LineTracker::finish()`.
- Line tracking is currently enabled only for `func_main_0` (the module entry point)
  when a source file is attached. Nested function methods carry `line_ops: vec![]`
  and fall back to a minimal `SET_FILE + END_SEQUENCE` LNP.
- `LineTracker` computes 1-based line numbers from byte offsets using `line_at(source, offset)`.
- The LNP section in the emitted file uses `line_ops` verbatim when non-empty, or emits
  a bare `0x00` (END_SEQUENCE) otherwise.

### 5. Incremental Compilation Cache

Mirror es2abc's `--cache-file` behavior:

- Cache stores function-level bytecode keyed by source hash
- On rebuild: compare source hash; emit cached bytecode for unchanged functions
- Cache format: msgpack (use `rmp-serde`)

---

## API

```rust
pub struct AbcCompiler {
    options: AbcCompileOptions,
}

pub struct AbcCompileOptions {
    pub debug_info: bool,
    pub opt_level: u8,          // 0, 1, or 2
    pub target_api_version: u32,
    pub merge_abc: bool,
    pub module: bool,           // upstream es2abc --module semantics
    pub cache_path: Option<PathBuf>,
}

pub struct AbcModule {
    pub record_name: String,
    pub source: String,             // normal JS/TS source file
    pub source_map: Option<SourceMap>,
}

impl AbcCompiler {
    pub fn new(options: AbcCompileOptions) -> Self;

    /// Compile one or more modules into a merged .abc file.
    /// For HAP builds: one call with all chunk modules → one modules.abc
    pub fn compile(&self, modules: Vec<AbcModule>, output: &Path)
        -> Result<AbcArtifact, AbcError>;
}
```

---

## Files to Create

```
crates/arkdown-abc/
├── Cargo.toml
└── src/
    ├── lib.rs
    ├── compiler.rs         (AbcCompiler: orchestration)
    ├── ir/
    │   ├── mod.rs
    │   ├── builder.rs      (IR builder: JS AST → Panda IR)
    │   └── opcodes.rs      (Panda ISA opcode definitions)
    ├── emit/
    │   ├── mod.rs
    │   ├── file.rs         (AbcFile: binary format writer)
    │   ├── record.rs       (record/class index writing)
    │   ├── method.rs       (method bytecode emission)
    │   ├── strings.rs      (string table)
    │   └── literals.rs     (literal array table)
    ├── cache.rs            (incremental compilation cache)
    └── debug.rs            (debug info section emission)
```

---

## Development Approach

1. Start from `arkcompiler_ets_frontend` source (C++) to understand the IR and emit stages
2. Implement binary format writer (`emit/`) first — can be tested against existing `.abc` files
3. Implement IR builder for basic JS constructs
4. Add OXC integration: use `oxc_parser` to parse source files in script or module mode
5. Validate output: compiled `.abc` files must be loadable by ArkRuntime

---

## Acceptance Criteria

- `AbcCompiler::compile()` on minimal module source produces a valid `.abc`
- Produced `.abc` is accepted by the OHOS ArkRuntime (installable via `hdc install`)
- Panda binary header bytes match expected magic and version
- `--merge-abc` equivalent: multiple modules merged into one `.abc` with correct records

---

## Design References

- `meta/design/abc-pipeline.md` — module input contract, source map chain, flags context
- `meta/design/chunk-model.md` — legacy IIFE chunk behavior where applicable
- OpenHarmony source: `https://gitcode.com/openharmony/arkcompiler_ets_frontend`
- Bytecode format: `https://gitcode.com/openharmony/arkcompiler_runtime_core`

---

## Rolldown Alignment

| arkdown concept                    | rolldown equivalent                                   | note                                                                                                       |
| ---------------------------------- | ----------------------------------------------------- | ---------------------------------------------------------------------------------------------------------- |
| `AbcCompiler::compile()`           | no rolldown equivalent                                | rolldown produces JS; arkdown goes further and compiles JS → Panda bytecode                                |
| `AbcModule::record_name`           | rolldown chunk `name` field                           | Used as the Panda record name in `--merge-abc` output                                                      |
| `AbcCompileOptions::opt_level`     | rolldown `minify` option                              | Different mechanism: rolldown uses JS minification; arkdown sets the bytecode optimizer level              |
| `AbcCompileOptions::debug_info`    | rolldown `sourcemap` option                           | Analogous: includes debug info (source positions) in the `.abc` output                                     |
| `AbcCompileOptions::merge_abc`     | rolldown multi-entry `input` → multiple output chunks | With `merge_abc: true`, multiple modules compile into one `.abc` file; with `false`, one `.abc` per module |
| Incremental cache (function-level) | rolldown module-level transform cache (SPEC-019)      | arkdown caches at the bytecode function level; rolldown caches at the JS module level                      |

**Key divergence:** This crate has no rolldown equivalent. rolldown is a JS bundler; it does not produce bytecode. `arkdown-abc` is a Rust port of `es2abc` (OpenHarmony's `arkcompiler_ets_frontend`). The input contract follows upstream `es2abc`: normal source files, with `--module` selecting ECMAScript module parsing. Legacy IIFE/CJS input remains a compatibility path.

---

## Test Requirements

- `AbcCompiler::compile()` on minimal module JS `export const x = 1;` with `module: true` produces a binary starting with `b"PANDA\x00\x00\x00"` (8 bytes)
- TypeScript normalization: TS/ETS syntax is normalized before IR lowering; the compiled `.abc` is valid (no type-annotation parse errors)
- Compiled `.abc` header: version field matches the expected version for `target_api_version = 12`
- Compiled `.abc` is accepted by ArkRuntime: install a minimal HAP on device/emulator and verify `hdc install` succeeds
- `merge_abc: true` with two `AbcModule` inputs produces one `.abc` file with two named records; verify record names match input `record_name` values
- `merge_abc: false` compiles each module to a separate `.abc` file
- `debug_info: true` includes source position entries in the `.abc`; `debug_info: false` produces a smaller file without them
- `opt_level: 0` vs `opt_level: 2`: release-mode output is smaller
- Incremental cache: compiling the same JS twice reuses cached bytecode (second call faster, output identical)
- Cache invalidation: changing the JS source produces different bytecode (not stale cached output)
- module exports map to module-variable metadata/opcodes, and legacy `globalThis['name_hash']` access maps to `ldglobalvar` in non-module compatibility tests
- `AbcError::CompilationFailed` is returned (not panic) when given syntactically invalid JS
