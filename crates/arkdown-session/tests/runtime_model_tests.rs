use arkdown_session::{
    BuildScriptFileInventory, BuildScriptFileInventoryRequest, BuildScriptModuleModelLoader,
    BuildScriptModuleModelRequest, BuildScriptRuntimeModelError, BuildScriptRuntimeModelLoader,
    BuildScriptRuntimeModelRequest,
};
use serde_json::json;

#[test]
fn runtime_model_reads_project_and_module_json5_once_in_rust() {
    let temp = tempfile::tempdir().expect("tempdir");
    let project_root = temp.path().join("project");
    let user_home = temp.path().join("user-home");
    let module_root = project_root.join("entry");
    std::fs::create_dir_all(module_root.join("src/main")).expect("create module");
    std::fs::create_dir_all(project_root.join("AppScope")).expect("create AppScope");
    std::fs::create_dir_all(project_root.join("hvigor")).expect("create hvigor config dir");
    std::fs::create_dir_all(&user_home).expect("create user home");

    std::fs::write(
        project_root.join("build-profile.json5"),
        r#"{
            // JSON5 comments and trailing commas are intentionally covered.
            app: { products: [{ name: 'tablet', }], },
            modules: [{ name: 'entry', srcPath: './entry', targets: ['phone'] }],
        }"#,
    )
    .expect("write project profile");
    std::fs::write(
        project_root.join("oh-package.json5"),
        r#"{ name: 'project-package' }"#,
    )
    .expect("write project package");
    std::fs::write(
        project_root.join("package.json"),
        r#"{ "name": "project-npm-package" }"#,
    )
    .expect("write project npm package");
    std::fs::write(
        project_root.join("AppScope/app.json5"),
        r#"{ app: { bundleName: 'com.example.native-model' } }"#,
    )
    .expect("write app model");
    std::fs::write(
        project_root.join("hvigor/hvigor-config.json5"),
        r#"{ properties: { projectFlag: true } }"#,
    )
    .expect("write project hvigor config");
    std::fs::write(project_root.join("hvigorconfig.mjs"), "export default {}")
        .expect("write lower-priority build config");
    std::fs::write(project_root.join("hvigorconfig.ts"), "export default {}")
        .expect("write build config");
    std::fs::write(
        user_home.join("hvigor-config.json5"),
        r#"{ properties: { userFlag: 7 } }"#,
    )
    .expect("write user hvigor config");
    std::fs::write(
        module_root.join("build-profile.json5"),
        r#"{ buildOption: { arkOptions: { runtimeOnly: true } } }"#,
    )
    .expect("write module profile");
    std::fs::write(
        module_root.join("src/main/module.json5"),
        r#"{ module: { type: 'entry' } }"#,
    )
    .expect("write module model");
    std::fs::write(
        module_root.join("oh-package.json5"),
        r#"{ name: 'entry-package' }"#,
    )
    .expect("write module package");
    std::fs::write(
        module_root.join("package.json"),
        r#"{ "name": "entry-npm-package" }"#,
    )
    .expect("write module npm package");

    let snapshot = BuildScriptRuntimeModelLoader::new(BuildScriptRuntimeModelRequest {
        project_root: project_root.clone(),
        hvigor_user_home: Some(user_home),
    })
    .load()
    .expect("load runtime model");

    assert_eq!(
        snapshot.project_profile["app"]["products"][0]["name"],
        "tablet"
    );
    assert_eq!(
        snapshot
            .build_config_file
            .as_ref()
            .expect("build config")
            .file_name()
            .expect("build config file name"),
        "hvigorconfig.ts"
    );
    assert_eq!(snapshot.project_package["name"], "project-package");
    assert_eq!(snapshot.project_npm_package["name"], "project-npm-package");
    assert!(snapshot.project_oh_package_exists);
    assert!(snapshot.project_npm_package_exists);
    assert_eq!(
        snapshot.app_json.as_ref().expect("app model")["app"]["bundleName"],
        "com.example.native-model"
    );
    assert_eq!(
        snapshot.project_hvigor_config["properties"]["projectFlag"],
        true
    );
    assert_eq!(
        snapshot
            .project_hvigor_config_path
            .as_ref()
            .expect("project hvigor config")
            .file_name()
            .expect("config file name"),
        "hvigor-config.json5"
    );
    assert_eq!(snapshot.user_hvigor_config["properties"]["userFlag"], 7);
    assert!(snapshot.fallback_module_roots.is_empty());
    assert_eq!(snapshot.modules.len(), 1);
    let module = &snapshot.modules[0];
    assert_eq!(module.name, "entry");
    assert_eq!(module.src_path, "./entry");
    assert_eq!(module.root, module_root);
    assert_eq!(module.descriptor["targets"][0], "phone");
    assert_eq!(
        module.build_profile["buildOption"]["arkOptions"]["runtimeOnly"],
        true
    );
    assert_eq!(
        module.module_json.as_ref().expect("module model")["module"]["type"],
        "entry"
    );
    assert_eq!(module.package["name"], "entry-package");
    assert_eq!(module.npm_package["name"], "entry-npm-package");
}

#[test]
fn runtime_model_fallback_and_hvigorfile_selection_are_deterministic() {
    let temp = tempfile::tempdir().expect("tempdir");
    for name in ["zeta", "alpha", "node_modules", ".hidden"] {
        std::fs::create_dir_all(temp.path().join(name)).expect("create child");
    }
    std::fs::write(
        temp.path().join("alpha/hvigorfile.mjs"),
        "export default {}",
    )
    .expect("write alpha mjs");
    std::fs::write(temp.path().join("alpha/hvigorfile.ts"), "export default {}")
        .expect("write alpha ts");
    std::fs::write(
        temp.path().join("zeta/hvigorfile.js"),
        "module.exports = {}",
    )
    .expect("write zeta js");
    std::fs::write(
        temp.path().join("node_modules/hvigorfile.ts"),
        "export default {}",
    )
    .expect("write ignored dependency script");
    std::fs::write(
        temp.path().join(".hidden/hvigorfile.ts"),
        "export default {}",
    )
    .expect("write ignored hidden script");

    let snapshot = BuildScriptRuntimeModelLoader::new(BuildScriptRuntimeModelRequest {
        project_root: temp.path().to_path_buf(),
        hvigor_user_home: None,
    })
    .load()
    .expect("load empty project model");
    let fallback_names = snapshot
        .fallback_module_roots
        .iter()
        .map(|root| {
            root.file_name()
                .expect("root name")
                .to_string_lossy()
                .into_owned()
        })
        .collect::<Vec<_>>();
    assert_eq!(fallback_names, ["alpha", "zeta"]);

    let files = BuildScriptFileInventory::new(BuildScriptFileInventoryRequest {
        roots: snapshot.fallback_module_roots,
    })
    .discover()
    .expect("discover scripts");
    assert_eq!(files.len(), 2);
    assert_eq!(files[0].file_name().expect("alpha file"), "hvigorfile.ts");
    assert_eq!(files[1].file_name().expect("zeta file"), "hvigorfile.js");
}

#[test]
fn module_runtime_model_supports_hvigorconfig_include_descriptors() {
    let temp = tempfile::tempdir().expect("tempdir");
    let module_root = temp.path().join("feature");
    std::fs::create_dir_all(&module_root).expect("create feature");
    std::fs::write(
        module_root.join("build-profile.json5"),
        r#"{ targets: [{ name: 'default' }] }"#,
    )
    .expect("write module profile");

    let snapshot = BuildScriptModuleModelLoader::new(BuildScriptModuleModelRequest {
        project_root: temp.path().to_path_buf(),
        descriptor: json!({
            "name": "feature",
            "srcPath": "feature",
            "extraOptions": { "fromConfig": true }
        }),
    })
    .load()
    .expect("load included module");

    assert_eq!(snapshot.root, module_root);
    assert_eq!(snapshot.build_profile["targets"][0]["name"], "default");
    assert_eq!(snapshot.descriptor["extraOptions"]["fromConfig"], true);
    assert!(snapshot.module_json.is_none());
    assert_eq!(snapshot.package, json!({}));
}

#[test]
fn runtime_model_rejects_declarative_compile_plugin_before_js_execution() {
    let temp = tempfile::tempdir().expect("tempdir");
    std::fs::write(
        temp.path().join("build-profile.json5"),
        r#"{
            app: { products: [{ name: 'default', buildOption: { arkOptions: {
                compilePluginFile: './build/hvigor_compile/plugin.ts'
            } } }] }
        }"#,
    )
    .expect("write invalid project profile");

    let error = BuildScriptRuntimeModelLoader::new(BuildScriptRuntimeModelRequest {
        project_root: temp.path().to_path_buf(),
        hvigor_user_home: None,
    })
    .load()
    .expect_err("Rust model inventory must own latest-hvigor schema validation");

    let BuildScriptRuntimeModelError::Schema(diagnostic) = error else {
        panic!("expected typed schema diagnostic, got {error}");
    };
    assert_eq!(diagnostic.code, "HVIGOR_SCHEMA_COMPILE_PLUGIN_FILE");
    assert_eq!(
        diagnostic.field_path,
        "app.products[0].buildOption.arkOptions.compilePluginFile"
    );
}
