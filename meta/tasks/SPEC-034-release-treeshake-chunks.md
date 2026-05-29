# SPEC-034: Release Mode Tree-shake and ManualChunks

**Crate**: `arkdown`, `arkdown-cli`
**Priority**: P1 (required for correct release HAP size)
**Blocked by**: SPEC-005
**Upstream refs**:
- `arkts-pack.js::generateConfig()` — `treeshake: n === RELEASE`
- `manualChunks(id, {getModuleInfo})` function in hvigor rollup config
- `arkts-pack.js::getOutputConfig()` — `chunkFileNames`, `sanitizeFileName`
- hvigor gap analysis: TASK-RELEASE-01, gaps F25-F26

---

## Background

hvigor configures rollup with two release-mode behaviors that arkdown does not currently
implement:

1. **Tree-shaking** (`treeshake: true` in release): removes unused exports and dead code.
   In debug mode, `treeshake: false` (disabled). arkdown always uses `treeshake: false`.

2. **Manual chunk splitting** (`manualChunks` when `supportChunks: true`): splits the
   bundle into multiple chunks — `commons.js` (node_modules), `vendors.js` (non-entry
   non-node_modules), and per-entry chunks. This is required for `supportChunks` projects
   where the HAP must load multiple ABC files.

Without these, release HAP sizes are larger than necessary and `supportChunks` projects
produce incorrect output.

---

## Part 1: Tree-shaking in Release Mode

### hvigor Behavior

```javascript
// arkts-pack.js::generateConfig()
const config = {
    treeshake: buildMode === "release",
    // ...
};
```

In release mode, rollup tree-shakes the entire module graph. Unused exports from all
modules (including entry modules) are removed.

### arkdown Implementation

```rust
// crates/arkdown/src/lib.rs

pub struct BundlerConfig {
    // ...existing fields...
    pub build_mode: BuildMode,
}

impl Bundler {
    fn build_rolldown_input_options(&self) -> rolldown::InputOptions {
        rolldown::InputOptions {
            treeshake: match self.config.build_mode {
                BuildMode::Release => rolldown::TreeshakeOptions::True,
                BuildMode::Debug => rolldown::TreeshakeOptions::False,
            },
            // ...
        }
    }
}
```

### Side Effects Annotation

Tree-shaking requires side-effect annotations to work correctly. Modules that have
side effects (e.g., `@Entry` components that register themselves) must be marked as
having side effects.

```rust
// In the module meta plugin (SPEC-014), annotate side-effect modules:
impl ArkdownPlugin for ModuleMetaPlugin {
    async fn module_parsed(&self, ctx: &mut ModuleContext, module: &Module) -> PluginResult<()> {
        // Mark @Entry component files as having side effects
        let has_side_effects = module.code.contains("@Entry") || module.code.contains("registerNamedRoute");
        ctx.set_module_side_effects(has_side_effects);
        Ok(())
    }
}
```

---

## Part 2: Manual Chunks for supportChunks

### hvigor Behavior

When `buildOption.arkOptions.supportChunks: true`:

```javascript
// arkts-pack.js::generateConfig()
const manualChunks = (id, { getModuleInfo }) => {
    if (id.includes('oh_modules/')) {
        return 'commons';
    }
    const info = getModuleInfo(id);
    if (!info.isEntry) {
        return 'vendors';
    }
    return null; // entry modules get their own chunk
};

const config = {
    manualChunks: supportChunks ? manualChunks : undefined,
    // ...
};
```

This produces:
- `commons.js`: all modules from `oh_modules/` (third-party dependencies)
- `vendors.js`: non-entry, non-node_modules modules (shared internal code)
- Per-entry chunks: one per entry point

### arkdown Implementation

```rust
// crates/arkdown/src/manual_chunks.rs

pub enum ChunkAssignment {
    /// Assign to named chunk
    Named(String),
    /// Let rolldown decide (entry modules)
    Default,
}

pub struct OhosManualChunks {
    pub support_chunks: bool,
}

impl OhosManualChunks {
    pub fn assign(&self, module_id: &str, is_entry: bool) -> ChunkAssignment {
        if !self.support_chunks {
            return ChunkAssignment::Default;
        }
        if module_id.contains("oh_modules/") || module_id.contains("oh_modules\\") {
            ChunkAssignment::Named("commons".to_string())
        } else if !is_entry {
            ChunkAssignment::Named("vendors".to_string())
        } else {
            ChunkAssignment::Default
        }
    }
}
```

Wire into rolldown output options:

```rust
// crates/arkdown/src/lib.rs

impl Bundler {
    fn build_rolldown_output_options(&self) -> rolldown::OutputOptions {
        let manual_chunks = if self.config.support_chunks {
            Some(Arc::new(OhosManualChunks { support_chunks: true }))
        } else {
            None
        };

        rolldown::OutputOptions {
            // hvigor: chunkFileNames: "[name].js", sanitizeFileName: false
            chunk_file_names: "[name].js".to_string(),
            sanitize_file_name: false,
            manual_chunks,
            // ...
        }
    }
}
```

---

## Part 3: Output Config Alignment

hvigor uses these rollup output options that arkdown must match:

| Option | hvigor value | arkdown current | Fix |
|---|---|---|---|
| `format` | `"cjs"` (JSBUNDLE) / not set (ESMODULE) | `"esm"` | ESMODULE: use `"esm"`; JSBUNDLE: use `"cjs"` |
| `chunkFileNames` | `"[name].js"` | default | Set to `"[name].js"` |
| `sanitizeFileName` | `false` | default (true) | Set to `false` |
| `shimMissingExports` | `true` | not set | Set to `true` |
| `context` | `"globalThis"` | not set | Set to `"globalThis"` |

### shimMissingExports

When `shimMissingExports: true`, rolldown inserts `undefined` for missing named exports
instead of throwing an error. This is required because OHOS SDK modules are external and
may not export all expected symbols in the bundler's view.

### context: "globalThis"

Sets the top-level `this` binding in the output. hvigor uses `"globalThis"` for both
ESMODULE and CJS output.

---

## Public API Changes

```rust
// crates/arkdown/src/lib.rs

pub struct BundlerConfig {
    pub build_mode: BuildMode,
    pub support_chunks: bool,        // new field
    pub compile_mode: CompileMode,   // ESMODULE | JSBUNDLE
}

// crates/arkdown-cli/src/main.rs
// Read support_chunks from build-profile.json5 buildOption.arkOptions.supportChunks
let support_chunks = config.build_option.ark_options.support_chunks.unwrap_or(false);
```

---

## Rolldown Alignment

| arkdown concept | rolldown equivalent | note |
|---|---|---|
| `TreeshakeOptions::True` in release | rolldown `treeshake: true` | Direct equivalent |
| `OhosManualChunks::assign()` | rolldown `manualChunks` callback | Same mechanism |
| `chunk_file_names: "[name].js"` | rolldown `chunkFileNames` | Same option |
| `sanitize_file_name: false` | rolldown `sanitizeFileName: false` | Same option |
| `shim_missing_exports: true` | rolldown `shimMissingExports` | Same option |

---

## Test Requirements

### Tree-shaking in Release Mode

- Build a module with an unused export: `export function unused() {}`
- Build in `debug` mode: assert `unused` function appears in the output chunk
- Build in `release` mode: assert `unused` function is NOT in the output chunk

### Tree-shaking Preserves @Entry Side Effects

- Build in `release` mode with a `@Entry @Component struct Index` file
- Assert: the `Index` component class is present in the output (not tree-shaken)
- Assert: `registerNamedRoute` call is present in the output

### Manual Chunks Disabled by Default

- Build without `supportChunks: true`
- Assert: single output chunk (no `commons.js` or `vendors.js`)

### Manual Chunks: oh_modules → commons

- Build with `supportChunks: true` and a dependency in `oh_modules/`
- Assert: the dependency code is in `commons.js`, not in the entry chunk

### Manual Chunks: Non-Entry Internal → vendors

- Build with `supportChunks: true` and a shared internal module (not in `oh_modules/`, not an entry)
- Assert: the shared module code is in `vendors.js`

### Manual Chunks: Entry Modules Get Own Chunk

- Build with `supportChunks: true` and two entry points
- Assert: each entry point has its own named chunk (not in `commons` or `vendors`)

### chunkFileNames Format

- Build any project
- Assert: all output chunk filenames match `[name].js` pattern (no hash suffix)

### sanitizeFileName Disabled

- Build a project with a module path containing characters that would normally be sanitized
- Assert: the chunk filename is NOT sanitized (characters preserved)

### shimMissingExports

- Build a module that imports a named export from an external `@ohos:*` module
- Assert: build succeeds even if the external module doesn't declare that export
- Assert: the missing export is shimmed as `undefined` in the output
