/*
 * Human-readable TypeScript reconstruction.
 * Generated from hvigor/ts by hvigor/tools/reconstruct-readable-ts.cjs.
 * Keep semantic edits in Rust ports or separate hand-refined notes.
 * Source baseline: hvigor-ohos-plugin/src/tasks/abstract/abstract-syscap-transform.js
 */
import * as wdk_1 from "@baize/wdk";
import * as hvigor_1 from "@ohos/hvigor";
import fs_1 from "fs";
import fs_extra_1 from "fs-extra";
import path_1 from "path";
import * as common_const_js_1 from "../../const/common-const";
import * as array_util_js_1 from "../../utils/array-util";
import * as file_util_js_1 from "../../utils/file-util";
import * as json_util_js_1 from "../../utils/json-util";
import * as ohos_logger_js_1 from "../../utils/log/ohos-logger";
import * as process_utils_js_1 from "../../utils/process-utils";
import * as pre_check_syscap_js_1 from "../pre-check-syscap";
import * as ohos_hap_task_js_1 from "../task/ohos-hap-task";
var __decorate = this && this.__decorate || function (t, e, s, o) {
  var r,
    a = arguments.length,
    n = a < 3 ? e : null === o ? o = Object.getOwnPropertyDescriptor(e, s) : o;
  if ("object" == typeof Reflect && "function" == typeof Reflect.decorate) n = Reflect.decorate(t, e, s, o);else for (var i = t.length - 1; i >= 0; i--) (r = t[i]) && (n = (a < 3 ? r(n) : a > 3 ? r(e, s, n) : r(e, s)) || n);
  return a > 3 && n && Object.defineProperty(e, s, n), n;
};
export class AbstractSyscapTransform extends ohos_hap_task_js_1.OhosHapTask {
  get sysCapTool() {
    return this.sdkInfo.getSysCapTool();
  }
  get sysCapPath() {
    return fs_1.existsSync(this.sdkInfo.getSdkJsDir()) && !fs_1.existsSync(this.sdkInfo.getSdkEtsDir()) ? this.sdkInfo.getSysCapFileInJs() : this.sdkInfo.getSysCapFileInEts();
  }
  get deviceTypes() {
    return this.getJsonProfileByModel().deviceTypes;
  }
  constructor(t, e) {
    var s;
    super(t, e), this._log = ohos_logger_js_1.OhosLogger.getLogger(AbstractSyscapTransform.name);
    const o = t.getTargetData().getTargetName();
    this.sourceRoot = null === (s = this.service.getModuleModel()) || undefined === s ? undefined : s.getSourceRootByTargetName(o), this.pathInfo = this.targetService.getTargetData().getPathInfo(), this.sysCapJsonPath = path_1.resolve(this.sourceRoot, common_const_js_1.CommonConst.SYSCAP_JSON), this.rpcidScPath = path_1.resolve(this.pathInfo.getIntermediatesSysCap(), common_const_js_1.CommonConst.RPCID_SC);
  }
  initTaskDepends() {
    this.declareDepends(pre_check_syscap_js_1.PreCheckSyscap.name);
  }
  async doTaskAction() {
    if (this.targetData.isHarmonyOS()) return;
    const t = "generate SysCap transform command",
      e = this.durationEvent.createSubEvent(t, "");
    e.start();
    const s = this.getJsonProfileByModel();
    let o = new Set();
    const r = s.deviceTypes,
      a = this.getIntersectedGeneralSysCapSet(r);
    if (fs_extra_1.existsSync(this.sysCapJsonPath)) {
      const t = json_util_js_1.JsonUtil.getJson5Obj(this.sysCapJsonPath),
        e = t.devices.custom,
        s = this.getIntersectedCustomSysCapSet(e);
      0 === r.length ? (this.checkCustomSysCapList(e), o = s) : o = e && 0 !== e.length ? (0, array_util_js_1.getIntersectedSet)(s, a) : a;
      const n = t.production;
      n && this.processProductionSysCap(n, o);
    }
    if (fs_extra_1.existsSync(this.sysCapJsonPath) || (o = a), this.moduleModel.isHarModule()) return e.stop(), void e.setLog(t, hvigor_1.MetricLogType.INFO);
    if (!(null == o ? undefined : o.size)) {
      const t = this.isFaMode ? this.targetData.getModuleSourceSetModel().getLegacyModuleTargetRes().getJsonPath() : this.targetData.getModuleSourceSetModel().getModuleTargetRes().getJsonPath();
      this._log.printErrorExit("INVALID_CUSTOM_SYSCAP", [fs_extra_1.existsSync(this.sysCapJsonPath) ? this.sysCapJsonPath : t], [[common_const_js_1.CommonConst.SYSCAP_JSON, this.moduleName, common_const_js_1.CommonConst.MODULE_JSON5, common_const_js_1.CommonConst.CONFIG_JSON]]);
    }
    const n = new Map();
    n.set(common_const_js_1.CommonConst.API_VERSION, this.compileApiVersion), n.set(common_const_js_1.CommonConst.SYSTEM_CAPABILITY, Array.from(o));
    const i = this.pathInfo.getIntermediatesSysCap();
    file_util_js_1.FileUtil.checkDirWithoutDelete(i);
    const c = path_1.resolve(i, common_const_js_1.CommonConst.RPCID_JSON);
    fs_extra_1.outputJsonSync(c, Object.fromEntries(n));
    const _ = {
      moduleName: this.moduleName,
      taskName: this.name,
      commandLine: [this.sysCapTool, "-R", "-e", "-i", c, "-o", i]
    };
    e.stop(), e.setLog(t, hvigor_1.MetricLogType.INFO);
    const l = "submit SysCap transform task to work pool",
      u = this.durationEvent.createSubEvent(l, "");
    u.start(), await new process_utils_js_1.ProcessUtils(_.moduleName, _.taskName).submitExecutionWithoutReturn(this, this.getWorkerPool(), _, wdk_1.noop, [], u), u.stop(), u.setLog(l, hvigor_1.MetricLogType.INFO);
  }
  getIntersectedGeneralSysCapSet(t) {
    var e;
    if (!(null == t ? undefined : t.length)) return new Set();
    if (1 === t.length) return null !== (e = this.getSysCapSetFromSdk(t[0], this.sysCapPath)) && undefined !== e ? e : new Set();
    const s = Array.from(t.map(t => {
      var e;
      return Array.from(null !== (e = this.getSysCapSetFromSdk(t, this.sysCapPath)) && undefined !== e ? e : new Set());
    }).reduce((t, e) => t.filter(t => e.includes(t))));
    return new Set(s);
  }
  getSysCapSetFromSdk(t, e) {
    const s = path_1.resolve(e, `${t}.json`);
    if (!fs_extra_1.existsSync(s)) return void this._log.warn(`The systemCapability set corresponding to the '${t}' is not found in the '${e}'.`);
    const o = new Set();
    return json_util_js_1.JsonUtil.getJson5Obj(s).SysCaps.forEach(t => {
      o.add(t);
    }), o;
  }
  getIntersectedCustomSysCapSet(t) {
    if (!(null == t ? undefined : t.length) || t.some(t => "{}" === JSON.stringify(t))) return new Set();
    const e = [];
    return t.forEach(t => {
      Object.keys(t).forEach(s => {
        e.push(t[s]);
      });
    }), new Set((0, wdk_1.intersection)(...e));
  }
  processProductionSysCap(t, e) {
    const s = t.addedSysCaps;
    s && s.forEach(t => {
      e.add(t);
    });
    const o = t.removedSysCaps;
    o && o.forEach(t => {
      e.delete(t);
    });
  }
  checkCustomSysCapList(t) {
    t && 0 !== t.length || this._log.printErrorExit("CUSTOM_SYSCAP_IN_EMPTY_DEVICE_TYPES", [common_const_js_1.CommonConst.MODULE_JSON5, this.moduleName, common_const_js_1.CommonConst.SYSCAP_JSON, this.sysCapJsonPath], [[common_const_js_1.CommonConst.MODULE_JSON5, common_const_js_1.CommonConst.SYSCAP_JSON, this.moduleName]]);
  }
}
__decorate([(0, hvigor_1.InputFile)()], AbstractSyscapTransform.prototype, "sysCapJsonPath", undefined), __decorate([(0, hvigor_1.OutputFile)()], AbstractSyscapTransform.prototype, "rpcidScPath", undefined), __decorate([(0, hvigor_1.InputFile)()], AbstractSyscapTransform.prototype, "sysCapTool", null), __decorate([(0, hvigor_1.InputFile)()], AbstractSyscapTransform.prototype, "sysCapPath", null), __decorate([(0, hvigor_1.Input)()], AbstractSyscapTransform.prototype, "deviceTypes", null);
