/*
 * Human-readable TypeScript reconstruction.
 * Generated from hvigor/ts by hvigor/tools/reconstruct-readable-ts.cjs.
 * Keep semantic edits in Rust ports or separate hand-refined notes.
 * Source baseline: hvigor-ohos-plugin/src/tasks/legacy-tasks/create/legacy-task-initializer.js
 */
import * as hvigor_1 from "@ohos/hvigor";
import * as code_type_enum_1 from "../../../enum/code-type-enum";
import * as build_native_with_cmake_1 from "../../build-native-with-cmake";
import * as build_native_with_ninja_1 from "../../build-native-with-ninja";
import * as cache_native_libs_1 from "../../cache-native-libs";
import * as task_names_1 from "../../common/task-names";
import * as configure_cmake_1 from "../../configure-cmake";
import * as do_native_binxo_1 from "../../do-native-binxo";
import * as do_native_strip_1 from "../../do-native-strip";
import * as generate_pkg_module_json_1 from "../../generate-pkg-module-json";
import * as ark_linter_1 from "../../har/ark-linter";
import * as assemble_hap_1 from "../../hook/assemble/assemble-hap";
import * as assemble_har_1 from "../../hook/assemble/assemble-har";
import * as compile_native_hook_1 from "../../hook/native/compile-native-hook";
import * as build_har_previewer_res_1 from "../../hook/previewer/build-har-previewer-res";
import * as build_previewer_res_1 from "../../hook/previewer/build-previewer-res";
import * as preview_build_1 from "../../hook/previewer/preview-build";
import * as legacy_package_har_1 from "../har/legacy-package-har";
import * as legacy_process_har_artifacts_1 from "../har/legacy-process-har-artifacts";
import * as legacy_process_oh_package_json_1 from "../har/legacy-process-oh-package-json";
import * as legacy_hook_compile_resource_1 from "../hook/legacy-hook-compile-resource";
import * as legacy_ark_compile_1 from "../legacy-ark-compile";
import * as legacy_compile_lite_node_1 from "../legacy-compile-lite-node";
import * as legacy_compile_node_1 from "../legacy-compile-node";
import * as legacy_gen_lite_source_1 from "../legacy-gen-lite-source";
import * as legacy_generate_har_js_manifest_1 from "../legacy-generate-har-js-manifest";
import * as legacy_generate_har_loader_json_1 from "../legacy-generate-har-loader-json";
import * as legacy_generate_js_manifest_1 from "../legacy-generate-js-manifest";
import * as legacy_generate_loader_json_1 from "../legacy-generate-loader-json";
import * as legacy_generate_metadata_1 from "../legacy-generate-metadata";
import * as legacy_make_pack_info_1 from "../legacy-make-pack-info";
import * as legacy_merge_profile_1 from "../legacy-merge-profile";
import * as legacy_ohos_test_ark_compile_1 from "../legacy-ohos-test-ark-compile";
import * as legacy_ohos_test_compile_node_1 from "../legacy-ohos-test-compile-node";
import * as legacy_package_hap_1 from "../legacy-package-hap";
import * as legacy_pre_build_1 from "../legacy-pre-build";
import * as legacy_previewer_ark_compile_1 from "../legacy-previewer-ark-compile";
import * as legacy_process_profile_1 from "../legacy-process-profile";
import * as legacy_sign_lite_bin_1 from "../legacy-sign-lite-bin";
import * as legacy_syscap_transform_1 from "../legacy-syscap-transform";
import * as merge_node_assets_1 from "../merge-node-assets";
import * as pre_check_syscap_1 from "../../pre-check-syscap";
import * as process_libs_1 from "../../process-libs";
import * as sign_hap_1 from "../../sign-hap";
import * as syscap_transform_1 from "../../syscap-transform";
import * as build_unit_test_hook_1 from "../../unitTest/build-unit-test-hook";
import * as legacy_unit_test_ark_compile_1 from "../unit-test/legacy-unit-test-ark-compile";
import * as unit_test_build_1 from "../../unitTest/unit-test-build";
import * as validate_sign_1 from "../../validate-sign";
import * as ohos_logger_1 from "../../../utils/log/ohos-logger";
import * as task_util_1 from "../../../utils/task-util";
const log = ohos_logger_1.OhosLogger.getLogger("TaskInitialization"),
  legacyHapTasksInitialize = e => {
    var r;
    const a = process.hrtime(),
      i = null === (r = e.getModuleModel()) || undefined === r ? undefined : r.getModule();
    if (!i) return void log.printErrorExit("INVALID_MODULE_MODEL");
    const _ = e.getTaskService();
    if (!_) return void log.printErrorExit("INVALID_MODULE_MODEL");
    const s = [];
    s.push(new assemble_hap_1.AssembleHap(_, true)), s.push(new build_previewer_res_1.BuildPreviewerRes(_, true)), s.push(...(0, legacyCommonHooksInitialize)(_)), initializeTargetsTasks(i, e.getNeedExecTargetServiceList(), s), log.debug(`Module ${i.getName()} task initialization takes ${(0, hvigor_1.formatTime)(process.hrtime(a))}`);
  };
export const legacyHarTasksInitialize = e => {
  var r;
  const a = process.hrtime(),
    i = e.getTaskService();
  if (!i) return void log.printErrorExit("INVALID_MODULE_MODEL");
  const _ = null === (r = e.getModuleModel()) || undefined === r ? undefined : r.getModule();
  if (!_) return void log.printErrorExit("INVALID_MODULE_MODEL");
  const s = [];
  s.push(new assemble_har_1.AssembleHar(i, true)), s.push(new build_har_previewer_res_1.BuildHarPreviewerRes(i, true)), s.push(...(0, legacyCommonHooksInitialize)(i)), initializeTargetsTasks(_, e.getNeedExecTargetServiceList(), s), log.debug(`Module ${_.getName()} task initialization takes ${(0, hvigor_1.formatTime)(process.hrtime(a))}`);
};
export const legacyCommonHooksInitialize = e => {
  const r = [];
  return r.push(new compile_native_hook_1.CompileNativeHook(e)), r.push(new preview_build_1.PreviewBuild(e, true)), r.push(new unit_test_build_1.UnitTestBuild(e, true)), r;
};
const initializeTargetsTasks = (e, r, a) => {
    r.forEach(r => {
      a.forEach(a => e.registry(a) && a.initTaskDepends(r)), (0, legacyTasksInitialize)(e, r);
    });
  },
  legacyTasksInitialize = (e, r) => {
    e.registry(new legacy_pre_build_1.LegacyPreBuild(r)), e.registry(new legacy_merge_profile_1.LegacyMergeProfile(r)), e.registry(new legacy_generate_js_manifest_1.LegacyGenerateJsManifest(r)), e.registry(new legacy_generate_loader_json_1.LegacyGenerateLoaderJson(r)), e.registry(new legacy_make_pack_info_1.LegacyMakePackInfo(r)), e.registry(new legacy_process_oh_package_json_1.LegacyProcessOHPackageJson(r)), e.registry(new legacy_process_profile_1.LegacyProcessProfile(r)), (0, task_util_1.dynamicResolveResourceTasks)(r, e), e.registry(new legacy_hook_compile_resource_1.LegacyHookCompileResource(r)), e.registry(new legacy_generate_har_loader_json_1.LegacyGenerateHarLoaderJson(r)), e.registry(new legacy_generate_har_js_manifest_1.LegacyGenerateHarJsManifest(r)), e.registry(new legacy_package_hap_1.LegacyPackageHap(r)), e.registry(new legacy_process_har_artifacts_1.LegacyProcessHarArtifacts(r)), e.registry(new legacy_package_har_1.LegacyPackageHar(r)), e.registry(new legacy_gen_lite_source_1.LegacyGenLiteSource(r)), e.registry(new merge_node_assets_1.MergeNodeAssets(r)), e.registry(new legacy_compile_lite_node_1.LegacyCompileLiteNode(r)), e.registry(new build_unit_test_hook_1.BuildUnitTestHook(r, true)), e.registry(new legacy_generate_metadata_1.LegacyGenerateMetadata(r)), e.registry(new legacy_syscap_transform_1.LegacySyscapTransform(r)), e.registry(new legacy_sign_lite_bin_1.LegacySignLiteBin(r)), e.registry(new validate_sign_1.ValidateSigning(r)), e.registry(new legacy_ark_compile_1.LegacyArkCompile(r, code_type_enum_1.CodeType.ETS, task_names_1.TaskNames.LegacyFATask.COMPILE_ARK)), e.registry(new legacy_compile_node_1.LegacyCompileNode(r, code_type_enum_1.CodeType.ETS, task_names_1.TaskNames.LegacyFATask.COMPILE_NODE)), e.registry(new legacy_ark_compile_1.LegacyArkCompile(r, code_type_enum_1.CodeType.JS, task_names_1.TaskNames.LegacyFATask.COMPILE_ARK)), e.registry(new legacy_compile_node_1.LegacyCompileNode(r, code_type_enum_1.CodeType.JS, task_names_1.TaskNames.LegacyFATask.COMPILE_NODE)), e.registry(new legacy_ohos_test_ark_compile_1.LegacyOhosTestArkCompile(r, code_type_enum_1.CodeType.ETS)), e.registry(new legacy_ohos_test_ark_compile_1.LegacyOhosTestArkCompile(r, code_type_enum_1.CodeType.JS)), e.registry(new legacy_ohos_test_compile_node_1.LegacyOhosTestCompileNode(r, code_type_enum_1.CodeType.ETS)), e.registry(new legacy_ohos_test_compile_node_1.LegacyOhosTestCompileNode(r, code_type_enum_1.CodeType.JS)), e.registry(new legacy_previewer_ark_compile_1.LegacyPreviewerArkCompile(r, code_type_enum_1.CodeType.ETS, {
      name: "LegacyPreviewCompile"
    })), e.registry(new legacy_previewer_ark_compile_1.LegacyPreviewerArkCompile(r, code_type_enum_1.CodeType.JS, {
      name: "LegacyPreviewCompile"
    })), e.registry(new legacy_unit_test_ark_compile_1.LegacyUnitTestArkCompile(r, code_type_enum_1.CodeType.ETS, {
      name: "LegacyUnitTest"
    })), e.registry(new legacy_unit_test_ark_compile_1.LegacyUnitTestArkCompile(r, code_type_enum_1.CodeType.JS, {
      name: "LegacyUnitTest"
    })), e.registry(new build_native_with_cmake_1.BuildNativeWithCmake(r)), e.registry(new build_native_with_ninja_1.BuildNativeWithNinja(r)), e.registry(new configure_cmake_1.ConfigureCmake(r)), e.registry(new process_libs_1.ProcessLibs(r)), e.registry(new do_native_binxo_1.DoNativeBinxo(r)), e.registry(new do_native_strip_1.DoNativeStrip(r)), e.registry(new cache_native_libs_1.CacheNativeLibs(r)), e.registry(new sign_hap_1.SignHap(r)), e.registry(new pre_check_syscap_1.PreCheckSyscap(r, task_names_1.TaskNames.CommonTask.PRE_CHECK_SYSCAP)), e.registry(new syscap_transform_1.SyscapTransform(r)), e.registry(new ark_linter_1.ArkLinter(r)), e.registry(new generate_pkg_module_json_1.GeneratePkgModuleJson(r));
  };
export { legacyHapTasksInitialize, legacyTasksInitialize };
