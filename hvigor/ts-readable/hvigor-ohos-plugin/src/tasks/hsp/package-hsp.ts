/*
 * Human-readable TypeScript reconstruction.
 * Generated from hvigor/ts by hvigor/tools/reconstruct-readable-ts.cjs.
 * Keep semantic edits in Rust ports or separate hand-refined notes.
 * Source baseline: hvigor-ohos-plugin/src/tasks/hsp/package-hsp.js
 */
import path_1 from "path";
import * as fse from "fs-extra";
import * as build_directory_const_js_1 from "../../const/build-directory-const";
import * as ohos_logger_js_1 from "../../utils/log/ohos-logger";
import * as task_util_js_1 from "../../utils/task-util";
import * as validate_util_js_1 from "../../utils/validate/validate-util";
import * as base_pack_hap_task_js_1 from "../base/base-pack-hap-task";
import * as task_names_js_1 from "../common/task-names";
import * as inject_util_js_1 from "../../utils/inject-util";
import * as hvigor_arkts_compose_1 from "@ohos/hvigor-arkts-compose";
var Task = task_names_js_1.TaskNames.Task;
export class PackageHsp extends base_pack_hap_task_js_1.BasePackHapTask {
  constructor(e) {
    super(e, Task.PACKAGE_HSP, "hsp", ohos_logger_js_1.OhosLogger.getLogger(PackageHsp.name));
  }
  declareInputs() {
    var e;
    const t = super.declareInputs();
    return t.set("harLocalDependencyCheck", !!(null === (e = this.targetService.getBuildOption().strictMode) || undefined === e ? undefined : e.harLocalDependencyCheck)), t;
  }
  declareInputFiles() {
    const e = super.declareInputFiles();
    return fse.existsSync(this.intermediateTempStartupFilePath) && e.addEntry(this.intermediateTempStartupFilePath), this.priorResourcePath && fse.existsSync(this.priorResourcePath) && e.addEntry(this.priorResourcePath, {
      isDirectory: true
    }), e;
  }
  async beforeAlwaysAction() {
    if (await super.beforeAlwaysAction(), inject_util_js_1.InjectUtil.isByteCodeHarOptimize()) {
      const e = this.getImporteesCachePath(hvigor_arkts_compose_1.DIRECT_IMPORTEE_CACHE_FILE_NAME, "CompileArkTS");
      this.doGlobalLocalDependencyCheck(e);
    }
    this.setHarDeduplicatePriorResourcePath();
  }
  async doTaskAction() {
    const e = path_1.resolve(this.pathInfo.getIntermediatesPatch(), build_directory_const_js_1.BuildArtifactConst.BASE_NATIVE_LIBS_JSON);
    (0, task_util_js_1.checkAppPreloadHintStartupTasksSrcEntry)(this.intermediateTempStartupFilePath, e, this._log, this.moduleName);
    const t = this.service.getAllDependencies();
    return validate_util_js_1.ValidateUtil.validateLocalDependency(this.moduleModel, this.targetService.getBuildOption(), t), super.doTaskAction();
  }
  setHarDeduplicatePriorResourcePath() {
    if (!this.targetService.shouldDoDeduplicateTask()) return;
    const e = path_1.resolve(this.pathInfo.getIntermediatesResDeduplicate(), build_directory_const_js_1.BuildDirConst.RESTOOL_BUILD_RESOURCES);
    fse.existsSync(e) && (this.priorResourcePath = e);
    const t = path_1.resolve(this.pathInfo.getIntermediatesResDeduplicate(), build_directory_const_js_1.BuildArtifactConst.RESOURCE_INDEX);
    fse.existsSync(t) && (this.priorResourcesIndexPath = t);
  }
}
