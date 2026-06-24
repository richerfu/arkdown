/*
 * Human-readable TypeScript reconstruction.
 * Generated from hvigor/ts by hvigor/tools/reconstruct-readable-ts.cjs.
 * Keep semantic edits in Rust ports or separate hand-refined notes.
 * Source baseline: hvigor-ohos-plugin/src/tasks/sign-hqf.js
 */
import path_1 from "path";
import fs_extra_1 from "fs-extra";
import * as sign_type_enum_js_1 from "../enum/sign-type-enum";
import * as ohos_logger_js_1 from "../utils/log/ohos-logger";
import * as task_names_js_1 from "./common/task-names";
import * as package_hqf_js_1 from "./package-hqf";
import * as sign_model_js_1 from "./sign/command-builder-impl/sign-model";
import * as sign_util_js_1 from "./sign/sign-util";
import * as ohos_hap_task_js_1 from "./task/ohos-hap-task";
export class SignHqf extends ohos_hap_task_js_1.OhosHapTask {
  constructor(e) {
    super(e, task_names_js_1.TaskNames.Task.SIGN_HQF), this._log = ohos_logger_js_1.OhosLogger.getLogger("SignHqf");
  }
  async doTaskAction() {
    const e = path_1.resolve(this.pathInfo.getModuleBuildOutputPath(), (0, sign_util_js_1.getHqfName)(this.moduleName, this.targetName, false)),
      t = (0, sign_util_js_1.getHqfName)(this.moduleName, this.targetName, true),
      s = path_1.resolve(this.pathInfo.getModuleBuildOutputPath(), t),
      i = new sign_model_js_1.SignModel(sign_type_enum_js_1.SignTypeEnum.HQF, e, s),
      a = new sign_util_js_1.SignUtil(this.service, sign_type_enum_js_1.SignTypeEnum.HQF, this.pathInfo, this.targetData.getProduct(), this.sdkInfo);
    await a.sign(i, this.durationEvent), a._signingConfig && (await this.updateQuickfixLog(t));
  }
  initTaskDepends() {
    this.declareDepends(package_hqf_js_1.PackageHqf.name);
  }
  async updateQuickfixLog(e) {
    const t = path_1.resolve(this.pathInfo.getModuleBuildOutputPath(), "quickfix.json");
    if (!fs_extra_1.existsSync(t)) return;
    const s = await fs_extra_1.readJSON(t, {
      encoding: "utf-8"
    });
    s.file = e, await fs_extra_1.writeJSON(t, s);
  }
  getPatchConfigMode() {
    var e;
    const t = this.pathInfo.getBuildConfigPath();
    if (fs_extra_1.existsSync(t)) {
      const s = fs_extra_1.readJsonSync(t);
      return null === (e = null == s ? undefined : s.patchConfig) || undefined === e ? undefined : e.mode;
    }
    this._log.error(`${t} is not exists`);
  }
}
