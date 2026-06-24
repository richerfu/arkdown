/*
 * Human-readable TypeScript reconstruction.
 * Generated from hvigor/ts by hvigor/tools/reconstruct-readable-ts.cjs.
 * Keep semantic edits in Rust ports or separate hand-refined notes.
 * Source baseline: hvigor-ohos-plugin/src/tasks/har/package-har.js
 */
import path_1 from "path";
import * as fse from "fs-extra";
import * as build_directory_const_js_1 from "../../const/build-directory-const";
import * as ohos_logger_js_1 from "../../utils/log/ohos-logger";
import * as task_util_js_1 from "../../utils/task-util";
import * as abstract_package_har_js_1 from "../abstract/abstract-package-har";
import * as task_names_js_1 from "../common/task-names";
import * as process_har_artifacts_js_1 from "./process-har-artifacts";
import * as inject_util_js_1 from "../../utils/inject-util";
import * as hvigor_arkts_compose_1 from "@ohos/hvigor-arkts-compose";
import * as process_module_privacy_profile_js_1 from "../pac-process/process-module-privacy-profile";
export class PackageHar extends abstract_package_har_js_1.AbstractPackageHar {
  initTaskDepends() {
    this.declareDependsList(process_har_artifacts_js_1.ProcessHarArtifacts.name, process_module_privacy_profile_js_1.ProcessModulePrivacyProfile.name);
  }
  constructor(e) {
    super(e, task_names_js_1.TaskNames.Task.PACKAGE_HAR), this._log = ohos_logger_js_1.OhosLogger.getLogger(PackageHar.name), this.taskTmpDir = this.getTaskTempDir(this.targetData, undefined, false);
  }
  doExpandImportPathCheck() {
    var e, t;
    (null === (t = null === (e = this.targetService.getBuildOption().arkOptions) || undefined === e ? undefined : e.expandImportPath) || undefined === t ? undefined : t.enable) && this.service.getModuleModel().isHarModule() && this._log.warn(`The expandImportPath does not take effect in the HAR ${this.moduleModel.getName()}.`);
  }
  async beforeAlwaysAction() {
    if (await super.beforeAlwaysAction(), (0, task_util_js_1.doObfuscationCheck)(this.targetService, this._log), this.targetService.isByteCodeHar() && inject_util_js_1.InjectUtil.isByteCodeHarOptimize()) {
      const e = this.getImporteesCachePath(hvigor_arkts_compose_1.DIRECT_IMPORTEE_CACHE_FILE_NAME, "HarCompileArkTS");
      this.doDevDependencyCheck(e), this.targetService.isBundledDependencies() && this.doGlobalLocalDependencyCheck(e);
    }
    this.doExpandImportPathCheck();
  }
  declareInputFiles() {
    const e = super.declareInputFiles();
    return fse.existsSync(this.intermediateTempStartupFilePath) && e.addEntry(this.intermediateTempStartupFilePath), e;
  }
  async doTaskAction() {
    const e = path_1.resolve(this.pathInfo.getIntermediatesPatch(), build_directory_const_js_1.BuildArtifactConst.BASE_NATIVE_LIBS_JSON);
    return (0, task_util_js_1.checkAppPreloadHintStartupTasksSrcEntry)(this.intermediateTempStartupFilePath, e, this._log, this.moduleName, this.targetService.isBundledDependencies()), super.doTaskAction();
  }
}
