# TEST-005: arkdown (Bundler Core)

**Crate**: `arkdown`

---

## Test File Layout

```
crates/arkdown/src/
├── tests/
│   ├── link.rs               ← cross-module symbol resolution
│   ├── tree_shake.rs         ← dead code exclusion (release only)
│   ├── chunk_split.rs        ← entry-based chunk assignment
│   ├── cjs_output.rs         ← output format is CJS, not ESM
│   └── loader_json.rs        ← loader.json reading + entry extraction
```

---

## link.rs

### TC-001: import resolves to matching export

- **Setup**: module A `export function foo() {}`, module B `import { foo } from './a'`
- **Expected**: `ImportEdge.resolved` for B's import points to A's `foo` export binding

### TC-002: circular dependency detected and reported as warning

- **Setup**: A imports B, B imports A
- **Expected**: warning emitted (not error); build continues

### TC-003: re-export chain resolved

- **Setup**: A exports `x`, B re-exports from A (`export { x } from './a'`), C imports from B
- **Expected**: C's import resolves transitively to A's `x`

---

## tree_shake.rs

### TC-004: unused export excluded in release build

- **Setup**: module exports `foo` (used) and `bar` (unused), `build_mode = Release`
- **Expected**: output chunk contains `foo` but not `bar`

### TC-005: tree-shake disabled in debug build

- **Setup**: same module, `build_mode = Debug`
- **Expected**: output chunk contains both `foo` and `bar`

### TC-006: module with side effects always included

- **Setup**: module has side effect (e.g., `console.log(...)` at module scope)
- **Expected**: module included in debug and release builds even if nothing imported

### TC-007: `sideEffects: false` in `oh-package.json5` enables shake of ohpm package

- **Setup**: ohpm package with `"sideEffects": false`, no exports imported
- **Expected**: package excluded from release build output

---

## chunk_split.rs

### TC-008: one entry = one chunk when `support_chunks = false`

- **Setup**: one entry file `EntryAbility.ets`, `support_chunks = false`
- **Expected**: one output chunk named after entry

### TC-009: ohpm deps → `"commons"` chunk when `support_chunks = true`

- **Setup**: module from `oh_modules/`, `support_chunks = true`
- **Expected**: assigned to chunk named `"commons"`

### TC-010: non-entry local module → `"vendors"` chunk

- **Setup**: local module, not an entry file, `support_chunks = true`
- **Expected**: assigned to chunk named `"vendors"`

### TC-011: entry module → entry-named chunk

- **Setup**: entry file `EntryAbility.ets`, `support_chunks = true`
- **Expected**: chunk name derived from entry filename (`"EntryAbility"`)

### TC-012: multiple entry files produce multiple entry chunks

- **Setup**: `EntryAbility.ets` and `Index.ets` both in `compileEntry`
- **Expected**: two separate entry chunks

---

## cjs_output.rs

### TC-013: cross-module import generates `require()` call in CJS output

- **Setup**: module B imports from module A
- **Expected**: generated code for B contains `var _a = require('./a');`

### TC-014: no `import` statement in output (not ESM)

- **Expected**: output JavaScript contains zero `import ` keywords

### TC-015: `export` statement converted to `exports.foo = foo;` pattern

- **Expected**: output contains `exports.foo = foo;` not `export { foo }`

---

## loader_json.rs

### TC-016: `compileEntry` array used as entry points

- **Input**: `loader.json` with `"compileEntry": ["./ets/entryability/EntryAbility.ets"]`
- **Expected**: bundler uses `EntryAbility.ets` as the sole entry module

### TC-017: missing `loader.json` returns error

- **Expected**: `Err(ArkdownError::Io { ... })` with path in message

### TC-018: `supportChunks` flag read from loader.json

- **Input**: `"supportChunks": true` in `loader.json`
- **Expected**: `Bundler::bundle()` applies manual chunks splitting
