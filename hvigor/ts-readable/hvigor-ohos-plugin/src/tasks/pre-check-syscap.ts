/*
 * Human-readable TypeScript reconstruction.
 * Generated from hvigor/ts by hvigor/tools/reconstruct-readable-ts.cjs.
 * Keep semantic edits in Rust ports or separate hand-refined notes.
 * Source baseline: hvigor-ohos-plugin/src/tasks/pre-check-syscap.js
 */
import fs_extra_1 from "fs-extra";
import path_1 from "path";
import * as common_const_js_1 from "../const/common-const";
import * as array_util_js_1 from "../utils/array-util";
import * as json_util_js_1 from "../utils/json-util";
import * as ohos_logger_js_1 from "../utils/log/ohos-logger";
import * as ohos_hap_task_js_1 from "./task/ohos-hap-task";
export class PreCheckSyscap extends ohos_hap_task_js_1.OhosHapTask {
  constructor(o, s) {
    var t;
    super(o, s), this._log = ohos_logger_js_1.OhosLogger.getLogger(PreCheckSyscap.name);
    const e = o.getTargetData().getTargetName();
    this.sourceRoot = null === (t = this.service.getModuleModel()) || undefined === t ? undefined : t.getSourceRootByTargetName(e), this.jsonFilePath = path_1.resolve(this.sourceRoot, this.service.isFaMode() ? common_const_js_1.CommonConst.CONFIG_JSON : common_const_js_1.CommonConst.MODULE_JSON5), this.sysCapJsonPath = path_1.resolve(this.sourceRoot, common_const_js_1.CommonConst.SYSCAP_JSON);
  }
  doTaskAction() {
    if (this.targetData.isHarmonyOS()) return;
    const o = this.moduleModel.getJsonProfileByModel(this.service, this.targetName);
    if (0 !== o.deviceTypes.length || fs_extra_1.existsSync(this.sysCapJsonPath) || this._log.printErrorExit("JSON_PROFILE_MISSING", [this.moduleName, common_const_js_1.CommonConst.SYSCAP_JSON, common_const_js_1.CommonConst.PCID_SC, this.jsonFilePath], [[common_const_js_1.CommonConst.MODULE_JSON5, this.moduleName, common_const_js_1.CommonConst.PCID_SC]]), fs_extra_1.existsSync(this.sysCapJsonPath)) {
      const s = json_util_js_1.JsonUtil.getJson5Obj(this.sysCapJsonPath).devices.general;
      0 === o.deviceTypes.length ? s && 0 !== s.length && this._log.printErrorExit("SYSCAP_FIELD_NOT_EMPTY", [common_const_js_1.CommonConst.DEVICES_GENERAL, common_const_js_1.CommonConst.SYSCAP_JSON, o.deviceConfig, this.moduleName, o.configurationProfile, this.sysCapJsonPath], [[common_const_js_1.CommonConst.DEVICES_GENERAL, common_const_js_1.CommonConst.SYSCAP_JSON, o.deviceConfig, this.moduleName, o.configurationProfile]]) : s && (0, array_util_js_1.checkArrayElementIsSame)(s, o.deviceTypes) || this._log.printErrorExit("DEVICE_TYPE_NOT_INCONSISTENT", [common_const_js_1.CommonConst.DEVICES_GENERAL, this.sysCapJsonPath], [[common_const_js_1.CommonConst.DEVICES_GENERAL, common_const_js_1.CommonConst.SYSCAP_JSON, o.deviceConfig, this.moduleName, o.configurationProfile]]);
    }
  }
  initTaskDepends() {}
}
