use crate::clean::{CleanApplication, CleanRequest};

#[test]
fn clean_application_removes_project_and_module_outputs() {
    let temp = tempfile::tempdir().expect("project root");
    let module_root = temp.path().join("entry");
    std::fs::create_dir_all(module_root.join("build/default"))
        .expect("module build output fixture");
    std::fs::create_dir_all(module_root.join(".test/default")).expect("module test output fixture");
    std::fs::create_dir_all(module_root.join(".preview/default"))
        .expect("module preview output fixture");
    std::fs::create_dir_all(temp.path().join("build/default"))
        .expect("project build output fixture");
    std::fs::write(module_root.join("source.ets"), b"@Entry struct Main {}")
        .expect("source sentinel");
    std::fs::write(
        temp.path().join("build-profile.json5"),
        r#"{
      app: { products: [{ name: "default", signingConfig: "default" }] },
      modules: [{ name: "entry", srcPath: "./entry", targets: [{ name: "default" }] }]
    }"#,
    )
    .expect("project build profile");

    CleanApplication::new(CleanRequest {
        project: Some(temp.path().to_path_buf()),
    })
    .execute()
    .expect("clean project");

    assert!(!temp.path().join("build").exists());
    assert!(!module_root.join("build").exists());
    assert!(!module_root.join(".test").exists());
    assert!(!module_root.join(".preview").exists());
    assert!(module_root.join("source.ets").is_file());
}
