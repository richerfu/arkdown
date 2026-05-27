# SPEC-001: Workspace & Core Types

**Crate**: `arkdown-core`
**Priority**: P0 — blocks all other crates
**Blocked by**: nothing
**Blocks**: SPEC-002, 003, 004, 005 (all crates)

---

## Summary

Initialize the Cargo workspace and implement the `arkdown-core` crate containing all shared type
definitions. Every other crate depends on this. No business logic here — only type definitions,
trait declarations, and utility re-exports.

---

## Workspace Layout

Create `Cargo.toml` at the repo root:

```toml
[workspace]
resolver = "2"
members = [
    "crates/arkdown-core",
    "crates/arkdown-config",
    "crates/arkdown-resolver",
    "crates/arkdown-loader",
    "crates/arkdown",
    "crates/arkdown-plugin-chunk-wrapper",
    "crates/arkdown-plugin-json5",
    "crates/arkdown-plugin-duplicate-check",
    "crates/arkdown-plugin-module-meta",
    "crates/arkdown-plugin-ohos-resolve",
    "crates/arkdown-plugin-har-dedup",
    "crates/arkdown-plugin-router-map",
    "crates/arkdown-plugin-sensitive-check",
    "crates/arkdown-abc",
    "crates/arkdown-resource",
    "crates/arkdown-profile",
    "crates/arkdown-pack",
    "crates/arkdown-cache",
    "crates/arkdown-sign",
    "crates/arkdown-cli",
]

[workspace.dependencies]
# Parser — ohos-rs/oxc fork, pinned to the rev used by ohos-rs/oxc-ark.
# DO NOT use floating branch refs; always pin the rev.
# ohos-rs/oxc-ark (the ArkTS lint/format CLI) is NOT a dep — only the parser fork is used.
oxc = { git = "https://github.com/ohos-rs/oxc", rev = "5caa6d54ae977b8a76ed68b8aba06b38e1c6a614", features = ["full"] }
oxc_allocator = { git = "https://github.com/ohos-rs/oxc", rev = "5caa6d54ae977b8a76ed68b8aba06b38e1c6a614" }
oxc_parser = { git = "https://github.com/ohos-rs/oxc", rev = "5caa6d54ae977b8a76ed68b8aba06b38e1c6a614" }
oxc_semantic = { git = "https://github.com/ohos-rs/oxc", rev = "5caa6d54ae977b8a76ed68b8aba06b38e1c6a614" }
oxc_transformer = { git = "https://github.com/ohos-rs/oxc", rev = "5caa6d54ae977b8a76ed68b8aba06b38e1c6a614" }
oxc_codegen = { git = "https://github.com/ohos-rs/oxc", rev = "5caa6d54ae977b8a76ed68b8aba06b38e1c6a614" }
oxc_minifier = { git = "https://github.com/ohos-rs/oxc", rev = "5caa6d54ae977b8a76ed68b8aba06b38e1c6a614" }

# Async
tokio = { version = "1", features = ["full"] }
async-trait = "0.1"

# Data structures
rustc-hash = "2"
index_vec = "0.1"
bitflags = "2"

# Serialization
serde = { version = "1", features = ["derive"] }
serde_json = "1"
json5 = "0.4"

# Error handling
thiserror = "2"
anyhow = "1"

# Source maps
sourcemap = "9"

# File system
fs-err = "3"
walkdir = "2"

# Parallel
rayon = "1"
```

---

## Types to Implement

Implement all types from `meta/design/types.md` verbatim. Key groups:

### Module Identity (`src/module.rs`)

```rust
pub struct ResolvedId { pub id: String, pub external: bool, pub module_type: ModuleType }
pub struct ModuleSource { pub id: ResolvedId, pub content: String, pub source_map: Option<SourceMap> }
pub enum ModuleType { Ets, Ts, Js, Json5, NativeResource, BytecodeHar, Declaration }
```

### Module Graph (`src/graph.rs`)

```rust
pub struct Module { pub id: ResolvedId, pub imports: Vec<ImportEdge>, pub exports: Vec<ExportBinding>,
                    pub meta: ModuleMeta, pub is_entry: bool, pub has_side_effects: bool, ... }
pub struct ModuleMeta { pub pkg_name: Option<String>, pub module_name: Option<String>, ... }
pub struct ModuleGraph { pub modules: IndexVec<ModuleIdx, Module>, pub entries: Vec<ModuleIdx>, ... }
pub struct ModuleIdx(u32);
```

### Chunk Graph (`src/chunk.rs`)

```rust
pub struct ArkChunk { pub name: String, pub chunk_hash: String, pub modules: Vec<ModuleIdx>,
                      pub is_entry: bool, pub code: String, pub source_map: Option<SourceMap>, ... }
pub struct ChunkGraph { pub chunks: IndexVec<ChunkIdx, ArkChunk>, ... }
pub struct IifeChunk { pub chunk: ArkChunk, pub require_replacements: Vec<RequireReplacement>,
                       pub registry_export: Option<String> }
```

### Build Config (`src/config.rs`)

```rust
pub struct ArkdownConfig { pub project_root: PathBuf, pub build_mode: BuildMode,
                           pub output_target: OutputTarget, pub product: ProductConfig,
                           pub modules: Vec<ModuleConfig>, pub sdk_config: SdkConfig }
pub enum BuildMode { Debug, Release }
pub enum OutputTarget { Hap, Hsp, Har, Preview, Test }
pub enum OhosModuleType { Entry, Feature, Shared, Har }
pub enum CompileMode { EsModule, JsBundle }
```

### Plugin Trait (`src/plugin.rs`)

```rust
#[async_trait]
pub trait ArkdownPlugin: Send + Sync {
    fn name(&self) -> &str;
    async fn build_start(&self, ctx: &mut BuildContext) -> PluginResult<()> { Ok(()) }
    async fn resolve_id(&self, ctx: &ResolveContext, specifier: &str, importer: Option<&str>)
        -> PluginResult<Option<ResolvedId>> { Ok(None) }
    async fn load(&self, ctx: &LoadContext, id: &str) -> PluginResult<Option<ModuleSource>> { Ok(None) }
    async fn transform(&self, ctx: &TransformContext, source: ModuleSource)
        -> PluginResult<Option<ModuleSource>> { Ok(None) }
    async fn module_parsed(&self, ctx: &ModuleParsedContext, module: &Module) -> PluginResult<()> { Ok(()) }
    async fn before_build_end(&self, ctx: &mut BuildContext) -> PluginResult<()> { Ok(()) }
    async fn build_end(&self, ctx: &BuildContext, error: Option<&BuildError>) -> PluginResult<()> { Ok(()) }
    async fn render_chunk(&self, ctx: &RenderChunkContext, code: String, chunk: &ArkChunk)
        -> PluginResult<Option<RenderedChunk>> { Ok(None) }
    async fn generate_bundle(&self, ctx: &mut GenerateBundleContext, bundle: &mut ChunkGraph,
        is_write: bool) -> PluginResult<()> { Ok(()) }
    fn should_invalidate_cache(&self, ctx: &CacheContext, cached: &CachedModule) -> bool { false }
}
```

### Error Types (`src/error.rs`)

```rust
#[derive(Debug, thiserror::Error)]
pub enum ArkdownError {
    #[error("Resolution failed for '{specifier}' from '{importer:?}': {reason}")]
    ResolutionFailed { specifier: String, importer: Option<String>, reason: String },
    #[error("Parse error in '{file}': {message}")]
    ParseError { file: String, message: String },
    #[error("Plugin '{plugin}' failed: {source}")]
    PluginError { plugin: String, #[source] source: Box<dyn std::error::Error + Send + Sync> },
    #[error("ABC compilation failed: {message}")]
    AbcCompilationFailed { message: String },
    #[error("IO error: {0}")]
    Io(#[from] std::io::Error),
}
pub type PluginResult<T> = Result<T, ArkdownError>;
```

### OhosResolvedModule (`src/resolution.rs`)

```rust
pub enum OhosResolvedModule {
    LocalSource { path: PathBuf, module_type: ModuleType, pkg_context: Option<PkgContext> },
    RemotePackage { path: PathBuf, pkg_name: String, pkg_version: String, ohm_url: Option<String>, ... },
    SdkDeclaration { path: PathBuf, specifier: String },
    BytecodeHar { abc_path: PathBuf, declaration_path: PathBuf, pkg_name: String, ohm_url: String },
    ExternalBundle { ohm_url: String },
    ExternalHsp { bundle_name: String, module_name: String, entry_path: String },
}
```

---

## Files to Create

```
crates/arkdown-core/
├── Cargo.toml
└── src/
    ├── lib.rs        (pub mod declarations + re-exports)
    ├── module.rs     (ResolvedId, ModuleSource, ModuleType)
    ├── graph.rs      (Module, ModuleMeta, ModuleGraph, ImportEdge, ExportBinding)
    ├── chunk.rs      (ArkChunk, ChunkGraph, IifeChunk, RequireReplacement)
    ├── config.rs     (ArkdownConfig, BuildMode, OutputTarget, SdkConfig, etc.)
    ├── plugin.rs     (ArkdownPlugin trait, plugin context types)
    ├── resolution.rs (OhosResolvedModule, PkgContext)
    ├── manifest.rs   (HapManifest, ModuleJson5, OhPackageJson5, AbcArtifact, OhosResource)
    └── error.rs      (ArkdownError, PluginResult)
```

---

## Acceptance Criteria

- `cargo build -p arkdown-core` succeeds with zero warnings
- All types from `meta/design/types.md` are represented
- `ArkdownPlugin` trait compiles with the full hook signature
- `ArkdownError` covers all failure modes needed by downstream crates
- No business logic — only type definitions and the plugin trait

---

## Design References

- `meta/design/types.md` — canonical type definitions (implement verbatim)
- `meta/design/pipeline.md` — to understand data flow between types
- `meta/design/chunk-model.md` — IifeChunk and ChunkGraph details

---

## Rolldown Alignment

| arkdown concept                        | rolldown equivalent                                               | note                                                                                                                                                     |
| -------------------------------------- | ----------------------------------------------------------------- | -------------------------------------------------------------------------------------------------------------------------------------------------------- |
| `ArkdownPlugin` trait                  | `Plugin` trait                                                    | arkdown uses `async_trait`; rolldown uses `register_hook_usage` bitflags — arkdown omits the bitflag registration (no parallel hook dispatch needed yet) |
| `resolve_id` hook                      | `resolve_id(&self, ctx, args: &HookResolveIdArgs)`                | Same semantics: first `Some` wins; arkdown passes `specifier + importer` directly instead of a wrapper args struct                                       |
| `load` hook                            | `load(&self, ctx, args: &HookLoadArgs)`                           | Same semantics; fallback to fs::read                                                                                                                     |
| `transform` hook                       | `transform(&self, ctx, args: &HookTransformArgs)`                 | Same; sequential, each plugin may modify source                                                                                                          |
| `render_chunk` hook                    | `render_chunk(&self, ctx, args)`                                  | Same; used by chunk-wrapper to IIFE-wrap                                                                                                                 |
| `generate_bundle` hook                 | `generate_bundle(&self, ctx, bundle, is_write)`                   | Same; post-render, pre-write                                                                                                                             |
| `should_invalidate_cache`              | `shouldInvalidCache` (rollup hook)                                | arkdown keeps this as a sync fn returning bool                                                                                                           |
| `Module` struct                        | `NormalModule` inside `Module::Normal(Box<NormalModule>)`         | arkdown has no `ExternalModule` type; external references are `ResolvedId { external: true }`                                                            |
| `ModuleGraph`                          | rolldown `ModuleTable` + `IndexVec<NormalModuleId, NormalModule>` | arkdown uses `IndexVec<ModuleIdx, Module>` — same pattern, different index name                                                                          |
| `ArkChunk`                             | rolldown `Chunk`                                                  | arkdown adds `chunk_hash: String` used for `globalThis` registry keys; rolldown calls this `content_hash`                                                |
| `IifeChunk`                            | rolldown has no equivalent                                        | OHOS-specific: carries `require_replacements` and `registry_export` produced by wrapper-chunk-plugin                                                     |
| `ChunkGraph`                           | rolldown `ChunkTable`                                             | same `IndexVec` pattern                                                                                                                                  |
| `ModuleType`                           | `ModuleType` enum in rolldown (`Js`, `Ts`, `Json`, etc.)          | arkdown adds `Ets`, `BytecodeHar`, `Declaration`, `NativeResource`                                                                                       |
| `HookResolveIdOutput { external, id }` | `ResolvedId { external, id, module_type }`                        | arkdown merges side-effects into module_type rather than a separate `HookSideEffects` field                                                              |
| `RenderedChunk`                        | `HookRenderChunkOutput { code, map }`                             | identical semantics                                                                                                                                      |
| `BuildMode::Debug/Release`             | rolldown `OutputFormat` + watch flags                             | arkdown combines build mode + output format in one enum                                                                                                  |
| `OutputTarget::Hap/Hsp/Har`            | no rolldown equivalent                                            | OHOS-specific artifact types                                                                                                                             |

**Key divergences from rolldown that must always be preserved:**

- No `ModuleDefFormat::Esm` output — always CJS internally, then IIFE-wrapped by chunk-wrapper-plugin
- `ArkChunk::chunk_hash` used in `globalThis['name_hash']` registry keys, not in file content-hashing
- `ModuleType::Ets` triggers mandatory ArkTS desugar before any other transform

---

## Test Requirements

- `cargo build -p arkdown-core` succeeds with zero warnings and zero `clippy -D warnings` findings
- All enum variants of `ModuleType`, `BuildMode`, `OutputTarget`, `OhosModuleType`, `CompileMode` derive `Debug + Clone + PartialEq + Eq`
- `ArkdownPlugin` trait compiles: a zero-size `struct NoopPlugin;` implementing only `fn name()` compiles and all other hooks use their default impls
- `ArkdownError::ResolutionFailed { specifier, importer, reason }` formats correctly via `Display`
- `ArkdownError::Io` converts from `std::io::Error` via the `From` impl
- `ModuleGraph::new()` produces an empty graph with zero modules and zero entries
- `ChunkGraph` indexing: `chunk_graph.chunks[ChunkIdx(0)]` returns the first chunk
- `ModuleIdx` and `ChunkIdx` are distinct types — assigning one to the other must be a compile error (enforced by newtype pattern)
- `PluginResult<()>` is `Result<(), ArkdownError>`; verify the alias resolves correctly in a type assertion test
