/*
 * Human-readable TypeScript reconstruction.
 * Generated from hvigor/ts by hvigor/tools/reconstruct-readable-ts.cjs.
 * Keep semantic edits in Rust ports or separate hand-refined notes.
 * Source baseline: hvigor-ohos-plugin/src/tasks/sign-hap.js
 */
import * as hvigor_1 from "@ohos/hvigor";
import fs_extra_1 from "fs-extra";
import path_1 from "path";
import * as file_util_js_1 from "../utils/file-util";
import * as base_sign_js_1 from "./base/base-sign";
import * as task_names_js_1 from "./common/task-names";
import * as legacy_package_hap_1 from "./legacy-tasks/legacy-package-hap";
import * as module_task_service_js_1 from "./service/module-task-service";
import * as sign_model_js_1 from "./sign/command-builder-impl/sign-model";
import * as package_hap_js_1 from "./package-hap";
import * as common_const_js_1 from "../const/common-const";
export class SignHap extends base_sign_js_1.BaseSign {
  constructor(e) {
    super(e, task_names_js_1.TaskNames.CommonTask.SIGN_HAP), this.signModelList = [], this.initSignModelData();
  }
  taskShouldDo() {
    var e;
    return !((null === (e = this.targetService.getBuildOption().packOptions) || undefined === e ? undefined : e.buildAppSkipSignHap) && hvigor_1.hvigorCore.isCommandEntryTask(task_names_js_1.TaskNames.CommonHookTask.ASSEMBLE_APP.name));
  }
  declareInputFiles() {
    const e = super.declareInputFiles();
    for (const t of this.signModelList) {
      const s = t.getInputFilePath();
      fs_extra_1.existsSync(s) && e.addEntry(s, {
        isDirectory: false
      });
    }
    return e;
  }
  declareOutputFiles() {
    const e = super.declareOutputFiles();
    for (const t of this.signModelList) {
      const s = t.getOutPutFilePath();
      s && e.addEntry(s, {
        isDirectory: false
      });
    }
    return e;
  }
  async doTaskAction() {
    for (const e of this.signModelList) {
      if (file_util_js_1.FileUtil.deleteFile(e.getOutPutFilePath()), !hvigor_1.hvigorCore.getParameter().getPropertyInternal(common_const_js_1.CommonConst.ENABLE_SIGN_TASK)) return;
      await this.signUtil.sign(e, this.durationEvent);
    }
  }
  initTaskDepends() {
    this.declareDepends(this.isFaMode ? legacy_package_hap_1.LegacyPackageHap.name : package_hap_js_1.PackageHap.name);
  }
  initSignModelData() {
    const e = this.service.getRelatedEntryModules(),
      t = this.service.getModuleModel().getModuleType();
    if (module_task_service_js_1.ModuleTaskService.checkEntryModules(t, e)) for (const t of e) this.addSignModel(t);else this.addSignModel("");
  }
  addSignModel(e) {
    if (this.targetData.isSingleDeviceTypeTarget()) {
      const t = this.targetData.getModuleTargetOutputFileName(e, false),
        s = path_1.resolve(this.pathInfo.getModuleBuildOutputPath(), t),
        i = path_1.resolve(this.pathInfo.getModuleBuildOutputPath(), this.targetData.getModuleTargetOutputFileName(e, true));
      this.signModelList.push(new sign_model_js_1.SignModel(this.signType, s, i));
    } else this.targetData.getTargetDeviceTypeClasses().forEach(t => {
      const s = this.targetData.getModuleTargetOutputFileName(e, false, undefined, t),
        i = path_1.resolve(this.pathInfo.getModuleBuildOutputPath(), s),
        a = this.targetData.getModuleTargetOutputFileName(e, true, undefined, t),
        o = path_1.resolve(this.pathInfo.getModuleBuildOutputPath(), a);
      this.signModelList.push(new sign_model_js_1.SignModel(this.signType, i, o));
    });
  }
}
