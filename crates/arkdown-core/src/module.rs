use std::path::PathBuf;

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
    /// Definition format of the module (ESM or unknown).
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

/// A transformed module source file handed to SDK `es2abc`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct HvigorModuleSourceFile {
    /// Rolldown module id for the original source module.
    pub module_id: String,
    /// Path to the transformed JS source written for `es2abc`.
    pub path: PathBuf,
    /// ABC record name generated from hvigor module ownership.
    pub record_name: String,
    /// Original source-file attribution written to `filesInfo.txt` column 4.
    pub source_file: String,
    /// Package name written to `filesInfo.txt` column 5.
    pub package_name: Option<String>,
    /// Whether rolldown classified the module as CommonJS.
    pub is_commonjs: bool,
    /// Whether the first non-import statement is the `"use shared"` directive.
    ///
    /// SDK `check_shared_module.ts::collectSharedModule` records this fact
    /// before `ModuleMode.generateCompileFilesInfo` writes column 6.
    pub is_shared_module: bool,
    /// Source language before ETS/TS transform.
    pub origin_source_lang: String,
    /// ABC record names for static imports/re-exports resolved from this module.
    pub imported_record_names: Vec<String>,
}

/// One `npmEntries.txt` mapping consumed by SDK `es2abc`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct HvigorNpmEntry {
    pub entry_path: String,
    pub build_path: String,
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

impl ModuleType {
    /// Infer the module type from a file path's extension.
    ///
    /// Used by the resolver and loader to classify source files without
    /// reading their content.
    pub fn infer(path: &std::path::Path) -> Self {
        match path.extension().and_then(|e| e.to_str()) {
            Some("ets") => ModuleType::Ets,
            Some("ts") => ModuleType::Ts,
            Some("js") | Some("mjs") | Some("cjs") => ModuleType::Js,
            _ => ModuleType::Js,
        }
    }
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    // ── ModuleDefFormat ──────────────────────────────────────────────────────

    #[test]
    fn test_module_def_format_default_is_unknown() {
        let fmt: ModuleDefFormat = Default::default();
        assert_eq!(fmt, ModuleDefFormat::Unknown);
    }

    #[test]
    fn test_module_def_format_variants_exist() {
        // Ensure all variants are constructible and distinguishable.
        assert_ne!(ModuleDefFormat::Unknown, ModuleDefFormat::Esm);
    }

    // ── ModuleType ───────────────────────────────────────────────────────────

    #[test]
    fn test_module_type_default_is_js() {
        let mt: ModuleType = Default::default();
        assert_eq!(mt, ModuleType::Js);
    }

    #[test]
    fn test_module_type_variants_all_exist() {
        let _ets = ModuleType::Ets;
        let _ts = ModuleType::Ts;
        let _js = ModuleType::Js;
        let _json5 = ModuleType::Json5;
        let _nr = ModuleType::NativeResource;
        let _bc = ModuleType::BytecodeHar;
        let _decl = ModuleType::Declaration;
        // All distinct
        assert_ne!(ModuleType::Ets, ModuleType::Ts);
        assert_ne!(ModuleType::Ts, ModuleType::Js);
        assert_ne!(ModuleType::Json5, ModuleType::NativeResource);
        assert_ne!(ModuleType::BytecodeHar, ModuleType::Declaration);
    }

    // ── ResolvedId ───────────────────────────────────────────────────────────

    #[test]
    fn test_resolved_id_default_has_empty_id_not_external() {
        let r: ResolvedId = Default::default();
        assert!(r.id.is_empty());
        assert!(!r.external);
        assert_eq!(r.module_type, ModuleType::Js);
        assert_eq!(r.module_def_format, ModuleDefFormat::Unknown);
    }

    #[test]
    fn test_resolved_id_full_construction() {
        let r = ResolvedId {
            id: "@bundle:com.example.app/entry/ets/entryability/EntryAbility".to_string(),
            external: true,
            module_type: ModuleType::Ets,
            module_def_format: ModuleDefFormat::Esm,
        };
        assert_eq!(
            r.id,
            "@bundle:com.example.app/entry/ets/entryability/EntryAbility"
        );
        assert!(r.external);
        assert_eq!(r.module_type, ModuleType::Ets);
        assert_eq!(r.module_def_format, ModuleDefFormat::Esm);
    }

    #[test]
    fn test_resolved_id_filesystem_path() {
        let r = ResolvedId {
            id: "/abs/path/to/src/main/ets/utils/helper.ets".to_string(),
            external: false,
            module_type: ModuleType::Ets,
            module_def_format: ModuleDefFormat::Unknown,
        };
        assert!(!r.external);
        assert!(r.id.starts_with('/'));
    }

    // ── ModuleSource ─────────────────────────────────────────────────────────

    #[test]
    fn test_module_source_construction() {
        let src = ModuleSource {
            id: ResolvedId::default(),
            content: "const x = 1;".to_string(),
            source_map: None,
        };
        assert_eq!(src.content, "const x = 1;");
        assert!(src.source_map.is_none());
    }

    #[test]
    fn test_module_source_with_source_map() {
        let src = ModuleSource {
            id: ResolvedId::default(),
            content: "export default 1;".to_string(),
            source_map: Some("{\"version\":3,\"sources\":[\"input.ts\"]}".to_string()),
        };
        assert!(src.source_map.is_some());
    }
}
