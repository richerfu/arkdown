use std::path::PathBuf;

use arkdown_config::{
    HvigorRuntimeParameterPlanRequest, HvigorRuntimeParameterPlanner,
    HvigorRuntimePropertyPlanRequest, HvigorRuntimePropertyPlanner,
};
use serde_json::{json, Map, Value};

fn object(value: Value) -> Map<String, Value> {
    value.as_object().cloned().expect("fixture object")
}

#[test]
fn parameter_plan_matches_global_data_merge_order_and_test_defaults() {
    // hvigor/src/base/internal/data/global-data.ts::initStartData applies
    // config properties, test defaults, and finally CLI overrides.
    let plan = HvigorRuntimeParameterPlanner::new(HvigorRuntimeParameterPlanRequest {
        user_config: json!({ "properties": { "owner": "user", "retained": 1 } }),
        project_config: json!({
            "properties": { "owner": "project" },
            "execution": { "incremental": true, "parallel": false, "daemon": false },
            "logging": { "level": "warn" }
        }),
        runtime_config: json!({
            "prop": ["custom=a=b", "product", "properties.owner=cli"],
            "configOverrides": ["properties.number=0x10"]
        }),
        command_entry_tasks: vec!["test".to_string()],
        workspace_dir: PathBuf::from("workspace"),
    })
    .plan();

    assert_eq!(plan.properties.get("owner"), Some(&json!("cli")));
    assert_eq!(plan.properties.get("retained"), Some(&json!(1)));
    assert_eq!(plan.properties.get("number"), Some(&json!(16)));
    assert_eq!(plan.ext_params.get("custom"), Some(&json!("a=b")));
    assert!(!plan.ext_params.contains_key("product"));
    assert_eq!(plan.undefined_ext_param_keys, vec!["product"]);
    assert_eq!(plan.ext_params.get("unitTestMode"), Some(&json!("true")));
    assert_eq!(
        plan.core_start_params.get("parallelExecution"),
        Some(&json!(false))
    );
    assert_eq!(
        plan.core_start_params.get("incrementalExecution"),
        Some(&json!(true))
    );
    assert_eq!(plan.core_start_params.get("daemon"), Some(&json!(false)));
    assert_eq!(plan.core_start_params.get("logLevel"), Some(&json!("warn")));
}

#[test]
fn runtime_property_plan_preserves_literal_and_truthy_hvigor_predicates() {
    // PreviewerArkCompile compares compileHspDependencies with the literal
    // string "true" while ReplacePreviewerPage uses JavaScript truthiness.
    let plan = HvigorRuntimePropertyPlanner::new(HvigorRuntimePropertyPlanRequest {
        properties: object(json!({
            "ohos.byteCodeHar.integratedOptimization": "true",
            "hvigor.keepDependency": false,
            "ohos.fallback.target": ["phone", "default"]
        })),
        ext_params: object(json!({
            "compileHspDependencies": "false",
            "ohos-enable-hwasan": "false",
            "ohos-enable-binxo": "true"
        })),
        core_start_params: object(json!({ "daemon": false })),
    })
    .plan()
    .expect("valid runtime property plan");

    assert_eq!(
        plan.runtime_properties.get("compileHspDependencies"),
        Some(&json!(false))
    );
    assert_eq!(
        plan.runtime_properties.get("compileHspDependenciesTruthy"),
        Some(&json!(true))
    );
    assert_eq!(
        plan.runtime_properties.get("hwasanConfig"),
        Some(&json!(false))
    );
    assert_eq!(
        plan.runtime_properties.get("binxoConfig"),
        Some(&json!(true))
    );
    assert!(!plan.native_properties.byte_code_har_integrated_optimization);
    assert!(!plan.native_properties.keep_dependency);
}

#[test]
fn runtime_property_plan_rejects_invalid_typed_values() {
    let error = HvigorRuntimePropertyPlanner::new(HvigorRuntimePropertyPlanRequest {
        properties: object(json!({ "ohos.fallback.target": ["phone", 7] })),
        ext_params: Map::new(),
        core_start_params: Map::new(),
    })
    .plan()
    .expect_err("mixed fallback target array must be rejected");

    assert_eq!(error.code, "INVALID_FALLBACK_TARGET");
    assert_eq!(error.property, "ohos.fallback.target");
}
