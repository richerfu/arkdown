/*
 * Human-readable TypeScript reconstruction.
 * Generated from hvigor/ts by hvigor/tools/reconstruct-readable-ts.cjs.
 * Keep semantic edits in Rust ports or separate hand-refined notes.
 * Source baseline: hvigor-ohos-plugin/src/tasks/manager/task-creation-manager.js
 */
import path_1 from "path";
import * as hvigor_1 from "@ohos/hvigor";
import * as fse from "fs-extra";
import * as build_directory_const_js_1 from "../../const/build-directory-const";
import * as common_const_js_1 from "../../const/common-const";
import * as inject_const_js_1 from "../../const/inject-const";
import * as sdk_const_js_1 from "../../const/sdk-const";
import * as module_type_enum_js_1 from "../../enum/module-type-enum";
import * as har_target_util_js_1 from "../../utils/har-target-util";
import * as hsp_target_utils_js_1 from "../../utils/hsp-target-utils";
import * as inject_util_js_1 from "../../utils/inject-util";
import * as meta_util_js_1 from "../../utils/meta-util";
import * as task_util_js_1 from "../../utils/task-util";
import * as base_pack_hap_task_js_1 from "../base/base-pack-hap-task";
import * as before_generate_device_coverage_js_1 from "../before-generate-device-coverage";
import * as build_native_with_cmake_js_1 from "../build-native-with-cmake";
import * as build_profile_task_js_1 from "../build-profile-task";
import * as collect_debug_symbol_js_1 from "../collect-debug-symbol";
import * as task_names_js_1 from "../common/task-names";
import * as compile_resource_js_1 from "../compile-resource";
import * as fast_package_js_1 from "../fast-package";
import * as generate_device_coverage_js_1 from "../generate-device-coverage";
import * as generate_loader_json_js_1 from "../generate-loader-json";
import * as generate_metadata_js_1 from "../generate-metadata";
import * as generate_pkg_context_info_js_1 from "../generate-pkg-context-info";
import * as generate_pkg_module_json_js_1 from "../generate-pkg-module-json";
import * as generate_pkg_sdk_info_js_1 from "../generate-pkg-sdk-info";
import * as process_id_defined_file_js_1 from "../har-deduplicate/process-id-defined-file";
import * as ark_linter_js_1 from "../har/ark-linter";
import * as har_build_profile_task_js_1 from "../har/har-build-profile-task";
import * as package_har_js_1 from "../har/package-har";
import * as package_sign_har_js_1 from "../har/package-sign-har";
import * as process_har_artifacts_js_1 from "../har/process-har-artifacts";
import * as process_obfuscation_files_js_1 from "../har/process-obfuscation-files";
import * as process_oh_package_json_js_1 from "../har/process-oh-package-json";
import * as process_package_json_js_1 from "../har/process-package-json";
import * as preview_hook_compile_resource_js_1 from "../hook/preview-hook-compile-resource";
import * as fast_package_hsp_js_1 from "../hsp/fast-package-hsp";
import * as package_hsp_js_1 from "../hsp/package-hsp";
import * as package_shared_har_js_1 from "../hsp/package-shared-har";
import * as package_shared_tgz_js_1 from "../hsp/package-shared-tgz";
import * as prepare_shared_har_res_js_1 from "../hsp/prepare-shared-har-res";
import * as sign_hsp_js_1 from "../hsp/sign-hsp";
import * as legacy_package_hap_js_1 from "../legacy-tasks/legacy-package-hap";
import * as make_pack_info_js_1 from "../make-pack-info";
import * as merge_profile_js_1 from "../merge-profile";
import * as create_har_module_info_js_1 from "../module-info/create-har-module-info";
import * as create_module_info_js_1 from "../module-info/create-module-info";
import * as generate_ohos_test_template_js_1 from "../ohosTest/generate-ohos-test-template";
import * as ohos_test_copy_mock_config_json_js_1 from "../ohosTest/ohos-test-copy-mock-config-json";
import * as process_module_privacy_profile_js_1 from "../pac-process/process-module-privacy-profile";
import * as package_hap_js_1 from "../package-hap";
import * as package_hqf_js_1 from "../package-hqf";
import * as packing_check_har_js_1 from "../packing-check/packing-check-har";
import * as packing_check_js_1 from "../packing-check/packing-check";
import * as pre_build_js_1 from "../pre-build";
import * as pre_check_syscap_js_1 from "../pre-check-syscap";
import * as preview_compile_resource_js_1 from "../preview-compile-resource";
import * as preview_update_assets_js_1 from "../preview-update-assets";
import * as copy_preview_profile_js_1 from "../previewer/copy-preview-profile";
import * as replace_previewer_page_js_1 from "../previewer/replace-previewer-page";
import * as process_compiled_resources_js_1 from "../process-compiled-resources";
import * as process_integrated_hsp_js_1 from "../process-integrated-hsp";
import * as process_profile_js_1 from "../process-profile";
import * as process_router_map_js_1 from "../process-router-map";
import * as process_share_config_js_1 from "../process-share-config";
import * as process_startup_config_js_1 from "../process-startup-config";
import * as global_project_data_service_js_1 from "../service/global-project-data-service";
import * as sign_hap_js_1 from "../sign-hap";
import * as sign_hqf_js_1 from "../sign-hqf";
import * as sign_module_remote_hsp_js_1 from "../sign-module-remote-hsp";
import * as sign_project_remote_hsp_js_1 from "../sign-project-remote-hsp";
import * as syscap_transform_js_1 from "../syscap-transform";
import * as task_creator_js_1 from "../task-creator";
import * as build_unit_test_hook_js_1 from "../unitTest/build-unit-test-hook";
import * as generate_unit_test_result_js_1 from "../unitTest/generate-unit-test-result";
import * as generate_unit_test_template_js_1 from "../unitTest/generate-unit-test-template";
import * as replace_sourcemap_in_coverage_mode_js_1 from "../unitTest/replace-sourcemap-in-coverage-mode";
import * as replace_unit_test_index_file_js_1 from "../unitTest/replace-unit-test-index-file";
import * as unit_test_copy_mock_config_json_js_1 from "../unitTest/unit-test-copy-mock-config-json";
import * as unit_test_process_profile_js_1 from "../unitTest/unit-test-process-profile";
import * as creation_util_js_1 from "./creation-util";
var Tasks = task_names_js_1.TaskNames.Task,
  CommonTask = task_names_js_1.TaskNames.CommonTask;
export class OhosTestCopyMockConfigJsonCA extends task_creator_js_1.TargetTaskCreator {
  constructor() {
    super(...arguments), this.provider = () => new ohos_test_copy_mock_config_json_js_1.OhosTestCopyMockConfigJson(this.targetService), this.declareDepends = () => ["OhosTestCompileArkTS"], this.declareTaskDetail = () => task_names_js_1.TaskNames.Task.OHOS_TEST_COPY_MOCK_CONFIG_JSON;
  }
}
export class UnitTestCopyMockConfigJsonCA extends task_creator_js_1.TargetTaskCreator {
  constructor() {
    super(...arguments), this.provider = () => new unit_test_copy_mock_config_json_js_1.UnitTestCopyMockConfigJson(this.targetService), this.declareDepends = () => ["UnitTestArkTS"], this.declareTaskDetail = () => task_names_js_1.TaskNames.Task.UNIT_TEST_COPY_MOCK_CONFIG_JSON;
  }
}
export class PreBuildCA extends task_creator_js_1.TargetTaskCreator {
  constructor() {
    super(...arguments), this.provider = () => new pre_build_js_1.PreBuild(this.targetService), this.declareDepends = () => (0, meta_util_js_1.hvigorOrToolChainsChanged)(this.targetService.getSdkInfo()) ? [CommonTask.CLEAN.name] : [], this.declareTaskDetail = () => task_names_js_1.TaskNames.Task.PRE_BUILD, this.afterConfigure = () => {
      this.depends = this.declareDepends().map(e => `${this.node.getName()}:${e}`);
    };
  }
}
export class GenerateUnitTestTemplateCA extends task_creator_js_1.TargetTaskCreator {
  constructor() {
    super(...arguments), this.provider = () => new generate_unit_test_template_js_1.GenerateUnitTestTemplate(this.targetService), this.declareDepends = () => [pre_build_js_1.PreBuild.name], this.declareTaskDetail = () => task_names_js_1.TaskNames.Task.GENERATE_UNIT_TEST_TEMPLATE;
  }
}
export class GenerateOhosTestTemplateCA extends task_creator_js_1.TargetTaskCreator {
  constructor() {
    super(...arguments), this.provider = () => new generate_ohos_test_template_js_1.GenerateOhosTestTemplate(this.targetService), this.declareDepends = () => [pre_build_js_1.PreBuild.name], this.declareTaskDetail = () => task_names_js_1.TaskNames.Task.GENERATE_OHOS_TEST_TEMPLATE;
  }
}
export class GenerateUnitTestResultCA extends task_creator_js_1.TargetTaskCreator {
  constructor() {
    super(...arguments), this.provider = () => new generate_unit_test_result_js_1.GenerateUnitTestResult(this.targetService), this.declareDepends = () => {
      const e = [this.isFa ? common_const_js_1.ArktSTaskConst.LEGACY_UNIT_TEST_ARKTS : common_const_js_1.ArktSTaskConst.UNIT_TEST_ARKTS];
      return hsp_target_utils_js_1.HspTargetUtils.calDepHspTargets(this.targetService).forEach((s, r) => {
        const a = `${r}:${s.getTargetName()}@${this.isFa ? "LegacyUnitTestArkTS" : "UnitTestArkTS"}`;
        return e.push(a);
      }), e;
    }, this.declareTaskDetail = () => task_names_js_1.TaskNames.Task.GENERATE_UNIT_TEST_RESULT;
  }
}
export class GenerateDeviceCoverageCA extends task_creator_js_1.TargetTaskCreator {
  constructor() {
    super(...arguments), this.provider = () => new generate_device_coverage_js_1.GenerateDeviceCoverage(this.targetService), this.declareDepends = () => {
      const e = [pre_build_js_1.PreBuild.name];
      if (!hvigor_1.globalData.cliOpts._.includes(common_const_js_1.CommonConst.ON_DEVICE_TEST)) return e;
      if ("default" !== this.targetService.getTargetData().getTargetName()) return e;
      if (!this.targetService.getModuleService().findTargetDataByName(common_const_js_1.DefaultTargetConst.OHOS_TEST_TARGET)) return e;
      const s = path_1.resolve(path_1.resolve(this.targetService.getModuleService().getModuleModel().getProjectDir(), build_directory_const_js_1.BuildDirConst.SRC), common_const_js_1.DefaultTargetConst.OHOS_TEST_TARGET);
      if (!fse.existsSync(s)) return e;
      const r = this.targetService.getModuleService().getModuleModel().getModuleType();
      let a = "";
      return r === module_type_enum_js_1.ModuleType.Entry || r === module_type_enum_js_1.ModuleType.Feature ? a = sign_hap_js_1.SignHap.name : r === module_type_enum_js_1.ModuleType.Shared ? a = sign_hsp_js_1.SignHsp.name : r === module_type_enum_js_1.ModuleType.Har && (a = package_har_js_1.PackageHar.name), a && (0, creation_util_js_1.deviceTestComputeDepends)(e, a, this.targetService, r), e.push(before_generate_device_coverage_js_1.BeforeGenerateDeviceCoverage.name), e;
    }, this.declareTaskDetail = () => task_names_js_1.TaskNames.Task.GENERATE_DEVICE_COVERAGE;
  }
}
export class BeforeGenerateDeviceCoverageCA extends task_creator_js_1.TargetTaskCreator {
  constructor() {
    super(...arguments), this.provider = () => new before_generate_device_coverage_js_1.BeforeGenerateDeviceCoverage(this.targetService), this.declareTaskDetail = () => task_names_js_1.TaskNames.Task.BEFORE_GENERATE_DEVICE_COVERAGE;
  }
  declareDepends() {
    const e = [pre_build_js_1.PreBuild.name];
    this.targetService.getModuleService().getModuleModel().getModuleType() === module_type_enum_js_1.ModuleType.Shared && e.push(sign_hsp_js_1.SignHsp.name);
    return hsp_target_utils_js_1.HspTargetUtils.calDepHspTargets(this.targetService).forEach((s, r) => e.push(`${r}:${s.getTargetName()}@${task_names_js_1.TaskNames.Task.BEFORE_GENERATE_DEVICE_COVERAGE.name}`)), e;
  }
}
export class GeneratePkgContextInfoCA extends task_creator_js_1.TargetTaskCreator {
  constructor() {
    super(...arguments), this.provider = () => new generate_pkg_context_info_js_1.GeneratePkgContextInfo(this.targetService), this.declareDepends = () => {
      var e, s;
      const r = [pre_build_js_1.PreBuild.name];
      this.targetService.getShouldBuildCmake() && (null === (e = this.targetService.getBuildOption().strictMode) || undefined === e ? undefined : e.useNormalizedOHMUrl) && r.push(build_native_with_cmake_js_1.BuildNativeWithCmake.name);
      const a = har_target_util_js_1.HarTargetUtil.calDepHarTargets(this.targetService);
      for (const [e, t] of a) {
        const a = this.targetService.getModuleService().getProjectModel().getTarget(e, t);
        a && a.getShouldBuildCmake() && (null === (s = a.getBuildOption().strictMode) || undefined === s ? undefined : s.useNormalizedOHMUrl) && r.push(`${e}:${t}@${build_native_with_cmake_js_1.BuildNativeWithCmake.name}`);
      }
      if (this.targetService.shouldEnableHarDeduplicate() && (this.targetService.getModuleService().getModuleModel().isHapModule() || this.targetService.getModuleService().getModuleModel().isHspModule())) {
        hsp_target_utils_js_1.HspTargetUtils.calDepHspTargets(this.targetService).forEach((e, s) => r.push(`${s}:${e.getTargetName()}@${task_names_js_1.TaskNames.Task.GENERATE_PKG_CONTEXT_INFO.name}`));
      }
      return r;
    }, this.declareTaskDetail = () => task_names_js_1.TaskNames.Task.GENERATE_PKG_CONTEXT_INFO;
  }
}
export class GeneratePkgSdkInfoCA extends task_creator_js_1.TargetTaskCreator {
  constructor() {
    super(...arguments), this.provider = () => new generate_pkg_sdk_info_js_1.GeneratePkgSdkInfo(this.targetService), this.declareDepends = () => [pre_build_js_1.PreBuild.name], this.declareTaskDetail = () => task_names_js_1.TaskNames.Task.GENERATE_PKG_SDK_INFO;
  }
}
export class ProcessProfileCA extends task_creator_js_1.TargetTaskCreator {
  constructor() {
    super(...arguments), this.provider = () => new process_profile_js_1.ProcessProfile(this.targetService), this.declareDepends = () => [merge_profile_js_1.MergeProfile.name, generate_pkg_context_info_js_1.GeneratePkgContextInfo.name], this.declareTaskDetail = () => task_names_js_1.TaskNames.Task.PROCESS_PROFILE;
  }
}
export class MergeProfileCA extends task_creator_js_1.TargetTaskCreator {
  constructor() {
    super(...arguments), this.provider = () => new merge_profile_js_1.MergeProfile(this.targetService), this.declareDepends = () => {
      const e = this.node.getName(),
        s = this.targetService.getTargetData().getTargetName(),
        r = [`${e}:${s}@${pre_build_js_1.PreBuild.name}`];
      hsp_target_utils_js_1.HspTargetUtils.calDepHspTargets(this.targetService).forEach((e, s) => r.push(`${s}:${e.getTargetName()}@${merge_profile_js_1.MergeProfile.name}`));
      return har_target_util_js_1.HarTargetUtil.calDepHarTargets(this.targetService).forEach((e, s) => r.push(`${s}:${e}@${Tasks.MERGE_PROFILE.name}`)), hvigor_1.hvigorCore.isCommandEntryTask(task_names_js_1.TaskNames.Task.TEST.name) && r.push(`${e}:${s}@${task_names_js_1.TaskNames.Task.GENERATE_UNIT_TEST_TEMPLATE.name}`), r;
    }, this.declareTaskDetail = () => task_names_js_1.TaskNames.Task.MERGE_PROFILE, this.afterConfigure = () => this.depends = this.declareDepends();
  }
}
export class HarBuildProfileTaskCA extends task_creator_js_1.TargetTaskCreator {
  constructor() {
    super(...arguments), this.declareDepends = () => [...[pre_build_js_1.PreBuild.name], ...getBuildProfileEtsDependencies(this.targetService)], this.declareTaskDetail = () => task_names_js_1.TaskNames.CommonTask.CREATE_HAR_BUILD_PROFILE, this.provider = () => new har_build_profile_task_js_1.HarBuildProfileTask(this.targetService);
  }
}
export class CreateHarModuleInfoCA extends task_creator_js_1.TargetTaskCreator {
  constructor() {
    super(...arguments), this.declareDepends = () => [pre_build_js_1.PreBuild.name], this.declareTaskDetail = () => task_names_js_1.TaskNames.CommonTask.CREATE_HAR_MODULE_INFO, this.provider = () => new create_har_module_info_js_1.CreateHarModuleInfo(this.targetService);
  }
}
export class CreateModuleInfoCA extends task_creator_js_1.TargetTaskCreator {
  constructor() {
    super(...arguments), this.declareDepends = () => [pre_build_js_1.PreBuild.name], this.declareTaskDetail = () => task_names_js_1.TaskNames.CommonTask.CREATE_MODULE_INFO, this.provider = () => new create_module_info_js_1.CreateModuleInfo(this.targetService);
  }
}
export class GenerateLoaderJsonCA extends task_creator_js_1.TargetTaskCreator {
  constructor() {
    super(...arguments), this.provider = () => new generate_loader_json_js_1.GenerateLoaderJson(this.targetService), this.declareDepends = () => {
      const e = [process_router_map_js_1.ProcessRouterMap.name, process_share_config_js_1.ProcessShareConfig.name, process_startup_config_js_1.ProcessStartupConfig.name];
      this.targetService.getModuleService().getModuleModel().isHarModule() ? e.push(create_har_module_info_js_1.CreateHarModuleInfo.name) : (this.targetService.getModuleService().getModuleModel().isHapModule() || this.targetService.getModuleService().getModuleModel().isHspModule()) && e.push(create_module_info_js_1.CreateModuleInfo.name);
      const s = hsp_target_utils_js_1.HspTargetUtils.calDepHspTargets(this.targetService);
      return inject_util_js_1.InjectUtil.isPreviewProcess() ? s.forEach((s, r) => e.push(`${r}:${s.getTargetName()}@${preview_compile_resource_js_1.PreviewCompileResource.name}`)) : s.forEach((s, r) => e.push(`${r}:${s.getTargetName()}@${compile_resource_js_1.CompileResource.name}`)), this.targetService.shouldEnableHarDeduplicate() && (this.targetService.getModuleService().getModuleModel().isHapModule() || this.targetService.getModuleService().getModuleModel().isHspModule()) && s.forEach((s, r) => e.push(`${r}:${s.getTargetName()}@${task_names_js_1.TaskNames.Task.COLLECT_DEDUPLICATE_HAR_ENTRIES.name}ArkTS`)), e;
    }, this.declareTaskDetail = () => task_names_js_1.TaskNames.Task.GENERATE_LOADER_JSON;
  }
}
export class ProcessRouterMapCA extends task_creator_js_1.TargetTaskCreator {
  constructor() {
    super(...arguments), this.provider = () => new process_router_map_js_1.ProcessRouterMap(this.targetService), this.declareDepends = () => [generate_pkg_context_info_js_1.GeneratePkgContextInfo.name], this.declareTaskDetail = () => task_names_js_1.TaskNames.Task.PROCESS_ROUTER_MAP;
  }
}
export class ProcessShareConfigCA extends task_creator_js_1.TargetTaskCreator {
  constructor() {
    super(...arguments), this.provider = () => new process_share_config_js_1.ProcessShareConfig(this.targetService), this.declareDepends = () => [generate_pkg_context_info_js_1.GeneratePkgContextInfo.name], this.declareTaskDetail = () => task_names_js_1.TaskNames.Task.PROCESS_SHARE_CONFIG;
  }
}
export class ProcessStartupConfigCA extends task_creator_js_1.TargetTaskCreator {
  constructor() {
    super(...arguments), this.provider = () => new process_startup_config_js_1.ProcessStartupConfig(this.targetService), this.declareDepends = () => {
      const e = [generate_pkg_context_info_js_1.GeneratePkgContextInfo.name];
      if (this.targetService.shouldEnableHarDeduplicate() && (this.targetService.getModuleService().getModuleModel().isHapModule() || this.targetService.getModuleService().getModuleModel().isHspModule())) {
        hsp_target_utils_js_1.HspTargetUtils.calDepHspTargets(this.targetService).forEach((s, r) => e.push(`${r}:${s.getTargetName()}@${task_names_js_1.TaskNames.Task.PROCESS_STARTUP_CONFIG.name}`));
      }
      return e;
    }, this.declareTaskDetail = () => task_names_js_1.TaskNames.Task.PROCESS_STARTUP_CONFIG;
  }
}
export class PackageHapCA extends task_creator_js_1.TargetTaskCreator {
  constructor() {
    super(...arguments), this.provider = () => new package_hap_js_1.PackageHap(this.targetService), this.declareDepends = () => [process_compiled_resources_js_1.ProcessCompiledResources.name, process_integrated_hsp_js_1.ProcessIntegratedHsp.name, generate_pkg_sdk_info_js_1.GeneratePkgSdkInfo.name], this.declareTaskDetail = () => task_names_js_1.TaskNames.Task.PACKAGE_HAP;
  }
}
export class ProcessCompiledResourcesCA extends task_creator_js_1.TargetTaskCreator {
  constructor() {
    super(...arguments), this.provider = () => new process_compiled_resources_js_1.ProcessCompiledResources(this.targetService), this.declareDepends = () => [generate_pkg_module_json_js_1.GeneratePkgModuleJson.name], this.declareTaskDetail = () => task_names_js_1.TaskNames.Task.PROCESS_COMPILED_RESOURCES;
  }
}
export class ProcessIdDefinedFileCA extends task_creator_js_1.TargetTaskCreator {
  constructor() {
    super(...arguments), this.provider = () => new process_id_defined_file_js_1.ProcessIdDefinedFile(this.targetService), this.declareDepends = () => [], this.declareTaskDetail = () => task_names_js_1.TaskNames.Task.PROCESS_ID_DEFINED_FILE;
  }
}
export class FastPackageCA extends task_creator_js_1.TargetTaskCreator {
  constructor() {
    super(...arguments), this.provider = () => new fast_package_js_1.FastPackageHap(this.targetService), this.declareDepends = () => [process_compiled_resources_js_1.ProcessCompiledResources.name, process_integrated_hsp_js_1.ProcessIntegratedHsp.name, generate_pkg_sdk_info_js_1.GeneratePkgSdkInfo.name], this.declareTaskDetail = () => task_names_js_1.TaskNames.Task.FAST_PACKAGE;
  }
}
export class PackageHqfCA extends task_creator_js_1.TargetTaskCreator {
  constructor() {
    super(...arguments), this.provider = () => new package_hqf_js_1.PackageHqf(this.targetService), this.declareDepends = () => [task_names_js_1.TaskNames.Task.PREPARE_QUICKFIX.name], this.declareTaskDetail = () => task_names_js_1.TaskNames.Task.PACKAGE_HQF;
  }
}
export class SignHqfCA extends task_creator_js_1.TargetTaskCreator {
  constructor() {
    super(...arguments), this.provider = () => new sign_hqf_js_1.SignHqf(this.targetService), this.declareDepends = () => [task_names_js_1.TaskNames.Task.PACKAGE_HQF.name], this.declareTaskDetail = () => task_names_js_1.TaskNames.Task.SIGN_HQF;
  }
}
export class GeneratePkgModuleJsonCA extends task_creator_js_1.TargetTaskCreator {
  constructor() {
    super(...arguments), this.provider = () => new generate_pkg_module_json_js_1.GeneratePkgModuleJson(this.targetService), this.declareDepends = () => (0, base_pack_hap_task_js_1.packageHapDepends)(this.targetService), this.declareTaskDetail = () => task_names_js_1.TaskNames.Task.GENERATE_PKG_MODULE_JSON;
  }
}
export class CollectDebugSymbolCA extends task_creator_js_1.TargetTaskCreator {
  constructor() {
    super(...arguments), this.provider = () => new collect_debug_symbol_js_1.CollectDebugSymbol(this.targetService), this.declareDepends = () => {
      const e = this.targetService.getModuleService().getModuleModel(),
        s = this.isFastBuild && hvigor_1.hvigorCore.isCommandEntryTask(task_names_js_1.TaskNames.CommonHookTask.ASSEMBLE_APP.name);
      switch (true) {
        case e.isHapModule():
          return s ? [fast_package_js_1.FastPackageHap.name] : [package_hap_js_1.PackageHap.name];
        case e.isHspModule():
          return s ? [fast_package_hsp_js_1.FastPackageHsp.name] : [package_hsp_js_1.PackageHsp.name];
        case e.isHarModule():
          return [package_har_js_1.PackageHar.name];
        default:
          return [];
      }
    }, this.declareTaskDetail = () => task_names_js_1.TaskNames.Task.COLLECT_DEBUG_SYMBOL;
  }
}
export class PackageHspCA extends task_creator_js_1.TargetTaskCreator {
  constructor() {
    super(...arguments), this.provider = () => new package_hsp_js_1.PackageHsp(this.targetService), this.declareDepends = () => [generate_pkg_module_json_js_1.GeneratePkgModuleJson.name, process_module_privacy_profile_js_1.ProcessModulePrivacyProfile.name, generate_pkg_sdk_info_js_1.GeneratePkgSdkInfo.name], this.declareTaskDetail = () => task_names_js_1.TaskNames.Task.PACKAGE_HSP;
  }
}
export class FastPackageHspCA extends task_creator_js_1.TargetTaskCreator {
  constructor() {
    super(...arguments), this.provider = () => new fast_package_hsp_js_1.FastPackageHsp(this.targetService), this.declareDepends = () => [generate_pkg_module_json_js_1.GeneratePkgModuleJson.name, generate_pkg_sdk_info_js_1.GeneratePkgSdkInfo.name], this.declareTaskDetail = () => task_names_js_1.TaskNames.Task.FAST_PACKAGE_HSP;
  }
}
export class SignHspCA extends task_creator_js_1.TargetTaskCreator {
  constructor() {
    super(...arguments), this.provider = () => new sign_hsp_js_1.SignHsp(this.targetService), this.declareDepends = () => [package_hsp_js_1.PackageHsp.name], this.declareTaskDetail = () => task_names_js_1.TaskNames.CommonTask.SIGN_HSP;
  }
}
export class PackageSignHarCA extends task_creator_js_1.TargetTaskCreator {
  constructor() {
    super(...arguments), this.provider = () => new package_sign_har_js_1.PackageSignHar(this.targetService), this.declareDepends = () => [package_har_js_1.PackageHar.name], this.declareTaskDetail = () => task_names_js_1.TaskNames.CommonTask.PACKAGE_SIGN_HAR;
  }
}
export class SignProjectRemoteHspCA extends task_creator_js_1.TargetTaskCreator {
  constructor() {
    super(...arguments), this.provider = () => new sign_project_remote_hsp_js_1.SignProjectRemoteHsp(this.targetService), this.declareDepends = () => [process_integrated_hsp_js_1.ProcessIntegratedHsp.name, pre_build_js_1.PreBuild.name], this.declareTaskDetail = () => task_names_js_1.TaskNames.CommonTask.SIGN_PROJECT_REMOTE_HSP;
  }
}
export class SignModuleRemoteHspCA extends task_creator_js_1.TargetTaskCreator {
  constructor() {
    super(...arguments), this.provider = () => new sign_module_remote_hsp_js_1.SignModuleRemoteHsp(this.targetService), this.declareDepends = () => {
      const e = this.targetService.getModuleService().getProjectModel().getRemoteHspPath();
      return fse.existsSync(e) ? [sign_project_remote_hsp_js_1.SignProjectRemoteHsp.name, process_integrated_hsp_js_1.ProcessIntegratedHsp.name] : [process_integrated_hsp_js_1.ProcessIntegratedHsp.name, pre_build_js_1.PreBuild.name];
    }, this.declareTaskDetail = () => task_names_js_1.TaskNames.CommonTask.SIGN_MODULE_REMOTE_HSP;
  }
}
export class ProcessIntegratedHspCA extends task_creator_js_1.TargetTaskCreator {
  constructor() {
    super(...arguments), this.provider = () => new process_integrated_hsp_js_1.ProcessIntegratedHsp(this.targetService), this.declareDepends = () => [pre_build_js_1.PreBuild.name], this.declareTaskDetail = () => task_names_js_1.TaskNames.CommonTask.PROCESS_INTEGRATED_HSP;
  }
}
export class PackageSharedHarCA extends task_creator_js_1.TargetTaskCreator {
  constructor() {
    super(...arguments), this.provider = () => new package_shared_har_js_1.PackageSharedHar(this.targetService), this.declareDepends = () => [task_names_js_1.TaskNames.Task.PREPARE_SHARED_HAR_RES.name], this.declareTaskDetail = () => task_names_js_1.TaskNames.Task.PACKAGE_SHARED_HAR;
  }
}
export class PrepareSharedHarResCA extends task_creator_js_1.TargetTaskCreator {
  constructor() {
    super(...arguments), this.provider = () => new prepare_shared_har_res_js_1.PrepareSharedHarRes(this.targetService), this.declareDepends = () => [this.targetService.getTargetData().getTargetName() === common_const_js_1.DefaultTargetConst.OHOS_TEST_TARGET ? "OhosTestCompileArkTS" : "CompileArkTS", task_names_js_1.TaskNames.Task.DO_NATIVE_STRIP.name], this.declareTaskDetail = () => task_names_js_1.TaskNames.Task.PREPARE_SHARED_HAR_RES;
  }
}
export class PackageSharedTgzCA extends task_creator_js_1.TargetTaskCreator {
  constructor() {
    super(...arguments), this.provider = () => new package_shared_tgz_js_1.PackageSharedTgz(this.targetService), this.declareDepends = () => [sign_hsp_js_1.SignHsp.name, package_shared_har_js_1.PackageSharedHar.name], this.declareTaskDetail = () => task_names_js_1.TaskNames.Task.PACKAGE_SHARED_TGZ;
  }
}
export class PreviewCompileResourceCA extends task_creator_js_1.TargetTaskCreator {
  constructor() {
    super(...arguments), this.provider = () => new preview_compile_resource_js_1.PreviewCompileResource(this.targetService), this.declareDepends = () => [task_names_js_1.TaskNames.Task.PREVIEW_PROCESS_RESOURCE.name], this.declareTaskDetail = () => task_names_js_1.TaskNames.Task.PREVIEW_COMPILE_RESOURCE;
  }
}
export class PreviewUpdateAssetsCA extends task_creator_js_1.TargetTaskCreator {
  constructor() {
    super(...arguments), this.provider = () => new preview_update_assets_js_1.PreviewUpdateAssets(this.targetService), this.declareDepends = () => ["buildPreviewerResource"], this.declareTaskDetail = () => task_names_js_1.TaskNames.Task.PREVIEW_UPDATE_ASSETS, this.afterConfigure = () => {
      this.depends = this.declareDepends().map(e => `${this.node.getName()}:${e}`);
    };
  }
}
export class ReplacePreviewerPageCA extends task_creator_js_1.TargetTaskCreator {
  constructor() {
    super(...arguments), this.provider = () => new replace_previewer_page_js_1.ReplacePreviewerPage(this.targetService), this.declareDepends = () => {
      const e = [preview_hook_compile_resource_js_1.PreviewHookCompileResource.name];
      return "true" === hvigor_1.hvigorCore.getExtraConfig().get(inject_const_js_1.InjectConst.PREVIEWER_COMPILE_RES_INC) && e.push(copy_preview_profile_js_1.CopyPreviewProfile.name), e;
    }, this.declareTaskDetail = () => task_names_js_1.TaskNames.CommonTask.REPLACE_PREVIEWER_PAGES;
  }
}
export class CopyPreviewProfileCA extends task_creator_js_1.TargetTaskCreator {
  constructor() {
    super(...arguments), this.provider = () => new copy_preview_profile_js_1.CopyPreviewProfile(this.targetService), this.declareDepends = () => [preview_hook_compile_resource_js_1.PreviewHookCompileResource.name], this.declareTaskDetail = () => task_names_js_1.TaskNames.CommonTask.COPY_PREVIEWER_PROFILE;
  }
}
export class ReplaceUnitTestIndexFileCA extends task_creator_js_1.TargetTaskCreator {
  constructor() {
    super(...arguments), this.provider = () => new replace_unit_test_index_file_js_1.ReplaceUnitTestIndexFile(this.targetService), this.declareDepends = () => [compile_resource_js_1.CompileResource.name], this.declareTaskDetail = () => task_names_js_1.TaskNames.CommonTask.REPLACE_UNIT_TEST_INDEX_FILE;
  }
}
export class BuildUnitTestHookCA extends task_creator_js_1.TargetTaskCreator {
  constructor() {
    super(...arguments), this.provider = () => new build_unit_test_hook_js_1.BuildUnitTestHook(this.targetService, false), this.declareDepends = () => {
      const e = [generate_loader_json_js_1.GenerateLoaderJson.name, replace_unit_test_index_file_js_1.ReplaceUnitTestIndexFile.name];
      return e.push(pre_check_syscap_js_1.PreCheckSyscap.name), e;
    }, this.declareTaskDetail = () => task_names_js_1.TaskNames.Task.BUILD_UNIT_TEST_RES;
  }
}
export class ReplaceSourceMapInCoverageModeCA extends task_creator_js_1.TargetTaskCreator {
  constructor() {
    super(...arguments), this.provider = () => new replace_sourcemap_in_coverage_mode_js_1.ReplaceSourcemapInCoverageMode(this.targetService), this.declareDepends = () => ["UnitTestArkTS"], this.declareTaskDetail = () => task_names_js_1.TaskNames.Task.REPLACE_SOURCEMAP_IN_COVERAGE_MODE;
  }
}
export class SignHapCA extends task_creator_js_1.TargetTaskCreator {
  constructor() {
    super(...arguments), this.provider = () => new sign_hap_js_1.SignHap(this.targetService), this.declareDepends = () => [this.isFa ? legacy_package_hap_js_1.LegacyPackageHap.name : package_hap_js_1.PackageHap.name], this.declareTaskDetail = () => task_names_js_1.TaskNames.CommonTask.SIGN_HAP;
  }
}
export class PreCheckSyscapCA extends task_creator_js_1.TargetTaskCreator {
  constructor() {
    super(...arguments), this.provider = () => new pre_check_syscap_js_1.PreCheckSyscap(this.targetService, task_names_js_1.TaskNames.CommonTask.PRE_CHECK_SYSCAP), this.declareDepends = () => [pre_build_js_1.PreBuild.name], this.declareTaskDetail = () => task_names_js_1.TaskNames.CommonTask.PRE_CHECK_SYSCAP;
  }
}
export class SyscapTransformCA extends task_creator_js_1.TargetTaskCreator {
  constructor() {
    super(...arguments), this.provider = () => new syscap_transform_js_1.SyscapTransform(this.targetService), this.declareDepends = () => [pre_check_syscap_js_1.PreCheckSyscap.name], this.declareTaskDetail = () => task_names_js_1.TaskNames.Task.SYSCAP_TRANSFORM;
  }
}
export class PackageHarCA extends task_creator_js_1.TargetTaskCreator {
  constructor() {
    super(...arguments), this.declareDepends = () => [task_names_js_1.TaskNames.Task.PROCESS_HAR_ARTIFACTS.name, process_module_privacy_profile_js_1.ProcessModulePrivacyProfile.name], this.declareTaskDetail = () => task_names_js_1.TaskNames.Task.PACKAGE_HAR, this.provider = () => new package_har_js_1.PackageHar(this.targetService);
  }
}
export class ProcessHarArtifactsCA extends task_creator_js_1.TargetTaskCreator {
  constructor() {
    super(...arguments), this.declareDepends = () => {
      const e = [process_obfuscation_files_js_1.ProcessObfuscationFiles.name, task_names_js_1.TaskNames.Task.CACHE_NATIVE_LIBS.name],
        s = this.targetService.getSdkInfo().hasRollUpPluginInEtsLoader;
      return e.push(this.targetService.getModuleService().getModuleModel().isOhpmProject() ? task_names_js_1.TaskNames.Task.PROCESS_OH_PACKAGE_JSON.name : task_names_js_1.TaskNames.Task.PROCESS_PACKAGE_JSON.name), e.push(this.targetService.isOpenSource() || s ? "HarCompileArkTS" : "HarBuildArkTS"), e.push(process_integrated_hsp_js_1.ProcessIntegratedHsp.name), e;
    }, this.declareTaskDetail = () => task_names_js_1.TaskNames.Task.PROCESS_HAR_ARTIFACTS, this.provider = () => new process_har_artifacts_js_1.ProcessHarArtifacts(this.targetService);
  }
}
export class ProcessOhPackageJsonCA extends task_creator_js_1.TargetTaskCreator {
  constructor() {
    super(...arguments), this.declareDepends = () => [pre_build_js_1.PreBuild.name], this.declareTaskDetail = () => task_names_js_1.TaskNames.Task.PROCESS_OH_PACKAGE_JSON, this.provider = () => new process_oh_package_json_js_1.ProcessOhPackageJson(this.targetService);
  }
}
export class ProcessPackageJsonCA extends task_creator_js_1.TargetTaskCreator {
  constructor() {
    super(...arguments), this.declareDepends = () => [pre_build_js_1.PreBuild.name], this.declareTaskDetail = () => task_names_js_1.TaskNames.Task.PROCESS_PACKAGE_JSON, this.provider = () => new process_package_json_js_1.ProcessPackageJson(this.targetService);
  }
}
export class ArkLinterCA extends task_creator_js_1.TargetTaskCreator {
  constructor() {
    super(...arguments), this.declareDepends = () => {
      const e = [process_profile_js_1.ProcessProfile.name];
      return (0, task_util_js_1.limitMinApiVersion)(this.targetService.getTargetData(), sdk_const_js_1.ApiVersion.API_VERSION_9) && e.push(task_names_js_1.TaskNames.CommonTask.CREATE_HAR_BUILD_PROFILE.name), e;
    }, this.declareTaskDetail = () => ({
      name: "LintArkTS"
    }), this.provider = () => new ark_linter_js_1.ArkLinter(this.targetService);
  }
}
export class PackingCheckCA extends task_creator_js_1.TargetTaskCreator {
  constructor() {
    super(...arguments), this.declareDepends = () => {
      const e = this.targetService.getModuleService().getModuleModel(),
        s = this.isFastBuild && hvigor_1.hvigorCore.isCommandEntryTask(task_names_js_1.TaskNames.CommonHookTask.ASSEMBLE_APP.name);
      switch (true) {
        case e.isHapModule():
          return s ? [fast_package_js_1.FastPackageHap.name] : [package_hap_js_1.PackageHap.name];
        case e.isHspModule():
          return s ? [fast_package_hsp_js_1.FastPackageHsp.name] : [package_hsp_js_1.PackageHsp.name];
        default:
          return [];
      }
    }, this.declareTaskDetail = () => task_names_js_1.TaskNames.Task.PACKING_CHECK, this.provider = () => new packing_check_js_1.PackingCheck(this.targetService);
  }
}
export class PackingCheckHarCA extends task_creator_js_1.TargetTaskCreator {
  constructor() {
    super(...arguments), this.declareDepends = () => [package_har_js_1.PackageHar.name], this.declareTaskDetail = () => task_names_js_1.TaskNames.Task.PACKING_CHECK_HAR, this.provider = () => new packing_check_har_js_1.PackingCheckHar(this.targetService);
  }
}
export class ProcessObfuscationFilesCA extends task_creator_js_1.TargetTaskCreator {
  constructor() {
    super(...arguments), this.declareDepends = () => [compile_resource_js_1.CompileResource.name], this.declareTaskDetail = () => ({
      name: process_obfuscation_files_js_1.ProcessObfuscationFiles.name
    }), this.provider = () => new process_obfuscation_files_js_1.ProcessObfuscationFiles(this.targetService);
  }
}
export class GenerateMetadataCA extends task_creator_js_1.TargetTaskCreator {
  constructor() {
    super(...arguments), this.declareDepends = () => {
      const e = [pre_build_js_1.PreBuild.name];
      hsp_target_utils_js_1.HspTargetUtils.calDepHspTargets(this.targetService).forEach((s, r) => e.push(`${r}:${s.getTargetName()}@${task_names_js_1.TaskNames.Task.GENERATE_METADATA.name}`));
      const s = this.targetService.getModuleService().getProjectModel().getRemoteHspPath(),
        r = this.targetService.getTargetData().getModuleModel().getRemoteHspPath(),
        a = this.node.getName(),
        t = global_project_data_service_js_1.GlobalProjectDataService.getInstance().getModuleRemoteHspPromise().has(a);
      return (fse.existsSync(s) || fse.existsSync(r)) && (t && !inject_util_js_1.InjectUtil.isHarDeviceTestWithOhosTest(this.targetService) || (global_project_data_service_js_1.GlobalProjectDataService.getInstance().setModuleRemoteHspPromise(a), e.push(sign_module_remote_hsp_js_1.SignModuleRemoteHsp.name))), e;
    }, this.declareTaskDetail = () => task_names_js_1.TaskNames.Task.GENERATE_METADATA, this.provider = () => new generate_metadata_js_1.GenerateMetadata(this.targetService);
  }
}
export class MakePackInfoCA extends task_creator_js_1.TargetTaskCreator {
  constructor() {
    super(...arguments), this.declareDepends = () => [merge_profile_js_1.MergeProfile.name], this.declareTaskDetail = () => task_names_js_1.TaskNames.Task.MAKE_PACK_INFO, this.provider = () => new make_pack_info_js_1.MakePackInfo(this.targetService);
  }
}
export class UnitTestProcessProfileCA extends task_creator_js_1.TargetTaskCreator {
  constructor() {
    super(...arguments), this.declareDepends = () => [process_profile_js_1.ProcessProfile.name], this.declareTaskDetail = () => task_names_js_1.TaskNames.CommonTask.UNIT_TEST_PROCESS_PROFILE, this.provider = () => new unit_test_process_profile_js_1.UnitTestProcessProfile(this.targetService);
  }
}
export class BuildProfileTaskCA extends task_creator_js_1.TargetTaskCreator {
  constructor() {
    super(...arguments), this.declareDepends = () => [...[pre_build_js_1.PreBuild.name], ...getBuildProfileEtsDependencies(this.targetService)], this.declareTaskDetail = () => task_names_js_1.TaskNames.CommonTask.CREATE_BUILD_PROFILE, this.provider = () => new build_profile_task_js_1.BuildProfileTask(this.targetService);
  }
}
export function getBuildProfileEtsDependencies(e) {
  const s = [];
  hsp_target_utils_js_1.HspTargetUtils.calDepHspTargets(e).forEach((e, r) => s.push(`${r}:${e.getTargetName()}@${CommonTask.CREATE_BUILD_PROFILE.name}`));
  return har_target_util_js_1.HarTargetUtil.calDepHarTargets(e).forEach((e, r) => s.push(`${r}:${e}@${CommonTask.CREATE_HAR_BUILD_PROFILE.name}`)), s;
}
export class ProcessModulePrivacyProfileCA extends task_creator_js_1.TargetTaskCreator {
  constructor() {
    super(...arguments), this.declareDepends = () => [pre_build_js_1.PreBuild.name], this.declareTaskDetail = () => task_names_js_1.TaskNames.Task.PROCESS_MODULE_PRIVACY_PROFILE, this.provider = () => new process_module_privacy_profile_js_1.ProcessModulePrivacyProfile(this.targetService);
  }
}
