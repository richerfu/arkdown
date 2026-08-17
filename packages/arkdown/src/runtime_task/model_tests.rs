use super::model::{RuntimeTaskDescriptor, RuntimeTaskModelBuilder};

struct RuntimeTaskModelFixture;

impl RuntimeTaskModelFixture {
  fn task(
    path: &str,
    node_name: &str,
    project_task: bool,
    dependencies: &[&str],
  ) -> RuntimeTaskDescriptor {
    RuntimeTaskDescriptor {
      path: path.to_string(),
      node_name: node_name.to_string(),
      task_name: path.rsplit(':').next().unwrap_or(path).to_string(),
      built_in: false,
      project_task,
      lazy: false,
      enabled: true,
      track_state: true,
      refresh_declarations: false,
      has_before_run: false,
      has_after_run: false,
      has_failed: false,
      has_task_should_do: false,
      graph_bound_custom: true,
      dependencies: dependencies
        .iter()
        .map(|value| (*value).to_string())
        .collect(),
      user_dependencies: Vec::new(),
      input_properties: Vec::new(),
      input_files: Vec::new(),
      output_files: Vec::new(),
    }
  }
}

#[test]
fn qualifies_live_overlay_dependencies_in_the_rust_task_model() {
  let tasks = RuntimeTaskModelBuilder::new(
    Vec::new(),
    vec![
      RuntimeTaskModelFixture::task("entry:custom", "entry", false, &["prepare", "shared:build"]),
      RuntimeTaskModelFixture::task(":projectCustom", "project", true, &["prepare"]),
    ],
    Vec::new(),
  )
  .build()
  .expect("build task model");

  let module = tasks
    .iter()
    .find(|task| task.path == "entry:custom")
    .unwrap();
  assert_eq!(module.dependencies, ["entry:prepare", "shared:build"]);
  let project = tasks
    .iter()
    .find(|task| task.path == ":projectCustom")
    .unwrap();
  assert_eq!(project.dependencies, [":prepare"]);
}
