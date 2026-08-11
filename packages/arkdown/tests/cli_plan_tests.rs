use arkdown_napi::cli_plan::{CliPathList, HvigorAnalyzeMode, PackageCliParser};

fn args(values: &[&str]) -> Vec<String> {
  values.iter().map(|value| (*value).to_string()).collect()
}

#[test]
fn latest_hvigor_cli_parsing_stays_in_rust() {
  let config = PackageCliParser::parse_hvigor(&args(&[
    "assembleHap",
    "--mode",
    "module",
    "-p",
    "module=entry@phone",
    "-p",
    "buildMode=release",
    "--no-daemon",
    "--analyze=verbose",
    "--max-old-space-size=0x400",
  ]))
  .expect("latest IDE command should parse");

  assert_eq!(config.tasks, ["assembleHap"]);
  assert_eq!(config.execution_mode.as_deref(), Some("module"));
  assert_eq!(config.module.as_deref(), Some("entry@phone"));
  assert_eq!(config.mode.as_deref(), Some("release"));
  assert_eq!(config.daemon, Some(false));
  assert_eq!(
    config.analyze,
    Some(HvigorAnalyzeMode::Named("advanced".to_string()))
  );
  assert_eq!(config.max_old_space_size, Some(1024.0));
}

#[test]
fn latest_hvigor_build_version_contract_is_typed() {
  let error = PackageCliParser::parse_hvigor(&args(&["assembleHap", "-p", "buildVersion=01.2"]))
    .expect_err("leading zero must be rejected");

  assert_eq!(error.code, "INVALID_BUILDVERSION_PARAMETER");
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
fn hvigor_test_injects_exact_default_properties_and_module_mode() {
  let config = PackageCliParser::parse_hvigor(&args(&["test"]))
    .expect("hvigor local-test command should parse");

  assert_eq!(config.execution_mode.as_deref(), Some("module"));
  assert_eq!(
    config.prop,
    [
      "pageType=page",
      "product=default",
      "buildRoot=.test",
      "unitTestMode=true",
      "isLocalTest=true",
      "ohos-test-coverage=true",
      "unit.test.replace.page=../../../.test/testability/pages/Index",
    ]
  );
  assert_eq!(config.ohos_test_coverage, Some(true));
  assert_eq!(
    config.unit_test_replace_page.as_deref(),
    Some("../../../.test/testability/pages/Index")
  );
}

#[test]
fn hvigor_test_preserves_first_property_order_and_explicit_overrides() {
  let config = PackageCliParser::parse_hvigor(&args(&[
    "test",
    "-p",
    "coverage=false",
    "-p",
    "scope=EntryAbilityTest",
    "-p",
    "coverage-mode=full",
  ]))
  .expect("hvigor local-test overrides should parse");

  assert_eq!(config.prop[0], "ohos-test-coverage=false");
  assert_eq!(config.prop[1], "scope=EntryAbilityTest");
  assert_eq!(config.prop[2], "coverage-mode=full");
  assert_eq!(config.ohos_test_coverage, Some(false));
  assert_eq!(config.scope.as_deref(), Some("EntryAbilityTest"));
  assert_eq!(config.coverage_mode.as_deref(), Some("full"));
}

#[test]
fn hvigor_test_property_map_matches_split_and_undefined_semantics() {
  let config = PackageCliParser::parse_hvigor(&args(&[
    "test",
    "-p",
    "scope=Suite=Case",
    "-p",
    "customFlag",
  ]))
  .expect("hvigor local-test properties should parse");

  assert!(config.prop.iter().any(|property| property == "scope=Suite"));
  assert!(config
    .prop
    .iter()
    .any(|property| property == "customFlag=undefined"));
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
