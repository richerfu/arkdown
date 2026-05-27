use std::path::{Path, PathBuf};
use async_trait::async_trait;
use arkdown_core::{
    ArkdownError, ArkdownPlugin, HookLoadOutput, HookResolveIdArgs, HookResolveIdOutput,
    HookUsage, ModuleType, PluginContext, PluginResult,
};

pub struct Json5Plugin;

#[async_trait]
impl ArkdownPlugin for Json5Plugin {
    fn name(&self) -> &str { "json5" }

    fn register_hook_usage(&self) -> HookUsage {
        HookUsage::ResolveId | HookUsage::Load
    }

    async fn resolve_id(
        &self,
        _ctx: &PluginContext,
        args: &HookResolveIdArgs<'_>,
    ) -> PluginResult<Option<HookResolveIdOutput>> {
        let specifier = args.specifier;
        if !specifier.ends_with(".json") && !specifier.ends_with(".json5") {
            return Ok(None);
        }
        let resolved_path = resolve_relative_path(specifier, args.importer)?;
        Ok(Some(HookResolveIdOutput {
            id: resolved_path.to_string_lossy().to_string(),
            external: false,
            module_type: ModuleType::Json5,
            side_effects: None,
        }))
    }

    async fn load(
        &self,
        _ctx: &PluginContext,
        id: &str,
    ) -> PluginResult<Option<HookLoadOutput>> {
        if !id.ends_with(".json") && !id.ends_with(".json5") {
            return Ok(None);
        }
        let raw = fs_err::read_to_string(id)
            .map_err(|e| ArkdownError::Io(e.into()))?;

        let value: serde_json::Value = if id.ends_with(".json5") {
            json5::from_str(&raw).map_err(|e| ArkdownError::ParseError {
                file: id.to_string(),
                message: e.to_string(),
            })?
        } else {
            serde_json::from_str(&raw).map_err(|e| ArkdownError::ParseError {
                file: id.to_string(),
                message: e.to_string(),
            })?
        };

        let code = generate_esm_from_json(&value);
        Ok(Some(HookLoadOutput {
            code,
            source_map: None,
            module_type: Some(ModuleType::Json5),
        }))
    }
}

fn resolve_relative_path(specifier: &str, importer: Option<&str>) -> PluginResult<PathBuf> {
    if let Some(imp) = importer {
        let base = Path::new(imp).parent().unwrap_or(Path::new("."));
        Ok(base.join(specifier).canonicalize().unwrap_or_else(|_| base.join(specifier)))
    } else {
        Ok(PathBuf::from(specifier))
    }
}

/// Generate a synthetic ES module from a JSON value.
/// Top-level object keys that are valid JS identifiers become named exports.
/// The whole value is always the default export.
pub fn generate_esm_from_json(value: &serde_json::Value) -> String {
    let default_json = serde_json::to_string(value).unwrap_or_else(|_| "null".to_string());
    let mut code = String::new();

    // Named exports for top-level object keys
    if let serde_json::Value::Object(map) = value {
        code.push_str(&format!("var __json_default = {};\n", default_json));
        for key in map.keys() {
            if is_valid_js_identifier(key) {
                code.push_str(&format!(
                    "export var {} = __json_default[{:?}];\n",
                    key, key
                ));
            }
        }
        code.push_str("export default __json_default;\n");
    } else {
        code.push_str(&format!("export default {};\n", default_json));
    }

    code
}

fn is_valid_js_identifier(s: &str) -> bool {
    if s.is_empty() { return false; }
    let mut chars = s.chars();
    let first = chars.next().unwrap();
    if !first.is_alphabetic() && first != '_' && first != '$' { return false; }
    // Check no reserved words (basic check — keywords would cause a parse error downstream)
    !matches!(s, "import" | "export" | "default" | "class" | "function" | "return" |
                 "var" | "let" | "const" | "if" | "else" | "for" | "while" | "do" |
                 "switch" | "case" | "break" | "continue" | "new" | "delete" | "typeof" |
                 "void" | "in" | "of" | "this" | "super" | "null" | "undefined" | "true" | "false")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_generate_esm_object() {
        let val: serde_json::Value = serde_json::json!({"name": "foo", "version": 2});
        let code = generate_esm_from_json(&val);
        assert!(code.contains("export var name"));
        assert!(code.contains("export var version"));
        assert!(code.contains("export default"));
    }

    #[test]
    fn test_generate_esm_array() {
        let val: serde_json::Value = serde_json::json!([1, 2, 3]);
        let code = generate_esm_from_json(&val);
        // Arrays get only default export
        assert!(code.contains("export default"));
        assert!(!code.contains("export var"));
    }

    #[test]
    fn test_invalid_identifier_skipped() {
        let val = serde_json::json!({"123invalid": "x", "valid_key": "y"});
        let code = generate_esm_from_json(&val);
        assert!(code.contains("export var valid_key"));
        assert!(!code.contains("export var 123invalid"));
    }

    #[test]
    fn test_is_valid_js_identifier() {
        assert!(is_valid_js_identifier("foo"));
        assert!(is_valid_js_identifier("_bar"));
        assert!(!is_valid_js_identifier("123"));
        assert!(!is_valid_js_identifier(""));
        assert!(!is_valid_js_identifier("class"));
    }

    #[tokio::test]
    async fn test_load_json_from_tempfile() {
        use std::io::Write;
        let mut tmp = tempfile::NamedTempFile::with_suffix(".json").unwrap();
        writeln!(tmp, r#"{{"hello": "world"}}"#).unwrap();
        let path = tmp.path().to_string_lossy().to_string();

        let plugin = Json5Plugin;
        let result = plugin.load(&PluginContext::default(), &path).await.unwrap();
        let out = result.unwrap();
        assert!(out.code.contains("export var hello"));
    }

    /// JSON5 files with comments and trailing commas must be parsed without error.
    #[tokio::test]
    async fn test_load_json5_with_comments_and_trailing_commas() {
        use std::io::Write;
        let mut tmp = tempfile::NamedTempFile::with_suffix(".json5").unwrap();
        // JSON5 allows single-line comments and trailing commas.
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

        let plugin = Json5Plugin;
        let result = plugin.load(&PluginContext::default(), &path).await;
        assert!(result.is_ok(), "JSON5 with comments/trailing commas must not error");
        let out = result.unwrap().unwrap();
        assert!(
            out.code.contains("export var name"),
            "named export for 'name' key must be generated"
        );
        assert!(
            out.code.contains("export default"),
            "default export must always be generated"
        );
    }

    /// The `load` hook returns `None` for files that are not `.json` or `.json5`.
    #[tokio::test]
    async fn test_load_returns_none_for_non_json_id() {
        let plugin = Json5Plugin;
        // A TypeScript file should not be handled by this plugin.
        let result = plugin
            .load(&PluginContext::default(), "/some/path/module.ts")
            .await
            .unwrap();
        assert!(
            result.is_none(),
            "load must return None for non-JSON/JSON5 files"
        );
    }

    /// `resolve_id` returns `None` for non-JSON specifiers.
    #[tokio::test]
    async fn test_resolve_id_returns_none_for_non_json() {
        use arkdown_core::{HookResolveIdArgs, ImportKind};

        let plugin = Json5Plugin;
        let args = HookResolveIdArgs {
            specifier: "./utils/helper.ts",
            importer: Some("/project/src/index.ts"),
            is_entry: false,
            kind: ImportKind::Import,
        };
        let result = plugin
            .resolve_id(&PluginContext::default(), &args)
            .await
            .unwrap();
        assert!(result.is_none(), "resolve_id must return None for non-JSON specifiers");
    }
}
