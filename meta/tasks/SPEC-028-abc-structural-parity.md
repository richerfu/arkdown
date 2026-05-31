# SPEC-028: ABC Structural and DebugInfo Parity

**Crate**: `arkdown-abc`
**Priority**: P1
**Blocked by**: SPEC-009
**Last updated**: 2026-05-31

## Upstream References

- `arkcompiler_ets_frontend/es2panda/compiler/core/emitter/emitter.cpp`
- `arkcompiler_runtime_core/static_core/assembler/assembly-emitter.cpp`
- `arkcompiler_runtime_core/libabckit/src/codegen/codegen_dynamic.cpp`
- `developtools_ace_ets2bundle/compiler/src/gen_merged_abc.ts`

## Current State

For `tests/fixtures/minimal-hap`, the unsigned HAP is byte-identical to hvigor
except for `ets/modules.abc`.

ABC status:

- hvigor `ets/modules.abc`: 9844B
- arkdown `ets/modules.abc`: 9705B
- all compared method CodeItems are byte-identical
- remaining difference is DebugInfo, LineNumberProgram, column-number events,
  and local-variable debug metadata

Older reports that described `func_main_0` OHM prefix, duplicate annotation
strings, external descriptor format, or per-statement-only LNP as the active
minimal-hap blockers are obsolete. Those structural gaps have either been fixed
or are not the current minimal-hap delta. The active blocker is now the exact
source-position event stream used to build DebugInfo/LNP.

## Completed Structural Fixes

These items are retained as historical context so future work does not repeat
the old assumptions:

- bare method names are used where hvigor uses bare `func_main_0`-style method
  NameItems; the OHM path belongs to the ClassRecord
- shared annotation/string table layout has been tightened enough for current
  minimal-hap CodeItem parity
- method index and literal-array placement match the current fixture
- LNP index is emitted at the file end, after literal arrays
- LNP opcodes and DebugInfo constant_pool architecture follow upstream
  `BuildLineNumberProgram`: opcodes in LNP `data_`, operands in DebugInfo
  `constant_pool`
- per-instruction tracking is wired through `CodeGen::push()`; the remaining
  problem is which source line/column is attached to each instruction

## Active Bug 1: Synthesized Constructor Source Positions

### Problem

`EntryAbility #~@0=#EntryAbility` has byte-identical CodeItem output, but hvigor
emits constructor body line/column events while `arkdown` currently emits only
scope local start/end rows for this method.

### Upstream Behavior

`emitter.cpp::GenInstructionDebugInfo()` takes the AST node attached to each IR
instruction. Normal instructions use `Range().start`; return instructions use
the enclosing function or block end position.

Generated or inserted instructions without a direct source position inherit the
nearest previous emitted source position, as seen in `codegen_dynamic.cpp`.

### Fix

Attach source positions to constructor field-initializer and synthesized body
instructions without changing CodeItem bytes:

1. set debug position before emitted constructor body and field-initializer
   instruction groups;
2. preserve inherited source position for generated helper instructions;
3. use function/block end position before generated return instructions.

## Active Bug 2: Top-Level Closure Source Positions

### Problem

Top-level call-argument closures such as `Index #*#` and `Index #*#^1` have
matching CodeItems but different LNP/column streams. `arkdown` currently loses
the arrow AST source position for some synthesized closure methods.

### Fix

When collecting top-level call-argument arrows:

- use the arrow AST start position for the first body instruction;
- use the arrow/function end position for return instructions;
- recurse through call/new/object/member expressions only according to upstream
  closure emission behavior, without adding extra closure methods.

## Active Bug 3: `func_main_0` Column Stream

### Problem

`Index func_main_0` has identical CodeItem bytes but different LNP and DebugInfo
constant_pool bytes. The remaining mismatch is line/column event attribution and
mid-method local-variable debug events, not executable bytecode.

### Fix

Align `CodeGen::set_debug_pos()` calls with es2panda instruction boundaries:

- apply positions before every expression/literal/member/call instruction group
  that es2panda attaches to an AST node;
- make inserted instructions inherit the previous source position;
- keep return handling aligned with `UpdateForReturnIns()` in `emitter.cpp`;
- keep local-variable start/end events interleaved according to
  `CollectLocalVariable()` and `BuildLineNumberProgram()`.

## Active Bug 4: Direct CompileArkTS Cache-TS Route

### Problem

The generated CompileArkTS cache TypeScript files match hvigor for the minimal
fixture, but compiling those files directly through `arkdown-abc` currently
shrinks the ABC and changes/misses some CodeItems. This proves the direct TS
input route has backend logic gaps.

### Fix

Do not switch final ABC generation to the cache-TS handoff until CodeItems match
on that input shape. Use the handoff files as an oracle:

- `filesInfo.txt` supplies record name, source file, package name, and module type;
- `npmEntries.txt` supplies native SDK module aliases;
- `sourceMaps.json` supplies the debug source-map oracle;
- direct TS compilation must first preserve the same CodeItems as hvigor.

## Acceptance Criteria

For the minimal-hap fixture:

1. HAP entry order remains identical to hvigor.
2. All non-ABC HAP entries remain byte-identical.
3. All method CodeItems remain byte-identical.
4. `ets/modules.abc` DebugInfo/LNP bytes converge to hvigor.
5. No fix is accepted if it reduces ABC size by dropping method bytecode or
   omitting closure/class generation logic.

## Test Plan

- `cargo fmt --all --check`
- `cargo check -p arkdown-cli`
- `cargo run -p arkdown-cli -- build --project tests/fixtures/minimal-hap`
- compare unsigned HAP entries with the local hvigor fixture
- compare ABC methods with a CodeItem-vs-DebugInfo diff helper

Future regression tests should separate:

- method CodeItem bytes;
- LineNumberProgram bytes;
- DebugInfo constant_pool bytes;
- StringTable/ClassRecord/LiteralArray structural sections.
