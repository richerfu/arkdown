use serde::Serialize;

use super::RuntimeTaskNodeKind;

/// Latest DevEco Studio 6.1.1 / hvigor 6.24.3 task metadata.
///
/// The source of truth is the bundled
/// `hvigor-ohos-plugin/src/tasks/common/task-names.js` plus a real
/// `.hvigor/outputs/sync/taskInfo.json`. Keeping this catalog in Rust avoids
/// loading or traversing task metadata in the JavaScript plugin bridge.
pub struct HvigorTaskInfoCatalog;

#[derive(Debug, Clone, Serialize)]
pub struct HvigorTaskInfo {
  #[serde(rename = "TASK_NAME")]
  pub task_name: String,
  #[serde(rename = "GROUP")]
  pub group: String,
  #[serde(rename = "DESCRIPTION")]
  pub description: String,
  #[serde(rename = "SOURCE_PLUGIN_ID")]
  pub source_plugin_id: String,
}

#[derive(Debug, Clone, Copy)]
struct HvigorTaskMetadata {
  group: &'static str,
  description: &'static str,
  source_plugin_id: &'static str,
}

impl HvigorTaskInfoCatalog {
  pub fn project_task_names() -> &'static [&'static str] {
    &[
      "tasks",
      "taskTree",
      "prune",
      "assembleApp",
      "clean",
      "collectCoverage",
      "init",
      "SignApp",
      "PackageApp",
      "SignPackagesFromApp",
      "ProcessProjectPrivacyProfile",
      "GeneratePackRes",
      "PreBuildApp",
      "DuplicateDependencyCheck",
      "MakeProjectPackInfo",
      "buildInfo",
    ]
  }

  pub fn module_task_names(
    kind: RuntimeTaskNodeKind,
    target_name: &str,
    sync_version_changed: bool,
  ) -> Vec<String> {
    let mut names = Vec::with_capacity(90);
    names.extend(
      Self::module_entry_task_names(kind)
        .iter()
        .map(|name| (*name).to_string()),
    );
    match kind {
      RuntimeTaskNodeKind::Hap => {
        Self::extend_target_names(&mut names, target_name, COMMON_TARGET_PREFIX);
        Self::extend_target_names(
          &mut names,
          target_name,
          &["PackingCheck", "PackageHap", "SignHap"],
        );
        Self::extend_target_names(&mut names, target_name, COMPILE_AND_TAIL_TASK_NAMES);
      }
      RuntimeTaskNodeKind::Hsp => {
        Self::extend_target_names(&mut names, target_name, COMMON_TARGET_PREFIX);
        Self::extend_target_names(
          &mut names,
          target_name,
          &[
            "PackageSharedHar",
            "PrepareSharedHarResource",
            "PackageSharedTgz",
            "PackingCheck",
            "PackageHsp",
            "SignHsp",
            "SignHap",
            "PackageHap",
          ],
        );
        Self::extend_target_names(&mut names, target_name, COMPILE_AND_TAIL_TASK_NAMES);
      }
      RuntimeTaskNodeKind::Har => {
        Self::extend_target_names(&mut names, target_name, HAR_TARGET_PREFIX);
        Self::extend_target_names(&mut names, target_name, &COMPILE_AND_TAIL_TASK_NAMES[6..]);
      }
      RuntimeTaskNodeKind::App => {}
    }
    names.extend(
      ["tasks", "taskTree", "init"]
        .into_iter()
        .map(str::to_string),
    );
    if sync_version_changed {
      // Latest hvigor exposes the regular clean task and appends the
      // version-change clean task to taskInfo.json as a second record.
      names.push("clean".to_string());
    }
    names
  }

  pub fn task_info(task_name: &str) -> anyhow::Result<HvigorTaskInfo> {
    let action_name = task_name
      .split_once('@')
      .map_or(task_name, |(_, action_name)| action_name);
    let metadata = HvigorTaskMetadata::for_name(action_name).ok_or_else(|| {
      anyhow::anyhow!(
        "ARKDOWN_HVIGOR_SYNC_TASK_METADATA_MISSING: latest-hvigor sync metadata is missing for task {task_name}"
      )
    })?;
    Ok(HvigorTaskInfo {
      task_name: task_name.to_string(),
      group: metadata.group.to_string(),
      description: metadata.description.to_string(),
      source_plugin_id: metadata.source_plugin_id.to_string(),
    })
  }

  fn module_entry_task_names(kind: RuntimeTaskNodeKind) -> &'static [&'static str] {
    match kind {
      RuntimeTaskNodeKind::Hap => &[
        "assembleHap",
        "buildPreviewerResource",
        "HotReloadBuild",
        "GenerateBuildProfile",
        "assembleDevHqf",
        "compileNative",
        "PreviewBuild",
        "UnitTestBuild",
        "test",
        "buildHotReloadResource",
        "clean",
        "onDeviceTest",
        "buildInfo",
      ],
      RuntimeTaskNodeKind::Hsp => &[
        "assembleHsp",
        "genOnDeviceTestHap",
        "buildPreviewerResource",
        "HotReloadBuild",
        "GenerateBuildProfile",
        "assembleDevHqf",
        "compileNative",
        "PreviewBuild",
        "UnitTestBuild",
        "test",
        "buildHotReloadResource",
        "clean",
        "onDeviceTest",
        "buildInfo",
      ],
      RuntimeTaskNodeKind::Har => &[
        "assembleHar",
        "buildPreviewerResource",
        "GenerateBuildProfile",
        "compileNative",
        "PreviewBuild",
        "UnitTestBuild",
        "test",
        "buildHotReloadResource",
        "clean",
        "onDeviceTest",
        "buildInfo",
        "genOnDeviceTestHap",
      ],
      RuntimeTaskNodeKind::App => &[],
    }
  }

  fn extend_target_names(output: &mut Vec<String>, target_name: &str, task_names: &[&str]) {
    output.extend(
      task_names
        .iter()
        .map(|task_name| format!("{target_name}@{task_name}")),
    );
  }
}

const COMMON_TARGET_PREFIX: &[&str] = &[
  "SyscapTransform",
  "CreateModuleInfo",
  "MakePackInfo",
  "GenerateMetadata",
  "PackageHqf",
  "SignHqf",
];

const HAR_TARGET_PREFIX: &[&str] = &[
  "ProcessHarArtifacts",
  "PackageHar",
  "PackageSignHar",
  "LintArkTS",
  "ProcessObfuscationFiles",
  "ProcessOHPackageJson",
  "ProcessPackageJson",
  "CreateHarBuildProfile",
  "CreateHarModuleInfo",
  "SyscapTransform",
  "MakePackInfo",
  "GenerateMetadata",
  "PackageHap",
  "SignHap",
  "PackingCheckHar",
  "HarCompileArkTS",
  "HarBuildArkTS",
  "CompileArkTS",
  "BuildArkTS",
  "BuildJS",
  "HotReloadArkTS",
  "ColdReloadArkTS",
];

const COMPILE_AND_TAIL_TASK_NAMES: &[&str] = &[
  "CompileArkTS",
  "CompileJS",
  "BuildArkTS",
  "BuildJS",
  "HotReloadArkTS",
  "ColdReloadArkTS",
  "PreviewArkTS",
  "OhosTestCompileArkTS",
  "OhosTestCompileJS",
  "OhosTestBuildArkTS",
  "OhosTestBuildJS",
  "UnitTestArkTS",
  "ProcessResource",
  "CompileResource",
  "PreviewProcessResource",
  "BuildNativeWithNinja",
  "BuildNativeWithCmake",
  "ProcessLibs",
  "DoNativeBinxo",
  "PrepareQuickfix",
  "DoNativeStrip",
  "CacheNativeLibs",
  "ConfigureCmake",
  "PreBuild",
  "GenerateLoaderJson",
  "ProcessProfile",
  "MergeProfile",
  "PreviewHookCompileResource",
  "CreateBuildProfile",
  "PreviewUpdateAssets",
  "BuildUnitTestHook",
  "ReplaceSourceMapInCoverageMode",
  "ReplaceUnitTestIndexFile",
  "UnitTestProcessProfile",
  "ReplacePreviewerPage",
  "CopyPreviewProfile",
  "PreviewCompileResource",
  "OhosTestCopyMockConfigJson",
  "UnitTestCopyMockConfigJson",
  "SignProjectRemoteHsp",
  "SignModuleRemoteHsp",
  "PreCheckSyscap",
  "GenerateUnitTestTemplate",
  "GenerateOhosTestTemplate",
  "GenerateUnitTestResult",
  "ProcessRouterMap",
  "ProcessShareConfig",
  "ProcessStartupConfig",
  "GenerateDeviceCoverage",
  "BeforeGenerateDeviceCoverage",
  "GeneratePkgContextInfo",
  "GeneratePkgSdkInfo",
  "ProcessIntegratedHsp",
  "GeneratePkgModuleJson",
  "ProcessCompiledResources",
  "CollectDebugSymbol",
  "ProcessModulePrivacyProfile",
];

impl HvigorTaskMetadata {
  const fn new(
    group: &'static str,
    description: &'static str,
    source_plugin_id: &'static str,
  ) -> Self {
    Self {
      group,
      description,
      source_plugin_id,
    }
  }

  fn for_name(name: &str) -> Option<Self> {
    match name {
      "tasks" => Some(Self::new(
        "Help",
        "Displays the tasks of the HvigorCoreNode.",
        "hvigor",
      )),
      "taskTree" => Some(Self::new(
        "Help",
        "Displays the task dag tree of the HvigorCoreNode.",
        "hvigor",
      )),
      "prune" => Some(Self::new(
        "Other",
        "Clean up old hvigor cache files and remove unreferenced packages from store.",
        "hvigor",
      )),
      "init" => Some(Self::new(
        "Sync",
        "Sync and refresh project",
        "hvigor-ohos-plugin",
      )),
      "clean" => Some(Self::new(
        "Other",
        "Clear the cache information.",
        "hvigor-ohos-plugin",
      )),
      "buildInfo" => Some(Self::new(
        "Help",
        "Displays the build information of the Project or Module.",
        "hvigor-ohos-plugin",
      )),
      "collectCoverage" => Some(Self::new(
        "Other",
        "Generate coverage statistics reports based on instrumentation dotting data.",
        "hvigor-ohos-plugin",
      )),
      "assembleApp" => Some(Self::new(
        "Hook",
        "Assemble the task for the packaged app.",
        "hvigor-ohos-plugin",
      )),
      "assembleHap" => Some(Self::new(
        "Hook",
        "Assemble the task for the packaged HAP file.",
        "hvigor-ohos-plugin",
      )),
      "assembleHsp" => Some(Self::new(
        "Hook",
        "Assemble the task for the packaged HSP file.",
        "hvigor-ohos-plugin",
      )),
      "assembleHar" => Some(Self::new(
        "Hook",
        "Assemble the task for the packaged HAR file.",
        "hvigor-ohos-plugin",
      )),
      "genOnDeviceTestHap" => Some(Self::new(
        "Hook",
        "Assemble the task for the packaged HAP file in HSP or HAR.",
        "hvigor-ohos-plugin",
      )),
      "buildPreviewerResource" => Some(Self::new(
        "Hook",
        "Build the preview resources.",
        "hvigor-ohos-plugin",
      )),
      "HotReloadBuild" => Some(Self::new(
        "Hook",
        "Build in hot reload mode.",
        "hvigor-ohos-plugin",
      )),
      "GenerateBuildProfile" => Some(Self::new(
        "Hook",
        "Create build profile hook task.",
        "hvigor-ohos-plugin",
      )),
      "assembleDevHqf" => Some(Self::new(
        "Hook",
        "Assemble the task for the packaged HQF file.",
        "hvigor-ohos-plugin",
      )),
      "compileNative" => Some(Self::new(
        "Hook",
        "Compile the hook task for native resources.",
        "hvigor-ohos-plugin",
      )),
      "PreviewBuild" => Some(Self::new(
        "Hook",
        "Build preview in the stage model.",
        "hvigor-ohos-plugin",
      )),
      "UnitTestBuild" => Some(Self::new(
        "Hook",
        "Build a unit test.",
        "hvigor-ohos-plugin",
      )),
      "test" => Some(Self::new(
        "Hook",
        "Build a unit test without DevEco Studio",
        "hvigor-ohos-plugin",
      )),
      "buildHotReloadResource" => Some(Self::new(
        "Hook",
        "Build the hot reload resources.",
        "hvigor-ohos-plugin",
      )),
      "onDeviceTest" => Some(Self::new(
        "Hook",
        "Build a device test without DevEco Studio",
        "hvigor-ohos-plugin",
      )),
      "SignApp" => Some(Self::new(
        "Sign",
        "Sign the app package.",
        "hvigor-ohos-plugin",
      )),
      "PackageApp" => Some(Self::new(
        "Package",
        "Build the app package in the stage model.",
        "hvigor-ohos-plugin",
      )),
      "SignPackagesFromApp" => Some(Self::new(
        "Package",
        "Build all unsigned app package in the stage model.",
        "hvigor-ohos-plugin",
      )),
      "ProcessProjectPrivacyProfile" => Some(Self::new(
        "Generate",
        "Process project pac.json5 in the stage model.",
        "hvigor-ohos-plugin",
      )),
      "GeneratePackRes" => Some(Self::new(
        "Generate",
        "Build the pack.res file in the stage model.",
        "hvigor-ohos-plugin",
      )),
      "MakeProjectPackInfo" => Some(Self::new(
        "Generate",
        "Generate project pack.info in the stage model.",
        "hvigor-ohos-plugin",
      )),
      "PreBuildApp" => Some(Self::new(
        "Verification",
        "Prepare before building the app.",
        "hvigor-ohos-plugin",
      )),
      "DuplicateDependencyCheck" => Some(Self::new(
        "Verification",
        "Check duplicated dependencies when building app.",
        "hvigor-ohos-plugin",
      )),
      "SyscapTransform" => Some(Self::new(
        "Generate",
        "Transform SysCap in the stage model.",
        "hvigor-ohos-plugin",
      )),
      "CreateModuleInfo" => Some(Self::new(
        "Generate",
        "Create the ModuleInfo.ts file for the hap/hsp package.",
        "hvigor-ohos-plugin",
      )),
      "MakePackInfo" => Some(Self::new(
        "Generate",
        "Generate module pack.info in the stage model.",
        "hvigor-ohos-plugin",
      )),
      "GenerateMetadata" => Some(Self::new(
        "Generate",
        "Generate metadata in the stage model.",
        "hvigor-ohos-plugin",
      )),
      "PackageHqf" => Some(Self::new(
        "Package",
        "Preform quickfix hqf packaging.",
        "hvigor-ohos-plugin",
      )),
      "SignHqf" => Some(Self::new(
        "Sign",
        "Signing for quickfix hqf package.",
        "hvigor-ohos-plugin",
      )),
      "PackingCheck" => Some(Self::new(
        "Verification",
        "Check package product",
        "hvigor-ohos-plugin",
      )),
      "PackingCheckHar" => Some(Self::new(
        "Verification",
        "Check package product for HAR",
        "hvigor-ohos-plugin",
      )),
      "PackageHap" => Some(Self::new(
        "Package",
        "Build the HAP package in the stage model.",
        "hvigor-ohos-plugin",
      )),
      "PackageHsp" => Some(Self::new(
        "Package",
        "Build the HSP package in the stage model.",
        "hvigor-ohos-plugin",
      )),
      "SignHap" => Some(Self::new(
        "Sign",
        "Sign the HAP package.",
        "hvigor-ohos-plugin",
      )),
      "SignHsp" => Some(Self::new(
        "Sign",
        "Sign the HAP package.",
        "hvigor-ohos-plugin",
      )),
      "PackageSignHar" => Some(Self::new(
        "Sign",
        "Package and sign the HAR package.",
        "hvigor-ohos-plugin",
      )),
      "CompileArkTS" => Some(Self::new(
        "ArkTS",
        "Compile ArkTS or JS components for large-system devices in the stage model.",
        "hvigor-ohos-plugin",
      )),
      "CompileJS" => Some(Self::new(
        "ArkTS",
        "Compile ArkTS or JS components for large-system devices in the stage model.",
        "hvigor-ohos-plugin",
      )),
      "BuildArkTS" => Some(Self::new(
        "ArkTS",
        "Compile ArkTS or JS components using Node.js for large-system devices in the stage model.",
        "hvigor-ohos-plugin",
      )),
      "BuildJS" => Some(Self::new(
        "ArkTS",
        "Compile ArkTS or JS components using Node.js for large-system devices in the stage model.",
        "hvigor-ohos-plugin",
      )),
      "HotReloadArkTS" => Some(Self::new(
        "ArkTS",
        "Compile ArkTS components during hot loading in the stage model.",
        "hvigor-ohos-plugin",
      )),
      "ColdReloadArkTS" => Some(Self::new(
        "ArkTS",
        "Compile ArkTS components during cold loading in the stage model.",
        "hvigor-ohos-plugin",
      )),
      "PreviewArkTS" => Some(Self::new(
        "Generate",
        "Compile ArkTS components on preview in the stage model.",
        "hvigor-ohos-plugin",
      )),
      "OhosTestCompileArkTS" => Some(Self::new(
        "ArkTS",
        "Compile ArkTS or JS components using Node.js for large-system devices in the stage model.",
        "hvigor-ohos-plugin",
      )),
      "OhosTestCompileJS" => Some(Self::new(
        "ArkTS",
        "Compile ArkTS or JS components using Node.js for large-system devices in the stage model.",
        "hvigor-ohos-plugin",
      )),
      "OhosTestBuildArkTS" => Some(Self::new(
        "ArkTS",
        "Compile ArkTS or JS components using Node.js for large-system devices in the stage model.",
        "hvigor-ohos-plugin",
      )),
      "OhosTestBuildJS" => Some(Self::new(
        "ArkTS",
        "Compile ArkTS or JS components using Node.js for large-system devices in the stage model.",
        "hvigor-ohos-plugin",
      )),
      "UnitTestArkTS" => Some(Self::new(
        "ArkTS",
        "Compile ArkTS components at unit test time in the stage model.",
        "hvigor-ohos-plugin",
      )),
      "ProcessResource" => Some(Self::new(
        "Resources",
        "Process resources in the stage model.",
        "hvigor-ohos-plugin",
      )),
      "CompileResource" => Some(Self::new(
        "Resources",
        "Compile project resources in the stage model.",
        "hvigor-ohos-plugin",
      )),
      "PreviewProcessResource" => Some(Self::new(
        "Resources",
        "Process preview resources incrementally in the stage model.",
        "hvigor-ohos-plugin",
      )),
      "BuildNativeWithNinja" => Some(Self::new(
        "Native",
        "Compile CPP source with Ninja in the stage model.",
        "hvigor-ohos-plugin",
      )),
      "BuildNativeWithCmake" => Some(Self::new(
        "Native",
        "Compile CPP source with CMake in the stage model.",
        "hvigor-ohos-plugin",
      )),
      "ProcessLibs" => Some(Self::new(
        "Resources",
        "Process .so files in the stage model.",
        "hvigor-ohos-plugin",
      )),
      "DoNativeBinxo" => Some(Self::new("Resources", "Binxo .so files to check file.", "")),
      "PrepareQuickfix" => Some(Self::new(
        "Generate",
        "Prepare changed assets for quickfix.",
        "hvigor-ohos-plugin",
      )),
      "DoNativeStrip" => Some(Self::new(
        "Native",
        "Strip .so files to decrease size.",
        "hvigor-ohos-plugin",
      )),
      "CacheNativeLibs" => Some(Self::new(
        "Native",
        "cache native strip .so fileInfo",
        "hvigor-ohos-plugin",
      )),
      "ConfigureCmake" => Some(Self::new(
        "Native",
        "cache native strip .so fileInfo",
        "hvigor-ohos-plugin",
      )),
      "PreBuild" => Some(Self::new(
        "Verification",
        "Pre-build in the stage model.",
        "hvigor-ohos-plugin",
      )),
      "GenerateLoaderJson" => Some(Self::new(
        "Generate",
        "Generate loader.json in the stage model.",
        "hvigor-ohos-plugin",
      )),
      "ProcessProfile" => Some(Self::new(
        "Config",
        "Process app config manifest files in the stage model.",
        "hvigor-ohos-plugin",
      )),
      "MergeProfile" => Some(Self::new(
        "Config",
        "Merge app config manifest files in the stage model.",
        "hvigor-ohos-plugin",
      )),
      "PreviewHookCompileResource" => Some(Self::new(
        "Hook",
        "Add a hook task for compiling preview resources.",
        "hvigor-ohos-plugin",
      )),
      "CreateBuildProfile" => Some(Self::new(
        "Generate",
        "Create the BuildProfile.ets file for the HAP/HSP package.",
        "hvigor-ohos-plugin",
      )),
      "PreviewUpdateAssets" => Some(Self::new(
        "Config",
        "Update main_pages.json and module.json before PreviewBuild.",
        "hvigor-ohos-plugin",
      )),
      "BuildUnitTestHook" => Some(Self::new(
        "Hook",
        "Build a unit test hook.",
        "hvigor-ohos-plugin",
      )),
      "ReplaceSourceMapInCoverageMode" => Some(Self::new(
        "Generate",
        "Replace sourcemap in coverage mode.",
        "hvigor-ohos-plugin",
      )),
      "ReplaceUnitTestIndexFile" => Some(Self::new(
        "Other",
        "Replace the index file in unit tests.",
        "hvigor-ohos-plugin",
      )),
      "UnitTestProcessProfile" => Some(Self::new(
        "Config",
        "Add the entry TestAbility to the profile file during unit test building.",
        "hvigor-ohos-plugin",
      )),
      "ReplacePreviewerPage" => Some(Self::new(
        "Other",
        "Accept previewer arguments, then use them to replace the ones in the page file.",
        "hvigor-ohos-plugin",
      )),
      "CopyPreviewProfile" => Some(Self::new(
        "Other",
        "Copy preview profile.",
        "hvigor-ohos-plugin",
      )),
      "PreviewCompileResource" => Some(Self::new(
        "Resources",
        "Use Preview to compile project resources in the stage model.",
        "hvigor-ohos-plugin",
      )),
      "OhosTestCopyMockConfigJson" => Some(Self::new(
        "Other",
        "Ohos test copy mock config json to hap resources.",
        "hvigor-ohos-plugin",
      )),
      "UnitTestCopyMockConfigJson" => Some(Self::new(
        "Other",
        "Unit test copy mock config json to hap resources.",
        "hvigor-ohos-plugin",
      )),
      "SignProjectRemoteHsp" => Some(Self::new(
        "Sign",
        "Sign the project-level remote HSP package.",
        "hvigor-ohos-plugin",
      )),
      "SignModuleRemoteHsp" => Some(Self::new(
        "Sign",
        "Sign the module-level remote HSP package.",
        "hvigor-ohos-plugin",
      )),
      "PreCheckSyscap" => Some(Self::new(
        "Verification",
        "Pre-check SysCap.",
        "hvigor-ohos-plugin",
      )),
      "GenerateUnitTestTemplate" => Some(Self::new(
        "Generate",
        "Building a unit test template without DevEco Studio",
        "hvigor-ohos-plugin",
      )),
      "GenerateOhosTestTemplate" => Some(Self::new(
        "Generate",
        "Building a ohosTest template without DevEco Studio",
        "hvigor-ohos-plugin",
      )),
      "GenerateUnitTestResult" => Some(Self::new(
        "Generate",
        "Building a unit test result without DevEco Studio",
        "hvigor-ohos-plugin",
      )),
      "ProcessRouterMap" => Some(Self::new(
        "Generate",
        "Process router map configuration.",
        "hvigor-ohos-plugin",
      )),
      "ProcessShareConfig" => Some(Self::new(
        "Generate",
        "Process share config configuration.",
        "hvigor-ohos-plugin",
      )),
      "ProcessStartupConfig" => Some(Self::new(
        "Generate",
        "Process startup configuration.",
        "hvigor-ohos-plugin",
      )),
      "GenerateDeviceCoverage" => Some(Self::new(
        "Generate",
        "Generate ohos test coverage report.",
        "hvigor-ohos-plugin",
      )),
      "BeforeGenerateDeviceCoverage" => Some(Self::new(
        "Generate",
        "Generate ohos test coverage report prerequisites.",
        "hvigor-ohos-plugin",
      )),
      "GeneratePkgContextInfo" => Some(Self::new(
        "Generate",
        "Generate pkgContextInfo.json to ets-loader.",
        "hvigor-ohos-plugin",
      )),
      "GeneratePkgSdkInfo" => Some(Self::new("Generate", "Generate pkgSdkInfo.json.", "")),
      "ProcessIntegratedHsp" => Some(Self::new(
        "Sign",
        "Use package tool process integrated hsp.",
        "hvigor-ohos-plugin",
      )),
      "GeneratePkgModuleJson" => Some(Self::new(
        "Package",
        "Generate the module.json in the stage model.",
        "hvigor-ohos-plugin",
      )),
      "ProcessCompiledResources" => Some(Self::new(
        "Resources",
        "Process compiled resources in stage model.",
        "hvigor-ohos-plugin",
      )),
      "CollectDebugSymbol" => Some(Self::new(
        "Generate",
        "Collect debug symbols to output dir.",
        "hvigor-ohos-plugin",
      )),
      "ProcessModulePrivacyProfile" => Some(Self::new(
        "Generate",
        "Process module pac.json5 in the stage model.",
        "hvigor-ohos-plugin",
      )),
      "PackageSharedHar" => Some(Self::new(
        "Package",
        "Build the shared HAR package in the stage model.",
        "hvigor-ohos-plugin",
      )),
      "PrepareSharedHarResource" => Some(Self::new(
        "Config",
        "Prepare resources for the shared HAR package in the stage model.",
        "hvigor-ohos-plugin",
      )),
      "PackageSharedTgz" => Some(Self::new(
        "Package",
        "Build the shared HAR and HSP files into a TGZ package in the stage model.",
        "hvigor-ohos-plugin",
      )),
      "ProcessHarArtifacts" => Some(Self::new(
        "Package",
        "process the HAR build artifacts in the stage model.",
        "hvigor-ohos-plugin",
      )),
      "PackageHar" => Some(Self::new(
        "Package",
        "Build the HAR package in the stage model.",
        "hvigor-ohos-plugin",
      )),
      "LintArkTS" => Some(Self::new("", "", "")),
      "ProcessObfuscationFiles" => Some(Self::new("", "", "")),
      "ProcessOHPackageJson" => Some(Self::new(
        "Generate",
        "Process HAR package.json in the stage model.",
        "hvigor-ohos-plugin",
      )),
      "ProcessPackageJson" => Some(Self::new(
        "Generate",
        "Process HAR package.json in the stage model.",
        "hvigor-ohos-plugin",
      )),
      "CreateHarBuildProfile" => Some(Self::new(
        "Generate",
        "Create the BuildProfile.ets file for the HAR package.",
        "hvigor-ohos-plugin",
      )),
      "CreateHarModuleInfo" => Some(Self::new(
        "Generate",
        "Create the ModuleInfo.ts file for the byte code HAR package.",
        "hvigor-ohos-plugin",
      )),
      "HarCompileArkTS" => Some(Self::new(
        "ArkTS",
        "Compile ArkTS components for packaged HAR files in the stage model.",
        "hvigor-ohos-plugin",
      )),
      "HarBuildArkTS" => Some(Self::new(
        "ArkTS",
        "Build ArkTS components for packaged HAR files in the stage model.",
        "hvigor-ohos-plugin",
      )),
      _ => None,
    }
  }
}
