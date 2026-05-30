use std::borrow::Cow;
use std::path::{Path, PathBuf};

use rolldown_plugin::{
    HookLoadArgs, HookLoadOutput, HookLoadReturn, HookResolveIdArgs, HookResolveIdOutput,
    HookResolveIdReturn, HookUsage, Plugin, SharedLoadPluginContext,
};

pub struct Json5Plugin;

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
            if !specifier.ends_with(".json") && !specifier.ends_with(".json5") {
                return Ok(None);
            }
            let resolved = resolve_relative_path(&specifier, importer.as_deref())
                .map_err(|e| anyhow::anyhow!("{e}"))?;
            Ok(Some(HookResolveIdOutput {
                id: resolved.to_string_lossy().as_ref().into(),
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
        async move { load_json_file(&id) }
    }
}

pub(crate) fn load_json_file(id: &str) -> HookLoadReturn {
    if !id.ends_with(".json") && !id.ends_with(".json5") {
        return Ok(None);
    }
    let raw = fs_err::read_to_string(id)
        .map_err(|e| anyhow::anyhow!("json5 plugin: failed to read {id}: {e}"))?;
    let value: serde_json::Value = if id.ends_with(".json5") {
        json5::from_str(&raw)
            .map_err(|e| anyhow::anyhow!("json5 plugin: parse error in {id}: {e}"))?
    } else {
        serde_json::from_str(&raw)
            .map_err(|e| anyhow::anyhow!("json5 plugin: parse error in {id}: {e}"))?
    };
    let code = generate_esm_from_json(&value);
    Ok(Some(HookLoadOutput {
        code: code.into(),
        ..Default::default()
    }))
}

fn resolve_relative_path(specifier: &str, importer: Option<&str>) -> anyhow::Result<PathBuf> {
    if let Some(imp) = importer {
        let base = Path::new(imp).parent().unwrap_or(Path::new("."));
        Ok(base
            .join(specifier)
            .canonicalize()
            .unwrap_or_else(|_| base.join(specifier)))
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
    if s.is_empty() {
        return false;
    }
    let mut chars = s.chars();
    let first = chars.next().unwrap();
    if !first.is_alphabetic() && first != '_' && first != '$' {
        return false;
    }
    !matches!(
        s,
        "import"
            | "export"
            | "default"
            | "class"
            | "function"
            | "return"
            | "var"
            | "let"
            | "const"
            | "if"
            | "else"
            | "for"
            | "while"
            | "do"
            | "switch"
            | "case"
            | "break"
            | "continue"
            | "new"
            | "delete"
            | "typeof"
            | "void"
            | "in"
            | "of"
            | "this"
            | "super"
            | "null"
            | "undefined"
            | "true"
            | "false"
    )
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

    #[test]
    fn test_load_json_from_tempfile() {
        use std::io::Write;
        let mut tmp = tempfile::NamedTempFile::with_suffix(".json").unwrap();
        writeln!(tmp, r#"{{"hello": "world"}}"#).unwrap();
        let path = tmp.path().to_string_lossy().to_string();

        let result = load_json_file(&path).unwrap();
        let out = result.unwrap();
        assert!(out.code.contains("export var hello"));
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

        let result = load_json_file(&path);
        assert!(
            result.is_ok(),
            "JSON5 with comments/trailing commas must not error"
        );
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

    #[test]
    fn test_load_returns_none_for_non_json_id() {
        let result = load_json_file("/some/path/module.ts").unwrap();
        assert!(
            result.is_none(),
            "load must return None for non-JSON/JSON5 files"
        );
    }
}
