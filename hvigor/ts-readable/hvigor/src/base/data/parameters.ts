/*
 * Human-readable TypeScript reconstruction.
 * Generated from hvigor/ts by hvigor/tools/reconstruct-readable-ts.cjs.
 * Keep semantic edits in Rust ports or separate hand-refined notes.
 * Source baseline: hvigor/src/base/data/parameters.js
 */
import * as wdk_1 from "@baize/wdk";
import * as hvigor_common_1 from "@ohos/hvigor-common";
import * as fs_extra_1 from "fs-extra";
import * as path_const_js_1 from "../../common/const/path-const";
import * as hvigor_trace_factory_js_1 from "../common/trace/hvigor-trace-factory";
import * as global_core_parameters_js_1 from "../internal/data/global-core-parameters";
import * as hvigor_log_js_1 from "../log/hvigor-log";
var __decorate = this && this.__decorate || function (e, r, t, a) {
  var o,
    _ = arguments.length,
    s = _ < 3 ? r : null === a ? a = Object.getOwnPropertyDescriptor(r, t) : a;
  if ("object" == typeof Reflect && "function" == typeof Reflect.decorate) s = Reflect.decorate(e, r, t, a);else for (var l = e.length - 1; l >= 0; l--) (o = e[l]) && (s = (_ < 3 ? o(s) : _ > 3 ? o(r, t, s) : o(r, t)) || s);
  return _ > 3 && s && Object.defineProperty(r, t, s), s;
};
export class ExternalParameter {
  constructor() {
    this.logger = hvigor_log_js_1.HvigorLogger.getLogger(ExternalParameter.name);
  }
  getExtParam(e) {
    var r;
    return null === (r = this._getExtParams()) || undefined === r ? undefined : r[e];
  }
  getExtParamInternal(e) {
    var r;
    return null === (r = this._getExtParams()) || undefined === r ? undefined : r[e];
  }
  getExtParams() {
    return this._getExtParams();
  }
  getExtParamsInternal() {
    return this._getExtParams();
  }
  _getExtParams() {
    return (0, hvigor_common_1.freezeDeep)((0, wdk_1.cloneDeep)(global_core_parameters_js_1.coreParameter.extParams));
  }
  getProperties() {
    return this._getProperties();
  }
  getPropertiesInternal() {
    return this._getProperties();
  }
  _getProperties() {
    return (0, hvigor_common_1.freezeDeep)((0, wdk_1.cloneDeep)(global_core_parameters_js_1.coreParameter.properties));
  }
  getProperty(e) {
    return this._getProperty(e);
  }
  getPropertyInternal(e) {
    return this._getProperty(e);
  }
  _getProperty(e) {
    var r;
    return null === (r = this._getProperties()) || undefined === r ? undefined : r[e];
  }
  setProperty(e, r) {
    return this._setProperty(e, r);
  }
  setPropertyInternal(e, r) {
    return this._setProperty(e, r);
  }
  _setProperty(e, r) {
    this.propertiesSchema || (this.propertiesSchema = (0, fs_extra_1.readJsonSync)(path_const_js_1.HVIGOR_CONFIG_SCHEMA_PATH).properties.properties);
    const t = `${path_const_js_1.HVIGOR_CONFIG_SCHEMA_PATH}.properties.properties`,
      {
        isValid: a,
        errMsg: o
      } = (0, hvigor_common_1.validateDataBySchema)({
        [e]: r
      }, t, this.propertiesSchema);
    let _ = this.getCallerPath();
    a || this.logger.printErrorExitWithoutStack("METHID_SETPROPERTY_VALIDATE_FAILED_IN_HVIGORFILE", [_], [[o]]), global_core_parameters_js_1.coreParameter.properties[e] = r;
  }
  getCallerPath() {
    const e = new Error().stack;
    if (!e) return "";
    const r = e.split("\n");
    if (!r) return "";
    const t = r.find(e => e.trim().startsWith("at plugin"));
    if (t) {
      const e = t.match(/$(.*?):\d+:\d+$$/);
      if (e && e[1]) return `At file: ${e[1]}`;
    }
    return "";
  }
  getStartParams() {
    return this._getStartParams();
  }
  getStartParamsInternal() {
    return this._getStartParams();
  }
  _getStartParams() {
    return (0, hvigor_common_1.freezeDeep)((0, wdk_1.cloneDeep)({
      daemon: global_core_parameters_js_1.coreParameter.startParams.daemon,
      analyze: global_core_parameters_js_1.AnalyzeModeKeyMap.get(global_core_parameters_js_1.coreParameter.startParams.analyze),
      parallel: global_core_parameters_js_1.coreParameter.startParams.parallelExecution,
      incremental: global_core_parameters_js_1.coreParameter.startParams.incrementalExecution,
      logLevel: global_core_parameters_js_1.coreParameter.startParams.logLevel.levelStr.toLowerCase(),
      typeCheck: global_core_parameters_js_1.coreParameter.startParams.hvigorfileTypeCheck,
      optimizationStrategy: global_core_parameters_js_1.coreParameter.startParams.optimizationStrategy,
      hotCompile: global_core_parameters_js_1.coreParameter.startParams.hotCompile,
      hotReloadBuild: global_core_parameters_js_1.coreParameter.startParams.hotReloadBuild
    }));
  }
  getWorkspaceDir() {
    return this._getWorkspaceDir();
  }
  getWorkspaceDirInternal() {
    return this._getWorkspaceDir();
  }
  _getWorkspaceDir() {
    return (0, hvigor_common_1.freezeDeep)((0, wdk_1.cloneDeep)(global_core_parameters_js_1.coreParameter.workspaceDir));
  }
}
__decorate([hvigor_trace_factory_js_1.TrackAPI], ExternalParameter.prototype, "getExtParam", null), __decorate([hvigor_trace_factory_js_1.TrackAPI], ExternalParameter.prototype, "getExtParams", null), __decorate([hvigor_trace_factory_js_1.TrackAPI], ExternalParameter.prototype, "getProperties", null), __decorate([hvigor_trace_factory_js_1.TrackAPI], ExternalParameter.prototype, "getProperty", null), __decorate([hvigor_trace_factory_js_1.TrackAPI], ExternalParameter.prototype, "setProperty", null), __decorate([hvigor_trace_factory_js_1.TrackAPI], ExternalParameter.prototype, "getStartParams", null), __decorate([hvigor_trace_factory_js_1.TrackAPI], ExternalParameter.prototype, "getWorkspaceDir", null);
