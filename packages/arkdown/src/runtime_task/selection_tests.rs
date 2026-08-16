use super::model::RuntimeTaskSelection;
use super::selection::{RuntimeDirectTaskCyclePlanner, RuntimeTaskSelectionSession};
use serde_json::json;

fn session() -> RuntimeTaskSelectionSession {
  RuntimeTaskSelectionSession::new("workspace".into(), true)
}

fn selection(
  session: &RuntimeTaskSelectionSession,
  validate_boundaries: bool,
) -> RuntimeTaskSelection {
  serde_json::from_value(
    session
      .selection(validate_boundaries)
      .expect("selection value"),
  )
  .expect("selection model")
}

#[test]
fn command_cycle_keeps_hvigor_root_roles_and_stable_node_order() {
  let mut session = session();
  session.prepare_command_cycle(
    vec!["entry".into(), "shared".into(), "entry".into()],
    vec!["entry:custom".into()],
    vec!["entry:assembleHap".into()],
  );

  let selection = selection(&session, true);
  assert_eq!(
    selection.expected_root_paths,
    Some(vec!["entry:assembleHap".into(), "entry:custom".into()])
  );
  assert_eq!(selection.required_boundary_paths, vec!["entry:assembleHap"]);
  assert_eq!(selection.command_root_paths, vec!["entry:custom"]);
  assert_eq!(selection.execution_node_names, vec!["entry", "shared"]);
  assert!(selection.validate_boundaries);
}

#[test]
fn clearing_a_cycle_restores_unselected_build_defaults() {
  let mut session = session();
  session.prepare_runtime_cycle(vec![":tasks".into()], vec!["app".into()]);
  session.clear();

  let selection = selection(&session, false);
  assert_eq!(selection.expected_root_paths, None);
  assert!(selection.required_boundary_paths.is_empty());
  assert!(selection.command_root_paths.is_empty());
  assert!(selection.execution_node_names.is_empty());
  assert!(!selection.validate_boundaries);
  assert!(selection.cache_enabled);
}

#[test]
fn direct_cycle_uses_rust_module_and_project_selection() {
  let model = json!({
    "nodes": [
      {
        "name": "demo",
        "kind": "app",
        "projectTask": true,
        "childNames": ["entry", "feature"]
      },
      { "name": "entry", "kind": "hap", "projectTask": false },
      { "name": "feature", "kind": "hap", "projectTask": false },
      { "name": "shared", "kind": "hsp", "projectTask": false }
    ]
  });
  let module_cycle =
    RuntimeDirectTaskCyclePlanner::from_value("hap", vec!["feature".to_string()], model.clone())
      .expect("module planner")
      .plan();
  assert_eq!(module_cycle.root_paths, ["feature:assembleHap"]);
  assert_eq!(module_cycle.execution_node_names, ["feature"]);

  let app_cycle = RuntimeDirectTaskCyclePlanner::from_value("app", Vec::new(), model)
    .expect("project planner")
    .plan();
  assert_eq!(app_cycle.root_paths, [":assembleApp"]);
  assert_eq!(app_cycle.execution_node_names, ["entry", "feature", "demo"]);
}
