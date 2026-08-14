use super::{HvigorTaskInfoCatalog, RuntimeTaskNodeKind};

#[test]
fn latest_sync_task_inventory_preserves_kind_order_and_version_clean() {
  for (kind, expected_count, assembly, package) in [
    (
      RuntimeTaskNodeKind::Hap,
      83,
      "assembleHap",
      "default@PackageHap",
    ),
    (
      RuntimeTaskNodeKind::Hsp,
      89,
      "assembleHsp",
      "default@PackageHsp",
    ),
    (
      RuntimeTaskNodeKind::Har,
      89,
      "assembleHar",
      "default@PackageHar",
    ),
  ] {
    let names = HvigorTaskInfoCatalog::module_task_names(kind, "default", true, false);
    assert_eq!(names.len(), expected_count);
    assert_eq!(names.first().map(String::as_str), Some(assembly));
    assert!(names.iter().any(|name| name == package));
    assert_eq!(names.last().map(String::as_str), Some("clean"));

    let repeated = HvigorTaskInfoCatalog::module_task_names(kind, "default", false, false);
    assert_eq!(repeated.len(), expected_count - 1);
    assert_eq!(repeated.last().map(String::as_str), Some("init"));
  }
}

#[test]
fn every_latest_sync_task_has_rust_owned_metadata() {
  for kind in [
    RuntimeTaskNodeKind::Hap,
    RuntimeTaskNodeKind::Hsp,
    RuntimeTaskNodeKind::Har,
  ] {
    for name in HvigorTaskInfoCatalog::module_task_names(kind, "ohosTest", true, true) {
      HvigorTaskInfoCatalog::task_info(&name).unwrap();
    }
  }
  for name in HvigorTaskInfoCatalog::project_task_names() {
    HvigorTaskInfoCatalog::task_info(name).unwrap();
  }
}

#[test]
fn har_deduplicate_tasks_are_registered_only_for_enabled_module_kinds() {
  let hap =
    HvigorTaskInfoCatalog::module_task_names(RuntimeTaskNodeKind::Hap, "default", false, true);
  assert!(hap
    .iter()
    .any(|name| name == "default@ProcessIdDefinedFile"));
  assert!(!hap
    .iter()
    .any(|name| name == "default@CollectDuplicateHarEntriesArkTS"));

  let hsp =
    HvigorTaskInfoCatalog::module_task_names(RuntimeTaskNodeKind::Hsp, "default", false, true);
  assert!(hsp
    .iter()
    .any(|name| name == "default@ProcessIdDefinedFile"));
  assert!(hsp
    .iter()
    .any(|name| name == "default@CollectDuplicateHarEntriesArkTS"));

  let har =
    HvigorTaskInfoCatalog::module_task_names(RuntimeTaskNodeKind::Har, "default", false, true);
  assert!(har
    .iter()
    .any(|name| name == "default@ProcessIdDefinedFile"));
  assert!(!har
    .iter()
    .any(|name| name == "default@CollectDuplicateHarEntriesArkTS"));
}

#[test]
fn task_info_keeps_latest_hvigor_metadata_shape() {
  let assembly = HvigorTaskInfoCatalog::task_info("default@CompileArkTS").unwrap();
  assert_eq!(assembly.task_name, "default@CompileArkTS");
  assert_eq!(assembly.group, "ArkTS");
  assert_eq!(
    assembly.description,
    "Compile ArkTS or JS components for large-system devices in the stage model."
  );
  assert_eq!(assembly.source_plugin_id, "hvigor-ohos-plugin");
}
