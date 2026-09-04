use std::path::Path;
use std::sync::Arc;

use arkdown_session::dependency_graph::snapshot::{
    DependencyGraphSnapshot, DependencyNodeSnapshot, DependencyNodeType,
};
use arkdown_session::module::snapshot::{ModuleSnapshot, OhosModuleType};
use arkdown_session::{DuplicateDependencyValidator, PackageSnapshot, ProjectSnapshot};
use indexmap::IndexMap;

#[test]
fn strict_app_check_reports_local_and_remote_hars_shared_by_hsps() {
    let temp = tempfile::tempdir().unwrap();
    let root = temp.path();
    let local_har = root.join("common");
    let remote_har = root.join("oh_modules/remote");
    for path in [
        &local_har,
        &remote_har,
        &root.join("shared-a"),
        &root.join("shared-b"),
    ] {
        std::fs::create_dir_all(path).unwrap();
    }
    std::fs::write(
        root.join("oh-package.json5"),
        r#"{ name: "app", dependencies: { common: "FILE:./common" } }"#,
    )
    .unwrap();

    let mut modules = IndexMap::new();
    modules.insert(
        "common".to_string(),
        Arc::new(module("common", &local_har, OhosModuleType::Har)),
    );
    modules.insert(
        "shared-a".to_string(),
        Arc::new(module(
            "shared-a",
            &root.join("shared-a"),
            OhosModuleType::Shared,
        )),
    );
    modules.insert(
        "shared-b".to_string(),
        Arc::new(module(
            "shared-b",
            &root.join("shared-b"),
            OhosModuleType::Shared,
        )),
    );
    let mut dependency_graphs = IndexMap::new();
    for hsp in ["shared-a", "shared-b"] {
        dependency_graphs.insert(hsp.to_string(), Arc::new(graph(hsp, &remote_har)));
    }
    let project = ProjectSnapshot {
        root: root.to_path_buf(),
        bundle_name: "com.example.app".to_string(),
        version_code: 1,
        version_name: "1.0.0".to_string(),
        build_version: None,
        min_compatible_version_code: None,
        bundle_type: "app".to_string(),
        product_name: "default".to_string(),
        module_order: vec![
            "common".to_string(),
            "shared-a".to_string(),
            "shared-b".to_string(),
        ],
        modules,
        root_package: Some(Arc::new(
            PackageSnapshot::from_root(root)
                .unwrap()
                .expect("root package"),
        )),
        dependency_graphs,
    };

    let error = DuplicateDependencyValidator::new(&project)
        .validate()
        .expect_err("both HSP modules share two HAR dependencies");

    assert_eq!(error.duplicates().len(), 2);
    assert!(error.duplicates().iter().any(|duplicate| {
        duplicate.package_name == "common" && duplicate.hsp_modules == ["shared-a", "shared-b"]
    }));
    assert!(error.duplicates().iter().any(|duplicate| {
        duplicate.package_name == "@remote/har" && duplicate.hsp_modules == ["shared-a", "shared-b"]
    }));
    let message = error.to_string();
    assert!(message.contains("FOUND_HSPS_THAT_HAVE_THE_SAME_HAR_DEPENDENCIES"));
    assert!(message.contains("[shared-a, shared-b] depend on the same HAR [common]."));
}

fn module(name: &str, root: &Path, module_type: OhosModuleType) -> ModuleSnapshot {
    ModuleSnapshot {
        name: name.to_string(),
        root: root.to_path_buf(),
        src_main: root.join("src/main"),
        module_json_path: None,
        module_json: None,
        module_json_value: None,
        syscap_value: None,
        ohos_test_module_json_value: None,
        module_type,
        package_path: None,
        package: None,
        source_roots: Vec::new(),
        resource_roots: Vec::new(),
        related_entry_modules: None,
    }
}

fn graph(host_module: &str, remote_har: &Path) -> DependencyGraphSnapshot {
    let dependency = DependencyNodeSnapshot {
        alias: "remote".to_string(),
        package_name: "@remote/har".to_string(),
        version_spec: Some("1.0.0".to_string()),
        package_root: Some(remote_har.to_path_buf()),
        package: None,
        module_profile: None,
        node_type: DependencyNodeType::SourceHar,
        is_runtime_only: false,
        is_dynamic: false,
        is_transitive: false,
        direct_deps: Vec::new(),
    };
    DependencyGraphSnapshot {
        host_module: host_module.to_string(),
        nodes: [(dependency.package_name.clone(), dependency)]
            .into_iter()
            .collect(),
        ..Default::default()
    }
}
