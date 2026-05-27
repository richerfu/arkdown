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

The compiled Rust implementation replaces the closed-source `es2abc` binary entirely.
No subprocess invocation. All compilation logic lives in this crate.

---

## Background: What es2abc Does

From reading `arkcompiler_ets_frontend`, `es2abc` (internally `es2panda`) performs:

1. **Parse** — JS/TS source → AST (using its own parser, not OXC)
2. **Check** — semantic analysis, type checking (for TS)
3. **Lower** — AST → IR (intermediate representation)
4. **Emit** — IR → Panda Bytecode (`.abc` binary format)

The input to `arkdown-abc` is already-bundled IIFE JS (not raw ArkTS), so:

- Steps 1–2 are simplified: OXC already parsed and transformed the source; we just need to
  re-parse the IIFE JS output (which is standard JavaScript after ArkTS desugaring)
- Steps 3–4 are the core of what we implement

## TypeScript Stripping (Required Pre-Step)

Before lowering to Panda IR, any TypeScript type annotations remaining in the IIFE output
**must be stripped** using `oxc_codegen`. Even after bundling, some `.ts`/`.ets` type syntax may
survive into the chunk JS (e.g., type casts, interface references). The Panda IR builder only
handles plain JavaScript.

```rust
// Strip TS types before bytecode generation
use oxc_codegen::{CodeGenerator, CodegenOptions};
use oxc_parser::Parser;
use oxc_span::SourceType;

fn strip_types(source: &str) -> String {
    let source_type = SourceType::ts();
    let allocator = Allocator::default();
    let ret = Parser::new(&allocator, source, source_type).parse();
    CodeGenerator::new()
        .with_options(CodegenOptions { ..Default::default() })
        .build(&ret.program)
        .code
}
```

This step produces clean JavaScript that the IR lowering stage can process without encountering
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

From the IIFE-wrapped CJS chunk (already a standard JS AST after bundling):

- Map JS constructs to Panda IR opcodes
- `function` → method record
- `var`/`let`/`const` → register allocation
- `globalThis['x']` access → `ldglobalvar` opcode

### 2. Panda Instruction Set

Key opcodes (from `arkcompiler_runtime_core/isa/isa.yaml`):

```
ldundefined          # load undefined
ldnull               # load null
ldtrue / ldfalse     # load boolean literals
ldai <imm>           # load integer immediate
fldai <imm>          # load float immediate
lda.str <id>         # load string from string table
lda <vreg>           # load from virtual register
sta <vreg>           # store to virtual register
add2 <vreg>          # add acc + vreg
call.short <method>, <vreg...>   # call method (≤ 4 args)
call <method>, <vreg...>         # call method (any args)
callthis.short <method>, <this>, <vreg...>
return               # return accumulator value
returnundefined      # return undefined
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

### 4. Incremental Compilation Cache

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
    pub cache_path: Option<PathBuf>,
}

pub struct AbcModule {
    pub record_name: String,
    pub source: String,             // IIFE JS from bundler output
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
4. Add OXC integration: use `oxc_parser` to parse the IIFE chunks (standard JS after bundling)
5. Validate output: compiled `.abc` files must be loadable by ArkRuntime

---

## Acceptance Criteria

- `AbcCompiler::compile()` on minimal IIFE JS produces a valid `.abc`
- Produced `.abc` is accepted by the OHOS ArkRuntime (installable via `hdc install`)
- Panda binary header bytes match expected magic and version
- `--merge-abc` equivalent: multiple modules merged into one `.abc` with correct records

---

## Design References

- `meta/design/abc-pipeline.md` — input format (IIFE JS), source map chain, flags context
- `meta/design/chunk-model.md` — the IIFE JS format this compiler receives as input
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

**Key divergence:** This crate has no rolldown equivalent. rolldown is a JS bundler; it does not produce bytecode. `arkdown-abc` is a novel Rust port of `es2abc` (OpenHarmony's `arkcompiler_ets_frontend`). The input to this crate is IIFE-wrapped CJS JavaScript produced by SPEC-006.

---

## Test Requirements

- `AbcCompiler::compile()` on minimal IIFE JS `(function() { var x = 1; })()` produces a binary starting with `b"PANDA\x00\x00\x00"` (8 bytes)
- TypeScript stripping: IIFE JS containing type annotations (e.g., `var x: number = 1`) is stripped to plain JS before IR lowering; the compiled `.abc` is valid (no type-annotation parse errors)
- Compiled `.abc` header: version field matches the expected version for `target_api_version = 12`
- Compiled `.abc` is accepted by ArkRuntime: install a minimal HAP on device/emulator and verify `hdc install` succeeds
- `merge_abc: true` with two `AbcModule` inputs produces one `.abc` file with two named records; verify record names match input `record_name` values
- `merge_abc: false` compiles each module to a separate `.abc` file
- `debug_info: true` includes source position entries in the `.abc`; `debug_info: false` produces a smaller file without them
- `opt_level: 0` vs `opt_level: 2`: release-mode output is smaller
- Incremental cache: compiling the same JS twice reuses cached bytecode (second call faster, output identical)
- Cache invalidation: changing the JS source produces different bytecode (not stale cached output)
- `globalThis['name_hash']` access in IIFE JS maps to `ldglobalvar` opcode in the output (verify via bytecode disassembly or binary inspection)
- `AbcError::CompilationFailed` is returned (not panic) when given syntactically invalid JS
