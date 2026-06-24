# SPEC-008 Declarative Compile Plugin Support

## Goal

Align arkdown's plugin capability with hvigor by supporting the two distinct
plugin surfaces that hvigor exposes, using the same architectural principles
as Rolldown.

---

## hvigor Plugin Surfaces

hvigor exposes two separate plugin surfaces with different semantics:

### Surface A — `hvigorfile.ts` module-level plugins

```typescript
// hvigorfile.ts (project-level or module-level)
import { moduleTasks, OhosHapContext } from "@ohos/hvigor-ohos-plugin";

const myPlugin = {
  pluginId: OhosPluginId.OHOS_HAP_PLUGIN,
  apply(context: OhosHapContext): void {
    context.targets((target) => {
      target.getCompileArkTsOption().compilePluginPath = "./my-rollup-plugin.js";
      target.getBuildProfile().buildOption.arkOptions.obfuscation.enable = true;
    });
  },
};

export default { system: moduleTasks, plugins: [myPlugin] };
```

**Purpose**: Mutate build configuration before the compile stage begins.
**APIs used**: `OhosHapContext`, `getBuildProfile()`, `getCompileArkTsOption()`.
**Not** Rollup/Rolldown hooks — these are lifecycle hooks that run before bundling.

**arkdown equivalent**: `BuildScriptEffect` entries extracted by OXC static
analysis in `arkdown-session::build_script::parse_build_script()`.

---

### Surface B — `compilePluginPath` Rollup-compatible plugins

```typescript
// ./my-rollup-plugin.js (referenced via compilePluginPath)
export default {
  name: "my-transform",
  order: "last",  // "first" | "last"
  resolveId(id) { if (id === "virtual") return "\0virtual"; return null; },
  load(id) { if (id === "\0virtual") return "export default 42;"; return null; },
  transform(code, id) { return code.replace(/OLD/g, "NEW"); },
};
```

**Purpose**: Hook into the Rollup/Rolldown module pipeline.
**APIs**: same hooks as Rolldown (`resolveId`, `load`, `transform`, `moduleParsed`,
`buildStart`, `buildEnd`, `renderChunk`, `generateBundle`).
**These ARE Rolldown plugins** — they implement the identical hook interface.

**arkdown equivalent**: `rolldown_plugin::Plugin` trait implementations.

---

## Architecture: Following Rolldown

Rolldown's plugin model:

```
User-supplied plugin
        │
        ├── Rust native  →  impl rolldown_plugin::Plugin directly
        │
        └── JavaScript   →  NAPI binding wraps JS function → impl Plugin
                             (rolldown_binding crate; requires Node.js)
```

arkdown's plugin model (same interface, different bridge):

```
compilePluginPath plugin file
        │
        ├── OXC static analysis  →  DeclarativePlugin (impl Plugin)    ← P1
        │   Supported patterns:
        │   · resolveId: if (id === "a") return "b"
        │   · load: if (id === "\0x") return "content"
        │   · transform: code.replace(/PAT/g, "REPL")
        │
        ├── Embedded QuickJS     →  ExternalJsPlugin (impl Plugin)     ← P2
        │   (via rquickjs crate — no Node.js required)
        │   Executes arbitrary hook code in a sandbox JS runtime.
        │
        └── Unsupported          →  typed error listing why             ← P0
            "Plugin 'name' uses patterns that require JS execution.
             Rewrite as a Rust arkdown plugin or simplify to declarative hooks."
```

**Key principle**: The plugin interface is always `rolldown_plugin::Plugin`.
The question is only HOW the implementation is produced — native Rust, static
analysis, or an embedded JS runtime.

---

## P1: Declarative Subset (Implemented)

### What's supported

`crates/arkdown/src/plugins/external.rs` implements:

```rust
pub enum PluginHookDecl {
    ResolveIdAlias { from: String, to: String },
    VirtualModule { id: String, content: String },
    TransformSubstitution { pattern: String, replacement: String, global: bool },
}
```

`analyze_compile_plugin(path)` parses a JS plugin file with OXC and extracts
`PluginHookDecl` values. `DeclarativePlugin` implements `rolldown_plugin::Plugin`
using these extracted values.

### Supported patterns

**`resolveId`:**
```javascript
resolveId(id) {
    if (id === "from-specifier") return "to-specifier";
    return null;
}
```
→ `ResolveIdAlias { from: "from-specifier", to: "to-specifier" }`

**`load`:**
```javascript
load(id) {
    if (id === "\0virtual-module") return "export const x = 42;";
    return null;
}
```
→ `VirtualModule { id: "\0virtual-module", content: "export const x = 42;" }`

**`transform`:**
```javascript
transform(code, id) {
    return code.replace(/PLACEHOLDER/g, "REPLACEMENT");
}
```
→ `TransformSubstitution { pattern: "PLACEHOLDER", replacement: "REPLACEMENT", global: true }`

### Unsupported (typed rejection)

Any hook that uses:
- External `require()` or `import()` calls
- `fs.*` or network access
- Conditional logic based on runtime values
- Multiple if-branches or complex logic

Results in `CompilePluginAnalysis::Unsupported { name, reasons }` with a
diagnostic listing each unsupported pattern.

---

## P2: Embedded QuickJS (Future)

For plugins that exceed the declarative subset, embed a QuickJS runtime
(via `rquickjs` crate) to execute the hook functions:

```rust
pub struct ExternalJsPlugin {
    runtime: Arc<rquickjs::AsyncRuntime>,
    // loaded module with hooks
}

impl Plugin for ExternalJsPlugin {
    async fn transform(&self, ctx, args) -> HookTransformReturn {
        // call the JS transform function via QuickJS
        self.call_hook("transform", args.code, args.id).await
    }
}
```

Benefits:
- Full compatibility with arbitrary hook code
- No Node.js required (QuickJS is embeddable)
- Sandboxed — no filesystem access unless explicitly exposed

Design notes:
- The `rquickjs` crate is MIT-licensed and actively maintained
- Async-compatible via `rquickjs::AsyncRuntime`
- A separate `arkdown-plugin-js` crate would own this

---

## P0: `hvigorfile.ts` Static Analysis (Implemented)

`arkdown-session::build_script::parse_build_script()` now performs real OXC
AST analysis instead of being a stub:

1. Parses the script with `oxc_parser::Parser`
2. Detects standard pattern (known system task + empty `plugins: []`) → no effects
3. Extracts non-empty `plugins: [...]` → `BuildScriptEffect::CustomTask`
4. Extracts `compilePluginPath`/`compilePluginFile` assignments →
   `BuildScriptEffect::CompilePluginFile` or `BuildOptionOverride`

The coordinator's `check_custom_hvigor_plugins()` function uses text-based
detection as a fast guard. The full OXC analysis provides richer effect
extraction for future `BuildCoordinator` integration.

---

## PluginFactory Integration

`PluginFactory::build()` can include external declarative plugins:

```rust
// Load all compilePluginPath entries from the build plan
for plugin_path in &plan.compile_plugin_paths {
    if let Some((plugin, order)) = load_declarative_plugin(plugin_path) {
        match order {
            PluginOrder::First => plugins.insert(0, plugin as SharedPluginable),
            PluginOrder::Last => plugins.push(plugin as SharedPluginable),
        }
    }
}
```

The `load_declarative_plugin()` function returns `None` with a warning when
the plugin is unsupported. The build continues without the plugin in that case.

---

## File Layout

```
crates/arkdown/src/plugins/
  external.rs              ← this spec; OXC analysis + DeclarativePlugin

crates/arkdown-session/src/build_script/
  parse.rs                 ← OXC analysis of hvigorfile.ts → BuildScriptEffect
  effects.rs               ← effect types: CompilePluginFile, BuildOptionOverride, etc.
```

---

## P2: Node.js CLI Package — Full Rolldown Alignment

The correct long-term architecture for complete plugin capability is to
distribute arkdown as a Node.js native addon (`.node` file), exactly as
Rolldown does.

### Why this fully aligns with Rolldown

Rolldown embeds Rust **inside** Node.js — JS plugins work because they run in
the same V8 process:

```text
Node.js process
├── import { rolldown } from 'rolldown'  ← Rust loaded as .node via NAPI-RS
├── plugins: [{ transform(code, id) { ... } }]
└── NAPI ThreadsafeFunction → JS called directly, no IPC, no serialization
```

`@arkdown/core` does the same:

```text
Node.js process
├── import { build } from '@arkdown/core'  ← arkdown-napi.node
├── plugins: [{ transform(code, id) { ... } }]
└── JsPlugin: impl rolldown_plugin::Plugin
    → tsfn.call_async((code, id)).await  (same V8 heap)
```

### Setup

Create `packages/arkdown-core/` using the official napi-rs pnpm template:

```bash
cd packages && napi new
# name: @arkdown/core, napi4, pnpm, type-def: yes
```

Then add to `src/lib.rs`:

```rust
// JsPlugin: wraps a JS plugin object, implements rolldown_plugin::Plugin
struct JsPlugin {
    name: String,
    transform_fn: Option<ThreadsafeFunction<(String, String), JsUnknown>>,
    resolve_id_fn: Option<ThreadsafeFunction<(String, Option<String>), JsUnknown>>,
    load_fn:       Option<ThreadsafeFunction<String, JsUnknown>>,
}
impl Plugin for JsPlugin { /* calls JS via tsfn.call_async().await */ }

// build() NAPI export — accepts config + JS plugins array
#[napi]
pub async fn build(config: JsBuildConfig, plugins: Option<Array>) -> NapiResult<JsBuildResult>
```

### Plugin API (identical to Rolldown / hvigor compilePluginPath)

```typescript
import { build } from '@arkdown/core';

await build({
  project: './my-app',
  target: 'hap',
  mode: 'debug',
  plugins: [{
    name: 'my-plugin',
    transform(code, id) { return code.replace(/OLD/g, 'NEW'); },
    resolveId(id)      { if (id === 'v') return '\0v'; },
    load(id)           { if (id === '\0v') return 'export default 42'; },
  }],
});
```

### Comparison

| Approach | Process | IPC | Node.js required | Compat |
|---|---|---|---|---|
| NAPI package (this spec) | 1 | none (V8) | embedded in pkg | 100% |
| Subprocess (current) | 2 | JSON-RPC | must be installed | 100% |
| Declarative OXC (P1) | 1 | none | no | ~30% |

---

## Non-Goals

- Executing hvigor-specific `OhosHapContext` APIs in TypeScript
- Supporting Vite-specific Rolldown plugins
- Replicating hvigor's full task graph runtime
