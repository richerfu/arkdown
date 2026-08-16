use serde_json::{json, Value};

use super::{catalog::RuntimeTaskCatalog, RuntimeCommandPlanner};

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
  RuntimeCommandPlanner::response_value(request).unwrap()
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
      "commandKind": "assembly",
      "executionLane": "native"
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
      { "taskPath": "entry:publish", "commandKind": "custom", "executionLane": "custom" },
      {
        "taskPath": "entry:default@CompileArkTS",
        "commandKind": "native",
        "executionLane": "native",
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
fn maps_hvigor_test_to_the_local_test_output_target() {
  let result = plan(json!({
    "projectName": "demo",
    "executionMode": "module",
    "taskNames": ["test"],
    "moduleSelections": [{ "moduleName": "entry" }],
    "buildTargetName": "default",
    "nodes": [{ "name": "entry", "kind": "hap", "projectTask": false }],
    "tasks": [
      task(
        "entry:test",
        "entry",
        "test",
        true,
        false,
        &["entry:default@GenerateUnitTestResult"]
      ),
      task(
        "entry:default@GenerateUnitTestResult",
        "entry",
        "default@GenerateUnitTestResult",
        true,
        false,
        &["entry:default@UnitTestArkTS"]
      ),
      task(
        "entry:default@UnitTestArkTS",
        "entry",
        "default@UnitTestArkTS",
        true,
        false,
        &[]
      )
    ]
  }));

  assert_eq!(
    result["plan"]["tasks"],
    json!([{
      "taskPath": "entry:test",
      "commandKind": "native",
      "executionLane": "native"
    }])
  );
  assert_eq!(
    result["plan"]["nativeRequests"],
    json!([{
      "target": "hap",
      "outputTarget": "test",
      "module": "entry",
      "targetName": "default",
      "nodeNames": ["entry"],
      "rootTaskPaths": ["entry:test"],
      "cutoffTaskRoots": [{
        "moduleName": "entry",
        "taskName": "test"
      }]
    }])
  );
}

#[test]
fn maps_har_lint_to_the_native_standalone_checker_boundary() {
  let result = plan(json!({
    "projectName": "demo",
    "executionMode": "module",
    "taskNames": ["default@LintArkTS"],
    "moduleSelections": [{ "moduleName": "library", "targetName": "default" }],
    "nodes": [{ "name": "library", "kind": "har", "projectTask": false }],
    "tasks": [
      task(
        "library:default@LintArkTS",
        "library",
        "default@LintArkTS",
        true,
        false,
        &[]
      )
    ]
  }));

  assert_eq!(
    result["plan"]["nativeRequests"],
    json!([{
      "target": "har",
      "module": "library",
      "targetName": "default",
      "nodeNames": ["library"],
      "rootTaskPaths": ["library:default@LintArkTS"],
      "cutoffTaskRoots": [{
        "moduleName": "library",
        "targetName": "default",
        "taskName": "LintArkTS"
      }]
    }])
  );
}

#[test]
fn maps_registered_compile_har_and_remote_hsp_actions_to_native_boundaries() {
  for action in [
    "BuildArkTS",
    "CompileJS",
    "HarBuildArkTS",
    "ProcessObfuscationFiles",
    "ProcessOHPackageJson",
    "ProcessHarArtifacts",
    "SignProjectRemoteHsp",
    "SignModuleRemoteHsp",
  ] {
    let task_name = format!("default@{action}");
    let task_path = format!("library:{task_name}");
    let result = plan(json!({
      "projectName": "demo",
      "executionMode": "module",
      "taskNames": [task_name],
      "moduleSelections": [{ "moduleName": "library", "targetName": "default" }],
      "nodes": [{ "name": "library", "kind": "har", "projectTask": false }],
      "tasks": [task(&task_path, "library", &format!("default@{action}"), true, false, &[])]
    }));

    assert_eq!(result["error"], Value::Null, "{action} must be executable");
    assert_eq!(
      result["plan"]["tasks"][0]["commandKind"], "native",
      "{action} must use the native coordinator"
    );
  }
}

#[test]
fn maps_assemble_dev_hqf_to_the_native_quickfix_lifecycle() {
  let result = plan(json!({
    "projectName": "demo",
    "executionMode": "module",
    "taskNames": ["assembleDevHqf"],
    "moduleSelections": [{ "moduleName": "entry", "targetName": "default" }],
    "buildTargetName": "default",
    "nodes": [{ "name": "entry", "kind": "hap", "projectTask": false }],
    "tasks": [
      task(
        "entry:assembleDevHqf",
        "entry",
        "assembleDevHqf",
        true,
        false,
        &["entry:default@SignHqf"]
      ),
      task(
        "entry:default@SignHqf",
        "entry",
        "default@SignHqf",
        true,
        false,
        &[]
      )
    ]
  }));

  assert_eq!(
    result["plan"]["tasks"],
    json!([{
      "taskPath": "entry:assembleDevHqf",
      "commandKind": "native",
      "executionLane": "native"
    }])
  );
  assert_eq!(
    result["plan"]["nativeRequests"],
    json!([{
      "target": "hap",
      "module": "entry",
      "targetName": "default",
      "nodeNames": ["entry"],
      "rootTaskPaths": ["entry:assembleDevHqf"],
      "cutoffTaskRoots": [{
        "moduleName": "entry",
        "taskName": "assembleDevHqf"
      }]
    }])
  );
}

#[test]
fn maps_cold_reload_arkts_to_target_scoped_native_compilation() {
  let result = plan(json!({
    "projectName": "demo",
    "executionMode": "module",
    "taskNames": ["default@ColdReloadArkTS"],
    "moduleSelections": [{ "moduleName": "entry", "targetName": "default" }],
    "buildTargetName": "default",
    "nodes": [{ "name": "entry", "kind": "hap", "projectTask": false }],
    "tasks": [task(
      "entry:default@ColdReloadArkTS",
      "entry",
      "default@ColdReloadArkTS",
      true,
      false,
      &[]
    )]
  }));

  assert_eq!(result["error"], serde_json::Value::Null);
  assert_eq!(
    result["plan"]["nativeRequests"][0]["cutoffTaskRoots"],
    json!([{
      "moduleName": "entry",
      "targetName": "default",
      "taskName": "ColdReloadArkTS"
    }])
  );
}

#[test]
fn projects_hsp_and_har_hap_actions_to_hap_without_changing_the_owner_kind() {
  for (kind, action) in [
    ("hsp", "PackageHap"),
    ("hsp", "SignHap"),
    ("hsp", "genOnDeviceTestHap"),
    ("har", "PackageHap"),
    ("har", "SignHap"),
    ("har", "genOnDeviceTestHap"),
  ] {
    let task_name = if action == "genOnDeviceTestHap" {
      action.to_string()
    } else {
      format!("default@{action}")
    };
    let task_path = format!("library:{task_name}");
    let result = plan(json!({
      "projectName": "demo",
      "executionMode": "module",
      "taskNames": [task_name],
      "moduleSelections": [{ "moduleName": "library", "targetName": "default" }],
      "nodes": [{ "name": "library", "kind": kind, "projectTask": false }],
      "tasks": [task(&task_path, "library", &task_path["library:".len()..], true, false, &[])]
    }));

    assert_eq!(result["error"], Value::Null, "{kind}:{action}");
    assert_eq!(
      result["plan"]["nativeRequests"][0]["target"], kind,
      "the runtime graph owner remains the {kind} module"
    );
    assert_eq!(
      result["plan"]["nativeRequests"][0]["outputTarget"], "hap",
      "{kind}:{action} must execute the HAP packager"
    );
  }
}

#[test]
fn accepts_every_registered_latest_hvigor_builtin_command_boundary() {
  let catalogs = [
    json!({
      "nodeName": "demo",
      "kind": "app",
      "projectTask": true,
      "childModules": [
        { "name": "entry", "kind": "hap" },
        { "name": "shared", "kind": "hsp" }
      ]
    }),
    maximal_module_catalog("hap", false),
    maximal_module_catalog("hap", true),
    maximal_module_catalog("hsp", false),
    maximal_module_catalog("hsp", true),
    maximal_module_catalog("har", false),
  ];

  for catalog_request in catalogs {
    let project_task = catalog_request["projectTask"].as_bool().unwrap_or(false);
    let kind = catalog_request["kind"].as_str().unwrap().to_string();
    let node_name = catalog_request["nodeName"].as_str().unwrap().to_string();
    let entries = RuntimeTaskCatalog::from_value(catalog_request)
      .and_then(RuntimeTaskCatalog::entries_value)
      .expect("latest Hvigor catalog");
    let entries = serde_json::from_value::<Vec<Value>>(entries).expect("catalog records");

    for entry in entries {
      let task_name = entry["name"].as_str().expect("catalog task name");
      let task_path = if project_task {
        format!(":{task_name}")
      } else {
        format!("{node_name}:{task_name}")
      };
      let request = json!({
        "projectName": "demo",
        "executionMode": if project_task { "project" } else { "module" },
        "taskNames": [task_name],
        "moduleSelections": if project_task {
          json!([])
        } else {
          json!([{ "moduleName": &node_name, "targetName": "default" }])
        },
        "nodes": [{
          "name": &node_name,
          "kind": &kind,
          "projectTask": project_task,
          "childNames": if project_task { json!(["entry", "shared"]) } else { json!([]) }
        }],
        "tasks": [task(&task_path, &node_name, task_name, true, project_task, &[])]
      });
      let result = plan(request);
      assert_eq!(
        result["error"],
        Value::Null,
        "registered task {kind}:{task_name} must have an executable command boundary: {result}"
      );
    }
  }
}

fn maximal_module_catalog(kind: &str, fast_build_app: bool) -> Value {
  json!({
    "nodeName": "library",
    "kind": kind,
    "projectTask": false,
    "buildTargetName": "default",
    "targetNames": ["default", "ohosTest"],
    "targetOptions": [
      {
        "name": "default",
        "deduplicateHar": true,
        "binxoEnabled": true,
        "ohpmProject": false,
        "compileApiVersion": 12,
        "compatibleApiVersion": 12,
        "projectRemoteHspExists": true,
        "moduleRemoteHspExists": true,
        "ohosTestSourceExists": true,
        "ohosTestTemplatesAfterMigration": true
      },
      {
        "name": "ohosTest",
        "deduplicateHar": true,
        "binxoEnabled": true,
        "ohpmProject": false,
        "compileApiVersion": 12,
        "compatibleApiVersion": 12,
        "ohosTestSourceExists": true,
        "ohosTestTemplatesAfterMigration": true
      }
    ],
    "fastBuildApp": fast_build_app,
    "commandEntryTest": true,
    "unitTestProcess": true,
    "ohosTestCoverage": true,
    "previewProcess": true,
    "previewCompileResourceIncremental": true,
    "inTest": true,
    "deviceTest": true,
    "hotReload": true,
    "coldReload": true
  })
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
      { "taskPath": "entry:clean", "commandKind": "clean", "executionLane": "standalone" },
      { "taskPath": ":clean", "commandKind": "clean", "executionLane": "standalone" },
      { "taskPath": "entry:init", "commandKind": "sync", "executionLane": "standalone" },
      { "taskPath": "entry:pluginSync", "commandKind": "custom", "executionLane": "syncCustom" },
      { "taskPath": ":init", "commandKind": "sync", "executionLane": "standalone" }
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
