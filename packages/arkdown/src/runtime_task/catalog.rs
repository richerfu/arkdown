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

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RuntimeTaskCatalogRequest {
  node_name: String,
  kind: Option<RuntimeTaskNodeKind>,
  project_task: bool,
  #[serde(default)]
  build_target_name: Option<String>,
  #[serde(default)]
  target_names: Vec<String>,
  #[serde(default)]
  child_modules: Vec<RuntimeTaskChildModule>,
  #[serde(default)]
  sync_version_changed: bool,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct RuntimeTaskChildModule {
  name: String,
  kind: Option<RuntimeTaskNodeKind>,
}

#[derive(Debug, Deserialize, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct RuntimeTaskCatalogEntry {
  name: String,
  dependencies: Vec<String>,
  group: String,
  description: String,
  source_plugin_id: String,
  catalog_order: u32,
}

pub struct RuntimeTaskCatalog {
  request: RuntimeTaskCatalogRequest,
  definitions: IndexMap<String, Vec<String>>,
}

impl RuntimeTaskCatalog {
  pub fn from_json(request_json: &str) -> anyhow::Result<Self> {
    let request = serde_json::from_str(request_json)?;
    Ok(Self {
      request,
      definitions: IndexMap::new(),
    })
  }

  pub fn entries_json(mut self) -> anyhow::Result<String> {
    self.build();
    let entries = self
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
      .collect::<anyhow::Result<Vec<_>>>()?;
    serde_json::to_string(&entries).map_err(Into::into)
  }

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
      vec![
        target("UnitTestArkTS"),
        target("UnitTestCopyMockConfigJson"),
      ],
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
        self.add_owned(
          "assembleHap",
          vec![
            target("CollectDebugSymbol"),
            target("PackingCheck"),
            target("SignHap"),
          ],
        );
        self.add_owned("HotReloadBuild", vec![target("HotReloadArkTS")]);
        self.add_owned("assembleDevHqf", vec![target("SignHqf")]);
      }
      Some(RuntimeTaskNodeKind::Hsp) => {
        self.add_owned(
          "assembleHsp",
          vec![
            target("PackageSharedTgz"),
            target("CollectDebugSymbol"),
            target("PackingCheck"),
          ],
        );
        self.add_owned("genOnDeviceTestHap", vec![target("SignHap")]);
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
        self.add_owned("genOnDeviceTestHap", vec![target("SignHap")]);
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
    let mut tasks: IndexMap<&'static str, Vec<&'static str>> = IndexMap::new();
    tasks.insert("SyscapTransform", vec!["PreCheckSyscap"]);
    tasks.insert("MakePackInfo", vec!["MergeProfile"]);
    tasks.insert("GenerateMetadata", vec!["PreBuild"]);
    tasks.insert(
      "PackageHap",
      vec![
        "ProcessCompiledResources",
        "ProcessIntegratedHsp",
        "GeneratePkgSdkInfo",
      ],
    );
    tasks.insert("SignHap", vec!["PackageHap"]);
    tasks.insert(
      "CompileArkTS",
      vec![
        "GenerateLoaderJson",
        "CompileResource",
        "CreateBuildProfile",
      ],
    );
    tasks.insert("BuildArkTS", vec!["GenerateLoaderJson", "CompileResource"]);
    tasks.insert("BuildJS", vec!["GenerateLoaderJson", "CompileResource"]);
    tasks.insert(
      "HotReloadArkTS",
      vec!["$root:buildHotReloadResource", "CreateBuildProfile"],
    );
    tasks.insert("ColdReloadArkTS", vec![]);
    tasks.insert(
      "PreviewArkTS",
      vec!["PreviewUpdateAssets", "CreateBuildProfile"],
    );
    tasks.insert(
      "OhosTestCompileArkTS",
      vec![
        "GenerateLoaderJson",
        "CompileResource",
        "CreateBuildProfile",
      ],
    );
    tasks.insert(
      "OhosTestCompileJS",
      vec![
        "GenerateLoaderJson",
        "CompileResource",
        "CreateBuildProfile",
      ],
    );
    tasks.insert(
      "OhosTestBuildArkTS",
      vec![
        "GenerateLoaderJson",
        "CompileResource",
        "CreateBuildProfile",
      ],
    );
    tasks.insert(
      "OhosTestBuildJS",
      vec![
        "GenerateLoaderJson",
        "CompileResource",
        "CreateBuildProfile",
      ],
    );
    tasks.insert(
      "UnitTestArkTS",
      vec![
        "BuildUnitTestHook",
        "UnitTestProcessProfile",
        "CreateBuildProfile",
      ],
    );
    tasks.insert("ProcessResource", vec!["ProcessProfile"]);
    tasks.insert(
      "CompileResource",
      vec![
        "ProcessResource",
        "ProcessRouterMap",
        "ProcessShareConfig",
        "ProcessStartupConfig",
      ],
    );
    tasks.insert("PreviewProcessResource", vec!["ProcessProfile"]);
    tasks.insert("BuildNativeWithNinja", vec!["BuildNativeWithCmake"]);
    tasks.insert("BuildNativeWithCmake", vec!["ConfigureCmake"]);
    tasks.insert("ProcessLibs", vec!["BuildNativeWithNinja"]);
    tasks.insert("DoNativeBinxo", vec!["ProcessLibs"]);
    tasks.insert("PrepareQuickfix", vec!["ColdReloadArkTS", "DoNativeStrip"]);
    tasks.insert("DoNativeStrip", vec!["ProcessLibs"]);
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
    tasks.insert("MergeProfile", vec!["PreBuild"]);
    tasks.insert("PreviewHookCompileResource", vec!["CompileResource"]);
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
    tasks.insert("ReplacePreviewerPage", vec!["PreviewHookCompileResource"]);
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
      vec!["ProcessIntegratedHsp", "PreBuild"],
    );
    tasks.insert("PreCheckSyscap", vec!["PreBuild"]);
    tasks.insert("GenerateUnitTestTemplate", vec!["PreBuild"]);
    tasks.insert("GenerateOhosTestTemplate", vec!["PreBuild"]);
    tasks.insert("GenerateUnitTestResult", vec!["UnitTestArkTS"]);
    tasks.insert("ProcessRouterMap", vec!["GeneratePkgContextInfo"]);
    tasks.insert("ProcessShareConfig", vec!["GeneratePkgContextInfo"]);
    tasks.insert("ProcessStartupConfig", vec!["GeneratePkgContextInfo"]);
    tasks.insert("GenerateDeviceCoverage", vec!["PreBuild"]);
    tasks.insert("BeforeGenerateDeviceCoverage", vec!["PreBuild"]);
    tasks.insert("GeneratePkgContextInfo", vec!["PreBuild"]);
    tasks.insert("GeneratePkgSdkInfo", vec!["PreBuild"]);
    tasks.insert("ProcessIntegratedHsp", vec!["PreBuild"]);
    tasks.insert(
      "GeneratePkgModuleJson",
      vec![
        "MakePackInfo",
        "CacheNativeLibs",
        "GenerateMetadata",
        "SyscapTransform",
        "ProcessRouterMap",
        "ProcessShareConfig",
        "CompileArkTS",
        "BuildJS",
        "ProcessIntegratedHsp",
      ],
    );
    tasks.insert("ProcessCompiledResources", vec!["GeneratePkgModuleJson"]);
    tasks.insert("ProcessModulePrivacyProfile", vec!["PreBuild"]);

    if !matches!(self.request.kind, Some(RuntimeTaskNodeKind::Har)) {
      tasks.insert("CreateModuleInfo", vec!["PreBuild"]);
      tasks.insert("PackageHqf", vec!["PrepareQuickfix"]);
      tasks.insert("SignHqf", vec!["PackageHqf"]);
      tasks.insert(
        "PackingCheck",
        vec![
          if matches!(self.request.kind, Some(RuntimeTaskNodeKind::Hsp)) {
            "PackageHsp"
          } else {
            "PackageHap"
          },
        ],
      );
      tasks.insert(
        "CompileJS",
        vec![
          "GenerateLoaderJson",
          "CompileResource",
          "CreateBuildProfile",
        ],
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
        vec![
          if matches!(self.request.kind, Some(RuntimeTaskNodeKind::Hsp)) {
            "PackageHsp"
          } else {
            "PackageHap"
          },
        ],
      );
    }
    if matches!(self.request.kind, Some(RuntimeTaskNodeKind::Hsp)) {
      tasks.insert("PackageSharedHar", vec!["PrepareSharedHarResource"]);
      tasks.insert(
        "PrepareSharedHarResource",
        vec!["CompileArkTS", "DoNativeStrip"],
      );
      tasks.insert("PackageSharedTgz", vec!["SignHsp", "PackageSharedHar"]);
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
    }
    if matches!(self.request.kind, Some(RuntimeTaskNodeKind::Har)) {
      tasks.insert(
        "ProcessHarArtifacts",
        vec![
          "ProcessObfuscationFiles",
          "CacheNativeLibs",
          "ProcessOHPackageJson",
          "HarCompileArkTS",
          "ProcessIntegratedHsp",
        ],
      );
      tasks.insert(
        "PackageHar",
        vec!["ProcessHarArtifacts", "ProcessModulePrivacyProfile"],
      );
      tasks.insert("PackageSignHar", vec!["PackageHar"]);
      tasks.insert("LintArkTS", vec!["ProcessProfile", "CreateHarBuildProfile"]);
      tasks.insert("ProcessObfuscationFiles", vec!["CompileResource"]);
      tasks.insert("ProcessOHPackageJson", vec!["PreBuild"]);
      tasks.insert("ProcessPackageJson", vec!["PreBuild"]);
      tasks.insert("CreateHarBuildProfile", vec!["PreBuild"]);
      tasks.insert("CreateHarModuleInfo", vec!["PreBuild"]);
      tasks.insert("PackingCheckHar", vec!["PackageHar"]);
      tasks.insert(
        "HarCompileArkTS",
        vec![
          "GenerateLoaderJson",
          "CompileResource",
          "CreateHarBuildProfile",
        ],
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
}
