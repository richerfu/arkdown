/*
 * Human-readable TypeScript reconstruction.
 * Generated from hvigor/ts by hvigor/tools/reconstruct-readable-ts.cjs.
 * Keep semantic edits in Rust ports or separate hand-refined notes.
 * Source baseline: hvigor-ohos-plugin/src/tasks/package-hqf.js
 */
import path_1 from "path";
import * as hvigor_1 from "@ohos/hvigor";
import fs_extra_1 from "fs-extra";
import * as ohos_logger_js_1 from "../utils/log/ohos-logger";
import * as process_utils_js_1 from "../utils/process-utils";
import * as task_names_js_1 from "./common/task-names";
import * as sign_util_js_1 from "./sign/sign-util";
import * as ohos_hap_task_js_1 from "./task/ohos-hap-task";
export class PackageHqf extends ohos_hap_task_js_1.OhosHapTask {
  constructor(t) {
    super(t, task_names_js_1.TaskNames.Task.PACKAGE_HQF), this._log = ohos_logger_js_1.OhosLogger.getLogger(PackageHqf.name);
  }
  async doTaskAction() {
    const t = this.getColdReloadCommand();
    await new process_utils_js_1.ProcessUtils().execute(t, {
      encoding: "utf-8"
    });
  }
  getHotReloadCommand() {
    const t = path_1.resolve(this.pathInfo.getModuleBuildOutputPath(), (0, sign_util_js_1.getHqfName)(this.moduleName, this.targetName, false)),
      e = this.sdkInfo.getPackageTool(),
      s = path_1.resolve(this.pathInfo.getModuleBuildIntermediates(), "hotReload", "patch.json"),
      a = fs_extra_1.readJsonSync(this.pathInfo.getBuildConfigPath()).patchConfig.patchAbcPath,
      o = ["java", "-jar", e, "--mode", "hqf"];
    return o.push("--json-path", s), o.push("--ets-path", a), o.push("--out-path", t), o.push("--force", "true"), o;
  }
  getColdReloadCommand() {
    const t = path_1.resolve(this.pathInfo.getPatchDir(), "resources"),
      e = path_1.resolve(this.pathInfo.getPatchDir(), "libs"),
      s = path_1.resolve(this.pathInfo.getModuleBuildOutputPath(), (0, sign_util_js_1.getHqfName)(this.moduleName, this.targetName, false)),
      a = path_1.resolve(t),
      o = this.sdkInfo.getPackageTool(),
      h = this.moduleModel.getName() === hvigor_1.hvigorCore.getExtraConfig().get("hotReload-module") ? path_1.resolve(this.pathInfo.getModulePath(), "patch.json") : path_1.resolve(this.pathInfo.getPatchDir(), "patch.json"),
      r = ["java", "-jar", o, "--mode", "hqf"];
    return r.push("--json-path", h), r.push("--lib-path", e), r.push("--ets-path", path_1.resolve(this.pathInfo.getPatchDir(), "ets")), r.push("--resources-path", a), r.push("--out-path", s), r.push("--force", "true"), r;
  }
  getPatchConfigMode() {
    var t;
    const e = this.pathInfo.getBuildConfigPath();
    if (fs_extra_1.existsSync(e)) {
      const s = fs_extra_1.readJsonSync(e);
      return null === (t = null == s ? undefined : s.patchConfig) || undefined === t ? undefined : t.mode;
    }
    this._log.error(`${e} is not exists`);
  }
  initTaskDepends() {
    this.declareDepends(task_names_js_1.TaskNames.Task.PREPARE_QUICKFIX.name);
  }
}
