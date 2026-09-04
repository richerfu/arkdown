use std::fs;

use arkdown_napi::services::clean::{self, CleanRequest};

#[test]
fn clean_removes_every_rust_owned_project_and_module_build_root() {
  let temp = tempfile::tempdir().expect("temporary clean project");
  let project = temp.path().join("project");
  let module = project.join("entry");
  fs::create_dir_all(&module).expect("create module root");
  fs::write(
    project.join("build-profile.json5"),
    r#"{
      app: {
        signingConfigs: [],
        products: [{
          name: "default",
          targetSdkVersion: "5.0.5(17)",
          compatibleSdkVersion: "5.0.5(17)",
          runtimeOS: "HarmonyOS"
        }],
        buildModeSet: [{ name: "debug" }]
      },
      modules: [{ name: "entry", srcPath: "./entry", targets: [{ name: "default" }] }]
    }"#,
  )
  .expect("write project profile");

  for output in [
    project.join("build"),
    module.join("build"),
    module.join(".test"),
    module.join(".preview"),
  ] {
    fs::create_dir_all(&output).expect("create generated output");
    fs::write(output.join("artifact.bin"), b"generated").expect("write generated output");
  }
  fs::write(module.join("source.ets"), b"@Entry struct Main {}").expect("write source sentinel");

  clean::run(CleanRequest {
    project: Some(project.clone()),
  })
  .expect("clean command should succeed without requiring an SDK");

  assert!(!project.join("build").exists());
  assert!(!module.join("build").exists());
  assert!(!module.join(".test").exists());
  assert!(!module.join(".preview").exists());
  assert!(module.join("source.ets").is_file());
}
