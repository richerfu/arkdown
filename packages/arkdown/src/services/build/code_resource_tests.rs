use std::path::PathBuf;

use arkdown_core::CopyCodeResource;

use super::code_resource::CodeResourceCopyPlan;

#[test]
fn copies_non_code_files_and_removes_stale_outputs() {
  let temp = tempfile::tempdir().expect("tempdir");
  let ets = temp.path().join("src/main/ets");
  let share = temp.path().join("src/main/share");
  let output = temp.path().join("build/loader_out/default/ets");
  fs_err::create_dir_all(ets.join("models")).expect("ets dir");
  fs_err::create_dir_all(&share).expect("share dir");
  fs_err::write(ets.join("Index.ets"), "export {};").expect("source");
  fs_err::write(ets.join("models/config.json"), "{}").expect("json");
  fs_err::write(share.join("schema.json"), "{}").expect("share json");

  let manifest = temp.path().join("build/cache/code-resources.json");
  let plan = CodeResourceCopyPlan::new(
    vec![ets.clone()],
    share,
    output.clone(),
    manifest.clone(),
    CopyCodeResource::default(),
  );
  let copied = plan.execute().expect("copy resources");
  assert_eq!(copied.len(), 2);
  assert!(output.join("models/config.json").is_file());
  assert!(output.join("../share/schema.json").is_file());
  assert!(!output.join("Index.ets").exists());

  fs_err::remove_file(ets.join("models/config.json")).expect("remove source");
  CodeResourceCopyPlan::new(
    vec![ets],
    PathBuf::from("missing-share"),
    output.clone(),
    manifest,
    CopyCodeResource::default(),
  )
  .execute()
  .expect("refresh resources");
  assert!(!output.join("models/config.json").exists());
}

#[test]
fn includes_are_authoritative_like_hvigor_copy_resources_util() {
  let temp = tempfile::tempdir().expect("tempdir");
  let ets = temp.path().join("ets");
  let output = temp.path().join("out");
  fs_err::create_dir_all(&ets).expect("ets dir");
  fs_err::write(ets.join("keep.data"), "keep").expect("keep");
  fs_err::write(ets.join("skip.json"), "skip").expect("skip");
  let keep_pattern = format!("{}/**/*.data", temp.path().to_string_lossy());

  CodeResourceCopyPlan::new(
    vec![ets],
    PathBuf::from("missing-share"),
    output.clone(),
    temp.path().join("manifest.json"),
    CopyCodeResource {
      enable: true,
      excludes: vec!["**/*.data".to_string()],
      includes: Some(vec![keep_pattern]),
    },
  )
  .execute()
  .expect("copy included resource");

  assert!(output.join("keep.data").is_file());
  assert!(!output.join("skip.json").exists());
}
