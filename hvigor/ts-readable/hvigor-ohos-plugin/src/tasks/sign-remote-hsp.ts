/*
 * Human-readable TypeScript reconstruction.
 * Generated from hvigor/ts by hvigor/tools/reconstruct-readable-ts.cjs.
 * Keep semantic edits in Rust ports or separate hand-refined notes.
 * Source baseline: hvigor-ohos-plugin/src/tasks/sign-remote-hsp.js
 */
import * as hvigor_1 from "@ohos/hvigor";
import fs_extra_1 from "fs-extra";
import path_1 from "path";
import * as build_directory_const_js_1 from "../const/build-directory-const";
import * as common_const_js_1 from "../const/common-const";
import * as sign_type_enum_js_1 from "../enum/sign-type-enum";
import * as ohos_logger_js_1 from "../utils/log/ohos-logger";
import * as remote_hsp_utils_js_1 from "../utils/remote-hsp-utils";
import * as base_sign_js_1 from "./base/base-sign";
import * as global_project_data_service_js_1 from "./service/global-project-data-service";
import * as sign_model_js_1 from "./sign/command-builder-impl/sign-model";
import * as sign_job_js_1 from "./worker/sign-job";
import * as sign_util_js_1 from "./sign/sign-util";
export class SignRemoteHsp extends base_sign_js_1.BaseSign {
  constructor(e, t, s) {
    super(e, t, s), this._log = ohos_logger_js_1.OhosLogger.getLoggerWithDurationId(SignRemoteHsp.name, this.durationEvent.getId()), this.allProjectSignedRemoteHspList = [], this.allModuleSignedRemoteHspList = [], this.allProjectRequiredSignRemoteHspMap = new Map(), this.allModuleRequiredSignRemoteHspMap = new Map(), this.allProjectHspPathList = [], this.allModuleHspPathList = [], this.allProjectRemoteHspPathList = [], this.allModuleRemoteHspPathList = [];
    const i = this.projectModel.getCacheRemoteHspPath(this.targetData.getProduct().name);
    this.projectRemoteHspCachePath = path_1.resolve(i, common_const_js_1.CommonConst.REMOTE_HSP_CACHE);
  }
  async doTaskAction() {}
  declareInputFiles() {
    const e = super.declareInputFiles();
    return this.projectRemoteHspCachePath && fs_extra_1.existsSync(this.projectRemoteHspCachePath) && e.addEntry(this.projectRemoteHspCachePath, {
      isDirectory: false
    }), e;
  }
  getProjectSignRemoteHspMap(e, t) {
    if (!fs_extra_1.existsSync(e)) return;
    const s = fs_extra_1.readdirSync(e),
      i = new Set(t.map(e => e.getPackageName()));
    for (const t of s) {
      const s = path_1.resolve(e, t),
        a = (0, remote_hsp_utils_js_1.getRemoteHspObj)(s, t, this._log);
      a && i.has(a.hspName) && this.allProjectRequiredSignRemoteHspMap.set(a.hspFileName, {
        ...a
      });
    }
  }
  createProjectRemoteHspCache() {
    (0, sign_util_js_1.isSigned)(this.projectModel) && (fs_extra_1.existsSync(this.projectRemoteHspCachePath) || (fs_extra_1.createFileSync(this.projectRemoteHspCachePath), fs_extra_1.writeJsonSync(this.projectRemoteHspCachePath, {})));
  }
  getModuleSignRemoteHspMap(e, t) {
    if (!fs_extra_1.existsSync(e)) return;
    const s = fs_extra_1.readdirSync(e),
      i = new Set(t.map(e => e.getPackageName()));
    for (const t of s) {
      const s = path_1.resolve(e, t),
        a = (0, remote_hsp_utils_js_1.getRemoteHspObj)(s, t, this._log);
      a && i.has(a.hspName) && this.allModuleRequiredSignRemoteHspMap.set(a.hspFileName, {
        ...a
      });
    }
  }
  initAllSignRemoteHspList(e, t, s, i) {
    const a = this.targetData.isHarmonyOS() ? sign_type_enum_js_1.SignTypeEnum.HOS_HAP : sign_type_enum_js_1.SignTypeEnum.HAP,
      o = this.projectModel.getCacheRemoteHspPath(this.targetData.getProduct().name);
    i.forEach(i => {
      const r = i.hspFileName,
        n = i.hspPath,
        _ = i.hspDirName,
        l = r.replace(build_directory_const_js_1.BuildArtifactExtension.DOT_HSP, `-signed${build_directory_const_js_1.BuildArtifactExtension.DOT_HSP}`),
        g = path_1.resolve(o, _, l);
      if (global_project_data_service_js_1.GlobalProjectDataService.getInstance().needSignedRemoteHsp(_, this.moduleModel.getName())) {
        const o = this.projectModel.getCacheIntegratedHspPath(this.targetData.getProduct().name),
          l = i.isIntegratedHsp ? path_1.resolve(o, _, r) : n;
        t.push(l), e.push(g), s.push(new sign_model_js_1.SignModel(a, l, g)), global_project_data_service_js_1.GlobalProjectDataService.getInstance().setNeedSignedRemoteHspMap(_, {
          hspFileName: r,
          hspDirName: _,
          version: i.hspVersion,
          hspPath: l,
          signedHspPath: g,
          isSigned: false
        });
      }
    });
  }
  initDefaultSignConfig(e, t, s) {
    return {
      signCommand: t,
      moduleName: this.service.getModuleModel().getName(),
      signType: e.getSignType(),
      bundleName: this.signUtil.getBundleNameFromHap(this.projectModel),
      verifySignCommands: s
    };
  }
  async submitSignWorker(e, t) {
    const s = this.signUtil.getSignCommand(e),
      i = this.signUtil.getVerifySignCommands(this.targetData.getPathInfo());
    if (!s || !i) return void this._log.debug("No signCommand, please check sign info.");
    const a = this.initDefaultSignConfig(e, s, i),
      o = path_1.resolve(__dirname, "./worker/sign-job.js/runSign"),
      r = {
        workInput: a,
        callback: t
      };
    if (fs_extra_1.existsSync(e.getOutPutFilePath()) || fs_extra_1.mkdirSync(path_1.dirname(e.getOutPutFilePath()), {
      recursive: true
    }), this.getWorkerPool().submit(this, o, r).getState() === hvigor_1.TaskState.REJECT) {
      const t = "sign remote hsp",
        s = this.durationEvent.createSubEvent(t, "");
      s.start(), this._log.debug("Worker dispatch failed, running native strip on main thread."), await this.signUtil.validateBundleName(e, this.targetData.getPathInfo(), s), await (0, sign_job_js_1.runSign)(a), s.stop(), s.setLog(t, hvigor_1.MetricLogType.INFO);
    }
  }
  signRemoteHspCallBack(e, t) {
    if (global_project_data_service_js_1.GlobalProjectDataService.getInstance().changeRemoteHspSigned(t), global_project_data_service_js_1.GlobalProjectDataService.getInstance().allRemoteHspSigned()) {
      const t = global_project_data_service_js_1.GlobalProjectDataService.getInstance().getRemoteHspCache();
      fs_extra_1.writeJsonSync(e, {
        signingConfig: t.signingConfig,
        signedRemoteHsps: Object.fromEntries(t.signedRemoteHsps)
      });
    }
  }
  initTaskDepends() {}
}
