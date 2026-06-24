# Canonical Type Definitions

Shared type definitions for all arkdown crates. Written as Rust-flavored pseudocode (not yet compilable, but close to the final design). These types form the interface contract between crates.

---

## 1. Module Identity and Source

```rust
/// A fully resolved module identifier.
/// In OHOS projects with useNormalizedOHMUrl=true, this is an OHM URL.
/// Otherwise, it is a canonical absolute filesystem path.
pub struct ResolvedId {
    /// The canonical identifier string.
    /// - OHM URL: "@bundle:com.example/entry/ets/entryability/EntryAbility"
    /// - File path: "/abs/path/to/src/main/ets/utils/helper.ets"
    pub id: String,
    /// Whether this is a bare external reference (never bundled).
    /// True for @bundle: and @shared: references.
    pub external: bool,
    /// Module type inferred from extension or resolver metadata.
    pub module_type: ModuleType,
}

/// The raw source text of a module, after loading but before parsing.
pub struct ModuleSource {
    pub id: ResolvedId,
    /// UTF-8 source content.
    pub content: String,
    /// Source map from a prior transform stage (e.g. ArkTS desugar).
    pub source_map: Option<SourceMap>,
}

/// The primary module type discriminator.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ModuleType {
    /// ArkTS source file (.ets) — requires ArkTS desugar transform.
    Ets,
    /// TypeScript source file (.ts).
    Ts,
    /// JavaScript source file (.js, .mjs, .cjs).
    Js,
    /// JSON or JSON5 file.
    Json5,
    /// Native resource file (non-code; excluded from JS bundle).
    NativeResource,
    /// Pre-compiled ByteCode HAR (.abc). No source bundling.
    BytecodeHar,
    /// TypeScript declaration file (.d.ts, .d.ets). Type-only; excluded from output.
    Declaration,
}
```

---

## 2. Module and Module Graph

```rust
/// A fully parsed and transformed module in the build graph.
pub struct Module {
    pub id: ResolvedId,
    pub source: ModuleSource,
    /// Parsed abstract syntax tree (oxc AST reference).
    pub ast: OxcAstRef,
    /// Static import edges (resolved).
    pub imports: Vec<ImportEdge>,
    /// Dynamic import edges (resolved).
    pub dynamic_imports: Vec<ImportEdge>,
    /// Exported bindings.
    pub exports: Vec<ExportBinding>,
    /// OHOS-specific metadata collected by moduleMetaInfoPlugin equivalent.
    pub meta: ModuleMeta,
    /// Whether this module is an entry point.
    pub is_entry: bool,
    /// Whether this module has side effects (used for tree-shaking).
    pub has_side_effects: bool,
}

/// OHOS-specific per-module metadata.
pub struct ModuleMeta {
    /// ohpm package name (e.g., "my-har", "@ohos/lib").
    pub pkg_name: Option<String>,
    /// ohpm package version.
    pub pkg_version: Option<String>,
    /// OHOS module name from module.json5 (e.g., "entry", "hsp-lib").
    pub module_name: Option<String>,
    /// Absolute path to the module's oh-package.json5 directory.
    pub pkg_path: Option<PathBuf>,
    /// Whether this is a local (non-ohpm) dependency.
    pub is_local_dependency: bool,
    /// Whether this file is the main entry file of its package.
    pub is_node_entry_file: bool,
    /// Modules that import this module (for hot-reload tracking).
    pub host_modules_info: Vec<HostModuleInfo>,
    /// Whether this module is tagged as needing preloaded SO.
    pub need_preload_so: bool,
    /// Path to the project root containing this module.
    pub belong_project_path: Option<PathBuf>,
    /// Path to the OHOS module root containing this module.
    pub belong_module_path: Option<PathBuf>,
}

/// A reverse-dependency entry (who imports this module).
pub struct HostModuleInfo {
    pub host_dependency_name: String,  // package name
    pub host_module_name: String,      // OHOS module name
}

/// A resolved import edge between modules.
pub struct ImportEdge {
    /// The raw specifier as written in source ("./utils/helper", "@ohos/hilog").
    pub specifier: String,
    /// Resolved target module ID.
    pub resolved: ResolvedId,
    /// Imported binding names (empty for namespace/side-effect imports).
    pub imported_names: Vec<ImportedName>,
}

/// An exported binding from a module.
pub struct ExportBinding {
    pub exported_name: String,
    pub local_name: String,
}

/// The full module graph for one OHOS module build.
pub struct ModuleGraph {
    /// All modules, indexed by ModuleIdx.
    pub modules: IndexVec<ModuleIdx, Module>,
    /// Map from ResolvedId → ModuleIdx.
    pub id_to_idx: FxHashMap<String, ModuleIdx>,
    /// Entry modules (from loader.json compileEntry).
    pub entries: Vec<ModuleIdx>,
}

/// Opaque index into ModuleGraph::modules.
pub struct ModuleIdx(u32);
```

---

## 3. Chunk Graph

```rust
/// An output chunk produced by the code-splitting stage.
pub struct ArkChunk {
    /// Chunk file name (e.g., "EntryAbility", "commons", "vendors").
    pub name: String,
    /// Content hash / fingerprint from rolldown metadata.
    pub chunk_hash: String,
    /// Modules included in this chunk.
    pub modules: Vec<ModuleIdx>,
    /// Whether this chunk is an entry chunk.
    pub is_entry: bool,
    /// Entry module index (for entry chunks).
    pub entry_module: Option<ModuleIdx>,
    /// Final JavaScript source emitted by rolldown render hooks.
    pub code: String,
    /// Source map for this chunk.
    pub source_map: Option<SourceMap>,
}

/// The full chunk assignment for one OHOS module build.
pub struct ChunkGraph {
    pub chunks: IndexVec<ChunkIdx, ArkChunk>,
    /// Maps ModuleIdx → ChunkIdx.
    pub module_to_chunk: IndexVec<ModuleIdx, Option<ChunkIdx>>,
    /// Maps entry ModuleIdx → entry ChunkIdx.
    pub entry_module_to_entry_chunk: FxHashMap<ModuleIdx, ChunkIdx>,
}

pub struct ChunkIdx(u32);

```

---

## 4. ABC Artifacts

```rust
/// The compiled ArkCompiler bytecode artifact for one OHOS module.
pub struct AbcArtifact {
    /// Absolute path to the output .abc file.
    pub path: PathBuf,
    /// API version this was compiled for (from compatibleSdkVersion).
    pub api_version: u32,
    /// Whether debug info is included.
    pub has_debug_info: bool,
    /// Source maps chain (chunk.js.map files used as input to es2abc).
    pub source_maps: Vec<PathBuf>,
    /// Path to es2abc incremental cache file.
    pub cache_path: Option<PathBuf>,
}

/// ByteCode HAR dependency information (from loader.json byteCodeHarInfo).
pub struct BytecodeHarInfo {
    pub package_name: String,
    pub abc_path: PathBuf,
    pub compatible_sdk_version: String,
    pub compatible_sdk_version_stage: Option<String>,
    pub source_maps_path: Option<PathBuf>,
    pub insight_intent_path: Option<PathBuf>,
}
```

---

## 5. Resources and Manifests

```rust
/// Compiled OHOS resources (output of restool).
pub struct OhosResource {
    /// Path to compiled resources.index file.
    pub resources_index: PathBuf,
    /// Path to ResourceTable.txt (used by HSP consumers).
    pub resource_table: Option<PathBuf>,
    /// Profile JSON files written to resources/base/profile/.
    pub profiles: Vec<ResourceProfile>,
}

pub struct ResourceProfile {
    pub name: String,   // e.g., "routerMap", "startup_config"
    pub path: PathBuf,
}

/// Processed module.json (output of ProcessProfile + MergeProfile).
pub struct HapManifest {
    pub module_name: String,
    pub module_type: OhosModuleType,    // entry | feature | shared | har
    pub device_types: Vec<String>,
    pub virtual_machine: String,        // "ark3.5.0.0"
    pub compile_mode: CompileMode,
    pub abilities: Vec<AbilityEntry>,
    pub extension_abilities: Vec<ExtensionAbilityEntry>,
    pub dependencies: Vec<HspDependency>,
    /// $profile: router map reference (if router map is generated).
    pub router_map: Option<String>,
    pub pages: Option<String>,          // "$profile:main_pages"
    pub main_element: Option<String>,
}

/// Raw parsed module.json5 (before processing).
pub struct ModuleJson5 {
    pub module: ModuleJson5Module,
}

pub struct ModuleJson5Module {
    pub name: String,
    pub type_: String,              // "entry" | "feature" | "shared" | "har"
    pub main_element: Option<String>,
    pub device_types: Vec<String>,
    pub pages: Option<String>,
    pub abilities: Vec<AbilityConfig>,
    pub extension_abilities: Vec<ExtensionAbilityConfig>,
    pub src_entry: Option<String>,
    pub metadata: Vec<MetadataEntry>,
}

pub struct AbilityConfig {
    pub name: String,
    pub src_entry: String,          // e.g., "./ets/entryability/EntryAbility.ets"
    pub exported: bool,
    pub skills: Vec<SkillConfig>,
}

/// Raw parsed oh-package.json5.
pub struct OhPackageJson5 {
    pub model_version: String,
    pub name: Option<String>,
    pub version: Option<String>,
    pub description: Option<String>,
    pub main: Option<String>,
    pub types: Option<String>,
    pub dependencies: HashMap<String, String>,
    pub dev_dependencies: HashMap<String, String>,
    pub dynamic_dependencies: HashMap<String, String>,
    pub overrides: HashMap<String, String>,
    /// Internal metadata (used by SDK, ByteCode HAR metadata).
    pub metadata: Option<PackageMetadata>,
}

pub struct PackageMetadata {
    pub custom_types: Vec<String>,
    pub declaration_entry: Vec<String>,
    pub runtime_only: Option<RuntimeOnlyConfig>,
    pub workers: Vec<String>,
    pub dependency_pkg_version: HashMap<String, String>,
}
```

---

## 6. Plugin Trait

```rust
/// The arkdown plugin trait — mirrors rolldown/rollup plugin hooks.
/// Plugins implement whichever hooks they need; all hooks have default no-op implementations.
#[async_trait]
pub trait ArkdownPlugin: Send + Sync {
    fn name(&self) -> &str;

    /// Called once at build start.
    async fn build_start(&self, ctx: &mut BuildContext) -> PluginResult<()> { Ok(()) }

    /// Resolve a module specifier to a ResolvedId.
    /// Return None to defer to the next plugin.
    async fn resolve_id(
        &self,
        ctx: &ResolveContext,
        specifier: &str,
        importer: Option<&str>,
    ) -> PluginResult<Option<ResolvedId>> { Ok(None) }

    /// Load the source content for a resolved module ID.
    /// Return None to defer to the default file loader.
    async fn load(
        &self,
        ctx: &LoadContext,
        id: &str,
    ) -> PluginResult<Option<ModuleSource>> { Ok(None) }

    /// Transform the source of a module.
    /// Return None to leave source unchanged.
    async fn transform(
        &self,
        ctx: &TransformContext,
        source: ModuleSource,
    ) -> PluginResult<Option<ModuleSource>> { Ok(None) }

    /// Called after a module is parsed.
    async fn module_parsed(
        &self,
        ctx: &ModuleParsedContext,
        module: &Module,
    ) -> PluginResult<()> { Ok(()) }

    /// Called before the build ends (all modules parsed).
    async fn before_build_end(&self, ctx: &mut BuildContext) -> PluginResult<()> { Ok(()) }

    /// Called when the build ends (all chunks generated).
    async fn build_end(
        &self,
        ctx: &BuildContext,
        error: Option<&BuildError>,
    ) -> PluginResult<()> { Ok(()) }

    /// Transform a rendered chunk's code.
    async fn render_chunk(
        &self,
        ctx: &RenderChunkContext,
        code: String,
        chunk: &ArkChunk,
    ) -> PluginResult<Option<RenderedChunk>> { Ok(None) }

    /// Called after all chunks are rendered, before file write.
    async fn generate_bundle(
        &self,
        ctx: &mut GenerateBundleContext,
        bundle: &mut ChunkGraph,
        is_write: bool,
    ) -> PluginResult<()> { Ok(()) }

    /// Decide whether an incremental cache entry is still valid.
    fn should_invalidate_cache(
        &self,
        ctx: &CacheContext,
        cached: &CachedModule,
    ) -> bool { false }
}

pub struct RenderedChunk {
    pub code: String,
    pub map: Option<SourceMap>,
}

pub type PluginResult<T> = Result<T, PluginError>;
```

---

## 7. Output Targets and Build Modes

```rust
/// The target artifact type.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum OutputTarget {
    /// Harmony Ability Package — deployable app module.
    Hap,
    /// Harmony Shared Package — shared library deployed once on-device.
    Hsp,
    /// Harmony Archive — library package (source or bytecode).
    Har,
    /// Preview mode — in-IDE live preview without signing.
    Preview,
    /// Unit test build.
    Test,
}

/// Debug or Release build.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum BuildMode {
    /// No optimization, debug info included, no obfuscation.
    Debug,
    /// Tree-shaking, optimization, obfuscation (if configured).
    Release,
}

/// OHOS module type (from module.json5 module.type field).
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum OhosModuleType {
    Entry,      // "entry" — main HAP module
    Feature,    // "feature" — feature HAP module
    Shared,     // "shared" — HSP module
    Har,        // "har" — HAR library module
}

/// Compilation mode.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum CompileMode {
    /// ES module mode (for ArkTS stage model, API >= 9).
    EsModule,
    /// JS bundle mode (legacy FA model).
    JsBundle,
}
```

---

## 8. OHOS Resolved Module

The complete resolution result for any OHOS specifier:

```rust
/// The complete resolution result for an OHOS import specifier.
pub enum OhosResolvedModule {
    /// A local source file (relative import or local HAR source).
    LocalSource {
        path: PathBuf,
        module_type: ModuleType,
        pkg_context: Option<PkgContext>,
    },
    /// A remote ohpm package entry file.
    RemotePackage {
        path: PathBuf,
        module_type: ModuleType,
        pkg_name: String,
        pkg_version: String,
        ohm_url: Option<String>,
    },
    /// An SDK API declaration (@ohos/*, @kit.*).
    SdkDeclaration {
        path: PathBuf,
        specifier: String,   // original "@ohos/hilog" or "@kit.ArkUI"
    },
    /// A ByteCode HAR — no source, reference abc directly.
    BytecodeHar {
        abc_path: PathBuf,
        declaration_path: PathBuf,
        pkg_name: String,
        ohm_url: String,
    },
    /// An external cross-bundle reference (@bundle:, @shared:).
    /// Not bundled; resolved at runtime by ArkRuntime.
    ExternalBundle {
        ohm_url: String,     // "@bundle:com.example/module/path"
    },
    /// An HSP reference — runtime-resolved.
    ExternalHsp {
        bundle_name: String,
        module_name: String,
        entry_path: String,
    },
}

/// Package context information used for OHM URL generation.
pub struct PkgContext {
    pub package_name: String,
    pub bundle_name: String,
    pub module_name: String,
    pub version: String,
    pub pkg_path: PathBuf,
    pub entry_path: Option<PathBuf>,
}
```

---

## 9. Build Configuration

```rust
/// Top-level arkdown configuration (parsed from build-profile.json5 and related files).
pub struct ArkdownConfig {
    pub project_root: PathBuf,
    pub build_mode: BuildMode,
    pub output_target: OutputTarget,
    pub product: ProductConfig,
    pub modules: Vec<ModuleConfig>,
    pub sdk_config: SdkConfig,
}

pub struct ProductConfig {
    pub name: String,
    pub bundle_name: String,
    pub compatible_sdk_version: String,
    pub runtime_os: String,
    pub signing_config: Option<SigningConfig>,
    pub build_option: BuildOption,
}

pub struct BuildOption {
    pub strict_mode: StrictModeConfig,
    pub ark_options: Option<ArkOptions>,
    pub res_options: Option<ResOptions>,
    pub source_option: Option<SourceOption>,
}

pub struct StrictModeConfig {
    pub case_sensitive_check: bool,
    pub use_normalized_ohm_url: bool,
}

pub struct ArkOptions {
    pub obfuscation: Option<ObfuscationConfig>,
    pub types: Vec<String>,
    pub runtime_only: Option<RuntimeOnlyConfig>,
}

pub struct SdkConfig {
    /// Path to ets-loader directory, used for SDK metadata and parity inputs.
    pub ets_loader_path: PathBuf,
    /// Path to es2abc binary for the SDK ABC backend.
    pub es2abc_path: PathBuf,
    /// Path to restool binary for the SDK resource backend.
    pub restool_path: PathBuf,
    /// SDK API version (numeric).
    pub compile_api_version: u32,
    /// Whether the SDK has the rollup plugin (required for stage mode).
    pub has_rollup_plugin: bool,
}

pub struct SigningConfig {
    pub cert_path: PathBuf,
    pub key_alias: String,
    pub key_password: String,      // encrypted
    pub profile_path: PathBuf,
    pub sign_alg: String,
    pub store_file: PathBuf,
    pub store_password: String,    // encrypted
}
```

---

## Cross-Reference

- [pipeline.md](pipeline.md) — how these types flow through the pipeline stages
- [chunk-model.md](chunk-model.md) — legacy chunk-wrapper background and ChunkGraph details
- [abc-pipeline.md](abc-pipeline.md) — AbcArtifact usage
- [module-resolution.md](module-resolution.md) — OhosResolvedModule
- [plugins.md](plugins.md) — ArkdownPlugin trait implementations
