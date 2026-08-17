//! Tests for the custom-task engine spawn short-circuit (CLAUDE.md rule 12
//! narrow exemption): builds with only built-in tasks and no active plugin
//! callbacks must not pay the actor/engine cost.

use super::bridge::needs_runtime_work;
use super::model::RuntimeTaskDescriptor;
use rustc_hash::FxHashSet;

fn descriptor(path: &str, built_in: bool) -> RuntimeTaskDescriptor {
  RuntimeTaskDescriptor {
    path: path.to_string(),
    node_name: "entry".to_string(),
    task_name: path.rsplit(':').next().unwrap_or(path).to_string(),
    built_in,
    project_task: false,
    lazy: false,
    enabled: true,
    track_state: true,
    refresh_declarations: false,
    has_before_run: false,
    has_after_run: false,
    has_failed: false,
    has_task_should_do: false,
    graph_bound_custom: false,
    dependencies: Vec::new(),
    user_dependencies: Vec::new(),
    input_properties: Vec::new(),
    input_files: Vec::new(),
    output_files: Vec::new(),
  }
}

#[test]
fn built_in_only_model_without_callbacks_skips_engine() {
  let tasks = vec![
    descriptor("entry:default@CompileArkTS", true),
    descriptor("entry:default@PackageHap", true),
  ];
  assert!(!needs_runtime_work(&tasks, &FxHashSet::default(), false));
}

#[test]
fn custom_task_requires_engine() {
  let tasks = vec![
    descriptor("entry:default@CompileArkTS", true),
    descriptor("entry:default@myCustomTask", false),
  ];
  assert!(needs_runtime_work(&tasks, &FxHashSet::default(), false));
}

#[test]
fn command_modifier_or_abc_consumer_requires_engine() {
  let tasks = vec![descriptor("entry:default@CompileArkTS", true)];
  let mut modifier_types = FxHashSet::default();
  modifier_types.insert(3u8);
  assert!(needs_runtime_work(&tasks, &modifier_types, false));
  assert!(needs_runtime_work(&tasks, &FxHashSet::default(), true));
}
