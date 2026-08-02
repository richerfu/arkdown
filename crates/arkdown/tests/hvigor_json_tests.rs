use arkdown::HvigorJsonParser;
use serde_json::{json, Value};

fn parse(source: &str, locations: bool) -> Value {
    let bridge = HvigorJsonParser::new(source, locations)
        .bridge_json()
        .expect("parse hvigor JSON5 expression");
    serde_json::from_str(&bridge).expect("decode bridge JSON")
}

#[test]
fn parser_matches_hvigor_value_and_location_contract() {
    let parsed = parse(
        r#"{
 foo: 1,
 nested: { bar: 'x' },
 array: [true, { z: 2 }, 3]
}"#,
        true,
    );

    assert_eq!(
        parsed,
        json!({
            "kind": "object",
            "properties": [
                {
                    "key": "foo",
                    "value": {
                        "kind": "number",
                        "value": "1",
                        "location": { "line": 2, "column": 7 }
                    }
                },
                {
                    "key": "nested",
                    "value": {
                        "kind": "object",
                        "properties": [{
                            "key": "bar",
                            "value": {
                                "kind": "string",
                                "value": "x",
                                "location": { "line": 3, "column": 19 }
                            }
                        }],
                        "location": { "line": 3, "column": 10 }
                    }
                },
                {
                    "key": "array",
                    "value": {
                        "kind": "array",
                        "values": [
                            {
                                "kind": "boolean",
                                "value": true,
                                "location": { "line": 4, "column": 13 }
                            },
                            {
                                "kind": "object",
                                "properties": [{
                                    "key": "z",
                                    "value": {
                                        "kind": "number",
                                        "value": "2",
                                        "location": { "line": 4, "column": 21 }
                                    }
                                }],
                                "location": { "line": 4, "column": 16 }
                            },
                            {
                                "kind": "number",
                                "value": "3",
                                "location": { "line": 4, "column": 26 }
                            }
                        ],
                        "location": { "line": 4, "column": 9 }
                    }
                }
            ],
            "location": { "line": 1, "column": 1 }
        })
    );
}

#[test]
fn parser_preserves_non_finite_numbers_and_negative_zero() {
    let parsed = parse("[Infinity, -Infinity, NaN, -0, +2]", false);
    let values = parsed["values"].as_array().expect("array bridge values");

    assert_eq!(values[0]["value"], "Infinity");
    assert_eq!(values[1]["value"], "-Infinity");
    assert_eq!(values[2]["value"], "NaN");
    assert_eq!(values[3]["value"], "-0");
    assert_eq!(values[4]["value"], "2");
}

#[test]
fn parser_rejects_javascript_outside_hvigor_json5_value_grammar() {
    for source in [
        "{ ...other }",
        "{ [name]: 1 }",
        "{ shorthand }",
        "{ method() {} }",
        "factory()",
    ] {
        assert!(
            HvigorJsonParser::new(source, false).bridge_json().is_err(),
            "expected rejection for {source}"
        );
    }
}

#[test]
fn parser_reports_hvigor_empty_input_diagnostic() {
    assert_eq!(
        HvigorJsonParser::new(" \n\t", false)
            .bridge_json()
            .expect_err("reject empty input")
            .to_string(),
        "JSON5: invalid end of input at 1:0"
    );
}
