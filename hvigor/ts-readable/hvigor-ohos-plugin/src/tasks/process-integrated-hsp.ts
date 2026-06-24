/*
 * Human-readable TypeScript reconstruction.
 * Generated from hvigor/ts by hvigor/tools/reconstruct-readable-ts.cjs.
 * Keep semantic edits in Rust ports or separate hand-refined notes.
 * Source baseline: hvigor-ohos-plugin/src/tasks/process-integrated-hsp.js
 */
import fs_extra_1 from "fs-extra";
import path_1 from "path";
import * as build_directory_const_js_1 from "../const/build-directory-const";
import * as integrated_hsp_utils_js_1 from "../utils/integrated-hsp-utils";
import * as process_utils_js_1 from "../utils/process-utils";
import * as remote_hsp_utils_js_1 from "../utils/remote-hsp-utils";
import * as wdk_1 from "@baize/wdk";
import * as hvigor_1 from "@ohos/hvigor";
import * as sdk_const_js_1 from "../const/sdk-const";
import * as ohos_logger_js_1 from "../utils/log/ohos-logger";
import * as task_util_js_1 from "../utils/task-util";
import * as task_names_js_1 from "./common/task-names";
import * as global_project_data_service_js_1 from "./service/global-project-data-service";
import * as ohos_module_task_js_1 from "./task/ohos-module-task";
import * as dependency_manager_js_1 from "../project/dependency/dependency-manager";
export class ProcessIntegratedHsp extends ohos_module_task_js_1.OhosModuleTask {
  initTaskDepends() {}
  constructor(e) {
    super(e, task_names_js_1.TaskNames.CommonTask.PROCESS_INTEGRATED_HSP), this._log = ohos_logger_js_1.OhosLogger.getLogger(ProcessIntegratedHsp.name), this.allRemoteHspPathMap = new Map(), this.integratedInputList = [], this.integratedOutputList = [], this.inputFile = path_1.resolve(this.pathInfo.getModuleBuildOutputPath(), this.targetData.getModuleTargetOutputFileName("", false, build_directory_const_js_1.BuildArtifactExtension.DOT_HSP)), this.integratedRemoteHspDir = this.projectModel.getCacheIntegratedHspPath(this.targetData.getProduct().name);
    const t = this.moduleModel.getRemoteHspPath(),
      s = this.service.getProjectModel().getRemoteHspPath(),
      i = new dependency_manager_js_1.DependencyManager(this.projectModel.isFaMode(), this.moduleModel, this.projectModel),
      {
        npmDependencies: r
      } = i.collectAllDependencies();
    (0, remote_hsp_utils_js_1.initSignRemoteHspMap)(t, this.allRemoteHspPathMap, r), (0, remote_hsp_utils_js_1.initSignRemoteHspMap)(s, this.allRemoteHspPathMap, r);
    const a = global_project_data_service_js_1.GlobalProjectDataService.getInstance().getIntegratedHspUtils();
    if (a) {
      a.initIntegratedHspCache(this.projectModel);
      for (const e of this.allRemoteHspPathMap.values()) e.isIntegratedHsp && a.isNeedIntegrated(e.hspDirName) && (this.integratedInputList.push(e.hspPath), this.integratedOutputList.push(path_1.resolve(this.integratedRemoteHspDir, e.hspDirName)), a && (a.setIntegratedRemoteHsps((0, wdk_1.cloneDeep)({
        ...e,
        isIntegratedHsp: false
      })), global_project_data_service_js_1.GlobalProjectDataService.getInstance().setNeedSignedRemoteHspMap(e.hspDirName, {
        hspFileName: "",
        hspDirName: "",
        version: "",
        hspPath: "",
        signedHspPath: "",
        moduleName: this.moduleModel.getName(),
        isSigned: false
      })));
    }
  }
  declareInputs() {
    const e = super.declareInputs(),
      {
        bundleName: t,
        versionCode: s,
        buildVersion: i
      } = (0, integrated_hsp_utils_js_1.getAppInfoByProject)(this.projectModel);
    return e.set("bundleName", t), e.set("versionCode", s), e.set("buildVersion", null != i ? i : ""), e;
  }
  declareInputFiles() {
    return super.declareInputFiles().addEntries([...this.integratedInputList], {
      isDirectory: false
    });
  }
  declareOutputFiles() {
    const e = super.declareOutputFiles();
    e.addEntries([...this.integratedOutputList], {
      isDirectory: false
    });
    const t = this.targetData.getProduct().name,
      s = path_1.resolve(this.projectModel.getCacheIntegratedHspPath(t), build_directory_const_js_1.BuildArtifactConst.INTEGRATED_HSP_CACHE);
    return e.addEntry(s, {
      isDirectory: false
    }), e;
  }
  async doTaskAction() {
    const e = global_project_data_service_js_1.GlobalProjectDataService.getInstance().getIntegratedHspUtils();
    if (e && 0 !== this.integratedInputList.length) {
      (0, task_util_js_1.limitMinApiVersion)(this.targetData, sdk_const_js_1.ApiVersion.API_VERSION_11) || this._log.printErrorExit("INTEGRATED_HSP_UNSUPPORTED_API", [this.moduleModel.getParentProject().getProfilePath()]);
      for (let t = 0; t < this.integratedInputList.length; t++) await this.generateIntegratedHsp(this.integratedInputList[t], this.integratedOutputList[t]), e.changeIntegratedHspStatus(path_1.basename(this.integratedOutputList[t]));
      e.writeIntegratedHspCache();
    }
  }
  async generateIntegratedHsp(e, t) {
    const s = this.durationEvent.createSubEvent("process remote integrated HSPcommand", "");
    s.start(), fs_extra_1.existsSync(t) || (await fs_extra_1.mkdir(t, {
      recursive: true
    }));
    const i = (0, integrated_hsp_utils_js_1.generateIntegratedHspCommand)(this.projectModel, this.sdkInfo.getPackageTool(), e, t),
      r = {
        moduleName: this.moduleName,
        taskName: this.name,
        commandLine: i
      };
    this._log._printDebugCommand(this.sdkInfo.getPackageTool(), i), s.stop(), s.setLog(s.getName(), hvigor_1.MetricLogType.INFO);
    const a = "submit generate integrated hsp task to work pool",
      o = this.durationEvent.createSubEvent(a, "");
    o.start(), await new process_utils_js_1.ProcessUtils(r.moduleName, r.taskName).submitExecutionWithoutReturn(this, this.getWorkerPool(), r, wdk_1.noop, [], o), o.stop(), o.setLog(a, hvigor_1.MetricLogType.INFO);
  }
}
