use std::collections::BTreeMap;
use std::path::{Path, PathBuf};
use std::process::Command;

use anyhow::{Context, Result};
use indexmap::IndexMap;
use rustc_hash::FxHashSet;
use serde::Serialize;

use arkdown_config::build_profile::parse_api_version;
use arkdown_core::{ArkdownConfig, BuildMode, HapManifest, ModuleConfig, StaticArktsSdkLayout};
use arkdown_path::{lexical_normalize, ArtifactPathSerializer};
use arkdown_profile::LoaderJson;
use arkdown_session::dependency_graph::snapshot::DependencyNodeType;
use arkdown_session::ModuleCompileContext;

use super::layout::ModuleBuildLayout;

pub(super) struct StaticArktsCompileInput<'a> {
  pub config: &'a ArkdownConfig,
  pub module: &'a ModuleConfig,
  pub manifest: &'a HapManifest,
  pub loader_json: &'a LoaderJson,
  pub module_context: &'a ModuleCompileContext,
  pub module_layout: &'a ModuleBuildLayout<'a>,
  pub loader_json_path: &'a Path,
  pub node_executable: PathBuf,
  pub output_abc: &'a Path,
  pub preview_mode: bool,
  pub check_entry: bool,
  pub ui_transform_optimization: bool,
  pub bundle_type: &'a str,
}

/// ArkTS 1.2 SDK driver adapter.
///
/// OXC and Rolldown own parsing and graph construction. The SDK driver is
/// retained solely as the compatibility host for the source tree's parsed and
/// checked TS/JS plugins and the native es2panda state machine.
pub(super) struct StaticArktsCompiler<'a> {
  input: StaticArktsCompileInput<'a>,
  sdk: StaticArktsSdkLayout,
}

impl<'a> StaticArktsCompiler<'a> {
  pub(super) fn new(input: StaticArktsCompileInput<'a>) -> Result<Self> {
    let sdk = input
      .config
      .sdk_config
      .static_arkts_layout()
      .ok_or_else(|| {
        anyhow::anyhow!(
          "ARKDOWN_ARKTS12_SDK_LAYOUT: cannot derive ets/static from {}",
          input.config.sdk_config.ets_loader_path.display()
        )
      })?;
    let compiler = Self { input, sdk };
    compiler.validate_sdk()?;
    Ok(compiler)
  }

  pub(super) fn compile(&self) -> Result<()> {
    let build_config = StaticArktsDriverConfigBuilder::new(&self.input, &self.sdk).build()?;
    let config_path = PathBuf::from(&self.input.loader_json.cache_path)
      .join("arkts12")
      .join("build_config.json");
    if let Some(parent) = config_path.parent() {
      fs_err::create_dir_all(parent)
        .with_context(|| format!("create ArkTS 1.2 config directory {}", parent.display()))?;
    }
    fs_err::write(&config_path, serde_json::to_vec_pretty(&build_config)?)
      .with_context(|| format!("write ArkTS 1.2 config {}", config_path.display()))?;
    if let Some(parent) = self.input.output_abc.parent() {
      fs_err::create_dir_all(parent)
        .with_context(|| format!("create ArkTS 1.2 output {}", parent.display()))?;
    }

    let output = self.command(&config_path).output().with_context(|| {
      format!(
        "launch ArkTS 1.2 SDK driver {}",
        self.sdk.driver_entry.display()
      )
    })?;
    if !output.status.success() {
      anyhow::bail!(
        "ARKDOWN_ARKTS12_COMPILE_FAILED: driver {} exited with {}; stdout: {}; stderr: {}",
        self.sdk.driver_entry.display(),
        output.status,
        String::from_utf8_lossy(&output.stdout).trim(),
        String::from_utf8_lossy(&output.stderr).trim()
      );
    }
    if !self.input.output_abc.is_file() {
      anyhow::bail!(
        "ARKDOWN_ARKTS12_OUTPUT_MISSING: static compiler completed without writing {}",
        self.input.output_abc.display()
      );
    }
    Ok(())
  }

  fn command(&self, config_path: &Path) -> Command {
    let mut command = Command::new(&self.input.node_executable);
    command.arg(&self.sdk.driver_entry).arg(config_path);
    self.prepend_native_library_path(&mut command);
    command
  }

  fn prepend_native_library_path(&self, command: &mut Command) {
    let variable = if cfg!(target_os = "macos") {
      "DYLD_LIBRARY_PATH"
    } else if cfg!(windows) {
      "PATH"
    } else {
      "LD_LIBRARY_PATH"
    };
    let mut paths = vec![self.sdk.panda_sdk.join("lib")];
    if let Some(existing) = std::env::var_os(variable) {
      paths.extend(std::env::split_paths(&existing));
    }
    if let Ok(joined) = std::env::join_paths(paths) {
      command.env(variable, joined);
    }
  }

  fn validate_sdk(&self) -> Result<()> {
    for (name, path) in [
      ("driver", &self.sdk.driver_entry),
      ("es2panda", &self.sdk.es2panda),
      ("ark_link", &self.sdk.abc_linker),
      ("dependency_analyzer", &self.sdk.dependency_analyzer),
    ] {
      if !path.is_file() {
        anyhow::bail!(
          "ARKDOWN_ARKTS12_SDK_TOOL_MISSING: {name} is not available at {}",
          path.display()
        );
      }
    }
    for (name, path) in [
      ("api", &self.sdk.api),
      ("kits", &self.sdk.kits),
      ("arkts", &self.sdk.arkts),
    ] {
      if !path.is_dir() {
        anyhow::bail!(
          "ARKDOWN_ARKTS12_SDK_DECLARATIONS_MISSING: {name} declarations are not available at {}",
          path.display()
        );
      }
    }
    for (name, path) in [
      ("ui_syntax_plugin", &self.sdk.ui_syntax_plugin),
      ("ui_plugin", &self.sdk.ui_plugin),
      ("memo_plugin", &self.sdk.memo_plugin),
    ] {
      if !path.is_file() && !path.with_extension("js").is_file() {
        anyhow::bail!(
          "ARKDOWN_ARKTS12_PLUGIN_MISSING: {name} is not available at {}[.js]",
          path.display()
        );
      }
    }
    Ok(())
  }
}

struct StaticArktsDriverConfigBuilder<'a> {
  input: &'a StaticArktsCompileInput<'a>,
  sdk: &'a StaticArktsSdkLayout,
}

impl<'a> StaticArktsDriverConfigBuilder<'a> {
  fn new(input: &'a StaticArktsCompileInput<'a>, sdk: &'a StaticArktsSdkLayout) -> Self {
    Self { input, sdk }
  }

  fn build(&self) -> Result<StaticArktsDriverConfig> {
    let compile_files = self.compile_files()?;
    let source_roots = if self.input.module_context.module.source_roots.is_empty() {
      vec![self.path(&self.input.module_context.module.src_main)?]
    } else {
      self
        .input
        .module_context
        .module
        .source_roots
        .iter()
        .map(|path| self.path(path))
        .collect::<Result<Vec<_>>>()?
    };
    let dependent_module_list = self.dependent_modules()?;
    let plugins = IndexMap::from([
      (
        "ui_syntax_plugin".to_string(),
        self.path(&self.sdk.ui_syntax_plugin)?,
      ),
      ("ui_plugin".to_string(), self.path(&self.sdk.ui_plugin)?),
      ("memo_plugin".to_string(), self.path(&self.sdk.memo_plugin)?),
    ]);
    let external_api_paths = [&self.sdk.api, &self.sdk.kits, &self.sdk.arkts]
      .into_iter()
      .map(|path| self.path(path))
      .collect::<Result<Vec<_>>>()?;
    let module_root = self.path(&self.input.module.src_path)?;
    let project_path = self.input.module_layout.source().ets();
    let output_dir = self
      .input
      .output_abc
      .parent()
      .ok_or_else(|| anyhow::anyhow!("ArkTS 1.2 ABC output has no parent directory"))?;
    let dependent_module_list_copy = dependent_module_list.clone();

    Ok(StaticArktsDriverConfig {
      plugins,
      compile_files: compile_files.clone(),
      entry_files: compile_files,
      build_mode: StaticArktsBuildMode::new(&self.input.config.build_mode).to_string(),
      project_root_path: self.path(&self.input.config.project_root)?,
      module_root_path: module_root,
      ace_profile_path: self.path(&self.input.module_layout.res_profile_dir())?,
      cache_path: self.path(Path::new(&self.input.loader_json.cache_path))?,
      loader_out_path: self.path(output_dir)?,
      compile_sdk_version: self.input.config.sdk_config.compile_api_version,
      compatible_sdk_version: parse_api_version(&self.input.config.product.compatible_sdk_version)
        .unwrap_or(self.input.config.sdk_config.compile_api_version),
      ace_module_json_path: self.path(&self.input.module_layout.res_ark_module_json())?,
      bundle_name: self.input.config.product.bundle_name.clone(),
      use_normalized_ohm_url: self.input.loader_json.use_normalized_ohm_url,
      build_type: "build".to_string(),
      package_name: self.input.loader_json.module_name.clone(),
      build_sdk_path: self.path(&self.sdk.root)?,
      source_roots,
      module_type: self.input.module.module_type.to_string(),
      module_name: self.input.loader_json.module_name.clone(),
      dependent_module_list,
      dependency_module_list: dependent_module_list_copy,
      has_main_module: self
        .input
        .config
        .modules
        .iter()
        .any(|module| matches!(module.module_type, arkdown_core::OhosModuleType::Entry)),
      build_loader_json: self.path(self.input.loader_json_path)?,
      integrated_hsp: self
        .input
        .config
        .product
        .build_option
        .ark_options
        .as_ref()
        .and_then(|options| options.integrated_hsp)
        .unwrap_or(false),
      allow_empty_bundle_name: false,
      declgen_v2_out_path: String::new(),
      external_api_paths,
      device_types: self.input.manifest.device_types.clone(),
      permissions: BTreeMap::new(),
      byte_code_har: self.input.loader_json.byte_code_har.unwrap_or(false),
      ui_transform_optimization: self.input.ui_transform_optimization,
      runtime_os: self.input.config.product.runtime_os.clone(),
      sdk_config_paths: Vec::new(),
      is_preview: self.input.preview_mode,
      check_entry: self.input.check_entry,
      code_root_path: self.path(&project_path)?,
      is_crossplatform: false,
      bundle_type: self.input.bundle_type.to_string(),
      level: StaticArktsLogLevel::default(),
      is_build_config_modified: false,
      panda_sdk_path: self.path(&self.sdk.panda_sdk)?,
      abc_linker_path: self.path(&self.sdk.abc_linker)?,
      dependency_analyzer_path: self.path(&self.sdk.dependency_analyzer)?,
      api_path: self.path(&self.sdk.api)?,
      kits_path: self.path(&self.sdk.kits)?,
      dep_analyzer_path: self.path(&self.sdk.dependency_analyzer)?,
      alias_config: BTreeMap::new(),
      sdk_alias_map: BTreeMap::new(),
      interop_sdk_paths: BTreeMap::new(),
      es2panda_mode: "simultaneous".to_string(),
      app_resource: self.path(&self.input.module_layout.resource_table_txt())?,
      raw_file_resource: self
        .path(&self.input.module_layout.res_resources_dir().join("rawfile"))?,
      project_path: self.path(&project_path)?,
      hsp_resources_map: !self.input.loader_json.hsp_resources_map.is_empty(),
      compile_har: matches!(
        self.input.module.module_type,
        arkdown_core::OhosModuleType::Har
      ),
      reset_bundle_name: self
        .input
        .config
        .product
        .build_option
        .ark_options
        .as_ref()
        .and_then(|options| options.integrated_hsp)
        .unwrap_or(false),
      ignore_error: false,
    })
  }

  fn compile_files(&self) -> Result<Vec<String>> {
    StaticArktsCompileEntryPlan::new(
      Path::new(&self.input.loader_json.module_root_path),
      &self.input.loader_json.hvigor_compile_entry,
      &self.input.loader_json.compile_entry,
    )
    .resolve()?
    .iter()
    .map(|path| self.path(path))
    .collect()
  }

  fn dependent_modules(&self) -> Result<Vec<StaticArktsDependentModule>> {
    let mut modules = Vec::new();
    for node in self.input.module_context.deps.nodes.values() {
      if !matches!(
        node.node_type,
        DependencyNodeType::SourceHar
          | DependencyNodeType::IntegratedHsp
          | DependencyNodeType::OhpmSource
      ) {
        continue;
      }
      let root = node.package_root.as_ref().ok_or_else(|| {
        anyhow::anyhow!(
          "ARKDOWN_ARKTS12_DEPENDENCY_ROOT_MISSING: {}",
          node.package_name
        )
      })?;
      let package = node.package.as_ref().ok_or_else(|| {
        anyhow::anyhow!(
          "ARKDOWN_ARKTS12_DEPENDENCY_METADATA_MISSING: {}",
          node.package_name
        )
      })?;
      let entry = package.package_json.main.as_deref().ok_or_else(|| {
        anyhow::anyhow!(
          "ARKDOWN_ARKTS12_DEPENDENCY_ENTRY_MISSING: package '{}' has no main entry",
          node.package_name
        )
      })?;
      let entry_file = lexical_normalize(&root.join(entry));
      if !entry_file.is_file() {
        anyhow::bail!(
          "ARKDOWN_ARKTS12_DEPENDENCY_ENTRY_MISSING: package '{}' main entry does not exist at {}",
          node.package_name,
          entry_file.display()
        );
      }
      let source_root = entry_file.parent().unwrap_or(root);
      modules.push(StaticArktsDependentModule {
        package_name: node.package_name.clone(),
        module_name: node
          .module_profile
          .as_ref()
          .and_then(|profile| profile.module_name.clone())
          .unwrap_or_else(|| node.package_name.clone()),
        module_type: node
          .module_profile
          .as_ref()
          .and_then(|profile| profile.module_type.as_ref())
          .map(ToString::to_string)
          .unwrap_or_else(|| "har".to_string()),
        module_path: self.path(root)?,
        source_roots: vec![self.path(source_root)?],
        entry_file: self.path(&entry_file)?,
        language: "1.2".to_string(),
        dependencies: node.direct_deps.clone(),
      });
    }
    modules.sort_by(|left, right| {
      left
        .package_name
        .cmp(&right.package_name)
        .then_with(|| left.module_name.cmp(&right.module_name))
        .then_with(|| left.module_path.cmp(&right.module_path))
    });
    Ok(modules)
  }

  fn path(&self, path: &Path) -> Result<String> {
    ArtifactPathSerializer::path_to_slash_string(path)
      .map_err(|error| anyhow::anyhow!("serialize ArkTS 1.2 path {}: {error}", path.display()))
  }
}

struct StaticArktsCompileEntryPlan<'a> {
  root: &'a Path,
  serialized_entries: &'a [String],
  internal_entries: &'a [String],
}

impl<'a> StaticArktsCompileEntryPlan<'a> {
  fn new(root: &'a Path, serialized_entries: &'a [String], internal_entries: &'a [String]) -> Self {
    Self {
      root,
      serialized_entries,
      internal_entries,
    }
  }

  fn resolve(&self) -> Result<Vec<PathBuf>> {
    let mut files = Vec::new();
    let mut seen = FxHashSet::default();
    for entry in self
      .serialized_entries
      .iter()
      .chain(self.internal_entries.iter())
    {
      let entry = entry.strip_suffix("?entry").unwrap_or(entry);
      let path = Path::new(entry);
      let path = if path.has_root() {
        lexical_normalize(path)
      } else {
        lexical_normalize(&self.root.join(entry.strip_prefix("./").unwrap_or(entry)))
      };
      if !path.is_file() {
        anyhow::bail!("ARKDOWN_ARKTS12_COMPILE_ENTRY_MISSING: {}", path.display());
      }
      if seen.insert(path.clone()) {
        files.push(path);
      }
    }
    if files.is_empty() {
      anyhow::bail!("ARKDOWN_ARKTS12_COMPILE_ENTRY_EMPTY: no static compiler roots");
    }
    Ok(files)
  }
}

struct StaticArktsBuildMode<'a> {
  mode: &'a BuildMode,
}

impl<'a> StaticArktsBuildMode<'a> {
  fn new(mode: &'a BuildMode) -> Self {
    Self { mode }
  }
}

impl std::fmt::Display for StaticArktsBuildMode<'_> {
  fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
    formatter.write_str(match self.mode {
      BuildMode::Debug => "Debug",
      BuildMode::Release => "Release",
      BuildMode::Test => "Test",
    })
  }
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
struct StaticArktsDependentModule {
  package_name: String,
  module_name: String,
  module_type: String,
  module_path: String,
  source_roots: Vec<String>,
  entry_file: String,
  language: String,
  dependencies: Vec<String>,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct StaticArktsDriverConfig {
  plugins: IndexMap<String, String>,
  compile_files: Vec<String>,
  entry_files: Vec<String>,
  build_mode: String,
  project_root_path: String,
  module_root_path: String,
  ace_profile_path: String,
  cache_path: String,
  loader_out_path: String,
  compile_sdk_version: u32,
  compatible_sdk_version: u32,
  ace_module_json_path: String,
  bundle_name: String,
  use_normalized_ohm_url: bool,
  build_type: String,
  package_name: String,
  build_sdk_path: String,
  source_roots: Vec<String>,
  module_type: String,
  module_name: String,
  dependent_module_list: Vec<StaticArktsDependentModule>,
  dependency_module_list: Vec<StaticArktsDependentModule>,
  has_main_module: bool,
  build_loader_json: String,
  integrated_hsp: bool,
  allow_empty_bundle_name: bool,
  declgen_v2_out_path: String,
  external_api_paths: Vec<String>,
  device_types: Vec<String>,
  permissions: BTreeMap<String, serde_json::Value>,
  byte_code_har: bool,
  ui_transform_optimization: bool,
  #[serde(rename = "runtimeOS")]
  runtime_os: String,
  sdk_config_paths: Vec<String>,
  is_preview: bool,
  check_entry: bool,
  code_root_path: String,
  is_crossplatform: bool,
  bundle_type: String,
  level: StaticArktsLogLevel,
  is_build_config_modified: bool,
  panda_sdk_path: String,
  abc_linker_path: String,
  dependency_analyzer_path: String,
  api_path: String,
  kits_path: String,
  dep_analyzer_path: String,
  alias_config: BTreeMap<String, serde_json::Value>,
  sdk_alias_map: BTreeMap<String, serde_json::Value>,
  interop_sdk_paths: BTreeMap<String, serde_json::Value>,
  es2panda_mode: String,
  app_resource: String,
  raw_file_resource: String,
  project_path: String,
  hsp_resources_map: bool,
  compile_har: bool,
  reset_bundle_name: bool,
  ignore_error: bool,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct StaticArktsLogLevel {
  level: u32,
  level_str: String,
  colour: String,
}

impl Default for StaticArktsLogLevel {
  fn default() -> Self {
    Self {
      level: 20_000,
      level_str: "INFO".to_string(),
      colour: "green".to_string(),
    }
  }
}

#[cfg(test)]
mod tests {
  use super::StaticArktsCompileEntryPlan;

  #[test]
  fn compile_entries_preserve_source_order_strip_query_and_deduplicate() {
    let temp = tempfile::tempdir().expect("tempdir");
    let root = temp.path().join("src/main");
    std::fs::create_dir_all(root.join("ets/pages")).expect("source directory");
    let page = root.join("ets/pages/Index.ets");
    let ability = root.join("ets/EntryAbility.ets");
    std::fs::write(&page, "").expect("page source");
    std::fs::write(&ability, "").expect("ability source");
    let serialized = vec![format!("{}?entry", ability.display())];
    let internal = vec![
      "./ets/pages/Index.ets?entry".to_string(),
      ability.display().to_string(),
    ];

    let resolved = StaticArktsCompileEntryPlan::new(&root, &serialized, &internal)
      .resolve()
      .expect("static compile entry plan");

    assert_eq!(resolved, [ability, page]);
  }

  #[test]
  fn missing_static_compile_entry_is_a_typed_boundary_error() {
    let temp = tempfile::tempdir().expect("tempdir");
    let internal = vec!["./ets/pages/Missing.ets".to_string()];
    let error = StaticArktsCompileEntryPlan::new(temp.path(), &[], &internal)
      .resolve()
      .expect_err("missing static root must be rejected before launching Node");

    assert!(
      error
        .to_string()
        .contains("ARKDOWN_ARKTS12_COMPILE_ENTRY_MISSING"),
      "{error}"
    );
  }
}
