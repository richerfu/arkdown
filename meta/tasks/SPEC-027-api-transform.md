# SPEC-027: API Transform Pass (apiTransform)

**Crate**: `arkdown-plugin-ets-transform`
**Priority**: P2 — @kit.* imports and OHOS API call rewrites
**Blocked by**: SPEC-004a (ets-transform plugin)
**Blocks**: —

---

## Summary

Implement the `apiTransform` pass that rewrites OHOS-specific API call patterns at the
AST level. The primary work is expanding `@kit.*` kit imports into individual
`@ohos:*` module imports and rewriting the call sites accordingly. This mirrors the
`t.apiTransform()` function loaded from the SDK's `compile_plugin.js` in hvigor.

---

## Background: What hvigor Does

From `load-sdk-functions.js`, hvigor loads three SDK transform functions:

```javascript
sdkPlugins = [
    { name: "etsChecker",     transform: t.etsChecker },
    { name: "etsTransform",   transform: t.etsTransform },
    { name: "apiTransform",   transform: t.apiTransform },  // ← this SPEC
    { name: "createProgramPlugin", buildStart: t.createProgramPlugin },
    { name: "genAbc",         buildEnd: t.genAbc },
];
```

`apiTransform` runs in the rollup `transform` hook, after `etsTransform`. Its primary
responsibilities (from reading the SDK `ets-loader` source):

1. **Kit import expansion**: `import { ability } from '@kit.AbilityKit'` →
   `import ability from '@ohos.app.ability.UIAbility'` (individual `@ohos:*` imports).
   The kit-to-module mapping is defined in the SDK's `@ohos/hvigor-arkts-compose` kit
   map files.

2. **`@ohos:*` → ArkRuntime module name normalization**: Converts the `@ohos:*` import
   specifiers to the canonical ArkRuntime module names used in the ABC `moduleRecordIdx`
   annotation.

3. **Permission decorator lowering**: `@Permission('ohos.permission.X')` decorators on
   class members are rewritten into metadata that the ArkRuntime can read.

4. **Event emitter pattern rewrites**: Certain OHOS event subscription patterns are
   rewritten to use the canonical ArkRuntime subscription API.

From `arkts-pack.js::getBuiltInPlugins()`, `apiTransform` is only loaded for non-HAR-
dedup builds (it is excluded from `getHarDeduplicateBuiltInPlugins()`).

---

## Scope

### 1. Kit Map Loading

The kit-to-module mapping is stored in the SDK at:
```
<OHOS_SDK_HOME>/ets/build-tools/ets-loader/kit_configs/
```

Each kit config file maps a kit name (e.g., `@kit.AbilityKit`) to an array of
`{ symbol, from }` entries where `from` is the `@ohos:*` module path.

For arkdown, embed a snapshot of this mapping as a static Rust data structure
(updated when the target SDK version changes):

```rust
/// A single symbol exported from a kit.
pub struct KitSymbol {
    pub symbol: &'static str,
    pub from: &'static str,   // e.g. "@ohos.app.ability.UIAbility"
}

/// Kit name → symbols map, loaded from embedded kit_configs snapshot.
pub fn kit_map() -> &'static HashMap<&'static str, Vec<KitSymbol>>;
```

Alternatively, load the kit config JSON files at runtime from a configurable SDK path
when `OHOS_SDK_HOME` is set.

### 2. Kit Import Expansion

Input:
```typescript
import { UIAbility, Want } from '@kit.AbilityKit';
```

Output (after expansion):
```typescript
import UIAbility from '@ohos.app.ability.UIAbility';
import Want from '@ohos.app.ability.Want';
```

The transform operates on `ImportDeclaration` nodes in the OXC AST:

```rust
pub fn expand_kit_import(
    import_decl: &ImportDeclaration,
    kit_map: &HashMap<&str, Vec<KitSymbol>>,
) -> Option<Vec<ImportDeclaration>>;
```

When the import specifier matches a kit name (starts with `@kit.`), replace the
single import with N individual imports, one per imported symbol.

### 3. `@ohos:*` Normalization

After kit expansion, `@ohos.app.ability.UIAbility` specifiers must be normalized to
the ArkRuntime module name format used in `ldexternalmodulevar` and the
`moduleRecordIdx` annotation. The normalization rule:

```
"@ohos.app.ability.UIAbility" → "@ohos:app.ability.UIAbility"
```

(Replace the first `.` after `@ohos` with `:` to match the ArkRuntime module namespace
separator.)

### 4. Plugin Integration

Add `ApiTransformPlugin` to `arkdown-plugin-ets-transform`:

```rust
pub struct ApiTransformPlugin {
    kit_map: Arc<HashMap<String, Vec<KitSymbol>>>,
    sdk_path: Option<PathBuf>,
}

impl ArkdownPlugin for ApiTransformPlugin {
    fn name(&self) -> &'static str { "api-transform" }

    async fn transform(
        &self,
        ctx: &TransformContext,
        code: String,
        id: &str,
    ) -> PluginResult<Option<TransformOutput>>;
}
```

The `transform` hook:
1. Parse the source with the pinned `ohos-rs/oxc` parser.
2. Walk `ImportDeclaration` nodes; expand kit imports.
3. Walk call expressions for permission decorator lowering (if applicable).
4. Re-emit via `oxc_codegen::Codegen`.

### 5. Plugin Order

`ApiTransformPlugin` must run after `EtsTransformPlugin` (which handles decorator
desugaring) and before the resolver. In the CLI plugin list:

```rust
plugins: vec![
    // ...
    Box::new(EtsTransformPlugin::new(config.clone())),
    Box::new(ApiTransformPlugin::new(kit_map.clone())),
    // ...
]
```

---

## Files to Change

```
crates/arkdown-plugin-ets-transform/src/
├── api_transform.rs    — NEW: ApiTransformPlugin, expand_kit_import(),
│                         kit_map(), KitSymbol
├── kit_configs/        — NEW: embedded kit config JSON snapshots (API 12)
│   └── AbilityKit.json
│   └── ... (other kits)
└── lib.rs              — re-export ApiTransformPlugin
```

---

## Test Plan

- `expand_kit_import()` on `import { UIAbility } from '@kit.AbilityKit'` produces one
  `import UIAbility from '@ohos.app.ability.UIAbility'`.
- `expand_kit_import()` on a multi-symbol kit import produces N individual imports.
- `expand_kit_import()` returns `None` for non-kit imports (no transformation).
- Unknown kit name: `@kit.UnknownKit` passes through unchanged (no panic).
- `@ohos.app.ability.UIAbility` is normalized to `@ohos:app.ability.UIAbility` in the
  resolved module specifier.
- `ApiTransformPlugin::transform()` on a source file with `@kit.AbilityKit` import
  produces output with individual `@ohos.*` imports and no `@kit.*` specifiers.
- Plugin is inactive (returns `None`) for non-ETS/TS files (e.g., `.json`).
- Kit map loads correctly from embedded JSON snapshots for API version 12.

---

## Upstream References

- `~/Downloads/command-line-tools/sdk/default/openharmony/ets/build-tools/ets-loader/`
  — `compile_plugin.js` `apiTransform` function; `kit_configs/` directory
- `~/Downloads/command-line-tools/hvigor/hvigor-ohos-plugin/node_modules/@ohos/hvigor-arkts-compose/dist/src/util/load-sdk-functions.js`
  — `loadSdkFunctions()` loading `t.apiTransform()`
- `meta/design/module-resolution.md` — `@ohos:*` module name format
- `meta/tasks/SPEC-004a-ets-transform.md` — `EtsTransformPlugin` (runs before this)
