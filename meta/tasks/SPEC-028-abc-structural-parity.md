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

For `tests/fixtures/minimal-hap`, the unsigned HAP entry order and all entries
are byte-identical to hvigor.

ABC status:

- hvigor `ets/modules.abc`: 9844B
- arkdown `ets/modules.abc`: 9844B
- all 23 compared method records, CodeItems, DebugInfo, LineNumberProgram
  streams, and final ABC bytes are byte-identical

Older reports that described `func_main_0` OHM prefix, duplicate annotation
strings, external descriptor format, or per-statement-only LNP as the active
minimal-hap blockers are obsolete. Those structural gaps have either been fixed
or are not the current minimal-hap delta. The active blocker is now preserving
this byte parity across broader fixtures.

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
- per-instruction tracking is wired through `CodeGen::push()`
- MethodItem tag order matches hvigor:
  `CODE -> SOURCE_LANG -> DEBUG_INFO -> ANNOTATION -> NOTHING`
- trailing LNP index is 4-byte aligned like hvigor

## Completed Fix: Default Derived Constructor Source Positions

`EntryAbility #~@0=#EntryAbility` now matches hvigor for the minimal fixture:
CodeItem bytes, LNP length, and DebugInfo constant_pool length all align. The
implementation adds a tracked line/column stream for the default derived
constructor without changing its bytecode.

Remaining constructor work is broader coverage: explicit constructors and
constructors with field initializers still need golden comparison.

## Active Bug 1: Top-Level Closure Source Positions

### Problem

Top-level call-argument closures such as `Index #*#` and `Index #*#^1` have
matching CodeItems. On the direct cache-TS route their LNP/column streams now
match hvigor after restoring upstream arrow return source positions.

### Fix

When collecting top-level call-argument arrows:

- use the arrow AST start position for the first body instruction;
- use the block end for empty-block generated returns;
- use the expression end for expression-body returns;
- recurse through call/new/object/member expressions only according to upstream
  closure emission behavior, without adding extra closure methods.

## Completed Fix: `func_main_0` Column Stream

### Problem

`Index func_main_0` now matches hvigor on the cache-TS route. The previous
mismatch was line/column event attribution and mid-method local-variable debug
events, not executable bytecode.

### Fix

Align `CodeGen::set_debug_pos()` calls with es2panda instruction boundaries:

- apply positions before every expression/literal/member/call instruction group
  that es2panda attaches to an AST node;
- make inserted instructions inherit the previous source position;
- keep return handling aligned with `UpdateForReturnIns()` in `emitter.cpp`;
- keep local-variable start/end events interleaved according to
  `CollectLocalVariable()` and `BuildLineNumberProgram()`.

## Active Bug 3: General Constructor Source Positions

### Problem

The default derived constructor path matches the minimal fixture, but explicit
constructors and field-initializer constructors still need the same
source-position treatment.

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

## Completed Fix: Direct CompileArkTS Cache-TS Route

### Problem

The generated CompileArkTS cache TypeScript files match hvigor for the minimal
fixture, and compiling those files through `arkdown-abc` now emits a 9844B
`modules.abc` that is byte-identical to hvigor. The direct route
reached bytecode parity after top-level block traversal, named default class
export lowering, lexical temporary scoping, class declaration missing property
initialization, and the upstream 0-based source-line/default-parameter/unary
logical-not/binary-expression-right/condition-comparison/member-write/new-expression/class-missing-property/class-method-return/super-call
source-position rules were aligned. It now also matches upstream
`FunctionBuilder::ImplicitReturn()` by using `INVALID_SOURCE_LOCATION` before the
generated implicit-return `ldundefined`, and matches `Condition::Compile()` plus
`Identifier::Compile()` by attributing right-side `undefined` comparisons to the
identifier node. It also matches `GenScopeVariableInfo()` by omitting debug
locals for lexical-bound `this`, filters hidden function `this` lexenv prologue
events, matches derived-constructor final super-call validation source
positions, and aligns top-level call-argument arrow return positions with
upstream source-backed LNP behavior. It also aligns class declaration/class
export stores, variable declarator stores, call argument stores, branch
conditions, call instruction line/column restoration, range-call argument
stores, getter/setter property-key stores, special-opcode pc-delta boundaries,
parameter-name string order, MethodItem tag order, and trailing LNP-index
alignment to upstream behavior.

### Fix

Keep final ABC generation on the cache-TS handoff, matching hvigor's `es2abc`
input shape:

- `filesInfo.txt` supplies record name, source file, package name, and module type;
- `npmEntries.txt` supplies native SDK module aliases;
- the cache TS source text supplies the AST line/column coordinates that
  upstream `es2abc` writes into pandasm DebugInfo;
- `sourceMaps.json`/`sourceMaps.map` are package debugger artifacts, not inputs
  to `BuildLineNumberProgram()`;
- direct TS compilation now preserves the same CodeItems and non-CodeItem ABC
  sections as hvigor for the minimal fixture.

## Acceptance Criteria

For the minimal-hap fixture:

1. HAP entry order is aligned to hvigor.
2. All same-name HAP entries remain byte-identical.
3. All method CodeItems remain byte-identical.
4. `ets/modules.abc` remains byte-identical to hvigor.
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
