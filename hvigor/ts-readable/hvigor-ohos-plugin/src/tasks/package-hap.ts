/*
 * Human-readable TypeScript reconstruction.
 * Generated from hvigor/ts by hvigor/tools/reconstruct-readable-ts.cjs.
 * Keep semantic edits in Rust ports or separate hand-refined notes.
 * Source baseline: hvigor-ohos-plugin/src/tasks/package-hap.js
 */
import path_1 from "path";
import * as wdk_1 from "@baize/wdk";
import * as fse from "fs-extra";
import * as find_target_product_js_1 from "../common/find-target-product";
import * as project_ohos_config_manager_js_1 from "../common/global/project-ohos-config-manager";
import * as build_directory_const_js_1 from "../const/build-directory-const";
import * as ohos_logger_js_1 from "../utils/log/ohos-logger";
import * as task_util_js_1 from "../utils/task-util";
import * as validate_systemHsp_js_1 from "../utils/validate/validate-systemHsp";
import * as base_pack_hap_task_js_1 from "./base/base-pack-hap-task";
import * as task_names_js_1 from "./common/task-names";
var Task = task_names_js_1.TaskNames.Task;
export class PackageHap extends base_pack_hap_task_js_1.BasePackHapTask {
  constructor(e) {
    super(e, Task.PACKAGE_HAP, "hap", ohos_logger_js_1.OhosLogger.getLogger(PackageHap.name));
    const t = e.getTargetData().getModuleSourceSetModel().getModuleTargetRes();
    this.targetModuleOptObj = t.getModuleJsonOpt(), this.targetJsonPath = t.getJsonPath();
    const s = this.projectModel.getAppRes();
    this.appOptObj = s.getAppResOpt();
  }
  declareInputs() {
    var e, t, s;
    const a = super.declareInputs(),
      r = (0, wdk_1.cloneDeep)(null === (e = project_ohos_config_manager_js_1.projectOhosConfigManager.getOverrides()) || undefined === e ? undefined : e.appOpt);
    return a.set("bundleType", null !== (s = null !== (t = null == r ? undefined : r.bundleType) && undefined !== t ? t : (0, find_target_product_js_1.findTargetProduct)(this.projectModel).bundleType) && undefined !== s ? s : this.projectModel.getBundleType()), a;
  }
  declareInputFiles() {
    const e = super.declareInputFiles();
    return fse.existsSync(this.intermediateTempStartupFilePath) && e.addEntry(this.intermediateTempStartupFilePath), this.priorResourcePath && fse.existsSync(this.priorResourcePath) && e.addEntry(this.priorResourcePath, {
      isDirectory: true
    }), e;
  }
  setPriorResourcePath() {
    const e = this.getTaskTempDir(this.targetData, `${this.targetData.getTargetName()}@${task_names_js_1.TaskNames.Task.PROCESS_COMPILED_RESOURCES.name}`, false),
      t = path_1.resolve(e, path_1.basename(this.resourcePath));
    this.isPreloadSystemSoJsonEffective && fse.existsSync(t) && (this.priorResourcePath = t);
  }
  async beforeAlwaysAction() {
    await super.beforeAlwaysAction(), this.setPriorResourcePath();
  }
  async doTaskAction() {
    const e = path_1.resolve(this.pathInfo.getIntermediatesPatch(), build_directory_const_js_1.BuildArtifactConst.BASE_NATIVE_LIBS_JSON);
    return (0, task_util_js_1.checkAppPreloadHintStartupTasksSrcEntry)(this.intermediateTempStartupFilePath, e, this._log, this.moduleName), (0, validate_systemHsp_js_1.validatePackageBySystemHspModel)(this.service, this.appOptObj, this.targetJsonPath, this.targetModuleOptObj), super.doTaskAction();
  }
}
