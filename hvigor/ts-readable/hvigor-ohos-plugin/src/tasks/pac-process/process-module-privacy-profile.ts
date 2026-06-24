/*
 * Human-readable TypeScript reconstruction.
 * Generated from hvigor/ts by hvigor/tools/reconstruct-readable-ts.cjs.
 * Keep semantic edits in Rust ports or separate hand-refined notes.
 * Source baseline: hvigor-ohos-plugin/src/tasks/pac-process/process-module-privacy-profile.js
 */
import * as task_names_js_1 from "../common/task-names";
import * as pac_util_1 from "./pac-util";
import * as ohos_module_task_js_1 from "../task/ohos-module-task";
import * as hvigor_1 from "@ohos/hvigor";
import fs_extra_1 from "fs-extra";
import * as pre_build_js_1 from "../pre-build";
import * as ohos_logger_1 from "../../utils/log/ohos-logger";
var Task = task_names_js_1.TaskNames.Task;
export class ProcessModulePrivacyProfile extends ohos_module_task_js_1.OhosModuleTask {
  constructor(e) {
    super(e, Task.PROCESS_MODULE_PRIVACY_PROFILE), this._log = ohos_logger_1.OhosLogger.getLogger(ProcessModulePrivacyProfile.name), this.originalModulePacJsonPath = this.pathInfo.getPacJsonPath(), this.processedPacJsonPath = this.pathInfo.getIntermediatesPacJsonPath();
  }
  taskShouldDo() {
    return super.taskShouldDo() && !this.moduleModel.isHapModule();
  }
  doTaskAction() {
    const e = this.sdkInfo.getPacSchema();
    if ((0, pac_util_1.pacFileValidate)(this.moduleName, this.originalModulePacJsonPath, e)) try {
      fs_extra_1.copySync(this.originalModulePacJsonPath, this.processedPacJsonPath);
    } catch (e) {
      this._log.printErrorExit("COPY_FILE_ERROR", [this.originalModulePacJsonPath]);
    }
  }
  declareInputFiles() {
    const e = super.declareInputFiles();
    return this.originalModulePacJsonPath && fs_extra_1.existsSync(this.originalModulePacJsonPath) && e.addEntry(this.originalModulePacJsonPath), e;
  }
  declareOutputFiles() {
    const e = new hvigor_1.FileSet();
    return e.addEntry(this.processedPacJsonPath), e;
  }
  initTaskDepends() {
    this.dependsOn(pre_build_js_1.PreBuild.name);
  }
}
