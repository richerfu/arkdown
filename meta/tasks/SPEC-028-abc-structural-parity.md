# SPEC-028: ABC Structural Parity Fixes

**Crate**: `arkdown-abc`
**Priority**: P1 (blocks real ArkUI runtime execution)
**Blocked by**: SPEC-009
**Upstream refs**:
- `arkcompiler_ets_frontend/assembler/assembly_emitter.cpp`
- `arkcompiler_ets_frontend/assembler/assembly_emitter.h`
- hvigor gap analysis: TASK-ABC-01 through TASK-ABC-04

---

## Background

As of 2026-05-29, `arkdown-abc` achieves 88.1% byte-size parity with hvigor's `es2abc` output
(8675B vs 9844B for the minimal-hap fixture). Four structural bugs account for the remaining
1169B gap and prevent the generated `.abc` from executing correctly on ArkRuntime. This SPEC
defines the fixes for all four bugs.

---

## Bug 1: func_main_0 OHM Prefix in Method NameItem (TASK-ABC-01)

### Problem

hvigor stores each module's top-level function as the bare name `func_main_0` in the method
NameItem. `arkdown-abc` incorrectly stores the full OHM-prefixed path, e.g.:

```
@bundle:com.example/entry/src/main/ets/pages/Index#func_main_0
```

The OHM path belongs only in the **ClassRecord** name (the module record name), not in the
method NameItem. At runtime, ArkRuntime looks up `func_main_0` by bare name within the class
context — the full OHM prefix in the method name causes a lookup failure.

### Root Cause

`MethodRecord::name` is set from the module OHM path in `arkdown-abc/src/compiler.rs` during
module record emission. The code that constructs the method name string appends the full
class/module descriptor prefix.

### Fix

In the method serialization path (`crates/arkdown-abc/src/emit/`), when writing the method
NameItem for `func_main_0` (and any other per-module entry function), use only the bare
function name after the `#` separator. Specifically:

```rust
// Before (incorrect):
let method_name = format!("{}#{}", module_ohm_url, func_name);

// After (correct):
let method_name = func_name.to_string(); // e.g. "func_main_0"
```

The ClassRecord name retains the full OHM URL descriptor. The method name is bare.

**Upstream ref**: `assembly_emitter.cpp::SerializeMethod()` — stores `method.GetName()` (bare
name), not the class-qualified name. The class name is stored separately in `SerializeClasses()`.

---

## Bug 2: Duplicate Annotation Strings in StringTable (TASK-ABC-02)

### Problem

hvigor emits annotation strings (`pkgName@entry`, `isCommonjs`, `hasTopLevelAwait`,
`isSharedModule`, `scopeNames`, `moduleRecordIdx`) exactly once in the StringTable, with all
module records sharing the same StringItem offsets.

`arkdown-abc` re-adds these strings during each per-chunk annotation pass, resulting in
duplicate StringItem entries. This inflates the StringTable and produces incorrect offsets
for downstream records.

### Root Cause

The annotation emission path in `crates/arkdown-abc/src/emit/` calls `StringPool::intern()`
(or equivalent) but the interning does not prevent duplicate insertion when the same string
is emitted from multiple code paths (one per chunk).

### Fix

Ensure all annotation key strings are interned into the StringPool **before** any module
records are serialized. The intern step must be idempotent — calling `intern("isCommonjs")`
twice must return the same offset both times.

```rust
// In StringPool (crates/arkdown-abc/src/emit/string_pool.rs):
pub fn intern(&mut self, s: &str) -> u32 {
    if let Some(&offset) = self.map.get(s) {
        return offset; // reuse existing entry
    }
    let offset = self.next_offset();
    self.map.insert(s.to_string(), offset);
    self.items.push(s.to_string());
    offset
}
```

Pre-intern all annotation key strings during `AbcCompiler::build_string_table()` before the
first module record is emitted:

```rust
const ANNOTATION_KEYS: &[&str] = &[
    "pkgName",
    "isCommonjs",
    "hasTopLevelAwait",
    "isSharedModule",
    "scopeNames",
    "moduleRecordIdx",
];

fn pre_intern_annotation_keys(pool: &mut StringPool) {
    for key in ANNOTATION_KEYS {
        pool.intern(key);
    }
}
```

**Upstream ref**: `assembly_emitter.cpp::SerializeStringTable()` — strings interned into a
single table before all records are serialized; the same string offset is reused.

---

## Bug 3: External Module Ref Descriptor Format (TASK-ABC-03)

### Problem

hvigor stores external SDK module class names in Panda class descriptor format:

```
L@ohos.app.ability.UIAbility;
```

`arkdown-abc` stores them as:

```
@native.ohos.app.ability.UIAbility
```

The `L...;` wrapper is the Panda VM's standard class descriptor format (analogous to JVM's
`Ljava/lang/Object;`). Without it, ArkRuntime cannot resolve the external class reference.

### Root Cause

`ExternalClass::class_name()` in `crates/arkdown-abc/src/emit/` returns the raw module path
without the Panda descriptor wrapping.

### Fix

Wrap external class names in the Panda descriptor format:

```rust
impl ExternalClass {
    pub fn panda_descriptor(&self) -> String {
        format!("L{};", self.class_name())
    }
}
```

Apply `panda_descriptor()` wherever external class names are serialized into the StringTable
or ClassRecord entries.

The mapping from `@ohos:` import specifier to Panda descriptor:

| Import specifier | Panda descriptor |
|---|---|
| `@ohos.app.ability.UIAbility` | `L@ohos.app.ability.UIAbility;` |
| `@ohos.base` | `L@ohos.base;` |
| `@kit.AbilityKit` | `L@kit.AbilityKit;` (after @kit expansion) |

**Upstream ref**: `assembly_emitter.cpp` — external class descriptor format matches Panda
`Lpackage/ClassName;` convention throughout the assembler.

---

## Bug 4: Per-Instruction LNP ADVANCE Density (TASK-ABC-04)

### Problem

hvigor/es2abc emits one LNP (Line Number Program) row per bytecode instruction, producing
approximately 105 bytes per LNP entry. `arkdown-abc` emits one LNP row per source statement,
producing approximately 58 bytes per LNP entry — 1.8× too sparse. This accounts for the
majority of the remaining 1169B gap.

### Root Cause

`LineTracker::advance()` is called once per statement in `CodeGen::emit_stmt()`, not once per
emitted bytecode instruction. The LNP SPECIAL_OPCODE encodes both a line delta and a PC
(program counter) delta. With per-statement calls, the PC delta spans an entire statement's
worth of instructions, which may exceed the SPECIAL_OPCODE range and fall back to explicit
`ADVANCE_LINE` + `ADVANCE_PC` pairs.

### Fix

Move `LineTracker::advance()` to fire after every `CodeGen::push()` call (i.e., after every
individual bytecode instruction is emitted):

```rust
impl CodeGen {
    pub fn push(&mut self, insn: Instruction) {
        let pc = self.bytecode.len();
        self.bytecode.push(insn);
        // Emit one LNP row per instruction
        self.line_tracker.advance(pc, self.current_line);
    }
}
```

Also verify the SPECIAL_OPCODE encoding formula:

```
opcode = (line_delta * 16 + pc_delta) + 8
```

Where `line_delta` is in range `[0, 15]` and `pc_delta` is in range `[0, 15]`. When either
delta exceeds the range, fall back to explicit opcodes:

```rust
fn emit_lnp_row(&mut self, line_delta: i32, pc_delta: u32) {
    if line_delta >= 0 && line_delta <= 15 && pc_delta <= 15 {
        let special = (line_delta as u8 * 16 + pc_delta as u8) + 8;
        self.lnp.push(LnpOpcode::Special(special));
    } else {
        if line_delta != 0 {
            self.lnp.push(LnpOpcode::AdvanceLine(line_delta));
        }
        if pc_delta != 0 {
            self.lnp.push(LnpOpcode::AdvancePc(pc_delta));
        }
        self.lnp.push(LnpOpcode::Special(8)); // no-op special
    }
}
```

**Upstream ref**: `assembly_emitter.cpp::EmitLineNumberProgram()` — one LNP entry per
bytecode instruction; SPECIAL_OPCODE formula from DWARF-derived LNP spec.

---

## Public API Changes

No new public API. All changes are internal to `crates/arkdown-abc/src/`.

The following internal types are affected:

```rust
// crates/arkdown-abc/src/emit/string_pool.rs
pub struct StringPool {
    map: HashMap<String, u32>,
    items: Vec<String>,
}
impl StringPool {
    pub fn intern(&mut self, s: &str) -> u32;  // must be idempotent
}

// crates/arkdown-abc/src/emit/external_class.rs (or equivalent)
impl ExternalClass {
    pub fn panda_descriptor(&self) -> String;  // returns "L<name>;"
}

// crates/arkdown-abc/src/compiler.rs
impl CodeGen {
    fn push(&mut self, insn: Instruction);  // calls line_tracker.advance() after push
}

// crates/arkdown-abc/src/emit/lnp.rs (or equivalent)
impl LineTracker {
    pub fn advance(&mut self, pc: usize, line: u32);
    fn emit_lnp_row(&mut self, line_delta: i32, pc_delta: u32);
}
```

---

## Rolldown Alignment

These are ABC compiler internals. No rolldown equivalent.

| arkdown concept | es2abc equivalent | note |
|---|---|---|
| `MethodRecord::name` (bare) | `method.GetName()` in `SerializeMethod()` | Method name is bare; class carries OHM path |
| `StringPool::intern()` idempotent | `SerializeStringTable()` single-pass interning | Same string → same offset |
| `ExternalClass::panda_descriptor()` | Panda `Lpackage/ClassName;` format | Standard Panda VM class descriptor |
| `LineTracker::advance()` per instruction | `EmitLineNumberProgram()` per instruction | One LNP row per bytecode op |

---

## Test Requirements

### Bug 1: func_main_0 OHM Prefix

- Compile a minimal module (e.g., `pages/Index.ets` with `@Entry @Component struct Index {}`).
- Inspect the generated `.abc` MethodRecord name for `func_main_0`.
- Assert: method NameItem equals `"func_main_0"` (bare), not the OHM-prefixed form.
- Assert: ClassRecord name contains the full OHM URL (e.g., `@bundle:com.example/entry/src/main/ets/pages/Index`).
- Regression: existing test `test_method_names` (if present) must still pass.

### Bug 2: Annotation String Deduplication

- Compile two modules in the same `.abc` (merge-abc mode).
- Parse the StringTable from the output bytes.
- Assert: `"pkgName"`, `"isCommonjs"`, `"hasTopLevelAwait"`, `"isSharedModule"`, `"scopeNames"`, `"moduleRecordIdx"` each appear exactly once in the StringTable.
- Assert: all annotation references across all module records point to the same StringItem offsets.

### Bug 3: External Module Ref Format

- Compile a module that imports `@ohos.app.ability.UIAbility`.
- Inspect the generated `.abc` external class entries.
- Assert: the external class name is `"L@ohos.app.ability.UIAbility;"` (with `L` prefix and `;` suffix).
- Assert: no entry of the form `"@native.ohos.app.ability.UIAbility"` exists.

### Bug 4: LNP Density

- Compile `pages/Index.ets` (minimal fixture).
- Parse the LNP section from the output bytes.
- Assert: LNP byte count per method is within 20% of hvigor's output (target: ~105B/LNP vs current ~58B/LNP).
- Assert: every bytecode instruction in the method body has a corresponding LNP row.
- Assert: SPECIAL_OPCODE values are in range `[8, 255]` (i.e., `line_delta * 16 + pc_delta + 8`).
- Assert: when line_delta > 15 or pc_delta > 15, explicit `ADVANCE_LINE`/`ADVANCE_PC` opcodes are emitted.

### Golden Parity

- After all four fixes, run the minimal-hap parity test.
- Assert: `ets/modules.abc` byte size is within 5% of hvigor golden (target: 9844B ± 500B).
- Assert: all 51 existing abc unit tests continue to pass.
