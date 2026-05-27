# SPEC-007: JSON5 Import Plugin

**Crate**: `arkdown-plugin-json5`
**Priority**: P0
**Blocked by**: SPEC-005
**Blocks**: —

---

## Summary

Allows importing `.json` and `.json5` files as ES modules. Direct Rust replacement for
`@rollup/plugin-json@6.0.0`. Also handles OHOS-specific `.json5` config files that projects
commonly import.

---

## Behavior

Given a source file:

```typescript
import config from "./config.json5";
import strings from "./resources/base/element/string.json";
```

The plugin intercepts `load()` for these files and returns a synthetic ES module:

```javascript
// For config.json5 containing { name: "foo", version: 2 }
export default { name: "foo", version: 2 };
```

For named exports (when JSON root is an object):

```javascript
export const name = "foo";
export const version = 2;
export default { name: "foo", version: 2 };
```

---

## Implementation

```rust
pub struct Json5Plugin;

impl ArkdownPlugin for Json5Plugin {
    fn name(&self) -> &str { "json5" }

    async fn resolve_id(&self, _ctx: &ResolveContext, specifier: &str, importer: Option<&str>)
        -> PluginResult<Option<ResolvedId>>
    {
        // Only handle .json and .json5 files
        if !specifier.ends_with(".json") && !specifier.ends_with(".json5") {
            return Ok(None);
        }
        // Resolve relative to importer using std path join
        let path = resolve_relative(specifier, importer)?;
        Ok(Some(ResolvedId { id: path.to_string_lossy().into(), external: false, module_type: ModuleType::Json5 }))
    }

    async fn load(&self, _ctx: &LoadContext, id: &str) -> PluginResult<Option<ModuleSource>> {
        if !id.ends_with(".json") && !id.ends_with(".json5") {
            return Ok(None);
        }
        let raw = fs_err::read_to_string(id)?;
        let value: serde_json::Value = if id.ends_with(".json5") {
            json5::from_str(&raw)?
        } else {
            serde_json::from_str(&raw)?
        };
        let code = generate_esm_from_json(&value);
        Ok(Some(ModuleSource { id: ..., content: code, source_map: None }))
    }
}

fn generate_esm_from_json(value: &serde_json::Value) -> String {
    let default_export = serde_json::to_string(value).unwrap();
    let mut code = String::new();
    if let serde_json::Value::Object(map) = value {
        for key in map.keys() {
            if is_valid_identifier(key) {
                code.push_str(&format!("export const {} = {}[{:?}];\n", key, "obj", key));
            }
        }
    }
    code.push_str(&format!("export default {};\n", default_export));
    code
}
```

---

## Acceptance Criteria

- `import x from "./foo.json5"` resolves and loads correctly
- Named exports emitted for top-level object keys that are valid JS identifiers
- Invalid JSON5 produces a descriptive `ArkdownError::ParseError`
- Both `.json` and `.json5` extensions handled

---

## Design References

- `meta/design/plugins.md` — `@rollup/plugin-json` entry

---

## Rolldown Alignment

| arkdown concept                  | rolldown equivalent                      | note                                                                                                                  |
| -------------------------------- | ---------------------------------------- | --------------------------------------------------------------------------------------------------------------------- |
| `Json5Plugin::resolve_id`        | `@rollup/plugin-json` `resolveId` hook   | Identical: intercepts `.json`/`.json5` extensions before the default resolver                                         |
| `Json5Plugin::load`              | `@rollup/plugin-json` `load` hook        | Identical semantics: reads file, returns synthetic ESM code                                                           |
| `generate_esm_from_json()`       | rolldown built-in JSON transform         | rolldown has native JSON module support; arkdown implements it explicitly to also handle `.json5` (the `json5` crate) |
| Named exports for top-level keys | `@rollup/plugin-json namedExports: true` | Same behavior: valid-identifier top-level keys become named exports                                                   |
| `export default { ... }`         | `@rollup/plugin-json` default export     | Identical                                                                                                             |

**Key difference from rolldown:** rolldown natively understands `.json` as `ModuleType::Json`. arkdown represents JSON5 as `ModuleType::Json5` and handles it entirely in this plugin; there is no built-in JSON module loader in arkdown.

---

## Test Requirements

- `import config from "./config.json5"` where `config.json5` is `{ name: "foo", version: 2 }` → module code contains `export const name = "foo"`, `export const version = 2`, `export default { name: "foo", version: 2 }`
- `import x from "./data.json"` (plain JSON, not JSON5) → same named exports behavior
- JSON5 with comments (`// ...` and `/* ... */`) parses correctly and generates valid ESM
- JSON5 with trailing commas parses correctly
- Top-level array (not object): only `export default [...]` is emitted; no named exports
- Top-level string (`"hello"`): `export default "hello"`; no named exports
- Key that is not a valid JS identifier (e.g., `"my-key"`, `"123abc"`) is not emitted as a named export; only `export default` is emitted
- Invalid JSON5 content returns `Err(ArkdownError::ParseError)` with the file path in the error message
- `resolve_id` returns `Ok(None)` for non-JSON/JSON5 extensions (`.ts`, `.ets`, `.js`)
- `load` returns `Ok(None)` for non-JSON/JSON5 IDs
