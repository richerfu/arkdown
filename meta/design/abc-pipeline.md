# ABC Pipeline: ArkCompiler Bytecode Compilation

This document describes the ArkCompiler bytecode (`.abc`) compilation step — the mandatory
transformation from JS/TS source to ArkRuntime-executable bytecode. In arkdown this
is implemented as a pure Rust crate (`arkdown-abc`), porting the reference implementation
at `https://github.com/openharmony/arkcompiler_ets_frontend`.

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

**Source**: `https://github.com/openharmony/arkcompiler_ets_frontend`
**Bytecode format**: `https://github.com/openharmony/arkcompiler_runtime_core`

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

The current Rust cache is an internal development cache and does **not** yet
mirror hvigor's `--cache-file @modules.cache` behavior. hvigor writes a
`modules.cache` mapping from each CompileArkTS cache source to a `.protoBin`
ProgramCache file, plus an entry for `npmEntries.txt -> npmEntries.protoBin`.
Upstream es2panda then merges those cached programs during `ResolveAndGenerate()`.

Implemented parity pieces:

- CLI writes hvigor-style `filesInfo.txt`, `npmEntries.txt`, and `modules.cache`.
- `arkdown-ts2abc` accepts `--npm-module-entry-list`.
- `arkdown-abc` lowers `npmEntries.txt` through the same record shape as
  upstream `CompileNpmModuleEntryList()`: one record per line, one `u8=false`
  field named by the build path. Record names are serialized in pandasm class
  form (`L<record>;`), and fixed SDK namespace stubs are de-duplicated when the
  npm-entry records provide the real SDK namespace classes.
- ABC header version is selected from target API, matching native es2abc
  (`0x0002000c` for API 12, `0x0006000c` for API 12..17 current/release).
- Diagnostic output uses the community `tracing` stack (`tracing::debug!`,
  `ARKDOWN_LOG=debug`) rather than a custom debug logging path.

Remaining parity work:

- replace the internal JSON bytecode cache with `modules.cache`/`.protoBin`
  ProgramCache semantics;
- include cached Program merge order and debug-info data exactly as upstream
  es2panda does;
- keep cache diagnostics on `tracing`.

---

## 7. Debug Info Section

When `opt_level = 0` (debug builds):

- Emit `DebugInfoSection` containing source position tables
- Maps each bytecode instruction to a source line/column
- ABC line/column positions come from the JS/TS source text that `es2abc`
  parses. Upstream `emitter.cpp::GenInstructionDebugInfo()` attaches AST
  `Range().start` positions to normal instructions and function/block end
  positions to return instructions; `assembly-emitter.cpp::BuildLineNumberProgram()`
  then emits LNP and column events from that pandasm instruction stream.
- `sourceMaps.map` is still packaged for debugger source remapping, but it is
  not an input to `BuildLineNumberProgram()` and must not be treated as the
  ABC DebugInfo coordinate source.

### DebugInfoItem binary layout

Each method with debug info emits one `DebugInfoItem` followed by one `LineNumberProgramItem`:

```
DebugInfoItem:
  line_start:           ULEB128    # initial 1-based source line
  num_parameters:       ULEB128    # count of named formal parameters
    [× ULEB128 string_idx]         # file offset of each parameter name StringItem
  constant_pool_count:  ULEB128    # count of constant pool entries (= 1 for source file)
    [× ULEB128 string_idx]         # file offset of each string (entry 0 = source file path)
  lnp_index:            ULEB128    # index into the LNP index table in the file header
```

`constant_pool_count` is the **count of entries**, not the byte length of encoded values.
One entry is always emitted: the file offset of the source file `StringItem`.

### LineNumberProgramItem opcodes

```
0x00  END_SEQUENCE                                           # terminates the program
0x01  ADVANCE_PC    (ULEB128 delta)                         # advance pc without emitting a row
0x02  ADVANCE_LINE  (SLEB128 delta)                         # change line counter without emitting a row
0x03  START_LOCAL   (str_u32, reg_u8, type_str_u32)        # begin variable scope (10 bytes)
0x04  START_LOCAL_EXTENDED (str_u32, reg_u8, type_str_u32, type_tag_u16)  # begin variable scope with type tag (12 bytes)
0x05  SET_FILE      (str_u32)                               # set current source file (4-byte LE file offset)
0x08  SET_PROLOGUE_END                                      # mark end of function prologue (1 byte, no operands)
0x09  SET_EPILOGUE_BEGIN                                    # mark start of function epilogue (1 byte, no operands)
0x08+ SPECIAL_OPCODE                                        # combined PC+line advance (no operand)
```

Note: SET_PROLOGUE_END (0x08) shares the opcode space with SPECIAL_OPCODEs. The minimum
SPECIAL_OPCODE is also 0x08 (zero-delta advance that "emits a row"). Parsers that understand
the prologue-end marker use it as a source-level debug annotation.

**SPECIAL_OPCODE formula**: `op = (line_delta * 16 + pc_delta) + 8`
Valid when `0 ≤ pc_delta ≤ 16`, `0 ≤ line_delta ≤ 15`, and `op ≤ 255`.
When the range is exceeded, fall back to explicit `ADVANCE_LINE` + `ADVANCE_PC` opcodes.

**Current implementation** (`arkdown-abc` `LineTracker`):

- `MethodRecord::line_ops: Vec<u8>` stores the pre-encoded LNP body
  (advance sequences + `END_SEQUENCE`) for each method.
- Line tracking is active for all methods that have a source file.
  `CodeGen::push()` emits one `LineTracker::advance()` call per bytecode instruction
  (per-instruction ADVANCE density, matching hvigor/es2abc).
- `CodeGen::set_debug_pos()` should model the upstream AST-node assignment:
  normal instructions use the attached AST node start, return instructions use
  the enclosing function or block end, and inserted instructions inherit the
  nearest previous emitted source position.
- 1-based line numbers and 0-based column numbers are computed from parsed
  source spans via `line_at()`/`col_at()`.

**Current LNP body structure** emitted by `arkdown-abc` for each method with debug info:

```
SET_FILE (0x05)                        5 bytes  — 4-byte LE string file offset
START_LOCAL_EXTENDED (0x04) × 3       36 bytes  — "this"(reg0), "newTarget"(reg1), "funcObj"(reg2)
  each entry: [0x04][name_off:u32 LE][reg:u8][type_off:u32 LE=0][type_tag:u16 LE=0] = 12 bytes
START_LOCAL_EXTENDED × N_params       12×N bytes — formal parameter names starting at reg3
SET_PROLOGUE_END (0x08)                1 byte   — marks end of function prologue
<line_ops>                            variable  — one SPECIAL_OPCODE (1B) per instruction;
                                                   ADVANCE_LINE for line jumps; ADVANCE_PC for
                                                   large PC deltas
SET_EPILOGUE_BEGIN (0x09)              1 byte   — emitted immediately before return instruction
END_SEQUENCE (0x00)                    1 byte   — terminates program
```

Minimum valid LNP body: `[0x05, <4 bytes source file string offset LE>]` +
`[0x04 × 3 scope vars × 12 bytes each]` + `[0x08]` + `[0x09]` + `[0x00]`
= 5 + 36 + 1 + 1 + 1 = 44 bytes for a zero-parameter, single-`returnundefined` method.

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
