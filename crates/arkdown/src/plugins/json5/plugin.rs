use std::borrow::Cow;
use std::path::{Path, PathBuf};

use arkdown_ets_transform::TransformSourceSnapshot;
use arkdown_path::{lexical_normalize, ArtifactPathSerializer};
use rolldown_plugin::{
    HookLoadArgs, HookLoadOutput, HookLoadReturn, HookResolveIdArgs, HookResolveIdOutput,
    HookResolveIdReturn, HookUsage, Plugin, SharedLoadPluginContext,
};

use super::module_emit::JsonModuleEmitter;

pub struct Json5Plugin {
    sources: TransformSourceSnapshot,
}

impl Json5Plugin {
    pub fn new(sources: TransformSourceSnapshot) -> Self {
        Self { sources }
    }
}

impl std::fmt::Debug for Json5Plugin {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Json5Plugin").finish()
    }
}

impl Plugin for Json5Plugin {
    fn name(&self) -> Cow<'static, str> {
        Cow::Borrowed("json5")
    }

    fn register_hook_usage(&self) -> HookUsage {
        HookUsage::ResolveId | HookUsage::Load
    }

    fn resolve_id(
        &self,
        _ctx: &rolldown_plugin::PluginContext,
        args: &HookResolveIdArgs<'_>,
    ) -> impl std::future::Future<Output = HookResolveIdReturn> + Send {
        let specifier = args.specifier.to_owned();
        let importer = args.importer.map(str::to_owned);
        async move {
            if !JsonModuleSpecifier::new(&specifier).is_json_module() {
                return Ok(None);
            }
            let resolved = JsonModuleResolver::new(&specifier, importer.as_deref())
                .resolve()
                .map_err(|e| anyhow::anyhow!("{e}"))?;
            Ok(Some(HookResolveIdOutput {
                id: ArtifactPathSerializer::path_to_slash_string(&resolved)?.into(),
                external: None,
                ..Default::default()
            }))
        }
    }

    fn load(
        &self,
        _ctx: SharedLoadPluginContext,
        args: &HookLoadArgs<'_>,
    ) -> impl std::future::Future<Output = HookLoadReturn> + Send {
        let id = args.id.to_owned();
        let sources = self.sources.clone();
        async move { JsonModuleLoader::new(&id, &sources).load() }
    }
}

struct JsonModuleLoader<'a> {
    id: &'a str,
    sources: &'a TransformSourceSnapshot,
}

impl<'a> JsonModuleLoader<'a> {
    fn new(id: &'a str, sources: &'a TransformSourceSnapshot) -> Self {
        Self { id, sources }
    }

    fn load(&self) -> HookLoadReturn {
        let specifier = JsonModuleSpecifier::new(self.id);
        if !specifier.is_json_module() {
            return Ok(None);
        }
        let path = Path::new(self.id);
        let raw = self.sources.source_text(path).ok_or_else(|| {
            anyhow::anyhow!(
                "json5 plugin: {} is missing from the immutable startup source snapshot",
                self.id
            )
        })?;
        let value: serde_json::Value = if specifier.is_json5() {
            json5::from_str(raw.as_ref())
                .map_err(|e| anyhow::anyhow!("json5 plugin: parse error in {}: {e}", self.id))?
        } else {
            serde_json::from_str(raw.as_ref())
                .map_err(|e| anyhow::anyhow!("json5 plugin: parse error in {}: {e}", self.id))?
        };
        let code = JsonModuleEmitter::new(&value).emit();
        Ok(Some(HookLoadOutput {
            code: code.into(),
            ..Default::default()
        }))
    }
}

struct JsonModuleResolver<'a> {
    specifier: &'a str,
    importer: Option<&'a str>,
}

impl<'a> JsonModuleResolver<'a> {
    fn new(specifier: &'a str, importer: Option<&'a str>) -> Self {
        Self {
            specifier,
            importer,
        }
    }

    fn resolve(&self) -> anyhow::Result<PathBuf> {
        if let Some(importer) = self.importer {
            let base = Path::new(importer).parent().unwrap_or(Path::new("."));
            let joined = base.join(self.specifier);
            return Ok(lexical_normalize(&joined));
        }
        Ok(PathBuf::from(self.specifier))
    }
}

struct JsonModuleSpecifier<'a> {
    specifier: &'a str,
}

impl<'a> JsonModuleSpecifier<'a> {
    fn new(specifier: &'a str) -> Self {
        Self { specifier }
    }

    fn is_json_module(&self) -> bool {
        matches!(self.extension(), Some("json" | "json5"))
    }

    fn is_json5(&self) -> bool {
        self.extension() == Some("json5")
    }

    fn extension(&self) -> Option<&str> {
        Path::new(self.specifier)
            .extension()
            .and_then(|extension| extension.to_str())
    }
}

#[cfg(test)]
mod tests {
    use super::super::module_emit::JsonModuleEmitter;
    use super::*;
    use oxc_allocator::Allocator;
    use oxc_ast::ast::{Declaration, Statement};
    use oxc_parser::Parser;
    use oxc_span::SourceType;

    fn source_snapshot(path: &Path, source: &str) -> TransformSourceSnapshot {
        let mut files = rustc_hash::FxHashMap::default();
        files.insert(path.to_path_buf(), std::sync::Arc::<str>::from(source));
        TransformSourceSnapshot::new(files)
    }

    struct JsonModuleAssertion<'a> {
        code: &'a str,
    }

    impl<'a> JsonModuleAssertion<'a> {
        fn new(code: &'a str) -> Self {
            Self { code }
        }

        fn has_named_var_export(&self, expected: &str) -> bool {
            let allocator = Allocator::default();
            let parsed =
                Parser::new(&allocator, self.code, SourceType::ts().with_module(true)).parse();
            parsed.program.body.iter().any(|statement| match statement {
                Statement::ExportDeclaration(export) => {
                    let Declaration::VariableDeclaration(declaration) = &export.declaration else {
                        return false;
                    };
                    declaration.declarations.iter().any(|declarator| {
                        declarator.id.get_identifier_name().as_deref() == Some(expected)
                    })
                }
                _ => false,
            })
        }

        fn has_default_export(&self) -> bool {
            let allocator = Allocator::default();
            let parsed =
                Parser::new(&allocator, self.code, SourceType::ts().with_module(true)).parse();
            parsed
                .program
                .body
                .iter()
                .any(|statement| matches!(statement, Statement::ExportDefaultDeclaration(_)))
        }
    }

    #[test]
    fn test_generate_esm_object() {
        let val: serde_json::Value = serde_json::json!({"name": "foo", "version": 2});
        let code = JsonModuleEmitter::new(&val).emit();
        let module = JsonModuleAssertion::new(&code);
        assert!(module.has_named_var_export("name"));
        assert!(module.has_named_var_export("version"));
        assert!(module.has_default_export());
    }

    #[test]
    fn test_generate_esm_array() {
        let val: serde_json::Value = serde_json::json!([1, 2, 3]);
        let code = JsonModuleEmitter::new(&val).emit();
        let module = JsonModuleAssertion::new(&code);
        assert!(module.has_default_export());
        assert!(!module.has_named_var_export("0"));
    }

    #[test]
    fn test_invalid_identifier_skipped() {
        let val = serde_json::json!({"123invalid": "x", "valid_key": "y"});
        let code = JsonModuleEmitter::new(&val).emit();
        let module = JsonModuleAssertion::new(&code);
        assert!(module.has_named_var_export("valid_key"));
        assert!(!module.has_named_var_export("123invalid"));
    }

    #[test]
    fn test_is_valid_js_identifier() {
        assert!(JsonModuleEmitter::is_valid_js_identifier("foo"));
        assert!(JsonModuleEmitter::is_valid_js_identifier("_bar"));
        assert!(!JsonModuleEmitter::is_valid_js_identifier("123"));
        assert!(!JsonModuleEmitter::is_valid_js_identifier(""));
        assert!(!JsonModuleEmitter::is_valid_js_identifier("class"));
    }

    #[test]
    fn test_load_json_from_tempfile() {
        use std::io::Write;
        let mut tmp = tempfile::NamedTempFile::with_suffix(".json").unwrap();
        writeln!(tmp, r#"{{"hello": "world"}}"#).unwrap();
        let path = tmp.path().to_string_lossy().to_string();
        let sources = source_snapshot(tmp.path(), r#"{"hello": "world"}"#);

        let result = JsonModuleLoader::new(&path, &sources).load().unwrap();
        let out = result.unwrap();
        assert!(JsonModuleAssertion::new(&out.code).has_named_var_export("hello"));
    }

    #[test]
    fn test_load_json5_with_comments_and_trailing_commas() {
        use std::io::Write;
        let mut tmp = tempfile::NamedTempFile::with_suffix(".json5").unwrap();
        writeln!(
            tmp,
            r#"{{
  // this is a comment
  "name": "arkdown",
  "version": "1.0",
}}"#
        )
        .unwrap();
        let path = tmp.path().to_string_lossy().to_string();
        let sources = source_snapshot(
            tmp.path(),
            "{ // this is a comment\n\"name\": \"arkdown\",\n\"version\": \"1.0\",\n}",
        );

        let result = JsonModuleLoader::new(&path, &sources).load();
        assert!(
            result.is_ok(),
            "JSON5 with comments/trailing commas must not error"
        );
        let out = result.unwrap().unwrap();
        let module = JsonModuleAssertion::new(&out.code);
        assert!(module.has_named_var_export("name"));
        assert!(module.has_default_export());
    }

    #[test]
    fn test_load_returns_none_for_non_json_id() {
        let sources = TransformSourceSnapshot::default();
        let result = JsonModuleLoader::new("/some/path/module.ts", &sources)
            .load()
            .unwrap();
        assert!(
            result.is_none(),
            "load must return None for non-JSON/JSON5 files"
        );
    }
}
