# TEST-007: arkdown JSON5 Rolldown adapter

**Crate**: `arkdown`
**Owner**: `crates/arkdown/src/plugins/json5/`
**Original source**: `@rollup/plugin-json@6.0.0` (JS) — behavior derived from plugin spec

---

## Test File Layout

```
crates/arkdown/src/plugins/json5/
├── mod.rs
├── plugin.rs                 ← Rolldown load hook
└── module_emit.rs            ← deterministic ESM emission
```

---

## load.rs

### TC-001: flat JSON object → default + named exports

- **Input file** `config.json5`:
  ```json5
  { name: "foo", version: 2, enabled: true }
  ```
- **Expected output**:
  ```javascript
  export const name = "foo";
  export const version = 2;
  export const enabled = true;
  export default { name: "foo", version: 2, enabled: true };
  ```

### TC-002: `.json` extension also handled

- **Input file** `data.json`: `{ "key": "value" }`
- **Expected**: same ESM output as TC-001

### TC-003: non-JSON file returns `None` (plugin defers)

- **Input**: `resolve_id("./component.ets", ...)`
- **Expected**: `Ok(None)` — plugin does not claim this file

### TC-004: JSON array at root → default export only, no named exports

- **Input file** `list.json5`: `[1, 2, 3]`
- **Expected**:
  ```javascript
  export default [1, 2, 3];
  ```
  (no named exports — array has no property names)

### TC-005: JSON primitive at root → default export only

- **Input**: `42`
- **Expected**: `export default 42;`

### TC-006: deeply nested JSON → default export only (no named exports for nested keys)

- **Input**: `{ "a": { "b": 1 } }`
- **Expected**: only top-level key `a` gets a named export; nested `b` does not:
  ```javascript
  export const a = { b: 1 };
  export default { a: { b: 1 } };
  ```

---

## named_exports.rs

### TC-007: key with non-identifier characters → no named export

- **Input**: `{ "my-key": 1, "123abc": 2, "valid": 3 }`
- **Expected**: named export only for `valid`:
  ```javascript
  export const valid = 3;
  export default { "my-key": 1, "123abc": 2, valid: 3 };
  ```

### TC-008: reserved word key → no named export

- **Input**: `{ "default": "x", "import": "y", "ok": "z" }`
- **Expected**: named export only for `ok`

---

## errors.rs

### TC-009: invalid JSON5 → structured error with filename

- **Input file** `bad.json5`: `{ unclosed: `
- **Expected**: `Err(ArkdownError::ParseError { file: "bad.json5", message: "..." })`

### TC-010: invalid standard JSON → structured error

- **Input file** `bad.json`: `{bad json}`
- **Expected**: `Err(ArkdownError::ParseError { ... })`
