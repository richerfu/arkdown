//! Local unit-test chain (`hvigorw test` parity).
//!
//! Mirrors hvigor 6.24.3 `GenerateUnitTestTemplate`, `UnitTestProcessProfile`
//! and `ReplaceUnitTestIndexFile` for the local unit-test build root
//! (`buildRoot=.test`):
//!
//! * `generate_unit_test_template` writes the three `@ohos/coverage` templates
//!   into `<module>/.test/testability/` — `pages/Index.ets` (parameterized),
//!   `TestAbility.ets` (fixed) and `phoneSettingConfig_Phone.json` (fixed).
//!   The byte-for-byte content matches `@ohos/coverage/lib/src/commandLine/
//!   localTest/testTemplate.js` with `fileOperte.js` write semantics.
//! * `inject_unit_test_ability` appends the synthetic `TestAbility` entry to
//!   `intermediates/process_profile/<target>/module.json` (HAR/HSP type
//!   rewrites included) and overwrites `intermediates/res/<target>/
//!   ark_module.json` with the same document.
//! * `replace_unit_test_index_file` rewrites `intermediates/res/<target>/
//!   module.json` `pages` and emits `resources/base/profile/main_pages.json`
//!   from the `unit.test.replace.page` default
//!   (`../../../.test/testability/pages/Index`).

use std::collections::BTreeSet;
use std::path::{Path, PathBuf};

use anyhow::Result;
use indexmap::IndexSet;
use serde::{Deserialize, Serialize};
use serde_json::Value;

use arkdown_core::{HvigorModuleSourceFile, OhosModuleType, SdkConfig};
use arkdown_session::dependency_graph::snapshot::{
  DependencyEdgeKind, DependencyGraphSnapshot, DependencyNodeType,
};

use super::layout::ModuleBuildLayout;

/// `CommonConst.TEST_ABILITY` — synthetic ability name.
const TEST_ABILITY: &str = "TestAbility";
/// `UnitTestConst.ENTRY_TEST_ABILITY_PATH` — srcEntrance of the synthetic ability.
const ENTRY_TEST_ABILITY_PATH: &str = "../../.test/testability/TestAbility.ets";
/// `CommonConst.ICON` / `CommonConst.LABEL` — HAR rewrites.
const ICON: &str = "icon";
const LABEL: &str = "label";
/// hvigor `defaultTestProps["unit.test.replace.page"]` (InjectConst.
/// UNIT_TEST_REPLACE_PAGE).
pub(crate) const UNIT_TEST_REPLACE_PAGE: &str = "../../../.test/testability/pages/Index";
/// `ReplaceUnitTestIndexFile` page resource id.
const PAGE_JSON_RESOURCE: &str = "$profile:main_pages";

/// Parameters for the local unit-test index page template
/// (`@ohos/coverage` `pageIndexTemplate`).
pub(crate) struct UnitTestTemplateParams<'a> {
  /// `app.json5` bundle name.
  pub bundle_name: &'a str,
  /// Module name (`-m` parameter).
  pub module_name: &'a str,
  /// hvigor extra config `scope` (`OHOS_TEST_COVERAGE_SCOPE`); empty when unset.
  pub scope: &'a str,
  /// `ohos-test-coverage` extra config: `true`/`false` string in the template.
  pub coverage: bool,
  /// Absolute module path (`pathInfo.getModulePath()`), `/`-normalized.
  pub module_path: &'a Path,
}

impl UnitTestTemplateParams<'_> {
  /// `@ohos/coverage` `pageIndexTemplate(testTemplateParam)` with
  /// `fileOperte.js` platform path serialization.
  pub(crate) fn render_page_index(&self) -> Result<String> {
    let module_path = arkdown_path::ArtifactPathSerializer::path_to_slash_string(self.module_path)?;
    let coverage = if self.coverage { "true" } else { "false" };
    Ok(
      PAGE_INDEX_TEMPLATE
        .replace("__MODULE_PATH__", &module_path)
        .replace("__BUNDLE_NAME__", self.bundle_name)
        .replace("__MODULE_NAME__", self.module_name)
        .replace("__SCOPE__", self.scope)
        .replace("__COVERAGE__", coverage),
    )
  }
}

/// Write the three local unit-test template files into
/// `<module>/.test/testability/`. Mirrors `GenerateUnitTestTemplate`
/// calling `@ohos/coverage` `generateTemplateFile`.
pub(crate) fn generate_unit_test_template(
  module_root: &Path,
  product_name: &str,
  params: &UnitTestTemplateParams<'_>,
) -> Result<()> {
  let layout = ModuleBuildLayout::for_test_product(module_root, product_name);
  std::fs::create_dir_all(layout.testability_pages_dir())?;
  std::fs::write(layout.unit_test_index_ets(), params.render_page_index()?)?;
  std::fs::write(layout.test_ability_ets(), TEST_ABILITY_TEMPLATE)?;
  std::fs::write(
    layout.test_phone_settings_json(),
    PHONE_SETTING_CONFIG_TEMPLATE,
  )?;
  Ok(())
}

/// Write the `ohosTest` template tree into
/// `<module>/.test/<product>/intermediates/src/ohosTest/` (hvigor
/// `GenerateOhosTestTemplate` calling `@ohos/coverage`
/// `generateOhosTestTemplateFile`). API >= 11 uses the `@kit.*` template
/// variants (`TestAbility_withKit.ets`, `OpenHarmonyTestRunner_mock.ets`);
/// older SDKs use the plain templates.
pub(crate) fn generate_ohos_test_template(
  module_root: &Path,
  product_name: &str,
  api_version: u32,
) -> Result<()> {
  let layout = ModuleBuildLayout::for_ohos_test_template(module_root, product_name);
  let runner_dir = layout.ohos_test_runner_dir();
  let ability_dir = layout.ohos_test_ability_dir();
  let pages_dir = layout.ohos_test_pages_dir();
  let element_dir = layout.ohos_test_resource_element_dir();
  let profile_dir = layout.ohos_test_resource_profile_dir();
  for dir in [
    &runner_dir,
    &ability_dir,
    &pages_dir,
    &element_dir,
    &profile_dir,
  ] {
    std::fs::create_dir_all(dir)?;
  }
  let (runner_src, ability_src) = if api_version >= 11 {
    (
      OPEN_HARMONY_TEST_RUNNER_MOCK_TEMPLATE,
      TEST_ABILITY_WITH_KIT_TEMPLATE,
    )
  } else {
    (
      OPEN_HARMONY_TEST_RUNNER_TEMPLATE,
      TEST_ABILITY_TEMPLATE_OHOS,
    )
  };
  std::fs::write(runner_dir.join("OpenHarmonyTestRunner.ets"), runner_src)?;
  std::fs::write(ability_dir.join("TestAbility.ets"), ability_src)?;
  std::fs::write(pages_dir.join("Index.ets"), OHOS_TEST_INDEX_TEMPLATE)?;
  std::fs::write(element_dir.join("color.json"), OHOS_TEST_COLOR_TEMPLATE)?;
  std::fs::write(element_dir.join("string.json"), OHOS_TEST_STRING_TEMPLATE)?;
  std::fs::write(
    profile_dir.join("test_pages.json"),
    OHOS_TEST_PAGES_TEMPLATE,
  )?;
  Ok(())
}

/// Whether the generated `ohosTest` template exists — used to gate the
/// ohosTest compile inputs.
pub(crate) fn has_ohos_test_template(module_root: &Path, product_name: &str) -> bool {
  ModuleBuildLayout::for_ohos_test_template(module_root, product_name)
    .ohos_test_ability_dir()
    .join("TestAbility.ets")
    .is_file()
}

/// Typed `GenerateUnitTestResult` input. The SDK component roots and HSP
/// dependency roots are resolved before execution, matching hvigor's
/// `GenerateUnitTestResult.doTaskAction` handoff to `@ohos/coverage`.
#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct UnitTestExecutionPlan {
  pub sdk_api_path: PathBuf,
  pub hms_sdk_api_path: PathBuf,
  pub runtime_type: UnitTestRuntimeType,
  pub module_path: PathBuf,
  pub project_path: PathBuf,
  pub coverage: bool,
  pub hsp_dependencies: Vec<PathBuf>,
  #[serde(skip)]
  pub node_executable: PathBuf,
  #[serde(skip)]
  pub coverage_package_path: PathBuf,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
pub(crate) enum UnitTestRuntimeType {
  OpenHarmony,
  HarmonyOS,
}

impl UnitTestExecutionPlan {
  /// Resolve the exact SDK/package/runtime handoff used by hvigor
  /// `GenerateUnitTestResult`. Component locations are derived from the
  /// selected SDK and dependency roots come from the immutable build session.
  pub(crate) fn from_build(
    sdk_config: &SdkConfig,
    runtime_os: &str,
    module_path: &Path,
    project_path: &Path,
    module_type: &OhosModuleType,
    dependencies: &DependencyGraphSnapshot,
    coverage: bool,
    node_executable: PathBuf,
  ) -> Result<Self> {
    let sdk_api_path = sdk_config.previewer_component_path().ok_or_else(|| {
      anyhow::anyhow!(
        "ARKDOWN_UNIT_TEST_PREVIEWER_COMPONENT_MISSING: selected SDK has no Previewer component ({})",
        sdk_config.ets_loader_path.display()
      )
    })?;
    let runtime_type = if arkdown_core::is_harmony_os(runtime_os) {
      UnitTestRuntimeType::HarmonyOS
    } else {
      UnitTestRuntimeType::OpenHarmony
    };
    let hms_sdk_api_path = if matches!(runtime_type, UnitTestRuntimeType::HarmonyOS) {
      sdk_config.hms_previewer_component_path().ok_or_else(|| {
        anyhow::anyhow!(
          "ARKDOWN_UNIT_TEST_HMS_PREVIEWER_COMPONENT_MISSING: selected HarmonyOS SDK has no HMS Previewer component ({})",
          sdk_config.ets_loader_path.display()
        )
      })?
    } else {
      PathBuf::new()
    };
    let coverage_package_path = sdk_config.hvigor_coverage_package_path().ok_or_else(|| {
      anyhow::anyhow!(
        "ARKDOWN_UNIT_TEST_COVERAGE_COMPONENT_MISSING: selected SDK has no @ohos/coverage package ({})",
        sdk_config.ets_loader_path.display()
      )
    })?;

    let mut seen = IndexSet::new();
    let hsp_dependencies = dependencies
      .edges
      .iter()
      .filter(|edge| !matches!(edge.kind, DependencyEdgeKind::DynamicDependency))
      .filter_map(|edge| {
        let node = dependencies.nodes.get(&edge.package_name)?;
        matches!(
          node.node_type,
          DependencyNodeType::IntegratedHsp | DependencyNodeType::SharedHsp
        )
        .then_some(&edge.package_root)
      })
      .filter(|root| seen.insert((*root).clone()))
      .cloned()
      .collect();

    let plan = Self {
      sdk_api_path,
      hms_sdk_api_path,
      runtime_type,
      module_path: arkdown_path::canonicalize_or_owned(module_path),
      project_path: arkdown_path::canonicalize_or_owned(project_path),
      coverage,
      hsp_dependencies,
      node_executable,
      coverage_package_path,
    };
    if matches!(module_type, OhosModuleType::Shared) {
      tracing::debug!(
        module_path = %plan.module_path.display(),
        "local-test HSP module will use the UnitTestArkTS buildConfig handoff"
      );
    }
    Ok(plan)
  }

  /// Invoke the installed `@ohos/coverage` owner rather than duplicating its
  /// Previewer command, log parser, source-map translation, HSP merge, and HTML
  /// report implementation in Rust.
  pub(crate) fn execute(&self) -> Result<()> {
    let command_line = self
      .coverage_package_path
      .join("lib")
      .join("src")
      .join("commandLine")
      .join("index.js");
    if !command_line.is_file() {
      anyhow::bail!(
        "ARKDOWN_UNIT_TEST_COVERAGE_COMPONENT_MISSING: @ohos/coverage command-line API was not found at {}",
        command_line.display()
      );
    }
    let previewer = self
      .sdk_api_path
      .join("common")
      .join("bin")
      .join(if cfg!(windows) {
        "Previewer.exe"
      } else {
        "Previewer"
      });
    if !previewer.is_file() {
      anyhow::bail!(
        "ARKDOWN_UNIT_TEST_PREVIEWER_COMPONENT_MISSING: Previewer was not found at {}",
        previewer.display()
      );
    }
    if matches!(self.runtime_type, UnitTestRuntimeType::HarmonyOS)
      && self.hms_sdk_api_path.as_os_str().is_empty()
    {
      anyhow::bail!(
        "ARKDOWN_UNIT_TEST_HMS_PREVIEWER_COMPONENT_MISSING: HarmonyOS local tests require the HMS Previewer component"
      );
    }

    let request = serde_json::to_string(self)?;
    let status = std::process::Command::new(&self.node_executable)
      .arg("-e")
      .arg(COVERAGE_COMMAND_RUNNER)
      .arg(&command_line)
      .arg(&request)
      .current_dir(&self.module_path)
      .status()
      .map_err(|error| anyhow::anyhow!("start @ohos/coverage runner failed: {error}"))?;
    if !status.success() {
      anyhow::bail!("ARKDOWN_UNIT_TEST_RESULT_FAILED: @ohos/coverage exited with status {status}");
    }
    Ok(())
  }
}

/// Fixed adapter for the package's Promise API. Arguments are transported as
/// JSON through `Command` (never a shell), and all behavioral work remains in
/// the exact hvigor dependency.
const COVERAGE_COMMAND_RUNNER: &str = r#"
const commandLine = require(process.argv[1]);
const request = JSON.parse(process.argv[2]);
const logger = {
  debug: (...args) => process.env.ARKDOWN_LOG ? console.error(...args) : undefined,
  info: (...args) => console.error(...args),
  warn: (...args) => console.error(...args),
  error: (...args) => console.error(...args),
};
commandLine.executeCoverageTest({ ...request, logger })
  .then(() => process.exit(0))
  .catch((error) => { console.error(error && error.stack || error); process.exit(1); });
"#;

/// SDK-owned coverage insertion plan for `UnitTestArkTS` and
/// `OhosTestArkCompile`.
///
/// Hvigor delegates this work to `@ohos/coverage`'s `bjcTransformer`, which
/// both inserts the runtime counters and writes `init_coverage.json`. Keeping
/// that owner avoids a second, subtly incompatible BJC implementation.
#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct TestCoverageInstrumentationPlan {
  module_path: PathBuf,
  project_path: PathBuf,
  init_coverage_path: PathBuf,
  ets_loader_path: PathBuf,
  coverage_package_path: PathBuf,
  coverage_mode: String,
  test_mode: String,
  sources: Vec<UnitTestCoverageSource>,
  #[serde(skip)]
  node_executable: PathBuf,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct UnitTestCoverageSource {
  source_path: PathBuf,
  module_id: PathBuf,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum TestCoverageMode {
  UnitTest,
  OhosTest,
}

impl TestCoverageMode {
  const fn as_str(self) -> &'static str {
    match self {
      Self::UnitTest => "unitTest",
      Self::OhosTest => "ohosTest",
    }
  }

  const fn path_segment(self) -> &'static str {
    match self {
      Self::UnitTest => "test",
      Self::OhosTest => "ohosTest",
    }
  }
}

impl TestCoverageInstrumentationPlan {
  pub(crate) fn from_build(
    sdk_config: &SdkConfig,
    project_path: &Path,
    module_path: &Path,
    test_mode: TestCoverageMode,
    coverage_mode: Option<&str>,
    module_sources: &[HvigorModuleSourceFile],
    node_executable: PathBuf,
  ) -> Result<Self> {
    let coverage_package_path = sdk_config.hvigor_coverage_package_path().ok_or_else(|| {
      anyhow::anyhow!(
        "ARKDOWN_UNIT_TEST_COVERAGE_COMPONENT_MISSING: selected SDK has no @ohos/coverage package ({})",
        sdk_config.ets_loader_path.display()
      )
    })?;
    let sources = module_sources
      .iter()
      .filter_map(|source| {
        let module_id = PathBuf::from(&source.module_id);
        module_id.is_file().then(|| UnitTestCoverageSource {
          source_path: source.path.clone(),
          module_id,
        })
      })
      .collect();
    // `@ohos/coverage` 1.0.24 `buildInstrument.beforeBuildEnd` uses this
    // fixed root even when the command overrides the build product.
    let layout = ModuleBuildLayout::for_test_product(module_path, "default");
    Ok(Self {
      module_path: arkdown_path::canonicalize_or_owned(module_path),
      project_path: arkdown_path::canonicalize_or_owned(project_path),
      init_coverage_path: layout
        .intermediates()
        .join(test_mode.path_segment())
        .join("init_coverage.json"),
      ets_loader_path: sdk_config.ets_loader_path.clone(),
      coverage_package_path,
      coverage_mode: coverage_mode.unwrap_or("white").to_string(),
      test_mode: test_mode.as_str().to_string(),
      sources,
      node_executable,
    })
  }

  pub(crate) fn execute(&self) -> Result<()> {
    let request = serde_json::to_string(self)?;
    let output = std::process::Command::new(&self.node_executable)
      .arg("-e")
      .arg(COVERAGE_INSTRUMENTATION_RUNNER)
      .arg(&request)
      .current_dir(&self.module_path)
      .output()
      .map_err(|error| anyhow::anyhow!("start @ohos/coverage instrumenter failed: {error}"))?;
    if !output.status.success() {
      anyhow::bail!(
        "ARKDOWN_TEST_COVERAGE_INSTRUMENTATION_FAILED: @ohos/coverage exited with status {}: {}",
        output.status,
        String::from_utf8_lossy(&output.stderr).trim()
      );
    }
    serde_json::from_slice::<BTreeSet<PathBuf>>(&output.stdout).map_err(|error| {
      anyhow::anyhow!(
        "ARKDOWN_TEST_COVERAGE_INSTRUMENTATION_FAILED: invalid @ohos/coverage result: {error}"
      )
    })?;
    Ok(())
  }
}

/// Process-local adapter around the installed SDK package. Every project and
/// artifact path is transported as a native path value; package-relative
/// modules are resolved with Node's platform-aware `path.join`.
const COVERAGE_INSTRUMENTATION_RUNNER: &str = r#"
const fs = require('fs');
const path = require('path');
const request = JSON.parse(process.argv[1]);
const coverageSource = (...parts) => path.join(request.coveragePackagePath, 'lib', 'src', ...parts);
const tsc = require(path.join(request.etsLoaderPath, 'node_modules', 'typescript'));
require(coverageSource('plugin', 'globals.js')).Globals.tsc = tsc;
const { bjcTransformer } = require(coverageSource('bjc', 'index.js'));
const filter = require(coverageSource('filter', 'filterUtil.js'));
const configured = filter.getCoverageFilterJson5(request.projectPath);
const blackList = filter.getBlackList(configured);
const whiteList = filter.getWhiteList(configured);
filter.checkBlackWhiteList(blackList, whiteList);
const defaultBlackList = filter.getDefaultBlackList(request.projectPath, request.modulePath);
fs.rmSync(request.initCoveragePath, { force: true });
const instrumentedModuleIds = [];
for (const source of request.sources) {
  const moduleId = source.moduleId.replace(/\\/g, '/');
  if (filter.needToFilter(moduleId, defaultBlackList, blackList, whiteList)) continue;
  // Hvigor's `buildInstrument.beforeBuildEnd` transforms
  // `share.ModuleSourceFile.module.source`: the already materialized
  // post-ArkUI TypeScript source, while retaining moduleId for filtering and
  // coverage attribution.
  const code = fs.readFileSync(source.sourcePath, 'utf8');
  const sourceFile = tsc.createSourceFile(source.moduleId, code, tsc.ScriptTarget.Latest, true);
  const transformed = tsc.transform(sourceFile, [bjcTransformer({
    projectTopDir: request.projectPath,
    modulePath: request.modulePath,
    coverageMode: request.coverageMode,
    testMode: request.testMode,
  })]);
  try {
    const printer = tsc.createPrinter({ newLine: tsc.NewLineKind.LineFeed });
    fs.writeFileSync(source.sourcePath, printer.printFile(transformed.transformed[0]), 'utf8');
    instrumentedModuleIds.push(source.moduleId);
  } finally {
    transformed.dispose();
  }
}
process.stdout.write(JSON.stringify(instrumentedModuleIds));
if (fs.existsSync(request.initCoveragePath)) {
  const content = fs.readFileSync(request.initCoveragePath, 'utf8').trim();
  if (content && !(content.startsWith('{') && content.endsWith('}'))) {
    fs.writeFileSync(request.initCoveragePath, `{\n${content}\n}`, 'utf8');
  }
}
"#;

/// Artifact handoffs owned by UnitTestArkTS and its two independently
/// selectable post-compile tasks.
pub(crate) struct UnitTestArtifactWriter<'a> {
  layout: ModuleBuildLayout<'a>,
}

impl<'a> UnitTestArtifactWriter<'a> {
  pub(crate) fn new(module_root: &'a Path, product_name: &'a str) -> Self {
    Self {
      layout: ModuleBuildLayout::for_test_product(module_root, product_name),
    }
  }

  /// Device-test variant of Hvigor's shared `TestCopyMockConfigJson` action.
  /// Unlike local unit tests it keeps the normal `build/<product>` root and
  /// writes into the selected `ohosTest` target's compiled resources.
  pub(crate) fn for_ohos_test(
    module_root: &'a Path,
    product_name: &'a str,
    target_name: &'a str,
  ) -> Self {
    Self {
      layout: ModuleBuildLayout::for_target_product(module_root, product_name, target_name),
    }
  }

  pub(crate) fn stage_compiled_assets(
    &self,
    modules_abc: &Path,
    source_maps: Option<&Path>,
  ) -> Result<()> {
    let output = self.layout.test_assets_ets_dir();
    std::fs::create_dir_all(&output)?;
    std::fs::copy(modules_abc, self.layout.test_assets_modules_abc())?;
    if let Some(source_maps) = source_maps.filter(|path| path.is_file()) {
      std::fs::copy(source_maps, self.layout.test_assets_source_maps())?;
    }
    Ok(())
  }

  /// `TestCopyMockConfigJson.copyMockConfigJson` — a missing compiler output
  /// is a debug-only no-op in hvigor.
  pub(crate) fn copy_mock_config_json(&self) -> Result<()> {
    let input = self.layout.intermediates_mock_config_json();
    if !input.is_file() {
      return Ok(());
    }
    let output = self.layout.test_mock_config_output_json();
    if let Some(parent) = output.parent() {
      std::fs::create_dir_all(parent)?;
    }
    std::fs::copy(input, output)?;
    Ok(())
  }

  /// `ReplaceSourcemapInCoverageMode` merges the coverage transformer's
  /// replacement mappings into the UnitTestArkTS asset source map.
  pub(crate) fn replace_source_map_in_coverage_mode(&self, coverage: bool) -> Result<()> {
    if !coverage {
      return Ok(());
    }
    let replacement = self.layout.test_coverage_source_map();
    let output = self.layout.test_assets_source_maps();
    if !replacement.is_file() || !output.is_file() {
      return Ok(());
    }

    let mut fragment = std::fs::read_to_string(&replacement)?;
    if fragment.ends_with(',') {
      fragment.pop();
    }
    let replacements =
      serde_json::from_str::<serde_json::Map<String, Value>>(&format!("{{{fragment}}}"))?;
    let mut source_maps = serde_json::from_slice::<Value>(&std::fs::read(&output)?)?;
    let Some(source_map_entries) = source_maps.as_object_mut() else {
      return Ok(());
    };
    for (source, replacement) in replacements {
      let Some(mapping) = replacement.get("mappings").cloned() else {
        continue;
      };
      for source_map in source_map_entries.values_mut() {
        let matches_source = source_map
          .get("sources")
          .and_then(Value::as_array)
          .is_some_and(|sources| {
            sources.len() == 1 && sources[0].as_str() == Some(source.as_str())
          });
        if matches_source {
          if let Some(source_map) = source_map.as_object_mut() {
            source_map.insert("mappings".to_string(), mapping.clone());
          }
          break;
        }
      }
    }
    std::fs::write(output, serde_json::to_vec_pretty(&source_maps)?)?;
    Ok(())
  }

  pub(crate) fn write_hsp_build_config(&self) -> Result<()> {
    let path = self.layout.test_build_config_json();
    if let Some(parent) = path.parent() {
      std::fs::create_dir_all(parent)?;
    }
    let ace_module_build = self.layout.test_assets_ets_dir();
    std::fs::write(
      path,
      serde_json::to_vec(&serde_json::json!({
        "aceModuleBuild": arkdown_path::ArtifactPathSerializer::path_to_slash_string(&ace_module_build)?
      }))?,
    )?;
    Ok(())
  }
}

/// `UnitTestProcessProfile.doTaskAction` — append the synthetic `TestAbility`
/// to the processed module document and write both the processed profile
/// (`module.json`, `\t` indentation) and the ark module json.
///
/// `module_type` is `"entry"`/`"feature"`/`"shared"`/`"har"` as resolved by
/// the module model; HAR/HSP modules get `deliveryWithInstall` rewritten and
/// HAR additionally `module.type = "entry"` plus `app.icon`/`app.label`.
pub(crate) fn inject_unit_test_ability(
  process_profile: &mut Value,
  module_type: &str,
) -> Result<()> {
  let document = process_profile
    .as_object_mut()
    .ok_or_else(|| anyhow::anyhow!("process_profile module.json is not a JSON object"))?;

  let is_har = module_type == "har";
  let is_hsp = module_type == "shared";
  if is_hsp || is_har {
    let module = document
      .get_mut("module")
      .and_then(Value::as_object_mut)
      .ok_or_else(|| anyhow::anyhow!("process_profile module.json lacks module object"))?;
    module.insert("deliveryWithInstall".into(), Value::Bool(is_har));
    if is_har {
      module.insert("type".into(), "entry".into());
      let app = document
        .get_mut("app")
        .and_then(Value::as_object_mut)
        .ok_or_else(|| anyhow::anyhow!("process_profile module.json lacks app object"))?;
      app.insert("icon".into(), ICON.into());
      app.insert("label".into(), LABEL.into());
    }
  }

  let module = document
    .get_mut("module")
    .and_then(Value::as_object_mut)
    .ok_or_else(|| anyhow::anyhow!("process_profile module.json lacks module object"))?;
  let abilities = module
    .entry("abilities")
    .or_insert_with(|| Value::Array(Vec::new()));
  let abilities = abilities
    .as_array_mut()
    .ok_or_else(|| anyhow::anyhow!("process_profile module.json abilities is not an array"))?;

  let (start_window_icon, start_window_background) = abilities
    .first()
    .and_then(Value::as_object)
    .map(|first| {
      (
        first
          .get("startWindowIcon")
          .cloned()
          .unwrap_or_else(|| Value::String("startIcon".to_string())),
        first
          .get("startWindowBackground")
          .cloned()
          .unwrap_or_else(|| Value::String("#FFFFFF".to_string())),
      )
    })
    .unwrap_or_else(|| {
      (
        Value::String("startIcon".to_string()),
        Value::String("#FFFFFF".to_string()),
      )
    });

  abilities.push(serde_json::json!({
    "name": TEST_ABILITY,
    "srcEntrance": ENTRY_TEST_ABILITY_PATH,
    "startWindowIcon": start_window_icon,
    "startWindowBackground": start_window_background,
  }));
  Ok(())
}

/// `ReplaceUnitTestIndexFile.doTaskAction` — set `module.pages =
/// "$profile:main_pages"` on the res module.json (falling back to the
/// processed profile when it does not exist) and write
/// `resources/base/profile/main_pages.json` as `{ "src": ["<replace page>"] }`.
///
/// HAP modules with an existing `pages` value keep it (hvigor's `!pages`
/// guard); HAR always rewrites; HSP rewrites only when `pages` is absent.
pub(crate) fn replace_unit_test_index_file(
  res_module_json: &Path,
  process_profile: &Value,
  res_profile_dir: &Path,
  module_type: &str,
  replace_page: &str,
) -> Result<()> {
  let page_json_file = res_profile_dir.join("main_pages.json");
  let write_main_pages = |page_json_file: &Path| -> Result<()> {
    if let Some(parent) = page_json_file.parent() {
      std::fs::create_dir_all(parent)?;
    }
    std::fs::write(
      page_json_file,
      serde_json::to_string_pretty(&serde_json::json!({ "src": [replace_page] }))?,
    )?;
    Ok(())
  };

  if module_type == "har" {
    write_module_json(res_module_json, process_profile)?;
    return write_main_pages(&page_json_file);
  }

  let existing_pages = if res_module_json.is_file() {
    let content = std::fs::read_to_string(res_module_json)?;
    serde_json::from_str::<Value>(&content)
      .ok()
      .and_then(|doc| doc.get("module").and_then(|m| m.get("pages")).cloned())
  } else {
    None
  };
  let is_hsp = module_type == "shared";
  if !is_hsp {
    // HAP: only rewrite when pages is absent.
    if existing_pages.is_some() {
      return write_main_pages(&page_json_file);
    }
  } else if existing_pages.is_some() {
    // HSP: keep an existing pages; still emit main_pages.json.
    return write_main_pages(&page_json_file);
  }
  write_module_json(res_module_json, process_profile)?;
  write_main_pages(&page_json_file)
}

fn write_module_json(res_module_json: &Path, process_profile: &Value) -> Result<()> {
  let mut document = if res_module_json.is_file() {
    let content = std::fs::read_to_string(res_module_json)?;
    serde_json::from_str::<Value>(&content).unwrap_or_else(|_| process_profile.clone())
  } else {
    process_profile.clone()
  };
  let module_pages = |document: &mut Value| {
    if let Some(module) = document.get_mut("module").and_then(Value::as_object_mut) {
      module.insert("pages".into(), PAGE_JSON_RESOURCE.into());
      true
    } else {
      false
    }
  };
  if !module_pages(&mut document) {
    // Neither the res module.json nor the processed profile has a module
    // section — hvigor synthesizes `{ "module": { "pages": ... } }`.
    document = serde_json::json!({ "module": { "pages": PAGE_JSON_RESOURCE } });
  }
  if let Some(parent) = res_module_json.parent() {
    std::fs::create_dir_all(parent)?;
  }
  std::fs::write(res_module_json, serde_json::to_string_pretty(&document)?)?;
  Ok(())
}

/// `@ohos/coverage` `pageIndexTemplate` — the local unit-test entry page.
/// Placeholders: `__MODULE_PATH__`, `__BUNDLE_NAME__`, `__MODULE_NAME__`,
/// `__SCOPE__`, `__COVERAGE__`.
const PAGE_INDEX_TEMPLATE: &str = "\n    import hilog from '@ohos.hilog';\n    import AbilityDelegatorRegistry from '@ohos.app.ability.abilityDelegatorRegistry'\n\n    import {Hypium} from '@ohos/hypium'\n    import testsuite from '../../../src/test/List.test'\n\n    const global: object = new Function(\"return this\")();\n    const savePath = '__savePath__';\n    const readPath = '__readPath__';\n    const testMode: string = '__testMode__';\n    global[savePath] = \"__MODULE_PATH__/.test/default/intermediates/test/coverage_data/js_coverage.json\";\n    global[readPath] = \"__MODULE_PATH__/.test/default/intermediates/test/coverage_data/js_coverage.json\"\n    global[testMode] = 'unitTest';\n\n    @Entry\n    @Component\n    struct Index1 {\n      aboutToAppear() {\n        console.info('[LOCAL_TEST] START')\n        let abilityDelegator: AbilityDelegatorRegistry.AbilityDelegator;\n        abilityDelegator = AbilityDelegatorRegistry.getAbilityDelegator();\n        let abilityDelegatorArguments: AbilityDelegatorRegistry.AbilityDelegatorArgs;\n        abilityDelegatorArguments = AbilityDelegatorRegistry.getArguments();\n        abilityDelegatorArguments.bundleName = \"__BUNDLE_NAME__\";\n        abilityDelegatorArguments.parameters = {\n          \"-b\": \"__BUNDLE_NAME__\",\n          \"-m\": \"__MODULE_NAME__\",\n          \"-s class\": \"__SCOPE__\",\n          \"-s timeout\": \"15000\",\n          \"-s coverage\": \"__COVERAGE__\"\n        };\n        abilityDelegatorArguments.testCaseNames = \"__SCOPE__\"\n        console.info(\"[LOCAL_TEST] \" + abilityDelegatorArguments);\n        Hypium.hypiumTest(abilityDelegator, abilityDelegatorArguments, testsuite)\n        console.info('[LOCAL_TEST] END')\n      }\n    \n      @State\n      message: string = 'Hello World'\n    \n      \n  build() {\n    Row() {\n      Column() {\n        Text(this.message)\n          .fontSize(50)\n          .fontWeight(FontWeight.Bold)\n        Button() {\n          Text('next page')\n            .fontSize(20)\n            .fontWeight(FontWeight.Bold)\n        }\n        .type(ButtonType.Capsule)\n        .margin({\n          top: 20\n        })\n        .backgroundColor('#0D9FFB')\n        .width('35%')\n        .height('5%')\n        .onClick(() => {\n        })\n      }\n      .width('100%')\n    }\n    .height('100%')\n  }\n\n    }";

/// `@ohos/coverage` `testAbilityTemplate` — fixed `EntryAbility` that loads
/// the generated test page.
const TEST_ABILITY_TEMPLATE: &str = "\n    import AbilityConstant from '@ohos.app.ability.AbilityConstant';\n    import hilog from '@ohos.hilog';\n    import UIAbility from '@ohos.app.ability.UIAbility';\n    import Want from '@ohos.app.ability.Want';\n    import window from '@ohos.window';\n    \n    export default class EntryAbility extends UIAbility {\n        onCreate(want: Want, launchParam: AbilityConstant.LaunchParam) {\n            hilog.info(0x0000, 'testTag', '%{public}s', 'Ability onCreate');\n        }\n    \n        onDestroy() {\n            hilog.info(0x0000, 'testTag', '%{public}s', 'Ability onDestroy');\n        }\n    \n        onWindowStageCreate(windowStage: window.WindowStage) {\n            // Main window is created, set main page for this ability\n            hilog.info(0x0000, 'testTag', '%{public}s', 'Ability onWindowStageCreate');\n    \n            windowStage.loadContent('../../../.test/testability/pages/Index', (err, data) => {\n                if (err.code) {\n                    hilog.error(0x0000, 'testTag', 'Failed to load the content. Cause: %{public}s', JSON.stringify(err) ?? '');\n                    return;\n                }\n                hilog.info(0x0000, 'testTag', 'Succeeded in loading the content. Data: %{public}s', JSON.stringify(data) ?? '');\n            });\n        }\n    \n        onWindowStageDestroy() {\n            // Main window is destroyed, release UI related resources\n            hilog.info(0x0000, 'testTag', '%{public}s', 'Ability onWindowStageDestroy');\n        }\n    \n        onForeground() {\n            // Ability has brought to foreground\n            hilog.info(0x0000, 'testTag', '%{public}s', 'Ability onForeground');\n        }\n    \n        onBackground() {\n            // Ability has back to background\n            hilog.info(0x0000, 'testTag', '%{public}s', 'Ability onBackground');\n        }\n    }\n  ";

/// `@ohos/coverage` `phoneSettingConfigTemplate` — fixed phone settings.
const PHONE_SETTING_CONFIG_TEMPLATE: &str = "\n    {\n      \"setting\": {\n        \"1.0.1\": {\n          \"Language\": {\n            \"args\": {\n              \"Language\": \"zh_CN\"\n            }\n          }\n        }\n      },\n      \"frontend\": {\n        \"1.0.0\": {\n          \"Resolution\": {\n            \"args\": {\n              \"Resolution\": \"360*780\"\n            }\n          },\n          \"DeviceType\": {\n            \"args\": {\n              \"DeviceType\": \"phone\"\n            }\n          }\n        }\n      }\n    }\n  ";

const OPEN_HARMONY_TEST_RUNNER_TEMPLATE: &str = "
    import hilog from '@ohos.hilog';\n
    import TestRunner from '@ohos.application.testRunner';\n
    import AbilityDelegatorRegistry from '@ohos.app.ability.abilityDelegatorRegistry';\n
    import Want from '@ohos.app.ability.Want';\n
    \n
    let abilityDelegator: AbilityDelegatorRegistry.AbilityDelegator\n
    let abilityDelegatorArguments: AbilityDelegatorRegistry.AbilityDelegatorArgs | undefined = undefined\n
    let tag: string = 'testTag';\n
    const UNLOCK_ERROR = -3;\n
    \n
    async function onAbilityCreateCallback() {\n
      hilog.info(0x0000, 'testTag', '%{public}s', 'onAbilityCreateCallback');\n
    }\n
    \n
    async function addAbilityMonitorCallback(err : Error) {\n
      hilog.info(0x0000, 'testTag', 'addAbilityMonitorCallback : %{public}s', JSON.stringify(err) ?? '');\n
    }\n
    \n
    export default class OpenHarmonyTestRunner implements TestRunner {\n
      constructor() {\n
      }\n
    \n
      onPrepare() {\n
        hilog.info(0x0000, 'testTag', '%{public}s', 'OpenHarmonyTestRunner OnPrepare ');\n
      }\n
    \n
      async onRun() {\n
        hilog.info(0x0000, 'testTag', '%{public}s', 'OpenHarmonyTestRunner onRun run');\n
        abilityDelegatorArguments = AbilityDelegatorRegistry.getArguments()\n
        abilityDelegator = AbilityDelegatorRegistry.getAbilityDelegator()\n
        const bundleName = abilityDelegatorArguments.bundleName;\n
        const testAbilityName = 'TestAbility';\n
        const parameters = abilityDelegatorArguments.parameters;\n
        const moduleName = parameters['-m'];\n
        const isDebug = parameters['-D'] == 'true';\n
        let lMonitor: AbilityDelegatorRegistry.AbilityMonitor = {\n
          abilityName: testAbilityName,\n
          onAbilityCreate: onAbilityCreateCallback,\n
          moduleName: moduleName\n
        };\n
        abilityDelegator.addAbilityMonitor(lMonitor, addAbilityMonitorCallback)\n
        await startAbility(bundleName, testAbilityName, moduleName, isDebug);\n
        hilog.info(0x0000, 'testTag', '%{public}s', 'OpenHarmonyTestRunner onRun end');\n
      }\n
    }\n
    \n
    async function startAbility(bundleName: string, abilityName: string, moduleName: string, isDebug: boolean) {\n
      let command = `aa start -b ${bundleName} -a ${abilityName} -m ${moduleName}`;\n
      if (isDebug) {\n
        command += ' -D';\n
      }\n
      await abilityDelegator.executeShellCommand(command).then(result => {\n
        if (result.stdResult.includes(\"error: unlock screen failed in developer mode\")\n
        || result.stdResult.includes(\"10106102\")) {\n
          hilog.info(0x0000, tag, '%{public}s', `startAbility failed ${result.stdResult}`);\n
          abilityDelegator.finishTest(\"\", UNLOCK_ERROR, () => {});\n
          return;\n
        }\n
      }).catch((err: string) => {\n
        hilog.info(0x0000, tag, '%{public}s', `startAbility error: ${err}`);\n
      })\n
    }
  ";

const OPEN_HARMONY_TEST_RUNNER_MOCK_TEMPLATE: &str = "
    import { abilityDelegatorRegistry, TestRunner } from '@kit.TestKit';\n
    import { UIAbility, common, application } from '@kit.AbilityKit';\n
    import { BusinessError } from '@kit.BasicServicesKit';\n
    import { hilog } from '@kit.PerformanceAnalysisKit';\n
    import { resourceManager } from '@kit.LocalizationKit';\n
    import { util } from '@kit.ArkTS';\n
    \n
    let abilityDelegator: abilityDelegatorRegistry.AbilityDelegator;\n
    let abilityDelegatorArguments: abilityDelegatorRegistry.AbilityDelegatorArgs;\n
    let jsonPath: string = 'mock/mock-config.json';\n
    let tag: string = 'testTag';\n
    const UNLOCK_ERROR = -3;\n
    const global: object = new Function(\"return this\")();\n
    \n
    async function onAbilityCreateCallback(data: UIAbility) {\n
      hilog.info(0x0000, tag, 'onAbilityCreateCallback, data: ${}', JSON.stringify(data));\n
    }\n
    \n
    async function addAbilityMonitorCallback(err: BusinessError) {\n
      hilog.info(0x0000, tag, 'addAbilityMonitorCallback : %{public}s', JSON.stringify(err) ?? '');\n
    }\n
    \n
    export default class OpenHarmonyTestRunner implements TestRunner {\n
      constructor() {\n
      }\n
    \n
      onPrepare() {\n
        hilog.info(0x0000, tag, '%{public}s', 'OpenHarmonyTestRunner OnPrepare');\n
      }\n
    \n
      async onRun() {\n
        hilog.info(0x0000, tag, '%{public}s', 'OpenHarmonyTestRunner onRun run');\n
        abilityDelegatorArguments = abilityDelegatorRegistry.getArguments()\n
        abilityDelegator = abilityDelegatorRegistry.getAbilityDelegator()\n
        let moduleName = abilityDelegatorArguments.parameters['-m'];\n
    \n
        const savePath: string = '__savePath__';\n
        const readPath: string = '__readPath__';\n
        const testMode: string = '__testMode__';\n
        let uid: number = Math.floor(abilityDelegator.getAppContext().applicationInfo.uid / 200000);\n
        const bundleName: string = abilityDelegatorArguments.bundleName;\n
        global[savePath] = '/data/storage/el2/base/js_coverage.json';\n
        global[readPath] = '/data/app/el2/' + uid + '/base/' + bundleName + '/js_coverage.json';\n
        global[testMode] = 'ohostest';\n
    \n
        try {\n
          application.createModuleContext(abilityDelegator.getAppContext().getApplicationContext(), moduleName)\n
            .then((data: Context)=> {\n
              doOnRun(data, moduleName);\n
          }).catch((error : BusinessError)=>{\n
            console.error(`test runner createModuleContext failed, error.code: ${(error as BusinessError).code}, error.message: ${(error as BusinessError).message}`);\n
            return;\n
          })\n
        } catch (error) {\n
          console.error(`test runner createModuleContext failed, error.code: ${(error as BusinessError).code}, error.message: ${(error as BusinessError).message}`);\n
          return;\n
        }\n
      }\n
    }\n
    \n
    async function doOnRun(context: common.Context, moduleName: string) {\n
      let mResourceManager = context.resourceManager;\n
      await checkMock(abilityDelegator, mResourceManager);\n
      const bundleName = abilityDelegatorArguments.bundleName;\n
      const testAbilityName: string = 'TestAbility';\n
      const parameters = abilityDelegatorArguments.parameters;\n
      const isDebug = parameters['-D'] == 'true';\n
      let lMonitor: abilityDelegatorRegistry.AbilityMonitor = {\n
        abilityName: testAbilityName,\n
        onAbilityCreate: onAbilityCreateCallback,\n
        moduleName: moduleName\n
      };\n
      abilityDelegator.addAbilityMonitor(lMonitor, addAbilityMonitorCallback)\n
      await startAbility(bundleName, testAbilityName, moduleName, isDebug);\n
      hilog.info(0x0000, tag, '%{public}s', 'OpenHarmonyTestRunner onRun end');\n
    }\n
    \n
    async function startAbility(bundleName: string, abilityName: string, moduleName: string, isDebug: boolean) {\n
      let command = `aa start -b ${bundleName} -a ${abilityName} -m ${moduleName}`;\n
      if (isDebug) {\n
        command += ' -D';\n
      }\n
      await abilityDelegator.executeShellCommand(command).then(result => {\n
        if (result.stdResult.includes(\"error: unlock screen failed in developer mode\")\n
        || result.stdResult.includes(\"10106102\")) {\n
          hilog.info(0x0000, tag, '%{public}s', `startAbility failed ${result.stdResult}`);\n
          abilityDelegator.finishTest(\"\", UNLOCK_ERROR, () => {});\n
          return;\n
        }\n
      }).catch((err: string) => {\n
        hilog.info(0x0000, tag, '%{public}s', `startAbility error: ${err}`);\n
      })\n
    }\n
    \n
    async function checkMock(abilityDelegator: abilityDelegatorRegistry.AbilityDelegator, resourceManager: resourceManager.ResourceManager) {\n
      let rawFile: Uint8Array;\n
      try {\n
        rawFile = resourceManager.getRawFileContentSync(jsonPath);\n
        hilog.info(0x0000, tag, 'MockList file exists');\n
        let mockStr: string = util.TextDecoder.create('utf-8', { ignoreBOM: true }).decodeToString(rawFile);\n
        let mockMap: Record<string, string> = getMockList(mockStr);\n
        try {\n
          abilityDelegator.setMockList(mockMap)\n
        } catch (error) {\n
          let code = (error as BusinessError).code;\n
          let message = (error as BusinessError).message;\n
          hilog.error(0x0000, tag, `abilityDelegator.setMockList failed, error code: ${code}, message: ${message}.`);\n
        }\n
      } catch (error) {\n
        let code = (error as BusinessError).code;\n
        let message = (error as BusinessError).message;\n
        hilog.error(0x0000, tag, `ResourceManager:callback getRawFileContent failed, error code: ${code}, message: ${message}.`);\n
      }\n
    }\n
    \n
    function getMockList(jsonStr: string) {\n
      let jsonObj: Record<string, Object> = JSON.parse(jsonStr);\n
      let map: Map<string, object> = new Map<string, object>(Object.entries(jsonObj));\n
      let mockList: Record<string, string> = {};\n
      map.forEach((value: object, key: string) => {\n
        let realValue: string = value['source'].toString();\n
        mockList[key] = realValue;\n
      });\n
      hilog.info(0x0000, tag, '%{public}s', 'mock-json value:' + JSON.stringify(mockList) ?? '');\n
      return mockList;\n
    }
  ";

const TEST_ABILITY_TEMPLATE_OHOS: &str = "
    import UIAbility from '@ohos.app.ability.UIAbility';\n
    import AbilityDelegatorRegistry from '@ohos.app.ability.abilityDelegatorRegistry';\n
    import hilog from '@ohos.hilog';\n
    import { Hypium } from '@ohos/hypium';\n
    import testsuite from '../../../../../../../src/ohosTest/ets/test/List.test';\n
    import window from '@ohos.window';\n
    import Want from '@ohos.app.ability.Want';\n
    import AbilityConstant from '@ohos.app.ability.AbilityConstant';\n
    \n
    export default class TestAbility extends UIAbility {\n
        onCreate(want: Want, launchParam: AbilityConstant.LaunchParam) {\n
            hilog.info(0x0000, 'testTag', '%{public}s', 'TestAbility onCreate');\n
            hilog.info(0x0000, 'testTag', '%{public}s', 'want param:' + JSON.stringify(want) ?? '');\n
            hilog.info(0x0000, 'testTag', '%{public}s', 'launchParam:'+ JSON.stringify(launchParam) ?? '');\n
            var abilityDelegator: any\n
            abilityDelegator = AbilityDelegatorRegistry.getAbilityDelegator()\n
            var abilityDelegatorArguments: any\n
            abilityDelegatorArguments = AbilityDelegatorRegistry.getArguments()\n
            hilog.info(0x0000, 'testTag', '%{public}s', 'start run testcase!!!');\n
            Hypium.hypiumTest(abilityDelegator, abilityDelegatorArguments, testsuite)\n
        }\n
    \n
        onDestroy() {\n
            hilog.info(0x0000, 'testTag', '%{public}s', 'TestAbility onDestroy');\n
        }\n
    \n
        onWindowStageCreate(windowStage: window.WindowStage) {\n
            hilog.info(0x0000, 'testTag', '%{public}s', 'TestAbility onWindowStageCreate');\n
            windowStage.loadContent('testability/pages/Index', (err, data) => {\n
                if (err.code) {\n
                    hilog.error(0x0000, 'testTag', 'Failed to load the content. Cause: %{public}s', JSON.stringify(err) ?? '');\n
                    return;\n
                }\n
                hilog.info(0x0000, 'testTag', 'Succeeded in loading the content. Data: %{public}s',\n
                    JSON.stringify(data) ?? '');\n
            });\n
        }\n
    \n
        onWindowStageDestroy() {\n
            hilog.info(0x0000, 'testTag', '%{public}s', 'TestAbility onWindowStageDestroy');\n
        }\n
    \n
        onForeground() {\n
            hilog.info(0x0000, 'testTag', '%{public}s', 'TestAbility onForeground');\n
        }\n
    \n
        onBackground() {\n
            hilog.info(0x0000, 'testTag', '%{public}s', 'TestAbility onBackground');\n
        }\n
    }
  ";

const TEST_ABILITY_WITH_KIT_TEMPLATE: &str = "
    import { AbilityConstant, UIAbility, Want } from '@kit.AbilityKit';\n
    import { abilityDelegatorRegistry } from '@kit.TestKit';\n
    import { hilog } from '@kit.PerformanceAnalysisKit';\n
    import { window } from '@kit.ArkUI';\n
    import { Hypium } from '@ohos/hypium';\n
    import testsuite from '../../../../../../../src/ohosTest/ets/test/List.test';\n
    \n
    const ON_DESTROY_ERROR = -2;\n
    \n
    export default class TestAbility extends UIAbility {\n
      abilityDelegator: abilityDelegatorRegistry.AbilityDelegator;\n
    \n
      constructor() {\n
        super();\n
        this.abilityDelegator = abilityDelegatorRegistry.getAbilityDelegator();\n
      }\n
    \n
      onCreate(want: Want, launchParam: AbilityConstant.LaunchParam) {\n
        hilog.info(0x0000, 'testTag', '%{public}s', 'TestAbility onCreate');\n
        hilog.info(0x0000, 'testTag', '%{public}s', 'want param:' + JSON.stringify(want) ?? '');\n
        hilog.info(0x0000, 'testTag', '%{public}s', 'launchParam:' + JSON.stringify(launchParam) ?? '');\n
        let abilityDelegatorArguments: abilityDelegatorRegistry.AbilityDelegatorArgs;\n
        abilityDelegatorArguments = abilityDelegatorRegistry.getArguments();\n
        hilog.info(0x0000, 'testTag', '%{public}s', 'start run testcase!!!');\n
        Hypium.hypiumTest(this.abilityDelegator, abilityDelegatorArguments, testsuite);\n
      }\n
    \n
      onDestroy() {\n
        hilog.info(0x0000, 'testTag', '%{public}s', 'TestAbility onDestroy');\n
        this.abilityDelegator.finishTest('TestAbility onDestroy unexpectedly!', ON_DESTROY_ERROR, () => {});\n
      }\n
    \n
      onWindowStageCreate(windowStage: window.WindowStage) {\n
        hilog.info(0x0000, 'testTag', '%{public}s', 'TestAbility onWindowStageCreate');\n
        windowStage.loadContent('testability/pages/Index', (err) => {\n
          if (err.code) {\n
            hilog.error(0x0000, 'testTag', 'Failed to load the content. Cause: %{public}s', JSON.stringify(err) ?? '');\n
            return;\n
          }\n
          hilog.info(0x0000, 'testTag', 'Succeeded in loading the content.');\n
        });\n
      }\n
    \n
      onWindowStageDestroy() {\n
        hilog.info(0x0000, 'testTag', '%{public}s', 'TestAbility onWindowStageDestroy');\n
      }\n
    \n
      onForeground() {\n
        hilog.info(0x0000, 'testTag', '%{public}s', 'TestAbility onForeground');\n
      }\n
    \n
      onBackground() {\n
        hilog.info(0x0000, 'testTag', '%{public}s', 'TestAbility onBackground');\n
      }\n
    }
  ";

const OHOS_TEST_INDEX_TEMPLATE: &str = "
    @Entry\n
    @Component\n
    struct Index {\n
      @State message: string = 'Hello World';\n
    \n
      build() {\n
        Row() {\n
          Column() {\n
            Text(this.message)\n
              .fontSize(50)\n
              .fontWeight(FontWeight.Bold)\n
          }\n
          .width('100%')\n
        }\n
        .height('100%')\n
      }\n
    }
  ";

const OHOS_TEST_COLOR_TEMPLATE: &str = "
    {\n
      \"color\": [\n
        {\n
          \"name\": \"start_window_background\",\n
          \"value\": \"#FFFFFF\"\n
        }\n
      ]\n
    }
  ";

const OHOS_TEST_STRING_TEMPLATE: &str = "
    {\n
      \"string\": [\n
        {\n
          \"name\": \"module_test_desc\",\n
          \"value\": \"test ability description\"\n
        },\n
        {\n
          \"name\": \"TestAbility_desc\",\n
          \"value\": \"the test ability\"\n
        },\n
        {\n
          \"name\": \"TestAbility_label\",\n
          \"value\": \"test label\"\n
        }\n
      ]\n
    }
  ";

const OHOS_TEST_PAGES_TEMPLATE: &str = "
    {\n
      \"src\": [\n
        \"testability/pages/Index\"\n
      ]\n
    }\n
    
  ";
