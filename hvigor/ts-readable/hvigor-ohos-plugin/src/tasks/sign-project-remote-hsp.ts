/*
 * Human-readable TypeScript reconstruction.
 * Generated from hvigor/ts by hvigor/tools/reconstruct-readable-ts.cjs.
 * Keep semantic edits in Rust ports or separate hand-refined notes.
 * Source baseline: hvigor-ohos-plugin/src/tasks/sign-project-remote-hsp.js
 */
import path_1 from "path";
import * as common_const_js_1 from "../const/common-const";
import * as task_names_js_1 from "./common/task-names";
import * as global_project_data_service_js_1 from "./service/global-project-data-service";
import * as sign_util_js_1 from "./sign/sign-util";
import * as sign_remote_hsp_js_1 from "./sign-remote-hsp";
import * as dependency_manager_1 from "../project/dependency/dependency-manager";
import * as ohos_logger_js_1 from "../utils/log/ohos-logger";
export class SignProjectRemoteHsp extends sign_remote_hsp_js_1.SignRemoteHsp {
  constructor(e) {
    super(e, task_names_js_1.TaskNames.CommonTask.SIGN_PROJECT_REMOTE_HSP), this._log = ohos_logger_js_1.OhosLogger.getLoggerWithDurationId(SignProjectRemoteHsp.name, this.durationEvent.getId());
    const t = this.service.getProjectModel().getRemoteHspPath(),
      s = new dependency_manager_1.DependencyManager(this.projectModel.isFaMode(), this.moduleModel, this.projectModel),
      {
        npmDependencies: o
      } = s.collectAllDependencies();
    this.getProjectSignRemoteHspMap(t, o), this.allProjectRequiredSignRemoteHspMap.size > 0 && (this.createProjectRemoteHspCache(), this.initAllSignRemoteHspList(this.allProjectHspPathList, this.allProjectRemoteHspPathList, this.allProjectSignedRemoteHspList, this.allProjectRequiredSignRemoteHspMap));
  }
  taskShouldDo() {
    return (0, sign_util_js_1.isSigned)(this.projectModel);
  }
  async doTaskAction() {
    const e = global_project_data_service_js_1.GlobalProjectDataService.getInstance().getProjectRemoteHspPromise();
    if (e && 0 === this.allProjectSignedRemoteHspList.length) return this._log.debug("the project remote_hsps are signing or signed, just await"), void (await e);
    const t = this.doSignAction();
    global_project_data_service_js_1.GlobalProjectDataService.getInstance().setProjectRemoteHspPromise(t), await t;
  }
  async doSignAction() {
    try {
      if (0 === this.allProjectSignedRemoteHspList.length) return;
      const e = this.projectModel.getCacheRemoteHspPath(this.targetData.getProduct().name),
        t = path_1.resolve(e, common_const_js_1.CommonConst.REMOTE_HSP_CACHE);
      this._log.debug(`this.allProjectSignedRemoteHspList ${this.allProjectSignedRemoteHspList}`);
      for (const e of this.allProjectSignedRemoteHspList) await this.submitSignWorker(e, () => {
        this.signRemoteHspCallBack(t, e.getOutPutFilePath());
      });
    } catch (e) {
      this._log._buildError(e)._printErrorAndExit();
    }
  }
  declareInputFiles() {
    return super.declareInputFiles().addEntries([...this.allProjectRemoteHspPathList], {
      isDirectory: false
    });
  }
  declareOutputFiles() {
    return super.declareOutputFiles().addEntries([...this.allProjectHspPathList], {
      isDirectory: false
    });
  }
  initTaskDepends() {}
}
