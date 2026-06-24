/*
 * Human-readable TypeScript reconstruction.
 * Generated from hvigor/ts by hvigor/tools/reconstruct-readable-ts.cjs.
 * Keep semantic edits in Rust ports or separate hand-refined notes.
 * Source baseline: hvigor-ohos-plugin/src/tasks/hsp/fast-package-hsp.js
 */
import path_1 from "path";
import * as fse from "fs-extra";
import * as build_directory_const_js_1 from "../../const/build-directory-const";
import * as ohos_logger_js_1 from "../../utils/log/ohos-logger";
import * as task_util_js_1 from "../../utils/task-util";
import * as validate_util_js_1 from "../../utils/validate/validate-util";
import * as base_pack_hap_task_js_1 from "../base/base-pack-hap-task";
import * as task_names_js_1 from "../common/task-names";
var Task = task_names_js_1.TaskNames.Task;
export class FastPackageHsp extends base_pack_hap_task_js_1.BasePackHapTask {
  constructor(e) {
    super(e, Task.FAST_PACKAGE_HSP, "hsp", ohos_logger_js_1.OhosLogger.getLogger(FastPackageHsp.name));
  }
  declareInputs() {
    var e;
    const t = super.declareInputs();
    return t.set("harLocalDependencyCheck", !!(null === (e = this.targetService.getBuildOption().strictMode) || undefined === e ? undefined : e.harLocalDependencyCheck)), t.set("alignDeviceTypes", this.alignDeviceTypes), t;
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
    (0, task_util_js_1.checkAppPreloadHintStartupTasksSrcEntry)(this.intermediateTempStartupFilePath, e, this._log, this.moduleName);
    const t = this.service.getAllDependencies();
    return validate_util_js_1.ValidateUtil.validateLocalDependency(this.moduleModel, this.targetService.getBuildOption(), t), this.copyIntermediates();
  }
}
