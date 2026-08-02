use serde_json::{json, Value};

use super::RuntimeCommandPlanner;

fn task(
  path: &str,
  node_name: &str,
  task_name: &str,
  built_in: bool,
  project_task: bool,
  dependencies: &[&str],
) -> Value {
  json!({
    "path": path,
    "nodeName": node_name,
    "taskName": task_name,
    "builtIn": built_in,
    "projectTask": project_task,
    "enabled": true,
    "trackState": true,
    "refreshDeclarations": false,
    "hasBeforeRun": false,
    "hasAfterRun": false,
    "hasFailed": false,
    "hasTaskShouldDo": false,
    "graphBoundCustom": !built_in && !dependencies.is_empty(),
    "dependencies": dependencies,
    "userDependencies": [],
    "inputProperties": [],
    "inputFiles": [],
    "outputFiles": []
  })
}

fn plan(request: Value) -> Value {
  let response = RuntimeCommandPlanner::response_json(&request.to_string()).unwrap();
  serde_json::from_str(&response).unwrap()
}

#[test]
fn selects_module_target_and_groups_assembly_in_rust() {
  let result = plan(json!({
    "projectName": "demo",
    "executionMode": "module",
    "taskNames": ["assembleHap"],
    "moduleSelections": [{ "moduleName": "entry", "targetName": "release" }],
    "buildTargetName": "default",
    "nodes": [
      {
        "name": "demo",
        "kind": "app",
        "projectTask": true,
        "childNames": ["entry"]
      },
      { "name": "entry", "kind": "hap", "projectTask": false }
    ],
    "tasks": [
      task(":assembleApp", "demo", "assembleApp", true, true, &[]),
      task("entry:assembleHap", "entry", "assembleHap", true, false, &[])
    ]
  }));

  assert_eq!(
    result["plan"]["tasks"],
    json!([{
      "taskPath": "entry:assembleHap",
      "commandKind": "assembly"
    }])
  );
  assert_eq!(result["plan"]["selectedNodeNames"], json!(["entry"]));
  assert_eq!(
    result["plan"]["nativeRequests"],
    json!([{
      "target": "hap",
      "module": "entry",
      "targetName": "release",
      "nodeNames": ["entry"],
      "rootTaskPaths": ["entry:assembleHap"],
      "cutoffTaskRoots": []
    }])
  );
}

#[test]
fn adds_custom_task_native_dependencies_and_cutoff_roots() {
  let result = plan(json!({
    "projectName": "demo",
    "taskNames": ["publish"],
    "buildTargetName": "default",
    "nodes": [{ "name": "entry", "kind": "hap", "projectTask": false }],
    "tasks": [
      task(
        "entry:publish",
        "entry",
        "publish",
        false,
        false,
        &["entry:helper"]
      ),
      task(
        "entry:helper",
        "entry",
        "helper",
        false,
        false,
        &["entry:default@CompileArkTS"]
      ),
      task(
        "entry:default@CompileArkTS",
        "entry",
        "default@CompileArkTS",
        true,
        false,
        &[]
      )
    ]
  }));

  assert_eq!(
    result["plan"]["tasks"],
    json!([
      { "taskPath": "entry:publish", "commandKind": "custom" },
      {
        "taskPath": "entry:default@CompileArkTS",
        "commandKind": "native",
        "dependencyOnly": true
      }
    ])
  );
  assert_eq!(
    result["plan"]["nativeRequests"],
    json!([{
      "target": "hap",
      "module": "entry",
      "targetName": "default",
      "nodeNames": ["entry"],
      "rootTaskPaths": ["entry:default@CompileArkTS"],
      "cutoffTaskRoots": [{
        "moduleName": "entry",
        "targetName": "default",
        "taskName": "CompileArkTS"
      }]
    }])
  );
}

#[test]
fn preserves_latest_hvigor_first_sync_order() {
  let result = plan(json!({
    "projectName": "demo",
    "sync": true,
    "syncVersionChanged": true,
    "nodes": [
      {
        "name": "demo",
        "kind": "app",
        "projectTask": true,
        "customSyncTaskPaths": []
      },
      {
        "name": "entry",
        "kind": "hap",
        "projectTask": false,
        "customSyncTaskPaths": ["entry:pluginSync"]
      }
    ],
    "tasks": [
      task(":clean", "demo", "clean", true, true, &[]),
      task(":init", "demo", "init", true, true, &[]),
      task("entry:clean", "entry", "clean", true, false, &[]),
      task("entry:init", "entry", "init", true, false, &[]),
      task("entry:pluginSync", "entry", "pluginSync", false, false, &[])
    ]
  }));

  assert_eq!(
    result["plan"]["tasks"],
    json!([
      { "taskPath": "entry:clean", "commandKind": "clean" },
      { "taskPath": ":clean", "commandKind": "clean" },
      { "taskPath": "entry:init", "commandKind": "sync" },
      { "taskPath": "entry:pluginSync", "commandKind": "custom" },
      { "taskPath": ":init", "commandKind": "sync" }
    ])
  );
  assert_eq!(
    result["plan"]["selectedNodeNames"],
    json!(["entry", "demo"])
  );
}

#[test]
fn returns_structured_hvigor_diagnostics() {
  let unknown_mode = plan(json!({
    "projectName": "demo",
    "executionMode": "worker",
    "taskNames": ["assembleHap"],
    "nodes": [],
    "tasks": []
  }));
  assert_eq!(
    unknown_mode["error"]["code"],
    "UNKNOWN_HVIGOR_EXECUTION_MODE"
  );
  assert_eq!(unknown_mode["error"]["executionMode"], "worker");

  let missing_task = plan(json!({
    "projectName": "demo",
    "taskNames": [],
    "nodes": [],
    "tasks": []
  }));
  assert_eq!(missing_task["error"]["code"], "HVIGOR_TASK_NOT_FOUND");
  assert_eq!(
    missing_task["error"]["message"],
    "Task [] was not found in the project demo. Invalid command to execute the build task, please verify the parameters in the command and try again."
  );
}
