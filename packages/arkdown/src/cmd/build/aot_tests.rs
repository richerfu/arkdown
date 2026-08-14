use std::fs;

use super::aot::AotArtifactWriter;
use super::layout::ModuleBuildLayout;

#[test]
fn partial_aot_matches_sdk_placeholder_outputs_and_cache_contract() {
  let temp = tempfile::tempdir().expect("tempdir");
  let module_root = temp.path().join("entry");
  let layout = ModuleBuildLayout::new(&module_root);
  let output_dir = layout.aot_output_dir();
  let abc = layout.loader_out_dir().join("ets").join("modules.abc");
  let profile = module_root.join("modules.ap");
  fs::create_dir_all(abc.parent().expect("ABC parent")).expect("create ABC parent");
  fs::write(&abc, b"PAND").expect("write ABC");
  fs::write(&profile, b"profile").expect("write AP");

  let writer = AotArtifactWriter::new(
    "entry",
    Some("partial"),
    output_dir.to_str(),
    profile.to_str(),
    &layout,
    false,
  );
  assert!(!writer.cache_ready());
  writer.generate(&abc).expect("generate AOT placeholders");
  assert!(writer.cache_ready());
  assert_eq!(fs::metadata(output_dir.join("entry.an")).unwrap().len(), 0);
  assert_eq!(fs::metadata(output_dir.join("entry.ai")).unwrap().len(), 0);
}

#[test]
fn coverage_and_type_modes_do_not_require_aot_outputs() {
  let temp = tempfile::tempdir().expect("tempdir");
  let module_root = temp.path().join("entry");
  let layout = ModuleBuildLayout::new(&module_root);
  let missing_output = layout.aot_output_dir();
  let missing_abc = layout.loader_out_dir().join("ets").join("modules.abc");

  let coverage = AotArtifactWriter::new(
    "entry",
    Some("partial"),
    missing_output.to_str(),
    None,
    &layout,
    true,
  );
  assert!(coverage.cache_ready());
  coverage
    .generate(&missing_abc)
    .expect("coverage disables AOT generation");

  let type_mode = AotArtifactWriter::new(
    "entry",
    Some("type"),
    missing_output.to_str(),
    None,
    &layout,
    false,
  );
  assert!(type_mode.cache_ready());
  type_mode
    .generate(&missing_abc)
    .expect("type mode does not invoke the AOT compiler");
}

#[test]
fn partial_aot_rejects_a_missing_profile_before_writing_outputs() {
  let temp = tempfile::tempdir().expect("tempdir");
  let module_root = temp.path().join("entry");
  let layout = ModuleBuildLayout::new(&module_root);
  let output_dir = layout.aot_output_dir();
  let abc = layout.loader_out_dir().join("ets").join("modules.abc");
  let missing_profile = module_root.join("missing.ap");
  fs::create_dir_all(abc.parent().expect("ABC parent")).expect("create ABC parent");
  fs::write(&abc, b"PAND").expect("write ABC");

  let error = AotArtifactWriter::new(
    "entry",
    Some("partial"),
    output_dir.to_str(),
    missing_profile.to_str(),
    &layout,
    false,
  )
  .generate(&abc)
  .expect_err("missing AP must fail");
  assert!(error.to_string().contains("PARTIAL_INVALID_PATH"));
  assert!(!output_dir.exists());
}
