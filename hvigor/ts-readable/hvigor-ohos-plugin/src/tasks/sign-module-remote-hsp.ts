/*
 * Human-readable TypeScript reconstruction.
 * Generated from hvigor/ts by hvigor/tools/reconstruct-readable-ts.cjs.
 * Keep semantic edits in Rust ports or separate hand-refined notes.
 * Source baseline: hvigor-ohos-plugin/src/tasks/sign-module-remote-hsp.js
 */
import * as fse from "fs-extra";
import path_1 from "path";
import * as common_const_js_1 from "../const/common-const";
import * as task_names_js_1 from "./common/task-names";
import * as sign_util_js_1 from "./sign/sign-util";
import * as sign_project_remote_hsp_js_1 from "./sign-project-remote-hsp";
import * as sign_remote_hsp_js_1 from "./sign-remote-hsp";
import * as dependency_manager_js_1 from "../project/dependency/dependency-manager";
import * as ohos_logger_js_1 from "../utils/log/ohos-logger";
export class SignModuleRemoteHsp extends sign_remote_hsp_js_1.SignRemoteHsp {
  constructor(e) {
    super(e, task_names_js_1.TaskNames.CommonTask.SIGN_MODULE_REMOTE_HSP), this._log = ohos_logger_js_1.OhosLogger.getLoggerWithDurationId(SignModuleRemoteHsp.name, this.durationEvent.getId());
    const t = this.moduleModel.getRemoteHspPath(),
      s = this.service.getProjectModel().getRemoteHspPath(),
      o = new dependency_manager_js_1.DependencyManager(this.projectModel.isFaMode(), this.moduleModel, this.projectModel),
      {
        npmDependencies: i
      } = o.collectAllDependencies();
    this.getModuleSignRemoteHspMap(t, i), this.getProjectSignRemoteHspMap(s, i), this.allModuleRequiredSignRemoteHspMap.size > 0 && (this.createProjectRemoteHspCache(), this.initAllSignRemoteHspList(this.allModuleHspPathList, this.allModuleRemoteHspPathList, this.allModuleSignedRemoteHspList, this.allModuleRequiredSignRemoteHspMap));
  }
  taskShouldDo() {
    return (0, sign_util_js_1.isSigned)(this.projectModel);
  }
  async doTaskAction() {
    await this.moduleSignAction();
  }
  async moduleSignAction() {
    const e = this.projectModel.getCacheRemoteHspPath(this.targetData.getProduct().name),
      t = path_1.resolve(e, common_const_js_1.CommonConst.REMOTE_HSP_CACHE);
    if (0 !== this.allModuleSignedRemoteHspList.length) {
      this._log.debug(`this.allModuleSignedRemoteHspList ${this.allModuleSignedRemoteHspList}`);
      for (const e of this.allModuleSignedRemoteHspList) await this.submitSignWorker(e, () => {
        this.signRemoteHspCallBack(t, e.getOutPutFilePath());
      });
    }
  }
  declareInputFiles() {
    return super.declareInputFiles().addEntries([...this.allModuleRemoteHspPathList], {
      isDirectory: false
    });
  }
  declareOutputFiles() {
    return super.declareOutputFiles().addEntries([...this.allModuleHspPathList], {
      isDirectory: false
    });
  }
  initTaskDepends() {
    const e = this.service.getProjectModel().getRemoteHspPath();
    fse.existsSync(e) && this.dependsOn(`${this.targetData.getTargetName()}@${sign_project_remote_hsp_js_1.SignProjectRemoteHsp.name}`);
  }
}
