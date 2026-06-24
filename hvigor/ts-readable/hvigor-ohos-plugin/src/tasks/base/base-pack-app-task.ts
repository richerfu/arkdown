/*
 * Human-readable TypeScript reconstruction.
 * Generated from hvigor/ts by hvigor/tools/reconstruct-readable-ts.cjs.
 * Keep semantic edits in Rust ports or separate hand-refined notes.
 * Source baseline: hvigor-ohos-plugin/src/tasks/base/base-pack-app-task.js
 */
import fs_extra_1 from "fs-extra";
import path_1 from "path";
import * as build_directory_const_js_1 from "../../const/build-directory-const";
import * as ohos_logger_js_1 from "../../utils/log/ohos-logger";
import * as ohos_app_task_js_1 from "../task/ohos-app-task";
export class BasePackAppTask extends ohos_app_task_js_1.OhosAppTask {
  constructor(t, e, s) {
    super(t, e, s), this._log = ohos_logger_js_1.OhosLogger.getLoggerWithDurationId(BasePackAppTask.name, this.durationEvent.getId()), this.packageTool = this.service.getSdkInfo().getPackageTool(), this.pathInfo = this.service.getPathInfo(), this.packInfoPath = path_1.resolve(this.pathInfo.getProjectOutputPath(), build_directory_const_js_1.BuildArtifactConst.PACK_INFO), this.packResPath = path_1.resolve(this.pathInfo.getProjectOutputPath(), build_directory_const_js_1.BuildArtifactConst.PACK_RES), this.entryCardDir = path_1.resolve(this.project.getNodeDir(), "EntryCard"), this.needPackRes = fs_extra_1.existsSync(this.entryCardDir) && fs_extra_1.statSync(this.entryCardDir).isDirectory();
  }
  declareExecutionTool() {
    return this.packageTool;
  }
}
