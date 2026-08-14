use std::path::{Path, PathBuf};

use indexmap::IndexMap;
use serde::{Deserialize, Serialize};

use super::task_info::HvigorTaskInfoCatalog;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Deserialize, Serialize)]
#[serde(rename_all = "lowercase")]
pub(crate) enum RuntimeTaskNodeKind {
  App,
  Hap,
  Hsp,
  Har,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct RuntimeTaskCatalogRequest {
  pub(super) node_name: String,
  pub(super) kind: Option<RuntimeTaskNodeKind>,
  pub(super) project_task: bool,
  #[serde(default)]
  project_root: Option<PathBuf>,
  #[serde(default)]
  node_root: Option<PathBuf>,
  #[serde(default)]
  ets_loader_path: Option<PathBuf>,
  #[serde(default)]
  js_loader_path: Option<PathBuf>,
  #[serde(default)]
  has_rollup_plugin_in_ets_loader: Option<bool>,
  #[serde(default)]
  has_rollup_plugin_in_js_loader: Option<bool>,
  #[serde(default)]
  build_target_name: Option<String>,
  #[serde(default)]
  target_names: Vec<String>,
  #[serde(default)]
  target_options: Vec<RuntimeTaskTargetOption>,
  #[serde(default)]
  child_modules: Vec<RuntimeTaskChildModule>,
  #[serde(default)]
  sync_version_changed: bool,
  #[serde(default)]
  fast_build_app: bool,
  #[serde(default)]
  build_app_skip_sign_hap: bool,
  #[serde(default)]
  command_entry_test: bool,
  #[serde(default)]
  unit_test_process: bool,
  #[serde(default)]
  ohos_test_coverage: bool,
  #[serde(default)]
  preview_process: bool,
  #[serde(default)]
  preview_compile_resource_incremental: bool,
  #[serde(default)]
  in_test: bool,
  #[serde(default)]
  device_test: bool,
  #[serde(default)]
  hot_reload: bool,
  #[serde(default)]
  cold_reload: bool,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
struct RuntimeTaskTargetOption {
  name: String,
  #[serde(default)]
  deduplicate_har: bool,
  #[serde(default)]
  ohos_test_templates_after_migration: Option<bool>,
  #[serde(default)]
  ohos_test_source_exists: Option<bool>,
  #[serde(default)]
  binxo_enabled: bool,
  #[serde(default)]
  project_remote_hsp_exists: Option<bool>,
  #[serde(default)]
  module_remote_hsp_exists: Option<bool>,
  #[serde(default = "RuntimeTaskTargetOption::default_true")]
  ohpm_project: bool,
  #[serde(default)]
  compile_api_version: Option<u32>,
  #[serde(default)]
  compatible_api_version: Option<u32>,
  #[serde(default)]
  has_rollup_plugin_in_ets_loader: Option<bool>,
  #[serde(default)]
  has_rollup_plugin_in_js_loader: Option<bool>,
  #[serde(default = "RuntimeTaskTargetOption::default_true")]
  open_source_har: bool,
  #[serde(default)]
  should_build_cmake: Option<bool>,
  #[serde(default)]
  external_native_options: bool,
  #[serde(default)]
  use_normalized_ohm_url: bool,
  #[serde(default)]
  hsp_dependencies: Vec<RuntimeTaskDependencyTarget>,
  #[serde(default)]
  har_dependencies: Vec<RuntimeTaskDependencyTarget>,
}

impl RuntimeTaskTargetOption {
  const fn default_true() -> bool {
    true
  }

  fn supports_build_profile(&self) -> bool {
    self.compile_api_version.unwrap_or(10) > 9 && self.compatible_api_version.unwrap_or(10) > 9
  }
}

#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
struct RuntimeTaskDependencyTarget {
  module_name: String,
  target_name: String,
  #[serde(default)]
  module_root: Option<PathBuf>,
  #[serde(default)]
  should_build_cmake: Option<bool>,
  #[serde(default)]
  external_native_options: bool,
  #[serde(default)]
  use_normalized_ohm_url: bool,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
struct RuntimeTaskChildModule {
  name: String,
  kind: Option<RuntimeTaskNodeKind>,
}

#[derive(Debug, Deserialize, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct RuntimeTaskCatalogEntry {
  pub(super) name: String,
  pub(super) dependencies: Vec<String>,
  pub(super) group: String,
  pub(super) description: String,
  pub(super) source_plugin_id: String,
  pub(super) catalog_order: u32,
}

pub struct RuntimeTaskCatalog {
  request: RuntimeTaskCatalogRequest,
  definitions: IndexMap<String, Vec<String>>,
  remote_hsp_dependency_registered: bool,
}

impl RuntimeTaskCatalog {
  pub fn from_json(request_json: &str) -> anyhow::Result<Self> {
    let request = serde_json::from_str(request_json)?;
    Ok(Self::new(request))
  }

  pub fn new(mut request: RuntimeTaskCatalogRequest) -> Self {
    request.resolve_inventory();
    Self {
      request,
      definitions: IndexMap::new(),
      remote_hsp_dependency_registered: false,
    }
  }

  pub fn entries(mut self) -> anyhow::Result<Vec<RuntimeTaskCatalogEntry>> {
    self.build();
    self
      .definitions
      .into_iter()
      .enumerate()
      .map(|(catalog_order, (name, dependencies))| {
        let task_info = HvigorTaskInfoCatalog::task_info(&name)?;
        Ok(RuntimeTaskCatalogEntry {
          name,
          dependencies,
          group: task_info.group,
          description: task_info.description,
          source_plugin_id: task_info.source_plugin_id,
          catalog_order: u32::try_from(catalog_order)?,
        })
      })
      .collect()
  }

  pub fn entries_json(self) -> anyhow::Result<String> {
    serde_json::to_string(&self.entries()?).map_err(Into::into)
  }
}

impl RuntimeTaskCatalogRequest {
  fn resolve_inventory(&mut self) {
    let project_remote_hsp_exists = self
      .project_root
      .as_deref()
      .is_some_and(|root| root.join("oh_modules").join(".hsp").is_dir());
    let module_remote_hsp_exists = self
      .node_root
      .as_deref()
      .is_some_and(|root| root.join("oh_modules").join(".hsp").is_dir());
    let ohos_test_source_exists = self
      .node_root
      .as_deref()
      .is_some_and(|root| root.join("src").join("ohosTest").is_dir());
    let ohos_test_templates_after_migration = self
      .node_root
      .as_deref()
      .is_some_and(Self::ohos_test_templates_after_migration);

    for target in &mut self.target_options {
      target
        .project_remote_hsp_exists
        .get_or_insert(project_remote_hsp_exists);
      target
        .module_remote_hsp_exists
        .get_or_insert(module_remote_hsp_exists);
      target
        .ohos_test_source_exists
        .get_or_insert(ohos_test_source_exists);
      target
        .ohos_test_templates_after_migration
        .get_or_insert(ohos_test_templates_after_migration);
      target
        .has_rollup_plugin_in_ets_loader
        .get_or_insert_with(|| {
          Self::has_rollup_plugin(
            self.ets_loader_path.as_deref(),
            self.has_rollup_plugin_in_ets_loader,
            target.compile_api_version.unwrap_or(10) >= 9,
          )
        });
      target
        .has_rollup_plugin_in_js_loader
        .get_or_insert_with(|| {
          Self::has_rollup_plugin(
            self.js_loader_path.as_deref(),
            self.has_rollup_plugin_in_js_loader,
            target.compile_api_version.unwrap_or(10) >= 9,
          )
        });
      target.should_build_cmake.get_or_insert_with(|| {
        target.external_native_options
          && self
            .node_root
            .as_deref()
            .is_some_and(|root| Self::has_cmake_source(root, &target.name))
      });
      for dependency in target
        .hsp_dependencies
        .iter_mut()
        .chain(target.har_dependencies.iter_mut())
      {
        dependency.should_build_cmake.get_or_insert_with(|| {
          dependency.external_native_options
            && dependency
              .module_root
              .as_deref()
              .is_some_and(|root| Self::has_cmake_source(root, &dependency.target_name))
        });
      }
    }
  }

  fn has_rollup_plugin(path: Option<&Path>, configured: Option<bool>, fallback: bool) -> bool {
    configured
      .unwrap_or_else(|| path.map_or(fallback, |root| root.join("rollup.config.js").is_file()))
  }

  fn has_cmake_source(root: &Path, target_name: &str) -> bool {
    root.join("src").join(target_name).join("cpp").is_dir()
      || (target_name != "default" && root.join("src").join("main").join("cpp").is_dir())
  }

  fn ohos_test_templates_after_migration(module_root: &Path) -> bool {
    let source_root = module_root.join("src").join("ohosTest");
    let module_json_path = source_root.join("module.json5");
    if let Ok(source) = std::fs::read_to_string(module_json_path) {
      if let Ok(module_json) = json5::from_str::<serde_json::Value>(&source) {
        if let Some(first_ability) = module_json
          .pointer("/module/abilities")
          .and_then(serde_json::Value::as_array)
          .and_then(|abilities| abilities.first())
        {
          return first_ability.get("srcEntry").is_none();
        }
      }
    }
    let runner_root = source_root.join("ets").join("testrunner");
    !["OpenHarmonyTestRunner.ets", "OpenHarmonyTestRunner.ts"]
      .iter()
      .any(|name| runner_root.join(name).is_file())
  }
}

impl RuntimeTaskCatalog {
  fn build(&mut self) {
    if self.request.project_task {
      self.add_project_tasks();
    } else if matches!(
      self.request.kind,
      Some(RuntimeTaskNodeKind::Hap | RuntimeTaskNodeKind::Hsp | RuntimeTaskNodeKind::Har)
    ) {
      self.add_module_tasks();
    }
  }

  fn add_project_tasks(&mut self) {
    let module_assemblies = self
      .request
      .child_modules
      .iter()
      .filter_map(|module| match module.kind {
        Some(RuntimeTaskNodeKind::Hap) => Some(format!("{}:assembleHap", module.name)),
        Some(RuntimeTaskNodeKind::Hsp) => Some(format!("{}:assembleHsp", module.name)),
        Some(RuntimeTaskNodeKind::App | RuntimeTaskNodeKind::Har) | None => None,
      })
      .collect::<Vec<_>>();
    self.add_static("tasks", &[]);
    self.add_static("taskTree", &[]);
    self.add_static("prune", &[]);
    self.add_static("assembleApp", &["SignApp"]);
    self.add_static("clean", &[]);
    self.add_static("collectCoverage", &[]);
    self.add_static("init", &[]);
    self.add_static("SignApp", &["PackageApp", "SignPackagesFromApp"]);
    self.add_static("PackageApp", &["GeneratePackRes"]);
    self.add_static("SignPackagesFromApp", &["PackageApp"]);
    self.add_static("ProcessProjectPrivacyProfile", &["MakeProjectPackInfo"]);
    self.add_static(
      "GeneratePackRes",
      &[
        "DuplicateDependencyCheck",
        "MakeProjectPackInfo",
        "ProcessProjectPrivacyProfile",
      ],
    );
    if self.request.sync_version_changed {
      self.add_static("PreBuildApp", &["clean"]);
    } else {
      self.add_static("PreBuildApp", &[]);
    }
    self.add_static("DuplicateDependencyCheck", &["PreBuildApp"]);
    self.add_owned("MakeProjectPackInfo", module_assemblies);
    self.add_static("buildInfo", &[]);
  }

  fn add_module_tasks(&mut self) {
    let mut target_names = self.request.target_names.clone();
    if target_names.is_empty() {
      target_names.push(
        self
          .request
          .build_target_name
          .clone()
          .unwrap_or_else(|| "default".to_string()),
      );
    }
    self.add_module_entry_tasks(&target_names[0]);
    for target_name in target_names {
      self.add_target_tasks(&target_name);
    }
  }

  fn add_module_entry_tasks(&mut self, target_name: &str) {
    let target = |task: &str| format!("{target_name}@{task}");
    self.add_static("tasks", &[]);
    self.add_static("taskTree", &[]);
    self.add_static("init", &[]);
    self.add_owned(
      "buildPreviewerResource",
      vec![
        target("GenerateLoaderJson"),
        target("ReplacePreviewerPage"),
        target("PreCheckSyscap"),
      ],
    );
    self.add_owned(
      "GenerateBuildProfile",
      vec![target(
        if matches!(self.request.kind, Some(RuntimeTaskNodeKind::Har)) {
          "CreateHarBuildProfile"
        } else {
          "CreateBuildProfile"
        },
      )],
    );
    self.add_owned("compileNative", vec![target("BuildNativeWithCmake")]);
    self.add_owned("PreviewBuild", vec![target("PreviewArkTS")]);
    self.add_owned(
      "UnitTestBuild",
      if self.request.ohos_test_coverage {
        vec![target("ReplaceSourceMapInCoverageMode")]
      } else {
        vec![
          target("UnitTestArkTS"),
          target("UnitTestCopyMockConfigJson"),
        ]
      },
    );
    self.add_owned("test", vec![target("GenerateUnitTestResult")]);
    self.add_owned(
      "buildHotReloadResource",
      vec![target("GenerateLoaderJson"), target("CompileResource")],
    );
    self.add_static("clean", &[]);
    self.add_owned("onDeviceTest", vec![target("GenerateDeviceCoverage")]);
    self.add_static("buildInfo", &[]);

    match self.request.kind {
      Some(RuntimeTaskNodeKind::Hap) => {
        let package_task = if self.request.fast_build_app {
          "FastPackageHap"
        } else if self.request.build_app_skip_sign_hap {
          "PackageHap"
        } else {
          "SignHap"
        };
        self.add_owned(
          "assembleHap",
          vec![
            target("CollectDebugSymbol"),
            target("PackingCheck"),
            target(package_task),
          ],
        );
        self.add_owned("HotReloadBuild", vec![target("HotReloadArkTS")]);
        self.add_owned("assembleDevHqf", vec![target("SignHqf")]);
      }
      Some(RuntimeTaskNodeKind::Hsp) => {
        self.add_owned(
          "assembleHsp",
          if self.request.fast_build_app {
            vec![
              target("FastPackageHsp"),
              target("CollectDebugSymbol"),
              target("PackingCheck"),
            ]
          } else {
            vec![
              target("PackageSharedTgz"),
              target("CollectDebugSymbol"),
              target("PackingCheck"),
            ]
          },
        );
        self.add_owned(
          "genOnDeviceTestHap",
          vec![target(if self.request.build_app_skip_sign_hap {
            "PackageHap"
          } else {
            "SignHap"
          })],
        );
        self.add_owned("HotReloadBuild", vec![target("HotReloadArkTS")]);
        self.add_owned("assembleDevHqf", vec![target("SignHqf")]);
      }
      Some(RuntimeTaskNodeKind::Har) => {
        self.add_owned(
          "assembleHar",
          vec![
            target("PackageSignHar"),
            target("PackingCheckHar"),
            target("CollectDebugSymbol"),
          ],
        );
        self.add_owned(
          "genOnDeviceTestHap",
          vec![target(if self.request.build_app_skip_sign_hap {
            "PackageHap"
          } else {
            "SignHap"
          })],
        );
        self.add_owned(
          "buildPreviewerResource",
          vec![
            target("GenerateLoaderJson"),
            target("PreviewHookCompileResource"),
            target("PreCheckSyscap"),
          ],
        );
      }
      Some(RuntimeTaskNodeKind::App) | None => {}
    }
  }

  fn add_target_tasks(&mut self, target_name: &str) {
    let target_option = self
      .request
      .target_options
      .iter()
      .find(|option| option.name == target_name)
      .cloned();
    let enable_har_deduplicate = target_option
      .as_ref()
      .is_some_and(|option| option.deduplicate_har)
      && !self.request.preview_process
      && !self.request.in_test;
    let generate_ohos_test_template = target_option
      .as_ref()
      .is_some_and(|option| option.ohos_test_templates_after_migration == Some(true));
    let supports_build_profile = target_option
      .as_ref()
      .is_none_or(RuntimeTaskTargetOption::supports_build_profile);
    let has_rollup_ets = target_option
      .as_ref()
      .is_none_or(|option| option.has_rollup_plugin_in_ets_loader.unwrap_or(true));
    // `initializeArkTsTasks` registers only `BuildJS` for HAR modules; unlike
    // HAP/HSP it never creates a `CompileJS` task. Consequently the shared
    // `packageHapDepends` branch must resolve to `BuildJS` for HAR even when
    // the ETS SDK exposes its Rollup loader.
    let has_rollup_js = !matches!(self.request.kind, Some(RuntimeTaskNodeKind::Har))
      && target_option
        .as_ref()
        .is_none_or(|option| option.has_rollup_plugin_in_js_loader.unwrap_or(true));
    let mut tasks: IndexMap<&'static str, Vec<&'static str>> = IndexMap::new();
    tasks.insert("SyscapTransform", vec!["PreCheckSyscap"]);
    tasks.insert("MakePackInfo", vec!["MergeProfile"]);
    let remote_hsp_exists = target_option.as_ref().is_some_and(|option| {
      option.project_remote_hsp_exists == Some(true)
        || option.module_remote_hsp_exists == Some(true)
    });
    let repeat_remote_hsp_dependency = self.request.device_test
      && matches!(self.request.kind, Some(RuntimeTaskNodeKind::Har))
      && target_name == "ohosTest";
    let mut generate_metadata_dependencies = vec!["PreBuild"];
    if remote_hsp_exists && (!self.remote_hsp_dependency_registered || repeat_remote_hsp_dependency)
    {
      generate_metadata_dependencies.push("SignModuleRemoteHsp");
      self.remote_hsp_dependency_registered = true;
    }
    tasks.insert("GenerateMetadata", generate_metadata_dependencies);
    tasks.insert(
      "CompileArkTS",
      if enable_har_deduplicate && matches!(self.request.kind, Some(RuntimeTaskNodeKind::Hsp)) {
        vec!["CollectDuplicateHarEntriesArkTS"]
      } else if supports_build_profile {
        vec![
          "GenerateLoaderJson",
          "CompileResource",
          "CreateBuildProfile",
        ]
      } else {
        vec!["GenerateLoaderJson", "CompileResource"]
      },
    );
    tasks.insert("BuildArkTS", vec!["GenerateLoaderJson", "CompileResource"]);
    tasks.insert("BuildJS", vec!["GenerateLoaderJson", "CompileResource"]);
    tasks.insert(
      "HotReloadArkTS",
      if supports_build_profile {
        vec!["$root:buildHotReloadResource", "CreateBuildProfile"]
      } else {
        vec!["$root:buildHotReloadResource"]
      },
    );
    tasks.insert("ColdReloadArkTS", vec![]);
    tasks.insert(
      "PreviewArkTS",
      if supports_build_profile {
        vec!["PreviewUpdateAssets", "CreateBuildProfile"]
      } else {
        vec!["PreviewUpdateAssets"]
      },
    );
    tasks.insert(
      "OhosTestCompileArkTS",
      if supports_build_profile {
        vec![
          "GenerateLoaderJson",
          "CompileResource",
          "CreateBuildProfile",
        ]
      } else {
        vec!["GenerateLoaderJson", "CompileResource"]
      },
    );
    tasks.insert(
      "OhosTestCompileJS",
      if supports_build_profile {
        vec![
          "GenerateLoaderJson",
          "CompileResource",
          "CreateBuildProfile",
        ]
      } else {
        vec!["GenerateLoaderJson", "CompileResource"]
      },
    );
    tasks.insert(
      "OhosTestBuildArkTS",
      if supports_build_profile {
        vec![
          "GenerateLoaderJson",
          "CompileResource",
          "CreateBuildProfile",
        ]
      } else {
        vec!["GenerateLoaderJson", "CompileResource"]
      },
    );
    tasks.insert(
      "OhosTestBuildJS",
      if supports_build_profile {
        vec![
          "GenerateLoaderJson",
          "CompileResource",
          "CreateBuildProfile",
        ]
      } else {
        vec!["GenerateLoaderJson", "CompileResource"]
      },
    );
    tasks.insert(
      "UnitTestArkTS",
      if supports_build_profile {
        vec![
          "BuildUnitTestHook",
          "UnitTestProcessProfile",
          "CreateBuildProfile",
        ]
      } else {
        vec!["BuildUnitTestHook", "UnitTestProcessProfile"]
      },
    );
    let mut process_resource_dependencies = vec!["ProcessProfile"];
    if self.request.unit_test_process {
      process_resource_dependencies.push("UnitTestProcessProfile");
    }
    if enable_har_deduplicate {
      process_resource_dependencies.push("ProcessIdDefinedFile");
    }
    tasks.insert("ProcessResource", process_resource_dependencies);
    let mut compile_resource_dependencies = vec![
      "ProcessResource",
      "ProcessRouterMap",
      "ProcessShareConfig",
      "ProcessStartupConfig",
    ];
    if target_name == "ohosTest" && generate_ohos_test_template {
      compile_resource_dependencies.push("GenerateOhosTestTemplate");
    }
    tasks.insert("CompileResource", compile_resource_dependencies);
    tasks.insert("PreviewProcessResource", vec!["ProcessProfile"]);
    if enable_har_deduplicate {
      tasks.insert("ProcessIdDefinedFile", vec![]);
    }
    tasks.insert("BuildNativeWithNinja", vec!["BuildNativeWithCmake"]);
    tasks.insert("BuildNativeWithCmake", vec!["ConfigureCmake"]);
    tasks.insert("ProcessLibs", vec!["BuildNativeWithNinja"]);
    tasks.insert("DoNativeBinxo", vec!["ProcessLibs"]);
    tasks.insert("PrepareQuickfix", vec!["ColdReloadArkTS", "DoNativeStrip"]);
    tasks.insert(
      "DoNativeStrip",
      if target_option
        .as_ref()
        .is_some_and(|option| option.binxo_enabled)
      {
        vec!["DoNativeBinxo"]
      } else {
        vec!["ProcessLibs"]
      },
    );
    tasks.insert("CacheNativeLibs", vec!["DoNativeStrip"]);
    tasks.insert("ConfigureCmake", vec!["PreBuild"]);
    tasks.insert(
      "PreBuild",
      if self.request.sync_version_changed {
        vec!["$root:clean"]
      } else {
        vec![]
      },
    );
    tasks.insert(
      "ProcessProfile",
      vec!["MergeProfile", "GeneratePkgContextInfo"],
    );
    tasks.insert(
      "MergeProfile",
      if self.request.command_entry_test {
        vec!["PreBuild", "GenerateUnitTestTemplate"]
      } else {
        vec!["PreBuild"]
      },
    );
    tasks.insert(
      "PreviewHookCompileResource",
      vec![if self.request.preview_compile_resource_incremental {
        "PreviewCompileResource"
      } else {
        "CompileResource"
      }],
    );
    tasks.insert("CreateBuildProfile", vec!["PreBuild"]);
    tasks.insert("PreviewUpdateAssets", vec!["$root:buildPreviewerResource"]);
    tasks.insert(
      "BuildUnitTestHook",
      vec![
        "GenerateLoaderJson",
        "ReplaceUnitTestIndexFile",
        "PreCheckSyscap",
      ],
    );
    tasks.insert("ReplaceSourceMapInCoverageMode", vec!["UnitTestArkTS"]);
    tasks.insert("ReplaceUnitTestIndexFile", vec!["CompileResource"]);
    tasks.insert("UnitTestProcessProfile", vec!["ProcessProfile"]);
    tasks.insert(
      "ReplacePreviewerPage",
      if self.request.preview_compile_resource_incremental {
        vec!["PreviewHookCompileResource", "CopyPreviewProfile"]
      } else {
        vec!["PreviewHookCompileResource"]
      },
    );
    tasks.insert("CopyPreviewProfile", vec!["PreviewHookCompileResource"]);
    tasks.insert("PreviewCompileResource", vec!["PreviewProcessResource"]);
    tasks.insert("OhosTestCopyMockConfigJson", vec!["OhosTestCompileArkTS"]);
    tasks.insert("UnitTestCopyMockConfigJson", vec!["UnitTestArkTS"]);
    tasks.insert(
      "SignProjectRemoteHsp",
      vec!["ProcessIntegratedHsp", "PreBuild"],
    );
    tasks.insert(
      "SignModuleRemoteHsp",
      if target_option
        .as_ref()
        .is_some_and(|option| option.project_remote_hsp_exists == Some(true))
      {
        vec!["SignProjectRemoteHsp", "ProcessIntegratedHsp"]
      } else {
        vec!["ProcessIntegratedHsp", "PreBuild"]
      },
    );
    tasks.insert("PreCheckSyscap", vec!["PreBuild"]);
    tasks.insert("GenerateUnitTestTemplate", vec!["PreBuild"]);
    tasks.insert("GenerateOhosTestTemplate", vec!["PreBuild"]);
    tasks.insert("GenerateUnitTestResult", vec!["UnitTestArkTS"]);
    tasks.insert("ProcessRouterMap", vec!["GeneratePkgContextInfo"]);
    tasks.insert("ProcessShareConfig", vec!["GeneratePkgContextInfo"]);
    tasks.insert("ProcessStartupConfig", vec!["GeneratePkgContextInfo"]);
    tasks.insert("BeforeGenerateDeviceCoverage", vec!["PreBuild"]);
    tasks.insert(
      "GeneratePkgContextInfo",
      if target_option.as_ref().is_some_and(|option| {
        option.should_build_cmake == Some(true) && option.use_normalized_ohm_url
      }) {
        vec!["PreBuild", "BuildNativeWithCmake"]
      } else {
        vec!["PreBuild"]
      },
    );
    tasks.insert("GeneratePkgSdkInfo", vec!["PreBuild"]);
    tasks.insert("ProcessIntegratedHsp", vec!["PreBuild"]);
    tasks.insert("GeneratePkgModuleJson", {
      let mut dependencies = vec![
        "MakePackInfo",
        "CacheNativeLibs",
        "GenerateMetadata",
        "SyscapTransform",
        "ProcessRouterMap",
        "ProcessShareConfig",
      ];
      if self.request.hot_reload || self.request.cold_reload {
        if has_rollup_ets {
          dependencies.push(if self.request.hot_reload {
            "HotReloadArkTS"
          } else {
            "ColdReloadArkTS"
          });
        }
        dependencies.push(if has_rollup_js {
          "CompileJS"
        } else {
          "BuildJS"
        });
      } else if target_name == "ohosTest" {
        dependencies.push("OhosTestCopyMockConfigJson");
        dependencies.push(if has_rollup_ets {
          "OhosTestCompileArkTS"
        } else {
          "OhosTestBuildArkTS"
        });
        dependencies.push(if has_rollup_js {
          "OhosTestCompileJS"
        } else {
          "OhosTestBuildJS"
        });
        dependencies.push("ProcessIntegratedHsp");
      } else {
        dependencies.push(if has_rollup_ets {
          "CompileArkTS"
        } else {
          "BuildArkTS"
        });
        dependencies.push(if has_rollup_js {
          "CompileJS"
        } else {
          "BuildJS"
        });
        dependencies.push("ProcessIntegratedHsp");
      }
      dependencies
    });
    tasks.insert("ProcessCompiledResources", vec!["GeneratePkgModuleJson"]);
    tasks.insert("ProcessModulePrivacyProfile", vec!["PreBuild"]);
    let package_hap_dependencies = vec![
      "ProcessCompiledResources",
      "ProcessIntegratedHsp",
      "GeneratePkgSdkInfo",
    ];
    match self.request.kind {
      Some(RuntimeTaskNodeKind::Hap) if self.request.fast_build_app => {
        tasks.insert("FastPackageHap", package_hap_dependencies);
      }
      Some(RuntimeTaskNodeKind::Hap) => {
        tasks.insert("PackageHap", package_hap_dependencies);
        tasks.insert("SignHap", vec!["PackageHap"]);
      }
      Some(RuntimeTaskNodeKind::Hsp | RuntimeTaskNodeKind::Har) => {
        tasks.insert("PackageHap", package_hap_dependencies);
        tasks.insert("SignHap", vec!["PackageHap"]);
      }
      Some(RuntimeTaskNodeKind::App) | None => {}
    }

    if !matches!(self.request.kind, Some(RuntimeTaskNodeKind::Har)) {
      tasks.insert("CreateModuleInfo", vec!["PreBuild"]);
      tasks.insert("PackageHqf", vec!["PrepareQuickfix"]);
      tasks.insert("SignHqf", vec!["PackageHqf"]);
      tasks.insert(
        "PackingCheck",
        vec![if self.request.fast_build_app
          && matches!(self.request.kind, Some(RuntimeTaskNodeKind::Hap))
        {
          "FastPackageHap"
        } else if self.request.fast_build_app
          && matches!(self.request.kind, Some(RuntimeTaskNodeKind::Hsp))
        {
          "FastPackageHsp"
        } else if matches!(self.request.kind, Some(RuntimeTaskNodeKind::Hsp)) {
          "PackageHsp"
        } else {
          "PackageHap"
        }],
      );
      tasks.insert(
        "CompileJS",
        if enable_har_deduplicate && matches!(self.request.kind, Some(RuntimeTaskNodeKind::Hsp)) {
          vec!["CollectDuplicateHarEntriesArkTS"]
        } else if supports_build_profile {
          vec![
            "GenerateLoaderJson",
            "CompileResource",
            "CreateBuildProfile",
          ]
        } else {
          vec!["GenerateLoaderJson", "CompileResource"]
        },
      );
      tasks.insert(
        "GenerateLoaderJson",
        vec![
          "ProcessRouterMap",
          "ProcessShareConfig",
          "ProcessStartupConfig",
          "CreateModuleInfo",
        ],
      );
      tasks.insert(
        "CollectDebugSymbol",
        vec![if self.request.fast_build_app
          && matches!(self.request.kind, Some(RuntimeTaskNodeKind::Hap))
        {
          "FastPackageHap"
        } else if self.request.fast_build_app
          && matches!(self.request.kind, Some(RuntimeTaskNodeKind::Hsp))
        {
          "FastPackageHsp"
        } else if matches!(self.request.kind, Some(RuntimeTaskNodeKind::Hsp)) {
          "PackageHsp"
        } else {
          "PackageHap"
        }],
      );
    }
    if matches!(self.request.kind, Some(RuntimeTaskNodeKind::Hsp)) {
      if self.request.fast_build_app {
        tasks.insert(
          "FastPackageHsp",
          vec!["GeneratePkgModuleJson", "GeneratePkgSdkInfo"],
        );
      } else {
        tasks.insert("PackageSharedHar", vec!["PrepareSharedHarResource"]);
        tasks.insert(
          "PrepareSharedHarResource",
          vec![
            if target_name == "ohosTest" {
              "OhosTestCompileArkTS"
            } else {
              "CompileArkTS"
            },
            "DoNativeStrip",
          ],
        );
        tasks.insert("PackageSharedTgz", vec!["SignHsp", "PackageSharedHar"]);
      }
      tasks.insert(
        "PackageHsp",
        vec![
          "GeneratePkgModuleJson",
          "ProcessModulePrivacyProfile",
          "GeneratePkgSdkInfo",
        ],
      );
      tasks.insert("SignHsp", vec!["PackageHsp"]);
      tasks.insert("BeforeGenerateDeviceCoverage", vec!["PreBuild", "SignHsp"]);
      if enable_har_deduplicate {
        tasks.insert(
          "CollectDuplicateHarEntriesArkTS",
          if supports_build_profile {
            vec![
              "GenerateLoaderJson",
              "CompileResource",
              "CreateBuildProfile",
            ]
          } else {
            vec!["GenerateLoaderJson", "CompileResource"]
          },
        );
      }
    }
    if matches!(self.request.kind, Some(RuntimeTaskNodeKind::Har)) {
      let process_package_task = if target_option
        .as_ref()
        .is_none_or(|option| option.ohpm_project)
      {
        "ProcessOHPackageJson"
      } else {
        "ProcessPackageJson"
      };
      let har_compile_task = if target_option
        .as_ref()
        .is_none_or(|option| option.open_source_har)
        || has_rollup_ets
      {
        "HarCompileArkTS"
      } else {
        "HarBuildArkTS"
      };
      tasks.insert(
        "ProcessHarArtifacts",
        vec![
          "ProcessObfuscationFiles",
          "CacheNativeLibs",
          process_package_task,
          har_compile_task,
          "ProcessIntegratedHsp",
        ],
      );
      tasks.insert(
        "PackageHar",
        vec!["ProcessHarArtifacts", "ProcessModulePrivacyProfile"],
      );
      tasks.insert("PackageSignHar", vec!["PackageHar"]);
      tasks.insert(
        "LintArkTS",
        if supports_build_profile {
          vec!["ProcessProfile", "CreateHarBuildProfile"]
        } else {
          vec!["ProcessProfile"]
        },
      );
      tasks.insert("ProcessObfuscationFiles", vec!["CompileResource"]);
      tasks.insert("ProcessOHPackageJson", vec!["PreBuild"]);
      tasks.insert("ProcessPackageJson", vec!["PreBuild"]);
      tasks.insert("CreateHarBuildProfile", vec!["PreBuild"]);
      tasks.insert("CreateHarModuleInfo", vec!["PreBuild"]);
      tasks.insert("PackingCheckHar", vec!["PackageHar"]);
      tasks.insert(
        "HarCompileArkTS",
        if supports_build_profile {
          vec![
            "GenerateLoaderJson",
            "CompileResource",
            "CreateHarBuildProfile",
          ]
        } else {
          vec!["GenerateLoaderJson", "CompileResource"]
        },
      );
      tasks.insert(
        "HarBuildArkTS",
        vec!["GenerateLoaderJson", "CompileResource"],
      );
      tasks.insert(
        "GenerateLoaderJson",
        vec![
          "ProcessRouterMap",
          "ProcessShareConfig",
          "ProcessStartupConfig",
          "CreateHarModuleInfo",
        ],
      );
      tasks.insert("CollectDebugSymbol", vec!["PackageHar"]);
    }

    for (name, dependencies) in tasks {
      let dependencies = dependencies
        .into_iter()
        .map(|dependency| {
          dependency
            .strip_prefix("$root:")
            .map_or_else(|| format!("{target_name}@{dependency}"), str::to_string)
        })
        .collect();
      self.add_owned(&format!("{target_name}@{name}"), dependencies);
    }

    if let Some(option) = target_option.as_ref() {
      self.add_cross_module_target_dependencies(
        target_name,
        option,
        supports_build_profile,
        enable_har_deduplicate,
      );
    }

    let mut generate_device_coverage_dependencies = vec![format!("{target_name}@PreBuild")];
    let has_ohos_test_target = self
      .request
      .target_options
      .iter()
      .any(|option| option.name == "ohosTest" && option.ohos_test_source_exists == Some(true));
    if self.request.device_test && target_name == "default" && has_ohos_test_target {
      let default_package_task = match self.request.kind {
        Some(RuntimeTaskNodeKind::Hap) => Some("SignHap"),
        Some(RuntimeTaskNodeKind::Hsp) => Some("SignHsp"),
        Some(RuntimeTaskNodeKind::Har) => Some("PackageHar"),
        Some(RuntimeTaskNodeKind::App) | None => None,
      };
      if let Some(default_package_task) = default_package_task {
        generate_device_coverage_dependencies.push(format!("default@{default_package_task}"));
        generate_device_coverage_dependencies.push("ohosTest@SignHap".to_string());
      }
      generate_device_coverage_dependencies
        .push("default@BeforeGenerateDeviceCoverage".to_string());
    }
    self.add_owned(
      &format!("{target_name}@GenerateDeviceCoverage"),
      generate_device_coverage_dependencies,
    );
  }

  /// Latest hvigor resolves local HSP/HAR target dependencies lazily through
  /// `HspTargetUtils` and `HarTargetUtil`. These edges are already qualified
  /// with the dependency module and target, so they must not pass through the
  /// current-node path rewriter used by ordinary creation-action dependencies.
  fn add_cross_module_target_dependencies(
    &mut self,
    target_name: &str,
    option: &RuntimeTaskTargetOption,
    supports_build_profile: bool,
    enable_har_deduplicate: bool,
  ) {
    for dependency in &option.hsp_dependencies {
      self.insert_cross_dependency_before(
        target_name,
        "GenerateMetadata",
        dependency,
        "GenerateMetadata",
        "SignModuleRemoteHsp",
      );
      self.insert_cross_dependency_before(
        target_name,
        "MergeProfile",
        dependency,
        "MergeProfile",
        "GenerateUnitTestTemplate",
      );
      for owner in ["CreateBuildProfile", "CreateHarBuildProfile"] {
        self.append_cross_dependency(target_name, owner, dependency, "CreateBuildProfile");
      }
      self.append_cross_dependency(
        target_name,
        "ConfigureCmake",
        dependency,
        "BuildNativeWithCmake",
      );
      self.append_cross_dependency(
        target_name,
        "BuildNativeWithNinja",
        dependency,
        "DoNativeStrip",
      );
      self.append_cross_dependency(
        target_name,
        "GenerateLoaderJson",
        dependency,
        if self.request.preview_process {
          "PreviewCompileResource"
        } else {
          "CompileResource"
        },
      );
      self.append_cross_dependency(
        target_name,
        "BeforeGenerateDeviceCoverage",
        dependency,
        "BeforeGenerateDeviceCoverage",
      );
      self.append_cross_dependency(
        target_name,
        "GenerateUnitTestResult",
        dependency,
        "UnitTestArkTS",
      );
      if enable_har_deduplicate
        && matches!(
          self.request.kind,
          Some(RuntimeTaskNodeKind::Hap | RuntimeTaskNodeKind::Hsp)
        )
      {
        self.append_cross_dependency(
          target_name,
          "GenerateLoaderJson",
          dependency,
          "CollectDuplicateHarEntriesArkTS",
        );
        self.append_cross_dependency(
          target_name,
          "GeneratePkgContextInfo",
          dependency,
          "GeneratePkgContextInfo",
        );
        self.append_cross_dependency(
          target_name,
          "ProcessStartupConfig",
          dependency,
          "ProcessStartupConfig",
        );
      }
    }

    for dependency in &option.har_dependencies {
      self.insert_cross_dependency_before(
        target_name,
        "MergeProfile",
        dependency,
        "MergeProfile",
        "GenerateUnitTestTemplate",
      );
      for owner in ["CreateBuildProfile", "CreateHarBuildProfile"] {
        self.append_cross_dependency(target_name, owner, dependency, "CreateHarBuildProfile");
      }
      self.append_cross_dependency(
        target_name,
        "ConfigureCmake",
        dependency,
        "BuildNativeWithCmake",
      );
      self.append_cross_dependency(
        target_name,
        "BuildNativeWithNinja",
        dependency,
        "DoNativeStrip",
      );
      if dependency.should_build_cmake == Some(true) && dependency.use_normalized_ohm_url {
        self.append_cross_dependency(
          target_name,
          "GeneratePkgContextInfo",
          dependency,
          "BuildNativeWithCmake",
        );
      }
      if supports_build_profile {
        for owner in [
          "CompileArkTS",
          "CompileJS",
          "OhosTestCompileArkTS",
          "OhosTestCompileJS",
          "OhosTestBuildArkTS",
          "OhosTestBuildJS",
          "UnitTestArkTS",
          "CollectDuplicateHarEntriesArkTS",
          "HarCompileArkTS",
          "HotReloadArkTS",
        ] {
          self.append_cross_dependency(target_name, owner, dependency, "CreateHarBuildProfile");
        }
      }
    }
  }

  fn append_cross_dependency(
    &mut self,
    owner_target: &str,
    owner_task: &str,
    dependency: &RuntimeTaskDependencyTarget,
    dependency_task: &str,
  ) {
    let owner = format!("{owner_target}@{owner_task}");
    let dependency = Self::cross_dependency_path(dependency, dependency_task);
    let Some(dependencies) = self.definitions.get_mut(&owner) else {
      return;
    };
    if !dependencies.contains(&dependency) {
      dependencies.push(dependency);
    }
  }

  fn insert_cross_dependency_before(
    &mut self,
    owner_target: &str,
    owner_task: &str,
    dependency: &RuntimeTaskDependencyTarget,
    dependency_task: &str,
    before_task: &str,
  ) {
    let owner = format!("{owner_target}@{owner_task}");
    let dependency = Self::cross_dependency_path(dependency, dependency_task);
    let Some(dependencies) = self.definitions.get_mut(&owner) else {
      return;
    };
    if dependencies.contains(&dependency) {
      return;
    }
    let suffix = format!("@{before_task}");
    let index = dependencies
      .iter()
      .position(|candidate| candidate.ends_with(&suffix))
      .unwrap_or(dependencies.len());
    dependencies.insert(index, dependency);
  }

  fn cross_dependency_path(
    dependency: &RuntimeTaskDependencyTarget,
    dependency_task: &str,
  ) -> String {
    format!(
      "{}:{}@{dependency_task}",
      dependency.module_name, dependency.target_name
    )
  }

  fn add_static(&mut self, name: &str, dependencies: &[&str]) {
    self.add_owned(
      name,
      dependencies
        .iter()
        .map(|dependency| (*dependency).to_string())
        .collect(),
    );
  }

  fn add_owned(&mut self, name: &str, dependencies: Vec<String>) {
    let dependencies = dependencies
      .into_iter()
      .map(|dependency| self.task_path(&dependency))
      .collect();
    self.definitions.insert(name.to_string(), dependencies);
  }

  fn task_path(&self, dependency: &str) -> String {
    if dependency.starts_with(':') || dependency.contains(':') {
      dependency.to_string()
    } else if self.request.project_task {
      format!(":{dependency}")
    } else {
      format!("{}:{dependency}", self.request.node_name)
    }
  }
}

#[cfg(test)]
mod tests {
  use super::*;

  fn entries(request: serde_json::Value) -> Vec<RuntimeTaskCatalogEntry> {
    let catalog = RuntimeTaskCatalog::from_json(&request.to_string()).unwrap();
    serde_json::from_str(&catalog.entries_json().unwrap()).unwrap()
  }

  #[test]
  fn builds_hap_target_graph_with_qualified_dependencies() {
    let entries = entries(serde_json::json!({
      "nodeName": "entry",
      "kind": "hap",
      "projectTask": false,
      "targetNames": ["default"],
      "syncVersionChanged": false
    }));

    assert_eq!(&entries[0].name, "tasks");
    assert_eq!(
      entries
        .iter()
        .find(|entry| entry.name == "assembleHap")
        .unwrap()
        .dependencies,
      [
        "entry:default@CollectDebugSymbol",
        "entry:default@PackingCheck",
        "entry:default@SignHap",
      ]
    );
    assert_eq!(
      entries
        .iter()
        .find(|entry| entry.name == "default@CompileArkTS")
        .unwrap()
        .dependencies,
      [
        "entry:default@GenerateLoaderJson",
        "entry:default@CompileResource",
        "entry:default@CreateBuildProfile",
      ]
    );
  }

  #[test]
  fn project_graph_tracks_module_assemblies_and_sync_clean() {
    let entries = entries(serde_json::json!({
      "nodeName": "fixture",
      "kind": "app",
      "projectTask": true,
      "childModules": [
        { "name": "entry", "kind": "hap" },
        { "name": "shared", "kind": "hsp" },
        { "name": "library", "kind": "har" }
      ],
      "syncVersionChanged": true
    }));

    assert_eq!(
      entries
        .iter()
        .find(|entry| entry.name == "MakeProjectPackInfo")
        .unwrap()
        .dependencies,
      ["entry:assembleHap", "shared:assembleHsp"]
    );
    assert_eq!(
      entries
        .iter()
        .find(|entry| entry.name == "PreBuildApp")
        .unwrap()
        .dependencies,
      [":clean"]
    );
  }

  #[test]
  fn preserves_hsp_and_har_specific_target_edges() {
    for (kind, task_name, dependencies) in [
      (
        "hsp",
        "default@PackageHsp",
        vec![
          "entry:default@GeneratePkgModuleJson",
          "entry:default@ProcessModulePrivacyProfile",
          "entry:default@GeneratePkgSdkInfo",
        ],
      ),
      (
        "har",
        "default@PackageHar",
        vec![
          "entry:default@ProcessHarArtifacts",
          "entry:default@ProcessModulePrivacyProfile",
        ],
      ),
    ] {
      let entries = entries(serde_json::json!({
        "nodeName": "entry",
        "kind": kind,
        "projectTask": false,
        "buildTargetName": "default"
      }));
      assert_eq!(
        entries
          .iter()
          .find(|entry| entry.name == task_name)
          .unwrap()
          .dependencies,
        dependencies
      );
    }
  }

  #[test]
  fn local_test_graph_selects_mock_copy_without_coverage() {
    let entries = entries(serde_json::json!({
      "nodeName": "entry",
      "kind": "hap",
      "projectTask": false,
      "targetNames": ["default"]
    }));
    let dependencies = |name: &str| {
      entries
        .iter()
        .find(|entry| entry.name == name)
        .unwrap_or_else(|| panic!("missing task {name}"))
        .dependencies
        .clone()
    };

    assert_eq!(
      dependencies("UnitTestBuild"),
      [
        "entry:default@UnitTestArkTS",
        "entry:default@UnitTestCopyMockConfigJson",
      ]
    );
    assert_eq!(
      dependencies("default@UnitTestCopyMockConfigJson"),
      ["entry:default@UnitTestArkTS"]
    );
    assert_eq!(
      dependencies("default@ReplaceSourceMapInCoverageMode"),
      ["entry:default@UnitTestArkTS"]
    );
    assert_eq!(
      dependencies("test"),
      ["entry:default@GenerateUnitTestResult"]
    );
  }

  #[test]
  fn local_test_coverage_and_template_edges_follow_injected_properties() {
    let entries = entries(serde_json::json!({
      "nodeName": "entry",
      "kind": "hap",
      "projectTask": false,
      "targetNames": ["default"],
      "ohosTestCoverage": true,
      "unitTestProcess": true,
      "commandEntryTest": true
    }));
    let dependencies = |name: &str| {
      entries
        .iter()
        .find(|entry| entry.name == name)
        .unwrap_or_else(|| panic!("missing task {name}"))
        .dependencies
        .clone()
    };

    assert_eq!(
      dependencies("UnitTestBuild"),
      ["entry:default@ReplaceSourceMapInCoverageMode"]
    );
    assert_eq!(
      dependencies("default@ProcessResource"),
      [
        "entry:default@ProcessProfile",
        "entry:default@UnitTestProcessProfile",
      ]
    );
    assert_eq!(
      dependencies("default@MergeProfile"),
      [
        "entry:default@PreBuild",
        "entry:default@GenerateUnitTestTemplate",
      ]
    );
  }

  #[test]
  fn fast_build_registers_hvigor_fast_package_tasks_and_edges() {
    for (kind, assemble, fast_package) in [
      ("hap", "assembleHap", "FastPackageHap"),
      ("hsp", "assembleHsp", "FastPackageHsp"),
    ] {
      let entries = entries(serde_json::json!({
        "nodeName": "entry",
        "kind": kind,
        "projectTask": false,
        "targetNames": ["default"],
        "fastBuildApp": true
      }));
      let names = entries
        .iter()
        .map(|entry| entry.name.as_str())
        .collect::<Vec<_>>();
      assert!(names.contains(&format!("default@{fast_package}").as_str()));
      assert!(entries
        .iter()
        .find(|entry| entry.name == assemble)
        .expect("assemble task")
        .dependencies
        .contains(&format!("entry:default@{fast_package}")));
      if kind == "hap" {
        assert!(!names.contains(&"default@PackageHap"));
        assert!(!names.contains(&"default@SignHap"));
      } else {
        assert!(!names.contains(&"default@PackageSharedTgz"));
      }
    }
  }

  #[test]
  fn hsp_har_dedup_registers_resource_and_collect_compile_boundaries() {
    let entries = entries(serde_json::json!({
      "nodeName": "shared",
      "kind": "hsp",
      "projectTask": false,
      "targetNames": ["default"],
      "targetOptions": [{ "name": "default", "deduplicateHar": true }]
    }));
    let dependencies = |name: &str| {
      entries
        .iter()
        .find(|entry| entry.name == name)
        .unwrap_or_else(|| panic!("missing task {name}"))
        .dependencies
        .clone()
    };

    assert_eq!(
      dependencies("default@CompileArkTS"),
      ["shared:default@CollectDuplicateHarEntriesArkTS"]
    );
    assert_eq!(
      dependencies("default@ProcessResource"),
      [
        "shared:default@ProcessProfile",
        "shared:default@ProcessIdDefinedFile",
      ]
    );
    assert_eq!(
      dependencies("default@CollectDuplicateHarEntriesArkTS"),
      [
        "shared:default@GenerateLoaderJson",
        "shared:default@CompileResource",
        "shared:default@CreateBuildProfile",
      ]
    );
  }

  #[test]
  fn native_strip_depends_on_binxo_only_when_hvigor_enables_it() {
    for (binxo_enabled, expected) in [
      (false, "entry:default@ProcessLibs"),
      (true, "entry:default@DoNativeBinxo"),
    ] {
      let entries = entries(serde_json::json!({
        "nodeName": "entry",
        "kind": "hap",
        "projectTask": false,
        "targetNames": ["default"],
        "targetOptions": [{ "name": "default", "binxoEnabled": binxo_enabled }]
      }));
      let strip = entries
        .iter()
        .find(|entry| entry.name == "default@DoNativeStrip")
        .expect("DoNativeStrip task");
      assert_eq!(strip.dependencies, [expected]);
    }
  }

  #[test]
  fn remote_hsp_tasks_follow_hvigor_repository_dependent_edges() {
    for (project_repository, expected_module_dependencies) in [
      (
        false,
        vec![
          "entry:default@ProcessIntegratedHsp",
          "entry:default@PreBuild",
        ],
      ),
      (
        true,
        vec![
          "entry:default@SignProjectRemoteHsp",
          "entry:default@ProcessIntegratedHsp",
        ],
      ),
    ] {
      let entries = entries(serde_json::json!({
        "nodeName": "entry",
        "kind": "hap",
        "projectTask": false,
        "targetNames": ["default"],
        "targetOptions": [{
          "name": "default",
          "projectRemoteHspExists": project_repository,
          "moduleRemoteHspExists": !project_repository
        }]
      }));
      let dependencies = |name: &str| {
        entries
          .iter()
          .find(|entry| entry.name == name)
          .unwrap_or_else(|| panic!("missing task {name}"))
          .dependencies
          .clone()
      };

      assert_eq!(
        dependencies("default@SignModuleRemoteHsp"),
        expected_module_dependencies
      );
      assert_eq!(
        dependencies("default@GenerateMetadata"),
        [
          "entry:default@PreBuild",
          "entry:default@SignModuleRemoteHsp",
        ]
      );
    }
  }

  #[test]
  fn generate_metadata_registers_remote_hsp_signing_once_per_module() {
    let entries = entries(serde_json::json!({
      "nodeName": "entry",
      "kind": "hap",
      "projectTask": false,
      "targetNames": ["default", "secondary"],
      "targetOptions": [
        { "name": "default", "projectRemoteHspExists": true },
        { "name": "secondary", "projectRemoteHspExists": true }
      ]
    }));
    let dependencies = |name: &str| {
      entries
        .iter()
        .find(|entry| entry.name == name)
        .unwrap_or_else(|| panic!("missing task {name}"))
        .dependencies
        .clone()
    };

    assert_eq!(
      dependencies("default@GenerateMetadata"),
      [
        "entry:default@PreBuild",
        "entry:default@SignModuleRemoteHsp",
      ]
    );
    assert_eq!(
      dependencies("secondary@GenerateMetadata"),
      ["entry:secondary@PreBuild"]
    );
  }

  #[test]
  fn device_test_default_target_depends_on_both_signed_variants() {
    let entries = entries(serde_json::json!({
      "nodeName": "entry",
      "kind": "hap",
      "projectTask": false,
      "targetNames": ["default", "ohosTest"],
      "targetOptions": [
        { "name": "default", "ohosTestSourceExists": true },
        { "name": "ohosTest", "ohosTestSourceExists": true }
      ],
      "deviceTest": true
    }));
    let dependencies = |name: &str| {
      entries
        .iter()
        .find(|entry| entry.name == name)
        .unwrap_or_else(|| panic!("missing task {name}"))
        .dependencies
        .clone()
    };

    assert_eq!(
      dependencies("default@GenerateDeviceCoverage"),
      [
        "entry:default@PreBuild",
        "entry:default@SignHap",
        "entry:ohosTest@SignHap",
        "entry:default@BeforeGenerateDeviceCoverage",
      ]
    );
    assert_eq!(
      dependencies("ohosTest@GenerateDeviceCoverage"),
      ["entry:ohosTest@PreBuild"]
    );
  }

  #[test]
  fn legacy_npm_har_selects_package_json_node_build_and_api9_task_edges() {
    let entries = entries(serde_json::json!({
      "nodeName": "legacy",
      "kind": "har",
      "projectTask": false,
      "targetNames": ["default"],
      "targetOptions": [{
        "name": "default",
        "ohpmProject": false,
        "compileApiVersion": 9,
        "compatibleApiVersion": 9,
        "hasRollupPluginInEtsLoader": false,
        "hasRollupPluginInJsLoader": false,
        "openSourceHar": false
      }]
    }));
    let dependencies = |name: &str| {
      entries
        .iter()
        .find(|entry| entry.name == name)
        .unwrap_or_else(|| panic!("missing task {name}"))
        .dependencies
        .clone()
    };

    assert_eq!(
      dependencies("default@ProcessHarArtifacts"),
      [
        "legacy:default@ProcessObfuscationFiles",
        "legacy:default@CacheNativeLibs",
        "legacy:default@ProcessPackageJson",
        "legacy:default@HarBuildArkTS",
        "legacy:default@ProcessIntegratedHsp",
      ]
    );
    assert_eq!(
      dependencies("default@HarCompileArkTS"),
      [
        "legacy:default@GenerateLoaderJson",
        "legacy:default@CompileResource",
      ]
    );
    assert_eq!(
      dependencies("default@LintArkTS"),
      ["legacy:default@ProcessProfile"]
    );
    assert!(dependencies("default@ColdReloadArkTS").is_empty());
  }

  #[test]
  fn generate_pkg_module_json_follows_rollup_and_reload_selection() {
    let ordinary = entries(serde_json::json!({
      "nodeName": "entry",
      "kind": "hap",
      "projectTask": false,
      "targetNames": ["default"],
      "targetOptions": [{
        "name": "default",
        "hasRollupPluginInEtsLoader": false,
        "hasRollupPluginInJsLoader": false
      }]
    }));
    let ordinary_dependencies = &ordinary
      .iter()
      .find(|entry| entry.name == "default@GeneratePkgModuleJson")
      .expect("GeneratePkgModuleJson")
      .dependencies;
    assert!(ordinary_dependencies.contains(&"entry:default@BuildArkTS".to_string()));
    assert!(ordinary_dependencies.contains(&"entry:default@BuildJS".to_string()));
    assert!(ordinary_dependencies.contains(&"entry:default@ProcessIntegratedHsp".to_string()));

    let hot_reload = entries(serde_json::json!({
      "nodeName": "entry",
      "kind": "hap",
      "projectTask": false,
      "targetNames": ["default"],
      "hotReload": true,
      "targetOptions": [{ "name": "default" }]
    }));
    let hot_dependencies = &hot_reload
      .iter()
      .find(|entry| entry.name == "default@GeneratePkgModuleJson")
      .expect("hot GeneratePkgModuleJson")
      .dependencies;
    assert!(hot_dependencies.contains(&"entry:default@HotReloadArkTS".to_string()));
    assert!(hot_dependencies.contains(&"entry:default@CompileJS".to_string()));
    assert!(!hot_dependencies.contains(&"entry:default@ProcessIntegratedHsp".to_string()));
    assert!(!hot_dependencies.contains(&"entry:default@CompileArkTS".to_string()));
  }

  #[test]
  fn cross_module_hsp_and_har_edges_match_hvigor_creation_actions() {
    let entries = entries(serde_json::json!({
      "nodeName": "entry",
      "kind": "hap",
      "projectTask": false,
      "targetNames": ["default"],
      "commandEntryTest": true,
      "targetOptions": [{
        "name": "default",
        "deduplicateHar": true,
        "hspDependencies": [{
          "moduleName": "shared",
          "targetName": "default"
        }],
        "harDependencies": [{
          "moduleName": "library",
          "targetName": "release",
          "shouldBuildCmake": true,
          "useNormalizedOhmUrl": true
        }]
      }]
    }));
    let dependencies = |name: &str| {
      entries
        .iter()
        .find(|entry| entry.name == name)
        .unwrap_or_else(|| panic!("missing task {name}"))
        .dependencies
        .clone()
    };

    assert_eq!(
      dependencies("default@MergeProfile"),
      [
        "entry:default@PreBuild",
        "shared:default@MergeProfile",
        "library:release@MergeProfile",
        "entry:default@GenerateUnitTestTemplate",
      ]
    );
    assert_eq!(
      dependencies("default@CreateBuildProfile"),
      [
        "entry:default@PreBuild",
        "shared:default@CreateBuildProfile",
        "library:release@CreateHarBuildProfile",
      ]
    );
    assert_eq!(
      dependencies("default@ConfigureCmake"),
      [
        "entry:default@PreBuild",
        "shared:default@BuildNativeWithCmake",
        "library:release@BuildNativeWithCmake",
      ]
    );
    assert_eq!(
      dependencies("default@BuildNativeWithNinja"),
      [
        "entry:default@BuildNativeWithCmake",
        "shared:default@DoNativeStrip",
        "library:release@DoNativeStrip",
      ]
    );
    assert_eq!(
      dependencies("default@GenerateMetadata"),
      ["entry:default@PreBuild", "shared:default@GenerateMetadata",]
    );
    assert_eq!(
      dependencies("default@GenerateLoaderJson"),
      [
        "entry:default@ProcessRouterMap",
        "entry:default@ProcessShareConfig",
        "entry:default@ProcessStartupConfig",
        "entry:default@CreateModuleInfo",
        "shared:default@CompileResource",
        "shared:default@CollectDuplicateHarEntriesArkTS",
      ]
    );
    assert_eq!(
      dependencies("default@GeneratePkgContextInfo"),
      [
        "entry:default@PreBuild",
        "shared:default@GeneratePkgContextInfo",
        "library:release@BuildNativeWithCmake",
      ]
    );
    assert_eq!(
      dependencies("default@ProcessStartupConfig"),
      [
        "entry:default@GeneratePkgContextInfo",
        "shared:default@ProcessStartupConfig",
      ]
    );
    assert_eq!(
      dependencies("default@GenerateUnitTestResult"),
      [
        "entry:default@UnitTestArkTS",
        "shared:default@UnitTestArkTS",
      ]
    );
    assert!(dependencies("default@CompileArkTS")
      .contains(&"library:release@CreateHarBuildProfile".to_string()));
  }
}
