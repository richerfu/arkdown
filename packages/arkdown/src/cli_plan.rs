use serde::Serialize;

#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct HvigorCliConfig {
  pub tasks: Vec<String>,
  pub prop: Vec<String>,
  #[serde(skip_serializing_if = "Option::is_none")]
  pub log_level: Option<String>,
  #[serde(skip_serializing_if = "Option::is_none")]
  pub sync: Option<bool>,
  #[serde(skip_serializing_if = "Option::is_none")]
  pub watch: Option<bool>,
  #[serde(skip_serializing_if = "Option::is_none")]
  pub hot_compile: Option<bool>,
  #[serde(skip_serializing_if = "Option::is_none")]
  pub hot_reload_build: Option<bool>,
  #[serde(skip_serializing_if = "Option::is_none")]
  pub enable_build_script_type_check: Option<bool>,
  #[serde(skip_serializing_if = "Option::is_none")]
  pub stop_daemon: Option<bool>,
  #[serde(skip_serializing_if = "Option::is_none")]
  pub stop_daemon_all: Option<bool>,
  #[serde(skip_serializing_if = "Option::is_none")]
  pub status_daemon: Option<bool>,
  #[serde(skip_serializing_if = "Option::is_none")]
  pub stacktrace: Option<bool>,
  #[serde(skip_serializing_if = "Option::is_none")]
  pub type_check: Option<bool>,
  #[serde(skip_serializing_if = "Option::is_none")]
  pub parallel: Option<bool>,
  #[serde(skip_serializing_if = "Option::is_none")]
  pub cache: Option<bool>,
  #[serde(skip_serializing_if = "Option::is_none")]
  pub daemon: Option<bool>,
  #[serde(skip_serializing_if = "Option::is_none")]
  pub generate_build_profile: Option<bool>,
  #[serde(skip_serializing_if = "Option::is_none")]
  pub analyze: Option<HvigorAnalyzeMode>,
  #[serde(skip_serializing_if = "Vec::is_empty")]
  pub config_overrides: Vec<String>,
  #[serde(skip_serializing_if = "Option::is_none")]
  pub execution_mode: Option<String>,
  #[serde(skip_serializing_if = "Option::is_none")]
  pub node_home: Option<String>,
  #[serde(skip_serializing_if = "Option::is_none")]
  pub optimization_strategy: Option<String>,
  #[serde(skip_serializing_if = "Option::is_none")]
  pub max_old_space_size: Option<f64>,
  #[serde(skip_serializing_if = "Option::is_none")]
  pub max_semi_space_size: Option<f64>,
  #[serde(skip_serializing_if = "Option::is_none")]
  pub xmx: Option<f64>,
  #[serde(skip_serializing_if = "Option::is_none")]
  pub product: Option<String>,
  #[serde(skip_serializing_if = "Option::is_none")]
  pub mode: Option<String>,
  #[serde(skip_serializing_if = "Option::is_none")]
  pub module: Option<String>,
  #[serde(skip_serializing_if = "Option::is_none")]
  pub build_version: Option<String>,
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
}

impl Default for HvigorCliConfig {
  fn default() -> Self {
    Self {
      tasks: Vec::new(),
      prop: Vec::new(),
      log_level: None,
      sync: None,
      watch: None,
      hot_compile: None,
      hot_reload_build: None,
      enable_build_script_type_check: None,
      stop_daemon: None,
      stop_daemon_all: None,
      status_daemon: None,
      stacktrace: None,
      type_check: None,
      parallel: None,
      cache: None,
      daemon: None,
      generate_build_profile: None,
      analyze: None,
      config_overrides: Vec::new(),
      execution_mode: None,
      node_home: None,
      optimization_strategy: None,
      max_old_space_size: None,
      max_semi_space_size: None,
      xmx: None,
      product: None,
      mode: None,
      module: None,
      build_version: None,
      coverage: None,
      scope: None,
      ohos_test_coverage: None,
      coverage_path_filter: None,
      coverage_mode: None,
      unit_test_replace_page: None,
    }
  }
}

#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(untagged)]
pub enum HvigorAnalyzeMode {
  Enabled(bool),
  Named(String),
}

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
  pub watch: Option<bool>,
  #[serde(skip_serializing_if = "Option::is_none")]
  pub preview: Option<bool>,
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
  pub disable_abc_cache: Option<bool>,
  #[serde(skip_serializing_if = "Option::is_none")]
  pub cache_file: Option<String>,
  #[serde(skip_serializing_if = "Option::is_none")]
  pub checker: Option<String>,
  #[serde(skip_serializing_if = "Option::is_none")]
  pub module_analyzer: Option<String>,
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
  pub fn parse_hvigor(args: &[String]) -> Result<HvigorCliConfig, CliDiagnostic> {
    HvigorArgsParser::new(args).parse()
  }

  pub fn parse_build(args: &[String]) -> Result<ArkdownBuildCliConfig, CliDiagnostic> {
    ArkdownBuildArgsParser::new(args).parse()
  }

  pub fn parse_hvigor_json(args: &[String]) -> Result<String, serde_json::Error> {
    CliParseEnvelope::from_result(Self::parse_hvigor(args)).to_json()
  }

  pub fn parse_build_json(args: &[String]) -> Result<String, serde_json::Error> {
    CliParseEnvelope::from_result(Self::parse_build(args)).to_json()
  }
}

struct HvigorArgsParser<'a> {
  args: &'a [String],
  config: HvigorCliConfig,
}

impl<'a> HvigorArgsParser<'a> {
  fn new(args: &'a [String]) -> Self {
    Self {
      args,
      config: HvigorCliConfig::default(),
    }
  }

  fn parse(mut self) -> Result<HvigorCliConfig, CliDiagnostic> {
    let mut positional_only = false;
    let mut index = 0;
    while index < self.args.len() {
      let arg = &self.args[index];
      if positional_only {
        self.config.tasks.push(arg.clone());
        index += 1;
        continue;
      }
      if arg == "--" {
        positional_only = true;
        index += 1;
        continue;
      }
      if self.assign_direct_flag(arg) || self.assign_flag_pair(arg) {
        index += 1;
        continue;
      }
      if let Some(value) = arg.strip_prefix("--analyze=") {
        self.config.analyze = Some(self.analyze_mode(value)?);
        index += 1;
        continue;
      }
      let (option, inline_value) = arg
        .split_once('=')
        .map_or((arg.as_str(), None), |(option, value)| {
          (option, Some(value))
        });
      if let Some(kind) = HvigorValueOption::from_flag(option) {
        let value = match inline_value {
          Some(value) => value.to_string(),
          None => {
            index += 1;
            self.required_value(index, arg)?.clone()
          }
        };
        self.assign_value(kind, value)?;
        index += 1;
        continue;
      }
      if arg.starts_with('-') {
        // `allowUnknownOption()` in hvigor 6.24.3 deliberately leaves wrapper
        // extension flags alone instead of rejecting them.
        index += 1;
        continue;
      }
      self.config.tasks.push(arg.clone());
      index += 1;
    }
    self.apply_test_defaults();
    self.apply_properties()?;
    Ok(self.config)
  }

  fn assign_direct_flag(&mut self, arg: &str) -> bool {
    match arg {
      "-e" | "--error" => self.config.log_level = Some("error".to_string()),
      "-w" | "--warn" => self.config.log_level = Some("warn".to_string()),
      "-i" | "--info" => self.config.log_level = Some("info".to_string()),
      "-d" | "--debug" => self.config.log_level = Some("debug".to_string()),
      "-s" | "--sync" => self.config.sync = Some(true),
      "--watch" => self.config.watch = Some(true),
      "--hot-compile" => self.config.hot_compile = Some(true),
      "--hot-reload-build" => self.config.hot_reload_build = Some(true),
      "--verbose-analyze" => {
        self.config.analyze = Some(HvigorAnalyzeMode::Named("advanced".to_string()))
      }
      "--enable-build-script-type-check" => self.config.enable_build_script_type_check = Some(true),
      "--stop-daemon" => self.config.stop_daemon = Some(true),
      "--stop-daemon-all" => self.config.stop_daemon_all = Some(true),
      "--status-daemon" => self.config.status_daemon = Some(true),
      _ => return false,
    }
    true
  }

  fn assign_flag_pair(&mut self, arg: &str) -> bool {
    let Some(raw) = arg.strip_prefix("--") else {
      return false;
    };
    let (enabled, name) = raw
      .strip_prefix("no-")
      .map_or((true, raw), |name| (false, name));
    match name {
      "stacktrace" => self.config.stacktrace = Some(enabled),
      "type-check" => self.config.type_check = Some(enabled),
      "parallel" => self.config.parallel = Some(enabled),
      "incremental" => self.config.cache = Some(enabled),
      "daemon" => self.config.daemon = Some(enabled),
      "generate-build-profile" => self.config.generate_build_profile = Some(enabled),
      "analyze" => self.config.analyze = Some(HvigorAnalyzeMode::Enabled(enabled)),
      _ => return false,
    }
    true
  }

  fn required_value(&self, index: usize, option: &str) -> Result<&String, CliDiagnostic> {
    self
      .args
      .get(index)
      .filter(|value| !value.starts_with('-'))
      .ok_or_else(|| {
        CliDiagnostic::new(
          "MISSING_HVIGOR_OPTION_VALUE",
          format!("{option} requires a value"),
        )
      })
  }

  fn assign_value(&mut self, kind: HvigorValueOption, value: String) -> Result<(), CliDiagnostic> {
    match kind {
      HvigorValueOption::Property => self.config.prop.push(value),
      HvigorValueOption::ConfigOverride => self.config.config_overrides.push(value),
      HvigorValueOption::ExecutionMode => {
        if value != "module" && value != "project" {
          return Err(CliDiagnostic::new(
            "UNKNOWN_HVIGOR_EXECUTION_MODE",
            format!("Unknown mode '{value}' is specified in the command line. Please check."),
          ));
        }
        self.config.execution_mode = Some(value);
      }
      HvigorValueOption::NodeHome => self.config.node_home = Some(value),
      HvigorValueOption::OptimizationStrategy => self.config.optimization_strategy = Some(value),
      HvigorValueOption::MaxOldSpaceSize => {
        self.config.max_old_space_size = Some(self.positive_integer(&value, "maxOldSpaceSize")?)
      }
      HvigorValueOption::MaxSemiSpaceSize => {
        self.config.max_semi_space_size = Some(self.positive_integer(&value, "maxSemiSpaceSize")?)
      }
      HvigorValueOption::Xmx => self.config.xmx = Some(self.positive_integer(&value, "xmx")?),
    }
    Ok(())
  }

  fn positive_integer(&self, value: &str, key: &str) -> Result<f64, CliDiagnostic> {
    let parsed = JsCliNumber::new(value).parse();
    if parsed.is_finite() && parsed > 0.0 && parsed.fract() == 0.0 {
      Ok(parsed)
    } else {
      Err(CliDiagnostic::new(
        "INVALID_HVIGOR_MEMORY_OPTION",
        format!("{key} must be a positive integer: {value}"),
      ))
    }
  }

  fn analyze_mode(&self, value: &str) -> Result<HvigorAnalyzeMode, CliDiagnostic> {
    match value {
      "default" | "normal" => Ok(HvigorAnalyzeMode::Named("normal".to_string())),
      "verbose" | "advanced" => Ok(HvigorAnalyzeMode::Named("advanced".to_string())),
      "false" => Ok(HvigorAnalyzeMode::Enabled(false)),
      "ultrafine" => Ok(HvigorAnalyzeMode::Named("ultrafine".to_string())),
      _ => Err(CliDiagnostic::new(
        "INVALID_HVIGOR_ANALYZE_MODE",
        format!("unsupported analyze mode '{value}'"),
      )),
    }
  }

  fn apply_properties(&mut self) -> Result<(), CliDiagnostic> {
    for property in &self.config.prop {
      let (key, value) = property
        .split_once('=')
        .map_or((property.as_str(), None), |(key, value)| (key, Some(value)));
      match key {
        "product" => self.config.product = value.map(str::to_string),
        "buildMode" => self.config.mode = value.map(str::to_string),
        "module" => self.config.module = value.map(str::to_string),
        "buildVersion" => {
          BuildVersionValidator::new(value).validate()?;
          self.config.build_version = value.map(str::to_string);
        }
        "coverage" | "ohos-test-coverage" => {
          let enabled = value == Some("true");
          self.config.coverage = Some(enabled);
          self.config.ohos_test_coverage = Some(enabled);
        }
        "scope" => self.config.scope = value.map(str::to_string),
        "coverage-path-filter" => self.config.coverage_path_filter = value.map(str::to_string),
        "coverage-mode" => self.config.coverage_mode = value.map(str::to_string),
        "unit.test.replace.page" => {
          self.config.unit_test_replace_page = value.map(str::to_string)
        }
        _ => {}
      }
    }
    Ok(())
  }

  /// Latest hvigor `global-data.ts::setTestDefaultParameters` runs after CLI
  /// parsing and before the OHOS plugin sees extra configuration. A JavaScript
  /// `Map` supplies last-value-wins semantics while retaining the first key's
  /// insertion position; `IndexMap` gives the native runtime the same stable
  /// property order.
  fn apply_test_defaults(&mut self) {
    if !self.config.tasks.iter().any(|task| task == "test") {
      return;
    }
    if self.config.execution_mode.is_none() {
      self.config.execution_mode = Some("module".to_string());
    }

    let mut properties = indexmap::IndexMap::<String, String>::new();
    for property in std::mem::take(&mut self.config.prop) {
      // Hvigor's `getPropMap` destructures the first two `split('=')`
      // elements. Preserve its truncation and `undefined` serialization.
      let mut parts = property.split('=');
      let key = parts.next().unwrap_or_default();
      let value = parts.next().unwrap_or("undefined");
      let key = if key == "coverage" {
        "ohos-test-coverage"
      } else {
        key
      };
      properties.insert(key.to_string(), value.to_string());
    }
    for (key, value) in [
      ("pageType", "page"),
      ("product", "default"),
      ("buildRoot", ".test"),
      ("unitTestMode", "true"),
      ("isLocalTest", "true"),
      ("ohos-test-coverage", "true"),
      (
        "unit.test.replace.page",
        "../../../.test/testability/pages/Index",
      ),
    ] {
      properties
        .entry(key.to_string())
        .or_insert_with(|| value.to_string());
    }
    self.config.prop = properties
      .into_iter()
      .map(|(key, value)| format!("{key}={value}"))
      .collect();
  }
}

enum HvigorValueOption {
  ExecutionMode,
  ConfigOverride,
  Property,
  NodeHome,
  OptimizationStrategy,
  MaxOldSpaceSize,
  MaxSemiSpaceSize,
  Xmx,
}

impl HvigorValueOption {
  fn from_flag(value: &str) -> Option<Self> {
    match value {
      "--mode" | "-m" => Some(Self::ExecutionMode),
      "--config" | "-c" => Some(Self::ConfigOverride),
      "--prop" | "-p" => Some(Self::Property),
      "--node-home" => Some(Self::NodeHome),
      "--optimization-strategy" => Some(Self::OptimizationStrategy),
      "--max-old-space-size" => Some(Self::MaxOldSpaceSize),
      "--max-semi-space-size" => Some(Self::MaxSemiSpaceSize),
      "--Xmx" => Some(Self::Xmx),
      _ => None,
    }
  }
}

struct JsCliNumber<'a> {
  value: &'a str,
}

impl<'a> JsCliNumber<'a> {
  fn new(value: &'a str) -> Self {
    Self { value }
  }

  fn parse(&self) -> f64 {
    let value = self.value.trim();
    if let Some(hex) = value
      .strip_prefix("0x")
      .or_else(|| value.strip_prefix("0X"))
    {
      return u128::from_str_radix(hex, 16)
        .map(|value| value as f64)
        .unwrap_or(f64::NAN);
    }
    if let Some(binary) = value
      .strip_prefix("0b")
      .or_else(|| value.strip_prefix("0B"))
    {
      return u128::from_str_radix(binary, 2)
        .map(|value| value as f64)
        .unwrap_or(f64::NAN);
    }
    if let Some(octal) = value
      .strip_prefix("0o")
      .or_else(|| value.strip_prefix("0O"))
    {
      return u128::from_str_radix(octal, 8)
        .map(|value| value as f64)
        .unwrap_or(f64::NAN);
    }
    value.parse::<f64>().unwrap_or(f64::NAN)
  }
}

struct BuildVersionValidator<'a> {
  value: Option<&'a str>,
}

impl<'a> BuildVersionValidator<'a> {
  fn new(value: Option<&'a str>) -> Self {
    Self { value }
  }

  fn validate(&self) -> Result<(), CliDiagnostic> {
    let Some(value) = self.value else {
      return Err(self.diagnostic("undefined"));
    };
    let components = value.split('.').collect::<Vec<_>>();
    let valid = !value.is_empty()
      && value.len() <= 18
      && components.len() <= 3
      && components.iter().all(|component| {
        !component.is_empty()
          && component.bytes().all(|byte| byte.is_ascii_digit())
          && (component == &"0" || !component.starts_with('0'))
      });
    if valid {
      Ok(())
    } else {
      Err(self.diagnostic(value))
    }
  }

  fn diagnostic(&self, value: &str) -> CliDiagnostic {
    CliDiagnostic::new(
      "INVALID_BUILDVERSION_PARAMETER",
      format!("Invalid buildVersion parameter: {value}"),
    )
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
        "--watch" => self.config.watch = Some(true),
        "--preview" => self.config.preview = Some(true),
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
        "--disable-abc-cache" => self.config.disable_abc_cache = Some(true),
        "--cache-file" => self.config.cache_file = Some(self.required_value(&mut index, arg)?),
        "--checker" => self.config.checker = Some(self.required_value(&mut index, arg)?),
        "--module-analyzer" => {
          self.config.module_analyzer = Some(self.required_value(&mut index, arg)?)
        }
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
