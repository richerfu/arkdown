/*
 * Human-readable TypeScript reconstruction.
 * Generated from hvigor/ts by hvigor/tools/reconstruct-readable-ts.cjs.
 * Keep semantic edits in Rust ports or separate hand-refined notes.
 * Source baseline: hvigor-ohos-plugin/src/tasks/manager/global-task-creation-manager.js
 */
import * as hvigor_1 from "@ohos/hvigor";
import * as find_target_product_js_1 from "../../common/find-target-product";
import * as common_const_js_1 from "../../const/common-const";
import * as inject_util_js_1 from "../../utils/inject-util";
import * as task_util_js_1 from "../../utils/task-util";
import * as build_info_js_1 from "../build-info/build-info";
import * as build_native_with_cmake_js_1 from "../build-native-with-cmake";
import * as collect_debug_symbol_js_1 from "../collect-debug-symbol";
import * as clean_js_1 from "../common/clean";
import * as task_names_js_1 from "../common/task-names";
import * as compile_resource_js_1 from "../compile-resource";
import * as build_hot_reload_resource_js_1 from "../debugger/build-hot-reload-resource";
import * as hot_reload_build_js_1 from "../debugger/hot-reload-build";
import * as device_test_hook_js_1 from "../device-test-hook";
import * as fast_package_js_1 from "../fast-package";
import * as generate_loader_json_js_1 from "../generate-loader-json";
import * as generated_build_profile_task_js_1 from "../generated-build-profile-task";
import * as assemble_hap_js_1 from "../hook/assemble/assemble-hap";
import * as assemble_har_js_1 from "../hook/assemble/assemble-har";
import * as assemble_hqf_js_1 from "../hook/assemble/assemble-hqf";
import * as assemble_hsp_js_1 from "../hook/assemble/assemble-hsp";
import * as gen_on_device_test_hap_js_1 from "../hook/gen-on-device-test-hap";
import * as compile_native_hook_js_1 from "../hook/native/compile-native-hook";
import * as preview_hook_compile_resource_js_1 from "../hook/preview-hook-compile-resource";
import * as build_har_previewer_res_js_1 from "../hook/previewer/build-har-previewer-res";
import * as build_previewer_res_js_1 from "../hook/previewer/build-previewer-res";
import * as preview_build_js_1 from "../hook/previewer/preview-build";
import * as fast_package_hsp_js_1 from "../hsp/fast-package-hsp";
import * as package_shared_tgz_js_1 from "../hsp/package-shared-tgz";
import * as legacy_hook_compile_resource_js_1 from "../legacy-tasks/hook/legacy-hook-compile-resource";
import * as legacy_generate_har_js_manifest_js_1 from "../legacy-tasks/legacy-generate-har-js-manifest";
import * as legacy_generate_har_loader_json_js_1 from "../legacy-tasks/legacy-generate-har-loader-json";
import * as legacy_generate_js_manifest_js_1 from "../legacy-tasks/legacy-generate-js-manifest";
import * as legacy_generate_loader_json_js_1 from "../legacy-tasks/legacy-generate-loader-json";
import * as legacy_package_hap_js_1 from "../legacy-tasks/legacy-package-hap";
import * as package_hap_js_1 from "../package-hap";
import * as packing_check_har_js_1 from "../packing-check/packing-check-har";
import * as packing_check_js_1 from "../packing-check/packing-check";
import * as pre_check_syscap_js_1 from "../pre-check-syscap";
import * as preview_compile_resource_js_1 from "../preview-compile-resource";
import * as replace_previewer_page_js_1 from "../previewer/replace-previewer-page";
import * as sign_hap_js_1 from "../sign-hap";
import * as task_creator_js_1 from "../task-creator";
import * as test_js_1 from "../unitTest/test";
import * as unit_test_build_js_1 from "../unitTest/unit-test-build";
export class AssembleHqfCA extends task_creator_js_1.GlobalTaskCreator {
  constructor(e, s, a) {
    assemble_hqf_js_1.AssembleHqf.isAdd = true, assemble_hqf_js_1.AssembleHqf.versionCode = undefined, super(e, s, a), this.declareDepends = () => [task_names_js_1.TaskNames.Task.SIGN_HQF.name], this.declareTaskDetail = () => task_names_js_1.TaskNames.CommonHookTask.ASSEMBLE_HQF, this.provider = () => new assemble_hqf_js_1.AssembleHqf(this.service);
  }
}
export class AssembleHapCA extends task_creator_js_1.GlobalTaskCreator {
  constructor() {
    super(...arguments), this.declareDepends = () => {
      var e, s;
      const a = [collect_debug_symbol_js_1.CollectDebugSymbol.name, packing_check_js_1.PackingCheck.name];
      return (0, task_util_js_1.isFastBuildApp)(this.service) ? (a.push(fast_package_js_1.FastPackageHap.name), a) : (null === (s = null === (e = (0, find_target_product_js_1.findTargetProduct)(this.service.getProjectModel()).buildOption) || undefined === e ? undefined : e.packOptions) || undefined === s ? undefined : s.buildAppSkipSignHap) && hvigor_1.hvigorCore.isCommandEntryTask(task_names_js_1.TaskNames.CommonHookTask.ASSEMBLE_APP.name) ? (this.isFa ? a.push(legacy_package_hap_js_1.LegacyPackageHap.name) : a.push(package_hap_js_1.PackageHap.name), a) : (a.push(sign_hap_js_1.SignHap.name), a);
    }, this.declareTaskDetail = () => task_names_js_1.TaskNames.CommonHookTask.ASSEMBLE_HAP, this.provider = () => new assemble_hap_js_1.AssembleHap(this.service, this.isFa);
  }
}
export class GenOnDeviceTestHapCA extends task_creator_js_1.GlobalTaskCreator {
  constructor() {
    super(...arguments), this.declareDepends = () => {
      var e, s;
      return (null === (s = null === (e = (0, find_target_product_js_1.findTargetProduct)(this.service.getProjectModel()).buildOption) || undefined === e ? undefined : e.packOptions) || undefined === s ? undefined : s.buildAppSkipSignHap) && hvigor_1.hvigorCore.isCommandEntryTask(task_names_js_1.TaskNames.CommonHookTask.ASSEMBLE_APP.name) ? this.isFa ? [legacy_package_hap_js_1.LegacyPackageHap.name] : [package_hap_js_1.PackageHap.name] : [sign_hap_js_1.SignHap.name];
    }, this.declareTaskDetail = () => task_names_js_1.TaskNames.CommonHookTask.GEN_ON_DEVICE_TEST_HAP, this.provider = () => new gen_on_device_test_hap_js_1.GenOnDeviceTestHap(this.service, this.isFa);
  }
}
export class AssembleHspCA extends task_creator_js_1.GlobalTaskCreator {
  constructor() {
    super(...arguments), this.declareDepends = () => (0, task_util_js_1.isFastBuildApp)(this.service) ? [fast_package_hsp_js_1.FastPackageHsp.name, collect_debug_symbol_js_1.CollectDebugSymbol.name, packing_check_js_1.PackingCheck.name] : [package_shared_tgz_js_1.PackageSharedTgz.name, collect_debug_symbol_js_1.CollectDebugSymbol.name, packing_check_js_1.PackingCheck.name], this.declareTaskDetail = () => task_names_js_1.TaskNames.CommonHookTask.ASSEMBLE_HSP, this.provider = () => new assemble_hsp_js_1.AssembleHsp(this.service, this.isFa);
  }
}
export class PreviewHookCompileResourceCA extends task_creator_js_1.TargetTaskCreator {
  constructor() {
    super(...arguments), this.provider = () => new preview_hook_compile_resource_js_1.PreviewHookCompileResource(this.targetService), this.declareDepends = () => [this.targetService.getSdkInfo().isPreviewCompileResourceIncrement(this.targetService.getTargetData().isHarmonyOS()) ? preview_compile_resource_js_1.PreviewCompileResource.name : compile_resource_js_1.CompileResource.name], this.declareTaskDetail = () => task_names_js_1.TaskNames.CommonTask.PREVIEW_HOOK_COMPILE_RESOURCE;
  }
}
export class AssembleHarCA extends task_creator_js_1.GlobalTaskCreator {
  constructor() {
    super(...arguments), this.declareDepends = () => {
      const e = [this.isFa ? task_names_js_1.TaskNames.LegacyFATask.PACKAGE_HAR.name : task_names_js_1.TaskNames.CommonTask.PACKAGE_SIGN_HAR.name];
      return e.push(packing_check_har_js_1.PackingCheckHar.name), e.push(collect_debug_symbol_js_1.CollectDebugSymbol.name), e;
    }, this.declareTaskDetail = () => task_names_js_1.TaskNames.CommonHookTask.ASSEMBLE_HAR, this.provider = () => new assemble_har_js_1.AssembleHar(this.service, this.isFa);
  }
}
export class LibraryBuildPreviewerResCA extends task_creator_js_1.GlobalTaskCreator {
  constructor() {
    super(...arguments), this.declareTaskDetail = () => task_names_js_1.TaskNames.CommonHookTask.BUILD_PREVIEWER_RES, this.provider = () => new build_har_previewer_res_js_1.BuildHarPreviewerRes(this.service, this.isFa);
  }
  declareDepends() {
    const e = [];
    return this.isFa ? e.push(legacy_generate_har_loader_json_js_1.LegacyGenerateHarLoaderJson.name, legacy_generate_har_js_manifest_js_1.LegacyGenerateHarJsManifest.name) : e.push(generate_loader_json_js_1.GenerateLoaderJson.name, preview_hook_compile_resource_js_1.PreviewHookCompileResource.name), e.push(pre_check_syscap_js_1.PreCheckSyscap.name), e;
  }
}
export class BuildPreviewerResCA extends task_creator_js_1.GlobalTaskCreator {
  constructor() {
    super(...arguments), this.declareTaskDetail = () => task_names_js_1.TaskNames.CommonHookTask.BUILD_PREVIEWER_RES, this.provider = () => new build_previewer_res_js_1.BuildPreviewerRes(this.service, this.isFa);
  }
  declareDepends() {
    const e = [];
    return this.isFa ? e.push(legacy_generate_loader_json_js_1.LegacyGenerateLoaderJson.name, legacy_generate_js_manifest_js_1.LegacyGenerateJsManifest.name, legacy_hook_compile_resource_js_1.LegacyHookCompileResource.name) : e.push(generate_loader_json_js_1.GenerateLoaderJson.name, replace_previewer_page_js_1.ReplacePreviewerPage.name), e.push(pre_check_syscap_js_1.PreCheckSyscap.name), e;
  }
}
export class HotReloadBuildCA extends task_creator_js_1.GlobalTaskCreator {
  constructor() {
    super(...arguments), this.declareDepends = () => ["HotReloadArkTS"], this.declareTaskDetail = () => task_names_js_1.TaskNames.Task.HOT_RELOAD_BUILD, this.provider = () => new hot_reload_build_js_1.HotReloadBuild(this.service);
  }
}
export class GeneratedBuildProfileCA extends task_creator_js_1.GlobalTaskCreator {
  constructor(e, s, a, r) {
    super(e, s, a), this.isHar = false, this.declareDepends = () => [this.isHar ? task_names_js_1.TaskNames.CommonTask.CREATE_HAR_BUILD_PROFILE.name : task_names_js_1.TaskNames.CommonTask.CREATE_BUILD_PROFILE.name], this.declareTaskDetail = () => task_names_js_1.TaskNames.CommonHookTask.GENERATE_BUILD_PROFILE, this.provider = () => new generated_build_profile_task_js_1.GeneratedBuildProfileTask(this.service, this.isHar), this.isHar = undefined !== r && r;
  }
}
export class PreviewBuildCA extends task_creator_js_1.GlobalTaskCreator {
  constructor() {
    super(...arguments), this.declareDepends = () => [this.isFa ? "LegacyPreviewCompileArkTS" : "PreviewArkTS"], this.declareTaskDetail = () => task_names_js_1.TaskNames.Task.PREVIEW_BUILD, this.provider = () => new preview_build_js_1.PreviewBuild(this.service, this.isFa);
  }
}
export class CompileNativeHookCA extends task_creator_js_1.GlobalTaskCreator {
  constructor() {
    super(...arguments), this.declareDepends = () => [build_native_with_cmake_js_1.BuildNativeWithCmake.name], this.declareTaskDetail = () => task_names_js_1.TaskNames.CommonHookTask.COMPILE_NATIVE, this.provider = () => new compile_native_hook_js_1.CompileNativeHook(this.service);
  }
}
export class BuildInfoCA extends task_creator_js_1.GlobalTaskCreator {
  constructor() {
    super(...arguments), this.declareDepends = () => [], this.declareTaskDetail = () => task_names_js_1.TaskNames.Task.BUILD_INFO, this.provider = () => new build_info_js_1.BuildInfo(this.service.getNode());
  }
}
export class CleanCA extends task_creator_js_1.GlobalTaskCreator {
  constructor() {
    super(...arguments), this.declareDepends = () => [], this.declareTaskDetail = () => task_names_js_1.TaskNames.CommonTask.CLEAN, this.provider = () => new clean_js_1.Clean(this.service.getNode(), this.service);
  }
}
export class UnitTestBuildCA extends task_creator_js_1.GlobalTaskCreator {
  constructor() {
    super(...arguments), this.declareDepends = () => this.isFa ? [common_const_js_1.ArktSTaskConst.LEGACY_UNIT_TEST_ARKTS] : inject_util_js_1.InjectUtil.isOhosTestCoverage() ? [task_names_js_1.TaskNames.Task.REPLACE_SOURCEMAP_IN_COVERAGE_MODE.name] : [common_const_js_1.ArktSTaskConst.UNIT_TEST_ARKTS, task_names_js_1.TaskNames.Task.UNIT_TEST_COPY_MOCK_CONFIG_JSON.name], this.declareTaskDetail = () => task_names_js_1.TaskNames.Task.UNIT_TEST_BUILD, this.provider = () => new unit_test_build_js_1.UnitTestBuild(this.service, this.isFa);
  }
}
export class TestCA extends task_creator_js_1.GlobalTaskCreator {
  constructor() {
    super(...arguments), this.declareDepends = () => [task_names_js_1.TaskNames.Task.GENERATE_UNIT_TEST_RESULT.name], this.declareTaskDetail = () => task_names_js_1.TaskNames.Task.TEST, this.provider = () => new test_js_1.Test(this.service, this.isFa);
  }
}
export class DeviceTestHookCA extends task_creator_js_1.GlobalTaskCreator {
  constructor() {
    super(...arguments), this.declareDepends = () => [task_names_js_1.TaskNames.Task.GENERATE_DEVICE_COVERAGE.name], this.declareTaskDetail = () => task_names_js_1.TaskNames.Task.ON_DEVICE_TEST, this.provider = () => new device_test_hook_js_1.DeviceTestHook(this.service, this.isFa);
  }
}
export class BuildHotReloadResourceCA extends task_creator_js_1.GlobalTaskCreator {
  constructor() {
    super(...arguments), this.declareDepends = () => [generate_loader_json_js_1.GenerateLoaderJson.name, compile_resource_js_1.CompileResource.name], this.declareTaskDetail = () => task_names_js_1.TaskNames.CommonHookTask.BUILD_HOT_RELOAD_RES, this.provider = () => new build_hot_reload_resource_js_1.BuildHotReloadResource(this.service);
  }
}
