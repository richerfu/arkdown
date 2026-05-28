# ABC Pipeline: ArkCompiler Bytecode Compilation

This document describes the ArkCompiler bytecode (`.abc`) compilation step — the mandatory
transformation from JS/TS source to ArkRuntime-executable bytecode. In arkdown this
is implemented as a pure Rust crate (`arkdown-abc`), porting the reference implementation
at `https://gitcode.com/openharmony/arkcompiler_ets_frontend`.

---

## 1. Why ABC Is Required

ArkRuntime does **not** execute raw JavaScript. All code must be compiled to ArkCompiler
Bytecode (`.abc`) before deployment. The `.abc` format is a Panda-derived binary format.

```
.ets/.ts source
    │
    ▼  arkdown bundler (ArkTS transform + hvigor-aligned module output)
.js/.ts  (normal source file; `--module` selects ESM parsing)
    │
    ▼  arkdown-abc  (Rust, ports arkcompiler_ets_frontend)
.abc (ArkCompiler Bytecode)
    │
    ▼  HAP zip
ArkRuntime executes .abc
```

There is no way to ship a HAP containing plain `.js` files.

---

## 2. Reference Implementation

**Source**: `https://gitcode.com/openharmony/arkcompiler_ets_frontend`
**Bytecode format**: `https://gitcode.com/openharmony/arkcompiler_runtime_core`

The reference `es2abc` tool (C++) performs:

1. **Parse** — JS/TS source → AST (`es2panda` parser)
2. **Check** — semantic analysis
3. **Lower** — AST → Panda IR
4. **Emit** — IR → `.abc` binary (Panda bytecode format)

arkdown-abc ports stages 3–4 in Rust. For stage 1–2, it reuses the pinned `ohos-rs/oxc`
fork. Upstream `es2abc` accepts source files directly (`./es2abc [options] file.js`);
`--module` controls module parsing. The `ts2abc.js` wrapper in upstream sources forwards
bytecode-version options to `es2abc`; it does not impose an IIFE entry file.

---

## 3. Panda Bytecode Format

Defined in `arkcompiler_runtime_core/libpandafile/`. Key sections:

```
.abc
├── Header           magic, version, file_size, offsets to sections
├── StringTable      interned UTF-8 strings; referenced by offset
├── LiteralArrayTable  constant arrays (JSON-like literal data)
├── ClassIndex       array of class record offsets
├── Classes          one ClassRecord per module record (--merge-abc)
│   └── Methods      one MethodRecord per function
│       └── Code     Panda bytecode instruction sequence
└── DebugInfoSection source positions (debug builds only)
```

### Key Opcodes (from `arkcompiler_runtime_core/isa/isa.yaml`)

| Opcode                              | Effect                  |
| ----------------------------------- | ----------------------- |
| `ldundefined`                       | acc ← undefined         |
| `ldnull`                            | acc ← null              |
| `ldtrue` / `ldfalse`                | acc ← boolean           |
| `ldai <imm32>`                      | acc ← integer immediate |
| `fldai <imm64>`                     | acc ← float immediate   |
| `lda.str <str_id>`                  | acc ← string from table |
| `lda <vreg>`                        | acc ← virtual register  |
| `sta <vreg>`                        | vreg ← acc              |
| `add2 <vreg>`                       | acc ← acc + vreg        |
| `sub2 <vreg>`                       | acc ← acc - vreg        |
| `call.short <method_id>, <args...>` | call (≤ 4 args)         |
| `call <method_id>, <args...>`       | call (any arity)        |
| `callthis.short`                    | call with `this`        |
| `return`                            | return acc              |
| `returnundefined`                   | return undefined        |
| `ldglobalvar <name_id>`             | acc ← globalThis[name]  |
| `stglobalvar <name_id>`             | globalThis[name] ← acc  |
| `jeq <vreg>, <label>`               | jump if equal           |
| `jmp <label>`                       | unconditional jump      |

---

## 4. Input Format: Source File Plus Module Mode

The input to `arkdown-abc` is the same source shape hvigor sends to `es2abc`: a normal
JS/TS source file plus compile flags. Stage-model builds use module mode, so imports and
exports remain top-level syntax and the compiler parses them as ECMAScript modules:

```javascript
import { hilog } from "@kit.PerformanceAnalysisKit";

export class EntryAbility {
  onCreate() {
    hilog.info(0, "entry", "created");
  }
}
```

Legacy/non-module compatibility may still compile an IIFE-wrapped CommonJS chunk, but
that is not the required `es2abc`/`ts2abc` entry contract. Key IR lowering rules include:

- top-level `import` / `export` → module record entries and module-variable opcodes
- `globalThis['key']` read → `ldglobalvar` opcode
- `globalThis['key'] = value` → `stglobalvar` opcode
- non-module `(function() { ... })()` → compatibility top-level body unwrap

---

## 5. `--merge-abc` Equivalent: Multi-Record Output

For a HAP module with multiple chunks (`EntryAbility.js`, `commons.js`, `vendors.js`),
`arkdown-abc` produces a single merged `.abc` containing one **record** per chunk.
Each record has its own class index and method index.

```
modules.abc
├── ClassRecord "EntryAbility"
│   └── methods from EntryAbility.js
├── ClassRecord "commons"
│   └── methods from commons.js
└── ClassRecord "vendors"
    └── methods from vendors.js
```

Records share the string table and literal array table, so common strings are deduplicated.

---

## 6. Incremental Compilation Cache

Cache strategy mirrors the original `--cache-file` behavior:

- Cache key per function: `SHA256(function_source_text)`
- On hit: emit cached bytecode directly, skip IR lowering
- On miss: lower to IR, emit bytecode, store in cache
- Cache format: msgpack file per module, stored in `build/.arkdown_cache/abc/`
- Full invalidation when: `target_api_version` changes, `opt_level` changes, or
  config hash changes (see SPEC-019)

---

## 7. Debug Info Section

When `opt_level = 0` (debug builds):

- Emit `DebugInfoSection` containing source position tables
- Maps each bytecode instruction to a source line/column
- Source positions chain: `.ets` → bundled JS (rolldown source map) → `.abc` offsets
- DevEco Studio debugger reads this chain to show breakpoints in `.ets` source

---

## 8. Optimization Levels

| Level       | Behavior                                                              |
| ----------- | --------------------------------------------------------------------- |
| 0 (debug)   | No optimization; debug info included; all variable names preserved    |
| 1           | Basic optimizations (constant folding, dead branch elimination)       |
| 2 (release) | Full optimization; debug info omitted; applies after obfuscation pass |

---

## 9. ArkTS Widget Compilation

Widgets (system launcher app extensions) require a separate compilation pass:

- Same pipeline but with `widget_compile = true` flag
- Uses a separate cache directory to avoid cross-contamination
- Entry points from a widget-specific `loader.json`

See SPEC-027 for details.

---

## Cross-Reference

- [chunk-model.md](chunk-model.md) — legacy IIFE wrapping and chunk registry behavior
- [pipeline.md](pipeline.md) — where ABC compilation fits in the overall build
- [hvigorw-analysis.md](hvigorw-analysis.md) — original ArkCompile task context
- [plugins.md](plugins.md) — compile-stage plugin context
