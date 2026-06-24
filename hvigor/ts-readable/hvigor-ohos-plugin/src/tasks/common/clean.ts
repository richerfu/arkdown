/*
 * Human-readable TypeScript reconstruction.
 * Generated from hvigor/ts by hvigor/tools/reconstruct-readable-ts.cjs.
 * Keep semantic edits in Rust ports or separate hand-refined notes.
 * Source baseline: hvigor-ohos-plugin/src/tasks/common/clean.js
 */
import * as fs from "fs";
import os_1 from "os";
import path_1 from "path";
import * as hvigor_1 from "@ohos/hvigor";
import * as hvigor_common_1 from "@ohos/hvigor-common";
import fs_extra_1 from "fs-extra";
import * as build_directory_const_js_1 from "../../const/build-directory-const";
import * as ohos_plugin_id_js_1 from "../../plugin/common/ohos-plugin-id";
import * as inject_util_js_1 from "../../utils/inject-util";
import * as ohos_logger_js_1 from "../../utils/log/ohos-logger";
import * as process_utils_js_1 from "../../utils/process-utils";
import * as task_util_js_1 from "../../utils/task-util";
import * as validator_store_js_1 from "../../utils/validate/validator-store";
import * as global_project_data_service_js_1 from "../service/global-project-data-service";
import * as module_task_service_js_1 from "../service/module-task-service";
import * as task_names_js_1 from "./task-names";
export class Clean extends hvigor_1.DefaultTask {
  constructor(e, t) {
    super(e, task_names_js_1.TaskNames.CommonTask.CLEAN), this._logger = ohos_logger_js_1.OhosLogger.getLogger(Clean.name), this.nativeClean = e => {
      if (!e) return;
      const t = new process_utils_js_1.ProcessUtils();
      if (!(this._taskService instanceof module_task_service_js_1.ModuleTaskService)) return;
      const r = path_1.resolve(this._taskService.getModuleModel().getProjectDir(), ".cxx"),
        i = e.getNativeNinjaTool();
      if (!fs.existsSync(r) || !fs.existsSync(i)) return;
      const s = [];
      this.getArchDirectories(r, s, this._taskService);
      const o = [];
      for (const e of s) {
        const r = path_1.resolve(e, "CMakeCache.txt"),
          s = path_1.resolve(e, "build.ninja"),
          a = fs.existsSync(path_1.resolve(e, "rules.ninja")) || fs.existsSync(path_1.resolve(e, "CMakeFiles", "rules.ninja"));
        if (!fs.existsSync(r) || !fs.existsSync(s) || !a) continue;
        const n = [i, "-C", e, "clean"];
        this.tryCatchExecute(n, o, e, t), fs.existsSync(r) && fs.unlinkSync(r);
      }
      if (o.length > 0) {
        let e = "Delete the following directories that are no longer needed:";
        for (const t of o) e = e.concat(os_1.EOL, t);
        this._logger.warn(e);
      }
    }, this.tryCatchExecute = (e, t, r, i) => {
      try {
        i.executeSync(e);
      } catch (e) {
        t.push(r);
      }
    }, this.getArchDirectories = (e, t, r) => {
      const i = (0, task_util_js_1.getBuildModeName)();
      for (const s of r.getProjectModel().getProductNames()) for (const o of r.getTargets()) {
        const r = path_1.resolve(e, s, o.getTargetNameInternal(), i);
        this.pushArchDirectories(r, t);
      }
    }, this.registryAction = () => async () => {
      if (await this.terminateWorkerPool(), await this.clean(path_1.resolve(this._taskService.getProjectModel().getProjectDir(), "build")), this.resetRemoteHspCache(), this._taskService instanceof module_task_service_js_1.ModuleTaskService) {
        const e = this._taskService.getModuleModel();
        if (!e) return;
        await this.clean(path_1.resolve(e.getProjectDir(), "build"), path_1.resolve(e.getProjectDir(), ".test"));
      }
    }, this.clean = async (e, t) => {
      var r;
      let i = false;
      i || (i = this.rmdirSyncWithBuildDir(e)), t && (i || (i = this.rmdirSyncWithBuildDir(t)));
      const s = inject_util_js_1.InjectUtil.getBuildCacheParentDir(e, path_1.join(hvigor_1.hvigorCore.getProject().getName(), e.substring(hvigor_1.hvigorCore.getProject().getNodeDir().length)));
      if (s !== e && (i || (i = this.rmdirSyncWithBuildDir(s)), (0, hvigor_common_1.isCI)())) {
        const e = path_1.join(path_1.dirname(s), build_directory_const_js_1.BuildDirConst.CPP_OUTPUT);
        i || (i = this.rmdirSyncWithBuildDir(e)), i || this._logger.debug("CI scenario cleared .cxx dir.");
      }
      if (i && this._logger.printErrorExit("UNABLE_TO_REMOVE_BUILD_DIR", [e]), "project" === this.node.classKind) return;
      const o = null === (r = this._taskService.getProjectModel().getProject().getPluginById(ohos_plugin_id_js_1.OhosPluginId.OHOS_APP_PLUGIN).getTaskService()) || undefined === r ? undefined : r.getSdkInfo();
      this.nativeClean(o);
    }, this.terminateWorkerPool = async () => {
      this.getWorkerPool().isActive() ? (this._logger.debug("clean: Start terminating worker pool."), await this.getWorkerPool().terminate(), this._logger.debug("clean: Terminating worker pool finished."), hvigor_1.TcbStore.clear(), validator_store_js_1.ValidatorStore.clear()) : this._logger.debug("clean: Worker pool is inactive.");
    }, this.rmdirSyncWithBuildDir = e => {
      let t = false;
      return fs.existsSync(e) && (t = this.rmdirSync(e, false)), t && this._logger.printErrorExit("UNABLE_TO_REMOVE_BUILD_DIR", [e]), t;
    }, this.rmdirSync = (e, t) => {
      fs.readdirSync(e).forEach(r => {
        const i = path_1.resolve(e, r),
          s = fs.lstatSync(i);
        if (s.isFile() || s.isSymbolicLink()) try {
          fs.unlinkSync(i);
        } catch (e) {
          hvigor_1.MetricFactory.createLogEvent(e.message, hvigor_1.MetricLogType.DEBUG), hvigor_1.MetricFactory.createLogEvent(`Cannot delete ${i}. The file may be opened by another program.`, hvigor_1.MetricLogType.WARN), this._logger.debug(e.message), this._logger.warn(`Cannot delete ${i}. The file may be opened by another program.`), t = true;
        } else s.isDirectory() && (t = this.rmdirSync(i, t));
      });
      try {
        let r = true;
        if (!fs.existsSync(e)) return t;
        fs.readdirSync(e).forEach(t => {
          const i = path_1.resolve(e, t);
          fs.existsSync(i) && (r = false);
        }), r && fs_extra_1.removeSync(e);
      } catch (r) {
        hvigor_1.MetricFactory.createLogEvent(r.message, hvigor_1.MetricLogType.DEBUG), hvigor_1.MetricFactory.createLogEvent(`Unable to delete ${e}. A file may be open by another program.`, hvigor_1.MetricLogType.WARN), this._logger.debug(r.message), this._logger.warn(`Unable to delete ${e}. A file may be open by another program.`), t = true;
      }
      return t;
    }, this._taskService = t;
  }
  pushArchDirectories(e, t) {
    if (!fs.existsSync(e) || !this.isDirectory(e)) return;
    const r = fs.readdirSync(e);
    for (const i of r) {
      const r = path_1.join(e, i);
      if ("arm64-v8a" === i || "armeabi-v7a" === i || "x86_64" === i) {
        if (!this.isDirectory(r)) continue;
        t.push(r);
      }
    }
  }
  isDirectory(e) {
    try {
      return fs.readdirSync(e), true;
    } catch (e) {
      return false;
    }
  }
  resetRemoteHspCache() {
    var e;
    global_project_data_service_js_1.GlobalProjectDataService.getInstance().resetRemoteHspCache(), null === (e = global_project_data_service_js_1.GlobalProjectDataService.getInstance().getIntegratedHspUtils()) || undefined === e || e.resetIntegratedHspCache();
  }
}
