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
