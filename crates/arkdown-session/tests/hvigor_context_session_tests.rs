use std::fs;

use arkdown_session::{HvigorContextModuleState, HvigorContextPathOptions, HvigorContextSession};
use serde_json::json;

fn session(root: &std::path::Path, product_is_explicit: bool) -> HvigorContextSession {
    let module_root = root.join("entry");
    HvigorContextSession::new(
        root,
        "default",
        product_is_explicit,
        json!({
            "app": {
                "products": [{
                    "name": "default",
                    "runtimeOS": "HarmonyOS"
                }]
            },
            "modules": [{ "name": "entry", "srcPath": "./entry" }]
        }),
        json!({ "name": "project", "version": "1.0.0" }),
        vec![HvigorContextModuleState {
            root: module_root,
            package: json!({ "name": "entry", "version": "1.0.0" }),
        }],
    )
}

#[test]
fn context_session_owns_hvigor_product_and_package_validation() {
    let project = tempfile::tempdir().expect("project tempdir");
    let module_root = project.path().join("entry");
    fs::create_dir_all(&module_root).expect("create module");
    let mut context = session(project.path(), false);

    let error = context
        .set_project_build_profile(json!({ "app": { "products": [] } }))
        .expect_err("product deletion must be rejected");
    assert_eq!(error.code, "CAN_NOT_ADD_OR_DELETE_PRODUCTS");

    let mutation = context
        .set_project_build_profile(json!({
            "app": {
                "products": [{
                    "name": "renamed",
                    "runtimeOS": "HarmonyOS"
                }]
            }
        }))
        .expect_err("product rename must be rejected");
    assert_eq!(mutation.code, "CAN_NOT_CHANGE_NAME_OR_RUNTIME_PRODUCTS");

    let error = context
        .set_project_package_field("dependencies", json!({ "broken": 1 }))
        .expect_err("non-string dependency must be rejected");
    assert_eq!(error.code, "DEPENDENCY_VERSION_IS_NOT_A_STRING");

    let error = context
        .set_module_version(&module_root, json!(8))
        .expect_err("non-string version must be rejected");
    assert_eq!(error.code, "VERSION_IS_NOT_A_STRING");

    context
        .set_module_package_field(&module_root, "dependencies", json!({ "dep": "2.0.0" }))
        .expect("valid dependency mutation");
    let overrides = context.runtime_overrides().expect("runtime overrides");
    assert_eq!(
        overrides
            .modules
            .get(&module_root)
            .and_then(|module| module.package.as_ref())
            .and_then(|package| package.pointer("/dependencies/dep")),
        Some(&json!("2.0.0"))
    );
}

#[test]
fn context_paths_match_inject_util_cache_and_output_branches() {
    let project = tempfile::tempdir().expect("project tempdir");
    let module_root = project.path().join("entry");
    fs::create_dir_all(&module_root).expect("create module");
    let cache = tempfile::tempdir().expect("cache tempdir");
    let context = session(project.path(), true);
    let project_name = project
        .path()
        .file_name()
        .expect("project name")
        .to_string_lossy();
    let options = HvigorContextPathOptions {
        build_cache_dir: Some(format!("\"{}\"", cache.path().display())),
        ..HvigorContextPathOptions::default()
    };

    let project_paths = context.project_paths(&options).expect("project paths");
    assert_eq!(project_paths.build_root_path, project.path().join("build"));

    let module_paths = context
        .module_target_paths(&module_root, "default", &options)
        .expect("module paths");
    assert_eq!(
        module_paths.module_build_path,
        fs::canonicalize(cache.path())
            .expect("canonical cache path")
            .join(project_name.as_ref())
            .join("entry")
            .join("build")
    );
    assert_eq!(
        module_paths.build_target_output_path,
        module_root
            .join("build")
            .join("default")
            .join("outputs")
            .join("default")
    );
    assert_eq!(module_paths.build_product_root_path, module_root);
}

#[test]
fn context_snapshot_is_invalidated_by_module_json_overlay() {
    let project = tempfile::tempdir().expect("project tempdir");
    let module_root = project.path().join("entry");
    fs::create_dir_all(module_root.join("src/main")).expect("create module source set");
    fs::write(
        project.path().join("build-profile.json5"),
        r#"{
          app: { products: [{ name: "default", runtimeOS: "HarmonyOS" }] },
          modules: [{ name: "entry", srcPath: "./entry" }]
        }"#,
    )
    .expect("write project profile");
    fs::write(
        module_root.join("src/main/module.json5"),
        r#"{ module: { name: "entry", type: "entry" } }"#,
    )
    .expect("write module profile");
    let mut context = session(project.path(), true);

    let before = context.all_module_name_hash().expect("initial module hash");
    context.set_module_json(
        &module_root,
        json!({ "module": { "name": "changed", "type": "entry" } }),
    );
    let after = context.all_module_name_hash().expect("updated module hash");

    assert_ne!(before, after);
}
