/*
 * Human-readable TypeScript reconstruction.
 * Generated from hvigor/ts by hvigor/tools/reconstruct-readable-ts.cjs.
 * Keep semantic edits in Rust ports or separate hand-refined notes.
 * Source baseline: hvigor-ohos-plugin/src/tasks/abstract-generate-loader-json.js
 */
import fs_extra_1 from "fs-extra";
import path_1 from "path";
import * as build_directory_const_js_1 from "../const/build-directory-const";
import * as ohos_logger_js_1 from "../utils/log/ohos-logger";
import * as ohos_hap_task_js_1 from "./task/ohos-hap-task";
export class AbstractGenerateLoaderJson extends ohos_hap_task_js_1.OhosHapTask {
  constructor(t, e) {
    super(t, e), this._log = ohos_logger_js_1.OhosLogger.getLogger(AbstractGenerateLoaderJson.name), this.workerConfig = this.getWorkerConfig(), this.modulePathMap = this.getModulePathMap(), this.projectRootPath = this.service.getModuleModel().getBelongProjectPath();
  }
  declareInputs() {
    const t = new Map().set("projectRootPath", this.projectRootPath).set("modulePathMap", JSON.stringify(this.modulePathMap));
    return undefined !== this.workerConfig && t.set("workerConfig", JSON.stringify(this.workerConfig)), t;
  }
  beforeTask() {
    const t = path_1.resolve(this.targetData.getPathInfo().getIntermediatesLoaderPath(), build_directory_const_js_1.BuildArtifactConst.LOADER_JSON);
    fs_extra_1.existsSync(t) && fs_extra_1.removeSync(t);
  }
  getWorkerConfig() {
    const t = this.targetService.getWorkerConfig();
    return 0 === t.length ? undefined : t;
  }
  getModulePathMap() {
    const t = {};
    return this.projectModel.getProject().getSubModules().forEach((e, o) => {
      t[o] = e.getNodeDir();
    }), t;
  }
}
