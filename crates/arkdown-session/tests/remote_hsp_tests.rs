use arkdown_session::{RemoteHspObjectError, RemoteHspObjectInventory, RemoteHspObjectRequest};

fn write_remote_hsp(root: &std::path::Path, directory: &str, file: &str, package: &str) {
    let directory = root.join("oh_modules").join(".hsp").join(directory);
    std::fs::create_dir_all(&directory).expect("remote HSP directory");
    std::fs::write(directory.join(file), "HSP").expect("remote HSP");
    std::fs::write(directory.join("oh-package.json5"), package).expect("remote package");
}

#[test]
fn project_remote_hsp_replaces_module_copy_only_for_higher_semver() {
    let root = tempfile::tempdir().expect("project");
    let module = root.path().join("entry");
    std::fs::create_dir_all(&module).expect("module");
    write_remote_hsp(
        &module,
        "module-copy",
        "shared.hsp",
        r#"{"name":"shared","version":"1.0.0"}"#,
    );
    write_remote_hsp(
        root.path(),
        "project-copy",
        "shared.hsp",
        r#"{"name":"shared","version":"2.0.0","metadata":{"integratedHsp":true}}"#,
    );

    let objects = RemoteHspObjectInventory::new(RemoteHspObjectRequest {
        module_root: module,
        project_root: root.path().to_path_buf(),
    })
    .inspect()
    .expect("remote HSP inventory");

    assert_eq!(objects.len(), 1);
    assert_eq!(objects[0].hsp_name.as_deref(), Some("shared"));
    assert_eq!(objects[0].hsp_version.as_ref(), "2.0.0");
    assert_eq!(objects[0].hsp_dir_name.as_ref(), "project-copy");
    assert!(objects[0].is_integrated_hsp);
}

#[test]
fn multiple_hsp_files_preserve_the_official_typed_rejection() {
    let root = tempfile::tempdir().expect("project");
    let directory = root.path().join("entry/oh_modules/.hsp/invalid-package");
    std::fs::create_dir_all(&directory).expect("remote HSP directory");
    std::fs::write(directory.join("one.hsp"), "HSP").expect("first HSP");
    std::fs::write(directory.join("two.hsp"), "HSP").expect("second HSP");
    std::fs::write(
        directory.join("oh-package.json5"),
        r#"{"name":"invalid","version":"1.0.0"}"#,
    )
    .expect("package");

    let error = RemoteHspObjectInventory::new(RemoteHspObjectRequest {
        module_root: root.path().join("entry"),
        project_root: root.path().to_path_buf(),
    })
    .inspect()
    .expect_err("multiple HSPs must fail");

    assert!(matches!(
        error,
        RemoteHspObjectError::AllowOneHsp { path } if path == directory
    ));
}
