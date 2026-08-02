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
