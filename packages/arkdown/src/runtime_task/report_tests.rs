use super::RuntimeTaskReportRenderer;

struct RuntimeTaskReportFixture;

impl RuntimeTaskReportFixture {
  fn render(request: serde_json::Value) -> String {
    RuntimeTaskReportRenderer::from_value(request)
      .unwrap()
      .render()
      .unwrap()
  }

  fn line_ending() -> &'static str {
    if cfg!(windows) {
      "\r\n"
    } else {
      "\n"
    }
  }
}

#[test]
fn tasks_report_uses_rust_metadata_and_plugin_authored_group_details() {
  let report = RuntimeTaskReportFixture::render(serde_json::json!({
    "nodeName": "entry",
    "projectTask": false,
    "kind": "hap",
    "reportKind": "tasks",
    "tasks": [
      {
        "nodeName": "entry",
        "taskName": "tasks",
        "builtIn": true,
        "projectTask": false,
        "realized": true,
        "group": "Help",
        "description": "Displays the tasks of the HvigorCoreNode.",
        "catalogOrder": 0,
        "realizationOrder": 0
      },
      {
        "nodeName": "entry",
        "taskName": "generateFixture",
        "builtIn": false,
        "projectTask": false,
        "realized": true,
        "group": "Fixture",
        "description": "Generate the fixture.",
        "realizationOrder": 1
      },
      {
        "nodeName": "entry",
        "taskName": "assembleHap",
        "builtIn": true,
        "projectTask": false,
        "realized": false,
        "group": "Hook",
        "catalogOrder": 2,
        "realizationOrder": 2
      }
    ]
  }));

  assert_eq!(
    report,
    [
      "--------------------------------------------------------------------------",
      "All tasks from hvigor node 'entry'",
      "--------------------------------------------------------------------------",
      "Help tasks",
      "-------------",
      "tasks - Displays the tasks of the HvigorCoreNode.",
      "taskTree - Displays the task dag tree of the HvigorCoreNode.",
      "",
      "Sync tasks",
      "-------------",
      "init - Sync and refresh project",
      "",
      "Fixture tasks",
      "-------------",
      "generateFixture - Generate the fixture.",
      "",
    ]
    .join(RuntimeTaskReportFixture::line_ending())
  );
}

#[test]
fn task_tree_report_preserves_dense_graph_order_and_cross_node_edges() {
  let report = RuntimeTaskReportFixture::render(serde_json::json!({
    "nodeName": "entry",
    "projectTask": false,
    "kind": "hap",
    "reportKind": "taskTree",
    "tasks": [
      {
        "nodeName": "entry",
        "taskName": "root",
        "builtIn": true,
        "projectTask": false,
        "group": "Hook",
        "catalogOrder": 0,
        "dependencies": ["entry:branch", "feature:assembleHap"]
      },
      {
        "nodeName": "entry",
        "taskName": "branch",
        "builtIn": true,
        "projectTask": false,
        "group": "Other",
        "catalogOrder": 1,
        "dependencies": ["entry:leaf"]
      },
      {
        "nodeName": "entry",
        "taskName": "leaf",
        "builtIn": true,
        "projectTask": false,
        "group": "Other",
        "catalogOrder": 2,
        "dependencies": []
      }
    ]
  }));

  assert_eq!(
    report,
    [
      "--------------------------------------------------------------------------",
      "All tasks dependencies from hvigor node 'entry'",
      "--------------------------------------------------------------------------",
      "",
      ":entry:root",
      "+--- :branch",
      "    \\--- :leaf",
      "\\--- :feature:assembleHap",
    ]
    .join(RuntimeTaskReportFixture::line_ending())
  );
}
