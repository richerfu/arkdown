use arkdown::{PluginAstParseError, PluginAstParseOptions, PluginAstParser, PluginCacheModule};
use serde_json::{json, Value};

struct ParsedPluginAst(Value);

impl ParsedPluginAst {
    fn parse(source: &str, options: PluginAstParseOptions) -> Self {
        let encoded = PluginAstParser::new(source, options)
            .parse()
            .and_then(|result| result.bridge_json())
            .expect("plugin source should parse");
        Self(serde_json::from_str(&encoded).expect("bridge payload should be JSON"))
    }

    fn program(&self) -> &Value {
        &self.0["ast"]["node"]
    }

    fn comments(&self) -> &Value {
        &self.0["comments"]
    }
}

#[test]
fn plugin_ast_matches_latest_ide_rollup_default_program_shape() {
    // DevEco Studio 6.1.1 / hvigor 6.24.3:
    // arkts-pack.js::getCompilePluginContext -> Rollup PluginContext.parse.
    let parsed = ParsedPluginAst::parse("(answer);", PluginAstParseOptions::default());
    let program = parsed.program();

    assert_eq!(program["type"], "Program");
    assert_eq!(program["sourceType"], "module");
    assert_eq!(program["start"], 0);
    assert_eq!(program["end"], 9);
    assert!(program.get("hashbang").is_none());
    assert_eq!(program["body"][0]["type"], "ExpressionStatement");
    assert_eq!(program["body"][0]["expression"]["type"], "Identifier");
    assert_eq!(program["body"][0]["expression"]["start"], 1);
    assert_eq!(program["body"][0]["expression"]["end"], 7);
}

#[test]
fn plugin_ast_uses_acorn_compatible_utf16_ranges_locations_and_comments() {
    let parsed = ParsedPluginAst::parse(
        "// 注释\nconst emoji = \"🤨\";",
        PluginAstParseOptions {
            ranges: true,
            locations: true,
            source_file: Some("plugin.ts".to_string()),
            capture_comments: true,
            ..PluginAstParseOptions::default()
        },
    );
    let program = parsed.program();

    assert_eq!(program["end"], 25);
    assert_eq!(program["range"], json!([0, 25]));
    assert_eq!(
        program["loc"],
        json!({
            "start": { "line": 1, "column": 0 },
            "end": { "line": 2, "column": 19 },
            "source": "plugin.ts"
        })
    );
    assert_eq!(program["body"][0]["start"], 6);
    assert_eq!(program["body"][0]["declarations"][0]["init"]["end"], 24);
    assert_eq!(
        parsed.comments(),
        &json!([{
            "type": "Line",
            "value": " 注释",
            "start": 0,
            "end": 5,
            "loc": {
                "start": { "line": 1, "column": 0 },
                "end": { "line": 1, "column": 5 },
                "source": "plugin.ts"
            },
            "range": [0, 5]
        }])
    );
}

#[test]
fn plugin_ast_records_only_bigint_and_regexp_literal_fix_paths() {
    let parsed = ParsedPluginAst::parse(
        "const n = 123n; const r = /ab+/gi;",
        PluginAstParseOptions::default(),
    );
    let ast = &parsed.0["ast"];

    assert_eq!(ast["fixes"].as_array().map(Vec::len), Some(2));
    assert_eq!(
        ast["node"]["body"][0]["declarations"][0]["init"]["value"],
        Value::Null
    );
    assert_eq!(
        ast["node"]["body"][0]["declarations"][0]["init"]["bigint"],
        "123"
    );
    assert_eq!(
        ast["node"]["body"][1]["declarations"][0]["init"]["regex"],
        json!({ "pattern": "ab+", "flags": "gi" })
    );
}

#[test]
fn plugin_ast_matches_rollup_script_return_policy() {
    let rejected = PluginAstParser::new(
        "return 1",
        PluginAstParseOptions {
            source_type: Some("script".to_string()),
            ..PluginAstParseOptions::default()
        },
    )
    .parse()
    .expect_err("top-level script return should be rejected");
    assert!(matches!(
        rejected,
        PluginAstParseError::Parse {
            position: 0,
            line: 1,
            column: 0,
            ..
        }
    ));

    let parsed = ParsedPluginAst::parse(
        "return 1",
        PluginAstParseOptions {
            source_type: Some("script".to_string()),
            allow_return_outside_function: true,
            ..PluginAstParseOptions::default()
        },
    );
    assert_eq!(parsed.program()["sourceType"], "script");
    assert_eq!(parsed.program()["body"][0]["type"], "ReturnStatement");
}

#[test]
fn plugin_cache_request_carries_rust_owned_ast_transport() {
    let request: Value = serde_json::from_str(
        &PluginCacheModule {
            id: "/entry.js".to_string(),
            code: "export const answer = 42".to_string(),
        }
        .hook_request_json()
        .expect("cache hook request should serialize"),
    )
    .expect("cache hook request should be JSON");

    assert_eq!(request["id"], "/entry.js");
    assert_eq!(request["code"], "export const answer = 42");
    assert!(request.get("ast").is_none());
    assert_eq!(request["__arkdownOxcAst"]["node"]["type"], "Program");
}
