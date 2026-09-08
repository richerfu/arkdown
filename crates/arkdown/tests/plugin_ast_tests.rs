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
fn plugin_ast_ignores_options_outside_rollup_4_parse_ast_contract() {
    let options = PluginAstParseOptions::from_json(Some(
        r#"{
            "allowReturnOutsideFunction": true,
            "sourceType": "script",
            "ranges": true,
            "locations": true,
            "onToken": [],
            "program": {"type": "Program"}
        }"#,
    ))
    .unwrap();
    let parsed = ParsedPluginAst::parse("// 注释\nreturn \"🤨\";", options);
    let program = parsed.program();

    assert_eq!(program["sourceType"], "module");
    assert_eq!(program["end"], 18);
    assert_eq!(program["body"][0]["type"], "ReturnStatement");
    assert!(program.get("range").is_none());
    assert!(program.get("loc").is_none());
    assert!(parsed.0.get("comments").is_none());
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
    let rejected = PluginAstParser::new("return 1", PluginAstParseOptions::default())
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
            allow_return_outside_function: true,
        },
    );
    assert_eq!(parsed.program()["sourceType"], "module");
    assert_eq!(parsed.program()["body"][0]["type"], "ReturnStatement");
}

#[test]
fn plugin_cache_request_carries_rust_owned_ast_transport() {
    let request: Value = PluginCacheModule {
        id: "/entry.js".to_string(),
        code: "export const answer = 42".to_string(),
    }
    .hook_request_value();

    assert_eq!(request["id"], "/entry.js");
    assert_eq!(request["code"], "export const answer = 42");
    assert!(request.get("ast").is_none());
    assert_eq!(request["__arkdownOxcAst"]["node"]["type"], "Program");
}
