use std::path::PathBuf;
use std::sync::Arc;

use arkdown_core::ModuleJson5;
use arkdown_profile::FormWidgetAbilityMerger;
use arkdown_session::module::snapshot::{ModuleSnapshot, OhosModuleType};
use arkdown_session::ProjectSnapshot;
use indexmap::IndexMap;

#[test]
fn form_widget_abilities_come_from_the_immutable_project_snapshot() {
    let extension_json: ModuleJson5 = serde_json::from_value(serde_json::json!({
        "module": {
            "name": "entry",
            "type": "entry",
            "extensionAbilities": [
                {
                    "name": "FormExtension",
                    "type": "form",
                    "srcEntry": "./ets/FormExtension.ets",
                    "metadata": [
                        { "name": "ohos.extension.form", "resource": "$profile:form_config" }
                    ]
                },
                {
                    "name": "WorkerExtension",
                    "type": "service",
                    "srcEntry": "./ets/WorkerExtension.ets"
                }
            ]
        }
    }))
    .expect("extension module json");
    let current_json: ModuleJson5 = serde_json::from_value(serde_json::json!({
        "module": {
            "name": "widgets",
            "type": "shared",
            "formExtensionModule": "entry"
        }
    }))
    .expect("widget module json");
    let module = ModuleSnapshot {
        name: "entry".to_string(),
        root: PathBuf::from("/project/entry"),
        src_main: PathBuf::from("/project/entry/src/main"),
        module_json_path: None,
        module_json: Some(extension_json),
        module_json_value: None,
        syscap_value: None,
        ohos_test_module_json_value: None,
        module_type: OhosModuleType::Entry,
        package_path: None,
        package: None,
        source_roots: Vec::new(),
        resource_roots: Vec::new(),
        related_entry_modules: None,
    };
    let project = ProjectSnapshot {
        root: PathBuf::from("/project"),
        bundle_name: "com.example.app".to_string(),
        version_code: 1,
        version_name: "1.0.0".to_string(),
        build_version: None,
        min_compatible_version_code: None,
        bundle_type: "app".to_string(),
        product_name: "default".to_string(),
        module_order: vec!["entry".to_string(), "widgets".to_string()],
        modules: IndexMap::from([("entry".to_string(), Arc::new(module))]),
        root_package: None,
        dependency_graphs: IndexMap::new(),
    };

    let abilities = FormWidgetAbilityMerger::new(&project).collect(&current_json);

    assert_eq!(
        abilities,
        vec![serde_json::json!({
            "name": "FormExtension",
            "type": "form",
            "metadata": [
                { "name": "ohos.extension.form", "resource": "$profile:form_config" }
            ]
        })]
    );
}
