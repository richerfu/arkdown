/// Placeholder for the OXC AST reference.
/// Replaced with a real reference type once oxc is integrated.
pub type OxcAstRef = ();

/// Source map as a serialized JSON string.
pub type SourceMap = String;

/// The format of a module's definition (how it exports).
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub enum ModuleDefFormat {
    #[default]
    Unknown,
    Esm,
    Cjs,
    Iife,
}

/// A fully resolved module identifier.
///
/// In OHOS projects with `useNormalizedOHMUrl=true`, this is an OHM URL.
/// Otherwise it is a canonical absolute filesystem path.
#[derive(Debug, Clone, Default)]
pub struct ResolvedId {
    /// The canonical identifier string.
    /// - OHM URL: `"@bundle:com.example/entry/ets/entryability/EntryAbility"`
    /// - File path: `"/abs/path/to/src/main/ets/utils/helper.ets"`
    pub id: String,
    /// Whether this is a bare external reference (never bundled).
    /// True for `@bundle:` and `@shared:` references.
    pub external: bool,
    /// Module type inferred from extension or resolver metadata.
    pub module_type: ModuleType,
    /// Definition format of the module (ESM, CJS, IIFE, or unknown).
    pub module_def_format: ModuleDefFormat,
}

/// The raw source text of a module, after loading but before parsing.
#[derive(Debug, Clone)]
pub struct ModuleSource {
    pub id: ResolvedId,
    /// UTF-8 source content.
    pub content: String,
    /// Source map from a prior transform stage (e.g. ArkTS desugar).
    pub source_map: Option<SourceMap>,
}

/// The primary module type discriminator.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub enum ModuleType {
    /// ArkTS source file (`.ets`) — requires ArkTS desugar transform.
    Ets,
    /// TypeScript source file (`.ts`).
    Ts,
    /// JavaScript source file (`.js`, `.mjs`, `.cjs`).
    #[default]
    Js,
    /// JSON or JSON5 file.
    Json5,
    /// Native resource file (non-code; excluded from JS bundle).
    NativeResource,
    /// Pre-compiled ByteCode HAR (`.abc`). No source bundling.
    BytecodeHar,
    /// TypeScript declaration file (`.d.ts`, `.d.ets`). Type-only; excluded from output.
    Declaration,
}
