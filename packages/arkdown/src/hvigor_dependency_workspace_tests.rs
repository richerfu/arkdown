use std::collections::BTreeMap;
use std::path::Path;

use serde_json::json;

use super::{
  HvigorPackageManagerKind, HvigorPluginWorkspaceError, HvigorPluginWorkspacePlan,
  HvigorPluginWorkspaceRequest, HvigorPluginWorkspaceRunner,
};

fn request(
  project_root: &Path,
  hvigor_user_home: &Path,
  project_config: serde_json::Value,
  user_config: serde_json::Value,
) -> HvigorPluginWorkspaceRequest {
  HvigorPluginWorkspaceRequest {
    project_root: project_root.to_path_buf(),
    hvigor_user_home: hvigor_user_home.to_path_buf(),
    home_dir: project_root.join("home"),
    process_executable: project_root.join("bin").join("node"),
    node_version: "v22.0.0".to_string(),
    project_config,
    user_config,
  }
}

#[test]
fn local_plugin_dependencies_are_normalized_from_the_project_hvigor_directory() {
  let temp = tempfile::tempdir().unwrap();
  let project_root = temp.path().join("project");
  let hvigor_user_home = temp.path().join("user-home");
  std::fs::create_dir_all(project_root.join("hvigor")).unwrap();

  let plan = HvigorPluginWorkspacePlan::from_request(&request(
    &project_root,
    &hvigor_user_home,
    json!({
      "dependencies": {
        "file-plugin": "file:./plugins/file-plugin",
        "archive-plugin": "./plugins/archive-plugin.tgz",
        "registry-plugin": "1.2.3"
      }
    }),
    json!({}),
  ))
  .unwrap();

  assert_eq!(
    plan.dependencies,
    BTreeMap::from([
      (
        "archive-plugin".to_string(),
        project_root
          .join("hvigor/plugins/archive-plugin.tgz")
          .to_str()
          .unwrap()
          .to_string(),
      ),
      (
        "file-plugin".to_string(),
        project_root
          .join("hvigor/plugins/file-plugin")
          .to_str()
          .unwrap()
          .to_string(),
      ),
      ("registry-plugin".to_string(), "1.2.3".to_string()),
    ])
  );
}

#[test]
fn project_package_manager_property_wins_before_user_fallback() {
  let temp = tempfile::tempdir().unwrap();
  let project_root = temp.path().join("project");
  let hvigor_user_home = temp.path().join("user-home");

  let project_plan = HvigorPluginWorkspacePlan::from_request(&request(
    &project_root,
    &hvigor_user_home,
    json!({ "properties": { "hvigor.dependency.useNpm": false } }),
    json!({ "properties": { "hvigor.dependency.useNpm": true } }),
  ))
  .unwrap();
  assert_eq!(
    project_plan.package_manager_kind(),
    HvigorPackageManagerKind::Pnpm
  );

  let user_plan = HvigorPluginWorkspacePlan::from_request(&request(
    &project_root,
    &hvigor_user_home,
    json!({}),
    json!({ "properties": { "hvigor.dependency.useNpm": true } }),
  ))
  .unwrap();
  assert_eq!(
    user_plan.package_manager_kind(),
    HvigorPackageManagerKind::Npm
  );

  let non_null_project_plan = HvigorPluginWorkspacePlan::from_request(&request(
    &project_root,
    &hvigor_user_home,
    json!({ "properties": false }),
    json!({ "properties": { "hvigor.dependency.useNpm": true } }),
  ))
  .unwrap();
  assert_eq!(
    non_null_project_plan.package_manager_kind(),
    HvigorPackageManagerKind::Pnpm
  );

  let mut legacy_node_request = request(
    &project_root,
    &hvigor_user_home,
    json!({ "properties": { "hvigor.dependency.useNpm": false } }),
    json!({}),
  );
  legacy_node_request.node_version = "v14.21.3".to_string();
  let legacy_node_plan = HvigorPluginWorkspacePlan::from_request(&legacy_node_request).unwrap();
  assert_eq!(
    legacy_node_plan.package_manager_kind(),
    HvigorPackageManagerKind::Npm
  );
}

#[test]
fn null_dependency_map_matches_hvigor_empty_dependency_default() {
  let temp = tempfile::tempdir().unwrap();
  let project_root = temp.path().join("project");
  let hvigor_user_home = temp.path().join("user-home");

  let plan = HvigorPluginWorkspacePlan::from_request(&request(
    &project_root,
    &hvigor_user_home,
    json!({ "dependencies": null }),
    json!({}),
  ))
  .unwrap();

  assert!(plan.dependencies.is_empty());
}

#[test]
fn workspace_cache_requires_matching_specs_and_a_resolvable_package_entry() {
  let temp = tempfile::tempdir().unwrap();
  let project_root = temp.path().join("project");
  let hvigor_user_home = temp.path().join("user-home");
  let config = json!({ "dependencies": { "fixture-plugin": "1.0.0" } });
  let plan = HvigorPluginWorkspacePlan::from_request(&request(
    &project_root,
    &hvigor_user_home,
    config.clone(),
    json!({}),
  ))
  .unwrap();
  let package_root = plan.node_modules().join("fixture-plugin");
  std::fs::create_dir_all(package_root.join("dist")).unwrap();
  std::fs::write(
    plan.workspace.join("package.json"),
    serde_json::to_vec(&json!({ "dependencies": plan.dependencies })).unwrap(),
  )
  .unwrap();
  std::fs::write(
    package_root.join("package.json"),
    serde_json::to_vec(&json!({
      "name": "fixture-plugin",
      "exports": { ".": { "require": "./dist/plugin.cjs" } }
    }))
    .unwrap(),
  )
  .unwrap();
  std::fs::write(package_root.join("dist/plugin.cjs"), "module.exports = {};").unwrap();

  let runner = HvigorPluginWorkspaceRunner {
    request: request(&project_root, &hvigor_user_home, config, json!({})),
  };
  assert!(runner.workspace_is_current(&plan).unwrap());

  std::fs::write(
    plan.workspace.join("package.json"),
    serde_json::to_vec(&json!({ "dependencies": { "fixture-plugin": "2.0.0" } })).unwrap(),
  )
  .unwrap();
  assert!(!runner.workspace_is_current(&plan).unwrap());
}

#[test]
fn non_string_dependency_is_rejected_before_workspace_mutation() {
  let temp = tempfile::tempdir().unwrap();
  let project_root = temp.path().join("project");
  let hvigor_user_home = temp.path().join("user-home");
  let error = HvigorPluginWorkspacePlan::from_request(&request(
    &project_root,
    &hvigor_user_home,
    json!({ "dependencies": { "fixture-plugin": 7 } }),
    json!({}),
  ))
  .unwrap_err();

  assert!(matches!(
    error,
    HvigorPluginWorkspaceError::InvalidDependency { ref name, .. }
      if name == "fixture-plugin"
  ));
  assert!(!hvigor_user_home.exists());
}

#[test]
fn failed_install_removes_the_partial_project_workspace() {
  let temp = tempfile::tempdir().unwrap();
  let project_root = temp.path().join("project");
  let hvigor_user_home = temp.path().join("user-home");
  let config = json!({
    "properties": { "hvigor.dependency.useNpm": true },
    "dependencies": { "fixture-plugin": "1.0.0" }
  });
  let request = request(&project_root, &hvigor_user_home, config, json!({}));
  let plan = HvigorPluginWorkspacePlan::from_request(&request).unwrap();
  let workspace = plan.workspace.clone();
  let error = HvigorPluginWorkspaceRunner { request }
    .execute()
    .unwrap_err();

  assert!(matches!(
    error,
    HvigorPluginWorkspaceError::PackageManagerStart { .. }
  ));
  assert!(!workspace.exists());
}
