use serde::Serialize;

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(untagged)]
pub enum CliPathList {
  One(String),
  Many(Vec<String>),
}

impl CliPathList {
  fn push(&mut self, value: String) {
    match self {
      Self::One(first) => *self = Self::Many(vec![std::mem::take(first), value]),
      Self::Many(values) => values.push(value),
    }
  }
}

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ArkdownBuildCliConfig {
  #[serde(skip_serializing_if = "Option::is_none")]
  pub project: Option<String>,
  #[serde(skip_serializing_if = "Option::is_none")]
  pub target: Option<String>,
  #[serde(skip_serializing_if = "Option::is_none")]
  pub mode: Option<String>,
  #[serde(skip_serializing_if = "Option::is_none")]
  pub module: Option<String>,
  #[serde(skip_serializing_if = "Option::is_none")]
  pub product: Option<String>,
  #[serde(skip_serializing_if = "Option::is_none")]
  pub build_target_name: Option<String>,
  #[serde(skip_serializing_if = "Option::is_none")]
  pub build_version: Option<String>,
  #[serde(skip_serializing_if = "Option::is_none")]
  pub arkts_version: Option<String>,
  #[serde(skip_serializing_if = "Option::is_none")]
  pub ohpm_mode: Option<String>,
  #[serde(skip_serializing_if = "Option::is_none")]
  pub watch: Option<bool>,
  #[serde(skip_serializing_if = "Option::is_none")]
  pub preview: Option<bool>,
  #[serde(skip_serializing_if = "Option::is_none")]
  pub check_entry: Option<bool>,
  #[serde(skip_serializing_if = "Option::is_none")]
  pub coverage: Option<bool>,
  #[serde(skip_serializing_if = "Option::is_none")]
  pub scope: Option<String>,
  #[serde(skip_serializing_if = "Option::is_none")]
  pub ohos_test_coverage: Option<bool>,
  #[serde(skip_serializing_if = "Option::is_none")]
  pub coverage_path_filter: Option<String>,
  #[serde(skip_serializing_if = "Option::is_none")]
  pub coverage_mode: Option<String>,
  #[serde(skip_serializing_if = "Option::is_none")]
  pub unit_test_replace_page: Option<String>,
  #[serde(skip_serializing_if = "Option::is_none")]
  pub ohos_debug_asan: Option<bool>,
  #[serde(skip_serializing_if = "Option::is_none")]
  pub ohos_enable_hwasan: Option<bool>,
  #[serde(skip_serializing_if = "Option::is_none")]
  pub ohos_enable_tsan: Option<bool>,
  #[serde(skip_serializing_if = "Option::is_none")]
  pub ohos_enable_ubsan: Option<bool>,
  #[serde(skip_serializing_if = "Option::is_none")]
  pub ohos_enable_binxo: Option<bool>,
  #[serde(skip_serializing_if = "Option::is_none")]
  pub enable_zig: Option<bool>,
  #[serde(skip_serializing_if = "Option::is_none")]
  pub disable_abc_cache: Option<bool>,
  #[serde(skip_serializing_if = "Option::is_none")]
  pub cache_file: Option<String>,
  #[serde(skip_serializing_if = "Option::is_none")]
  pub checker: Option<String>,
  #[serde(skip_serializing_if = "Option::is_none")]
  pub hvigor_plugin_file: Option<CliPathList>,
  #[serde(skip_serializing_if = "Option::is_none")]
  pub compile_plugin_path: Option<CliPathList>,
  #[serde(skip_serializing_if = "Option::is_none")]
  pub cache: Option<bool>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct CliDiagnostic {
  pub code: String,
  pub message: String,
}

impl CliDiagnostic {
  fn new(code: &str, message: impl Into<String>) -> Self {
    Self {
      code: code.to_string(),
      message: message.into(),
    }
  }
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct CliParseEnvelope<T> {
  #[serde(skip_serializing_if = "Option::is_none")]
  config: Option<T>,
  #[serde(skip_serializing_if = "Option::is_none")]
  error: Option<CliDiagnostic>,
}

impl<T: Serialize> CliParseEnvelope<T> {
  fn from_result(result: Result<T, CliDiagnostic>) -> Self {
    match result {
      Ok(config) => Self {
        config: Some(config),
        error: None,
      },
      Err(error) => Self {
        config: None,
        error: Some(error),
      },
    }
  }

  fn to_json(&self) -> Result<String, serde_json::Error> {
    serde_json::to_string(self)
  }
}

/// Rust owner for npm CLI argument semantics. The JavaScript executable keeps
/// only stdout/stderr and calls into the live plugin runtime after this typed
/// plan has been produced.
pub struct PackageCliParser;

impl PackageCliParser {
  pub fn parse_build(args: &[String]) -> Result<ArkdownBuildCliConfig, CliDiagnostic> {
    ArkdownBuildArgsParser::new(args).parse()
  }

  pub fn parse_build_json(args: &[String]) -> Result<String, serde_json::Error> {
    CliParseEnvelope::from_result(Self::parse_build(args)).to_json()
  }
}

struct ArkdownBuildArgsParser<'a> {
  args: &'a [String],
  config: ArkdownBuildCliConfig,
}

impl<'a> ArkdownBuildArgsParser<'a> {
  fn new(args: &'a [String]) -> Self {
    Self {
      args,
      config: ArkdownBuildCliConfig::default(),
    }
  }

  fn parse(mut self) -> Result<ArkdownBuildCliConfig, CliDiagnostic> {
    let mut index = 0;
    while index < self.args.len() {
      let arg = &self.args[index];
      match arg.as_str() {
        "--help" | "-h" => return Ok(self.config),
        "--project" => self.config.project = Some(self.required_value(&mut index, arg)?),
        "--target" => self.config.target = Some(self.required_value(&mut index, arg)?),
        "--mode" => self.config.mode = Some(self.required_value(&mut index, arg)?),
        "--module" | "-m" => self.config.module = Some(self.required_value(&mut index, arg)?),
        "--product" => self.config.product = Some(self.required_value(&mut index, arg)?),
        "--build-target-name" => {
          self.config.build_target_name = Some(self.required_value(&mut index, arg)?)
        }
        "--build-version" => {
          self.config.build_version = Some(self.required_value(&mut index, arg)?)
        }
        "--arkts-version" => {
          let value = self.required_value(&mut index, arg)?;
          if !matches!(value.as_str(), "1.1" | "1.2") {
            return Err(CliDiagnostic::new(
              "INVALID_ARKTS_LANGUAGE_VERSION",
              format!("{arg} must be 1.1 or 1.2: {value}"),
            ));
          }
          self.config.arkts_version = Some(value);
        }
        "--ohpm-mode" => {
          let value = self.required_value(&mut index, arg)?;
          if !matches!(value.as_str(), "offline" | "prefer-offline" | "online") {
            return Err(CliDiagnostic::new(
              "INVALID_OHPM_ACQUISITION_MODE",
              format!("{arg} must be offline, prefer-offline, or online: {value}"),
            ));
          }
          self.config.ohpm_mode = Some(value);
        }
        "--watch" => self.config.watch = Some(true),
        "--preview" => self.config.preview = Some(true),
        "--check-entry" => self.config.check_entry = Some(true),
        "--coverage" => self.config.coverage = Some(true),
        "--scope" => self.config.scope = Some(self.required_value(&mut index, arg)?),
        "--ohos-test-coverage" => {
          let value = self.required_value(&mut index, arg)?;
          self.config.ohos_test_coverage = Some(self.boolean(&value, arg)?);
        }
        "--coverage-path-filter" => {
          self.config.coverage_path_filter = Some(self.required_value(&mut index, arg)?)
        }
        "--coverage-mode" => {
          let value = self.required_value(&mut index, arg)?;
          if !matches!(value.as_str(), "black" | "white" | "full") {
            return Err(CliDiagnostic::new(
              "INVALID_ARKDOWN_COVERAGE_MODE",
              format!("{arg} must be black, white, or full: {value}"),
            ));
          }
          self.config.coverage_mode = Some(value);
        }
        "--unit-test-replace-page" => {
          self.config.unit_test_replace_page = Some(self.required_value(&mut index, arg)?)
        }
        "--ohos-debug-asan" => self.config.ohos_debug_asan = Some(true),
        "--ohos-enable-hwasan" => self.config.ohos_enable_hwasan = Some(true),
        "--ohos-enable-tsan" => self.config.ohos_enable_tsan = Some(true),
        "--ohos-enable-ubsan" => self.config.ohos_enable_ubsan = Some(true),
        "--ohos-enable-binxo" => self.config.ohos_enable_binxo = Some(true),
        "--enable-zig" => self.config.enable_zig = Some(true),
        "--disable-abc-cache" => self.config.disable_abc_cache = Some(true),
        "--cache-file" => self.config.cache_file = Some(self.required_value(&mut index, arg)?),
        "--checker" => self.config.checker = Some(self.required_value(&mut index, arg)?),
        "--hvigor-plugin-file" => {
          let value = self.required_value(&mut index, arg)?;
          Self::append_path(&mut self.config.hvigor_plugin_file, value);
        }
        "--compile-plugin-path" => {
          let value = self.required_value(&mut index, arg)?;
          Self::append_path(&mut self.config.compile_plugin_path, value);
        }
        "--cache" => self.config.cache = Some(true),
        "--no-cache" => self.config.cache = Some(false),
        _ => {
          return Err(CliDiagnostic::new(
            "UNKNOWN_ARKDOWN_BUILD_OPTION",
            format!("unknown build option '{arg}'"),
          ));
        }
      }
      index += 1;
    }
    Ok(self.config)
  }

  fn required_value(&self, index: &mut usize, option: &str) -> Result<String, CliDiagnostic> {
    *index += 1;
    self
      .args
      .get(*index)
      .filter(|value| !value.is_empty() && !value.starts_with('-'))
      .cloned()
      .ok_or_else(|| {
        CliDiagnostic::new(
          "MISSING_ARKDOWN_BUILD_OPTION_VALUE",
          format!("{option} requires a value"),
        )
      })
  }

  fn append_path(paths: &mut Option<CliPathList>, value: String) {
    match paths {
      Some(paths) => paths.push(value),
      None => *paths = Some(CliPathList::One(value)),
    }
  }

  fn boolean(&self, value: &str, option: &str) -> Result<bool, CliDiagnostic> {
    match value {
      "true" => Ok(true),
      "false" => Ok(false),
      _ => Err(CliDiagnostic::new(
        "INVALID_ARKDOWN_BOOLEAN_OPTION",
        format!("{option} must be true or false: {value}"),
      )),
    }
  }
}
