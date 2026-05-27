# TEST-004: arkdown-loader

**Crate**: `arkdown-loader`

---

## Test File Layout

```
crates/arkdown-loader/src/
├── tests/
│   ├── plugin_chain.rs       ← hook dispatch order (first Some() wins)
│   ├── parallel_load.rs      ← independent modules loaded concurrently
│   ├── ets_parsing.rs        ← .ets ArkTS parsing via ohos-rs/oxc
│   └── import_extraction.rs  ← import/export edges extracted from AST
```

---

## plugin_chain.rs

### TC-001: `resolve_id` — first plugin returning Some wins; later plugins not called

- **Setup**: two plugins: P1 returns `Some(ResolvedId{...})`, P2 returns `Some(other)`
- **Expected**: P2's `resolve_id` not called; result is P1's

### TC-002: `resolve_id` — None-returning plugins deferred in order

- **Setup**: P1 returns None, P2 returns Some
- **Expected**: result is P2's; P1 was called first

### TC-003: `transform` — all plugins called sequentially, each receives previous output

- **Setup**: P1 appends `"// P1"` to source, P2 appends `"// P2"`
- **Expected**: final source ends with `"// P1\n// P2"`

### TC-004: `module_parsed` — all plugins notified, no short-circuit

- **Setup**: two plugins both implement `module_parsed`
- **Expected**: both called regardless of return value

### TC-005: `load` fallback to filesystem when all plugins return None

- **Setup**: no plugin handles `.ts` files; file exists on disk
- **Expected**: file content read from disk

---

## parallel_load.rs

### TC-006: two independent modules loaded concurrently

- **Setup**: two entry files with no shared imports
- **Expected**: both modules in graph; no ordering dependency

### TC-007: shared dependency loaded once despite being imported by two modules

- **Setup**: A imports C; B imports C; C is a single file
- **Expected**: `ModuleGraph` contains C once (deduplicated by id)

---

## ets_parsing.rs

### TC-008: `.ets` file with `@Entry @Component struct` parsed without error

- **Input**: minimal ArkTS component
  ```typescript
  @Entry
  @Component
  struct Index {
    build() { Column() {} }
  }
  ```
- **Expected**: `Module` created with no parse errors; `ModuleType::Ets`

### TC-009: `.ts` file parsed with `ModuleType::Ts`

- **Input**: TypeScript source
- **Expected**: `module.id.module_type == ModuleType::Ts`

---

## import_extraction.rs

### TC-010: static import extracted as `ImportEdge`

- **Input**: `import { foo } from './utils';`
- **Expected**: `module.imports[0].specifier == "./utils"`,
  `imported_names` contains `"foo"`

### TC-011: dynamic import extracted as dynamic edge

- **Input**: `const x = await import('./lazy');`
- **Expected**: `module.dynamic_imports[0].specifier == "./lazy"`

### TC-012: re-export extracted as both import and export edge

- **Input**: `export { foo } from './utils';`
- **Expected**: edge in both `imports` (from utils) and `exports` (foo)

### TC-013: side-effect import with no bindings extracted

- **Input**: `import './polyfill';`
- **Expected**: `ImportEdge` with empty `imported_names` vector
