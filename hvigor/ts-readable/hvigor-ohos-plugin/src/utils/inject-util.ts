/*
 * Human-readable TypeScript reconstruction.
 * Generated from hvigor/ts by hvigor/tools/reconstruct-readable-ts.cjs.
 * Keep semantic edits in Rust ports or separate hand-refined notes.
 * Source baseline: hvigor-ohos-plugin/src/utils/inject-util.js
 */
import path_1 from "path";
import * as hvigor_1 from "@ohos/hvigor";
import fs_extra_1 from "fs-extra";
import * as common_const_js_1 from "../const/common-const";
import * as task_names_js_1 from "../tasks/common/task-names";
import * as ohos_logger_js_1 from "./log/ohos-logger";
export class InjectUtil {
  static isHotReload() {
    return "true" === hvigor_1.hvigorCore.getExtraConfig().get(common_const_js_1.CommonConst.HOT_RELOAD) && this.startParams.daemon;
  }
  static isColdReload() {
    return hvigor_1.hvigorCore.isCommandEntryTask("assembleDevHqf");
  }
  static isDeviceTest() {
    return hvigor_1.hvigorCore.isCommandEntryTask(common_const_js_1.CommonConst.ON_DEVICE_TEST);
  }
  static isAssembleDevHqf() {
    return hvigor_1.hvigorCore.isCommandEntryTask("assembleDevHqf");
  }
  static isDaemon() {
    return this.startParams.daemon;
  }
  static isPreviewProcess() {
    return "true" === hvigor_1.hvigorCore.getExtraConfig().get(common_const_js_1.CommonConst.PREVIEW_MODE);
  }
  static isUnitTestProcess() {
    return "true" === hvigor_1.hvigorCore.getExtraConfig().get(common_const_js_1.CommonConst.UNIT_TEST_MODE);
  }
  static isOhosTestCoverage() {
    return "true" === hvigor_1.hvigorCore.getExtraConfig().get(common_const_js_1.CommonConst.OHOS_TEST_COVERAGE);
  }
  static getCoveragePathFilter() {
    return hvigor_1.hvigorCore.getExtraConfig().get(common_const_js_1.CommonConst.COVERAGE_PATH_FILTER);
  }
  static getCoverageMode() {
    return hvigor_1.hvigorCore.getExtraConfig().get(common_const_js_1.CommonConst.COVERAGE_MODE);
  }
  static isDebugLineEnable() {
    return "true" === hvigor_1.hvigorCore.getExtraConfig().get(common_const_js_1.CommonConst.DEBUG_LINE);
  }
  static getBuildCacheParentDir(t, e, o = true) {
    let r = InjectUtil.getBuildDir(o);
    if (!r) return t;
    if (!path_1.isAbsolute(r)) return this._log && !this.customBuildDirHasLogged && (this._log.warn(`Invalid build cache dir: ${r}`), this.customBuildDirHasLogged = true), t;
    if (fs_extra_1.existsSync(r)) {
      const t = fs_extra_1.realpathSync.native(r);
      path_1.normalize(r) === path_1.normalize(t) || this.customBuildDirHasLogged || (this._log.warn(`The custom build cache path '${r}' has a case mismatch with the actual directory. It is recommended to use the exact same letter case as the existing path.`), this.customBuildDirHasLogged = true), r = path_1.normalize(t);
    }
    return path_1.join(r, e.replace(":", path_1.sep));
  }
  static getModuleBuildRootPath(t) {
    return InjectUtil.getBuildCacheParentDir(t, path_1.join(hvigor_1.hvigorCore.getProject().getName(), t.substring(hvigor_1.hvigorCore.getProject().getNodeDir().length)));
  }
  static getBuildDir(t = true) {
    let e = hvigor_1.hvigorCore.getExtraConfig().get(common_const_js_1.CommonConst.BUILD_CACHE_DIR);
    if ((!e || t) && (e || (e = hvigor_1.HvigorConfigLoader.getInstance().getPropertiesConfigValue(common_const_js_1.CommonConst.BUILD_DIR_KEY), e)) && !this.isPreviewProcess() && !this.isInTest()) return e.replace(/^'|'$/g, "").replace(/^"|"$/g, "");
  }
  static isLocalTest() {
    return "true" === hvigor_1.hvigorCore.getExtraConfig().get(common_const_js_1.CommonConst.IS_LOCAL_TEST);
  }
  static isOhosTest() {
    return "true" === hvigor_1.hvigorCore.getExtraConfig().get(common_const_js_1.CommonConst.IS_OHOS_TEST);
  }
  static isInTest() {
    return this.isLocalTest() || this.isOhosTest();
  }
  static isByteCodeHarOptimize() {
    return true === hvigor_1.hvigorCore.getParameter().getPropertyInternal(common_const_js_1.HvigorConfigConst.OHOS_BYTE_CODE_HAR_OPTIMIZE);
  }
  static isIntegratedHspUseGetCurrentBundleName() {
    return true === hvigor_1.hvigorCore.getParameter().getProperty(common_const_js_1.HvigorConfigConst.OHOS_INTEGRATED_HSP_USE_GET_CURRENT_BUNDLE_NAME);
  }
  static isDependenciesTypesEnable() {
    return true === hvigor_1.hvigorCore.getParameter().getPropertyInternal(common_const_js_1.HvigorConfigConst.OHOS_DEPENDENCIES_TYPES_ENABLE);
  }
  static getAlignDeviceTypes() {
    const t = hvigor_1.hvigorCore.getParameter().getPropertyInternal(common_const_js_1.HvigorConfigConst.OHOS_ALIGN_DEVICE_TYPES);
    return t ? "string" == typeof t ? t.split(",") : Array.isArray(t) ? t : [] : [];
  }
  static isTestFrameWork() {
    return hvigor_1.hvigorCore.isCommandEntryTask(task_names_js_1.TaskNames.CommonHookTask.GEN_ON_DEVICE_TEST_HAP.name) || hvigor_1.hvigorCore.isCommandEntryTask(task_names_js_1.TaskNames.Task.UNIT_TEST_BUILD.name) || hvigor_1.hvigorCore.isCommandEntryTask(task_names_js_1.TaskNames.Task.TEST.name) || hvigor_1.hvigorCore.isCommandEntryTask(task_names_js_1.TaskNames.Task.ON_DEVICE_TEST.name) || this.isOhosTestCoverage() || this.isInTest();
  }
  static getEntryModulesName() {
    const t = hvigor_1.hvigorCore.getExtraConfig().get("module");
    return t ? t.split(",").map(t => t.split("@")[0]) : [];
  }
  static isSingleFileEmitEnabled() {
    var t;
    return null !== (t = hvigor_1.hvigorCore.getParameter().getPropertyInternal(common_const_js_1.CommonConst.SINGLE_FILE_EMIT)) && undefined !== t && t;
  }
  static isSync() {
    return !!hvigor_1.globalData.cliOpts.sync;
  }
  static isHarDeviceTestWithOhosTest(t) {
    return InjectUtil.isDeviceTest() && t.isOhosTest() && t.getModuleService().getModuleModel().isHarModule();
  }
}
InjectUtil._log = ohos_logger_js_1.OhosLogger.getLogger("InjectUtil"), InjectUtil.startParams = hvigor_1.hvigorCore.getParameter().getStartParamsInternal(), InjectUtil.customBuildDirHasLogged = false;
