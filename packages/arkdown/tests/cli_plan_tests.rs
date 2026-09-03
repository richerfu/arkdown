use arkdown_napi::cli_plan::{CliPathList, PackageCliParser};

fn args(values: &[&str]) -> Vec<String> {
  values.iter().map(|value| (*value).to_string()).collect()
}

#[test]
fn direct_build_repeated_plugin_paths_keep_scalar_then_array_shape() {
  let one = PackageCliParser::parse_build(&args(&["--compile-plugin-path", "./compile-a.js"]))
    .expect("one plugin path should parse");
  assert_eq!(
    one.compile_plugin_path,
    Some(CliPathList::One("./compile-a.js".to_string()))
  );

  let many = PackageCliParser::parse_build(&args(&[
    "--compile-plugin-path",
    "./compile-a.js",
    "--compile-plugin-path",
    "./compile-b.js",
  ]))
  .expect("repeated plugin paths should parse");
  assert_eq!(
    many.compile_plugin_path,
    Some(CliPathList::Many(vec![
      "./compile-a.js".to_string(),
      "./compile-b.js".to_string(),
    ]))
  );
}

#[test]
fn direct_build_enables_the_typed_zig_native_toolchain() {
  let config = PackageCliParser::parse_build(&args(&["--enable-zig"]))
    .expect("Zig native toolchain flag should parse");

  assert_eq!(config.enable_zig, Some(true));
}

#[test]
fn direct_build_selects_arkts_1_2_only_from_the_explicit_flag() {
  let defaults = PackageCliParser::parse_build(&args(&[])).expect("default build args");
  assert_eq!(defaults.arkts_version, None);

  let selected = PackageCliParser::parse_build(&args(&["--arkts-version", "1.2"]))
    .expect("explicit ArkTS 1.2 should parse");
  assert_eq!(selected.arkts_version.as_deref(), Some("1.2"));

  let error = PackageCliParser::parse_build(&args(&["--arkts-version", "latest"]))
    .expect_err("implicit or symbolic language versions must be rejected");
  assert_eq!(error.code, "INVALID_ARKTS_LANGUAGE_VERSION");
}

#[test]
fn direct_build_forwards_the_preview_check_entry_override() {
  let defaults = PackageCliParser::parse_build(&args(&[])).expect("default build args");
  assert_eq!(defaults.check_entry, None);

  let selected =
    PackageCliParser::parse_build(&args(&["--check-entry"])).expect("check-entry should parse");
  assert_eq!(selected.check_entry, Some(true));
}

#[test]
fn direct_build_accepts_typed_local_test_options() {
  let config = PackageCliParser::parse_build(&args(&[
    "--target",
    "test",
    "--scope",
    "EntryAbilityTest",
    "--ohos-test-coverage",
    "false",
    "--coverage-path-filter",
    "src/main/ets",
    "--coverage-mode",
    "black",
    "--unit-test-replace-page",
    "../../../generated/pages/Index",
  ]))
  .expect("typed local-test options should parse");

  assert_eq!(config.target.as_deref(), Some("test"));
  assert_eq!(config.scope.as_deref(), Some("EntryAbilityTest"));
  assert_eq!(config.ohos_test_coverage, Some(false));
  assert_eq!(config.coverage_path_filter.as_deref(), Some("src/main/ets"));
  assert_eq!(config.coverage_mode.as_deref(), Some("black"));
  assert_eq!(
    config.unit_test_replace_page.as_deref(),
    Some("../../../generated/pages/Index")
  );
}
