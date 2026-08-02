use std::path::Path;

use arkdown_config::{BuildScriptBuildOptionValidationRequest, BuildScriptSchemaValidator};
use serde_json::json;

#[test]
fn validator_rejects_project_and_module_schema_paths_observed_in_hvigor() {
    let source_file = Path::new("/fixture/build-profile.json5");
    let project = json!({
        "app": {
            "products": [{
                "name": "default",
                "buildOption": { "arkOptions": { "compilePluginFile": "plugin.ts" } }
            }]
        }
    });
    let diagnostic = BuildScriptSchemaValidator::new(source_file)
        .validate_project_profile(&project)
        .expect_err("reject project compilePluginFile");
    assert_eq!(diagnostic.code, "HVIGOR_SCHEMA_COMPILE_PLUGIN_FILE");
    assert_eq!(
        diagnostic.field_path,
        "app.products[0].buildOption.arkOptions.compilePluginFile"
    );

    let module = json!({
        "targets": [{
            "name": "default",
            "config": {
                "buildOption": { "arkOptions": { "compilePluginFile": null } }
            }
        }]
    });
    let diagnostic = BuildScriptSchemaValidator::new(source_file)
        .validate_module_profile(&module)
        .expect_err("reject module compilePluginFile by field presence");
    assert_eq!(
        diagnostic.field_path,
        "targets[0].config.buildOption.arkOptions.compilePluginFile"
    );
}

#[test]
fn validator_accepts_context_registered_compile_plugin_paths() {
    let request = BuildScriptBuildOptionValidationRequest {
        build_option: json!({
            "arkOptions": {
                "compilePluginPath": ["plugin-a.ts", "plugin-b.ts"],
                "hvigorPluginFile": "hvigor-plugin.ts"
            }
        }),
        source_file: "/fixture/hvigorfile.ts".into(),
        field_path: "config.ohos.overrides.buildOption".to_string(),
    };

    BuildScriptSchemaValidator::validate_request(&request)
        .expect("latest schema still accepts non-declarative compatibility fields");
}
