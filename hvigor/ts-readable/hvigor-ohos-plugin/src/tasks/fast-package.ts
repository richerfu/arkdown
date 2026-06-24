/*
 * Human-readable TypeScript reconstruction.
 * Generated from hvigor/ts by hvigor/tools/reconstruct-readable-ts.cjs.
 * Keep semantic edits in Rust ports or separate hand-refined notes.
 * Source baseline: hvigor-ohos-plugin/src/tasks/fast-package.js
 */
import path_1 from "path";
import * as wdk_1 from "@baize/wdk";
import * as hvigor_1 from "@ohos/hvigor";
import * as fse from "fs-extra";
import * as find_target_product_js_1 from "../common/find-target-product";
import * as project_ohos_config_manager_js_1 from "../common/global/project-ohos-config-manager";
import * as build_directory_const_js_1 from "../const/build-directory-const";
import * as ohos_logger_js_1 from "../utils/log/ohos-logger";
import * as task_util_js_1 from "../utils/task-util";
import * as validate_systemHsp_js_1 from "../utils/validate/validate-systemHsp";
import * as base_pack_hap_task_js_1 from "./base/base-pack-hap-task";
import * as task_names_js_1 from "./common/task-names";
var __decorate = this && this.__decorate || function (e, t, a, s) {
  var r,
    o = arguments.length,
    i = o < 3 ? t : null === s ? s = Object.getOwnPropertyDescriptor(t, a) : s;
  if ("object" == typeof Reflect && "function" == typeof Reflect.decorate) i = Reflect.decorate(e, t, a, s);else for (var n = e.length - 1; n >= 0; n--) (r = e[n]) && (i = (o < 3 ? r(i) : o > 3 ? r(t, a, i) : r(t, a)) || i);
  return o > 3 && i && Object.defineProperty(t, a, i), i;
};
var Task = task_names_js_1.TaskNames.Task;
export class FastPackageHap extends base_pack_hap_task_js_1.BasePackHapTask {
  constructor(e) {
    super(e, Task.FAST_PACKAGE, "hap", ohos_logger_js_1.OhosLogger.getLogger(FastPackageHap.name));
    const t = e.getTargetData().getModuleSourceSetModel().getModuleTargetRes();
    this.targetModuleOptObj = t.getModuleJsonOpt(), this.targetJsonPath = t.getJsonPath();
    const a = this.projectModel.getAppRes();
    this.appOptObj = a.getAppResOpt(), this.preloadSoJsonPath = this.pathInfo.getPreloadSoFilePath();
  }
  declareInputs() {
    var e, t, a;
    const s = super.declareInputs(),
      r = (0, wdk_1.cloneDeep)(null === (e = project_ohos_config_manager_js_1.projectOhosConfigManager.getOverrides()) || undefined === e ? undefined : e.appOpt);
    return s.set("bundleType", null !== (a = null !== (t = null == r ? undefined : r.bundleType) && undefined !== t ? t : (0, find_target_product_js_1.findTargetProduct)(this.projectModel).bundleType) && undefined !== a ? a : this.projectModel.getBundleType()), s.set("alignDeviceTypes", this.alignDeviceTypes), s;
  }
  declareInputFiles() {
    const e = super.declareInputFiles();
    return fse.existsSync(this.intermediateTempStartupFilePath) && e.addEntry(this.intermediateTempStartupFilePath), e;
  }
  async beforeAlwaysAction() {
    await super.beforeAlwaysAction();
  }
  async doTaskAction() {
    const e = path_1.resolve(this.pathInfo.getIntermediatesPatch(), build_directory_const_js_1.BuildArtifactConst.BASE_NATIVE_LIBS_JSON);
    (0, task_util_js_1.checkAppPreloadHintStartupTasksSrcEntry)(this.intermediateTempStartupFilePath, e, this._log, this.moduleName), (0, validate_systemHsp_js_1.validatePackageBySystemHspModel)(this.service, this.appOptObj, this.targetJsonPath, this.targetModuleOptObj), await this.copyIntermediates(), this.isPreloadSystemSoJsonEffective && this.addPreloadSystemSoJson(this.getPreloadSystemSoJson(), this.fastAppDirPath);
  }
}
__decorate([(0, hvigor_1.InputFile)()], FastPackageHap.prototype, "preloadSoJsonPath", undefined);
