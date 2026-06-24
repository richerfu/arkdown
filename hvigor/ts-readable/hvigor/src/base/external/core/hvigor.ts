/*
 * Human-readable TypeScript reconstruction.
 * Generated from hvigor/ts by hvigor/tools/reconstruct-readable-ts.cjs.
 * Keep semantic edits in Rust ports or separate hand-refined notes.
 * Source baseline: hvigor/src/base/external/core/hvigor.js
 */
import * as hvigor_trace_factory_js_1 from "../../common/trace/hvigor-trace-factory";
import * as hvigor_lifecycle_hook_js_1 from "../../internal/lifecycle/hook/hvigor-lifecycle-hook";
import * as hvigor_log_js_1 from "../../log/hvigor-log";
import * as hvigor_config_js_1 from "./hvigor-config";
import * as hvigor_core_js_1 from "./hvigor-core";
import * as hvigor_user_home_js_1 from "../../../common/util/hvigor-user-home";
var __decorate = this && this.__decorate || function (e, r, o, t) {
  var i,
    a = arguments.length,
    _ = a < 3 ? r : null === t ? t = Object.getOwnPropertyDescriptor(r, o) : t;
  if ("object" == typeof Reflect && "function" == typeof Reflect.decorate) _ = Reflect.decorate(e, r, o, t);else for (var g = e.length - 1; g >= 0; g--) (i = e[g]) && (_ = (a < 3 ? i(_) : a > 3 ? i(r, o, _) : i(r, o)) || _);
  return a > 3 && _ && Object.defineProperty(r, o, _), _;
};
const _log = hvigor_log_js_1.HvigorLogger.getLogger("hvigor");
export class Hvigor {
  constructor() {
    this.hvigorLifecycleHook = hvigor_lifecycle_hook_js_1.HvigorLifecycleHook.getInstance();
  }
  getRootNode() {
    return this._getRootNode();
  }
  getRootNodeInternal() {
    return this._getRootNode();
  }
  _getRootNode() {
    var e;
    const r = null === (e = hvigor_core_js_1.hvigorCore.getProject()) || undefined === e ? undefined : e.getBuildFilePath();
    if (undefined === r) throw _log.printErrorExit("HVIGOR_CONFIG_FILE_ERROR", ["hvigorfile.ts or hvigorconfig.ts"]), new Error("The root node is not yet available for build.");
    const o = hvigor_core_js_1.hvigorCore.getHvigorNodeByScriptPath(r);
    if (undefined === o) throw _log.printErrorExit("HVIGOR_CONFIG_FILE_ERROR", [r]), new Error("The root node is not yet available for build.");
    return o;
  }
  getAllNodes() {
    return this._getAllNodes();
  }
  getAllNodesInternal() {
    return this._getAllNodes();
  }
  _getAllNodes() {
    var e;
    const r = this._getRootNode(),
      o = null === (e = hvigor_core_js_1.hvigorCore.getProject()) || undefined === e ? undefined : e.getBuildFilePath();
    if (undefined === o) throw _log.printErrorExit("HVIGOR_CONFIG_FILE_ERROR", ["hvigorfile.ts or hvigorconfig.ts"]), new Error("rootNode not found.");
    const t = hvigor_core_js_1.hvigorCore.getSubNodeByScriptPath(o);
    return undefined === t ? [r] : (t.push(r), t);
  }
  getNodeByName(e, r) {
    return this._getNodeByName(e, r);
  }
  getNodeByNameInternal(e, r) {
    return this._getNodeByName(e, r);
  }
  _getNodeByName(e, r) {
    return r ? hvigor.getAllNodesInternal().find(o => {
      const t = o.getNodeNameInternal() === e,
        i = undefined === r || o.classKind === r;
      return t && i;
    }) : this._getAllNodes().find(r => r.getNodeNameInternal() === e);
  }
  getHvigorConfig() {
    return this._getHvigorConfig();
  }
  getHvigorConfigInternal() {
    return this._getHvigorConfig();
  }
  _getHvigorConfig() {
    return hvigor_config_js_1.hvigorConfig || _log.printErrorExit("HVIGOR_CONFIG_NOT_AVAILABLE"), hvigor_config_js_1.hvigorConfig;
  }
  getParameter() {
    return this._getParameter();
  }
  getParameterInternal() {
    return this._getParameter();
  }
  _getParameter() {
    return hvigor_core_js_1.hvigorCore.getParameter();
  }
  beforeNodeEvaluate(e) {
    this._beforeNodeEvaluate(e);
  }
  beforeNodeEvaluateInternal(e) {
    this._beforeNodeEvaluate(e);
  }
  _beforeNodeEvaluate(e) {
    this.hvigorLifecycleHook.beforeNodeEvaluate(e);
  }
  afterNodeEvaluate(e) {
    this._afterNodeEvaluate(e);
  }
  afterNodeEvaluateInternal(e) {
    this._afterNodeEvaluate(e);
  }
  _afterNodeEvaluate(e) {
    this.hvigorLifecycleHook.afterNodeEvaluate(e);
  }
  configEvaluated(e) {
    this._configEvaluated(e);
  }
  configEvaluatedInternal(e) {
    this._configEvaluated(e);
  }
  _configEvaluated(e) {
    this.hvigorLifecycleHook.configEvaluated(e);
  }
  nodesInitialized(e) {
    this._nodesInitialized(e);
  }
  nodesInitializedInternal(e) {
    this._nodesInitialized(e);
  }
  _nodesInitialized(e) {
    this.hvigorLifecycleHook.nodesInitialized(e);
  }
  nodesEvaluated(e) {
    this._nodesEvaluated(e);
  }
  nodesEvaluatedInternal(e) {
    this._nodesEvaluated(e);
  }
  _nodesEvaluated(e) {
    this.hvigorLifecycleHook.nodesEvaluated(e);
  }
  taskGraphResolved(e) {
    this._taskGraphResolved(e);
  }
  taskGraphResolvedInternal(e) {
    this._taskGraphResolved(e);
  }
  _taskGraphResolved(e) {
    this.hvigorLifecycleHook.taskGraphResolved(e);
  }
  buildFinished(e) {
    this._buildFinished(e);
  }
  buildFinishedInternal(e) {
    this._buildFinished(e);
  }
  _buildFinished(e) {
    this.hvigorLifecycleHook.buildFinished(e);
  }
  onWatchWorkerMessage(e) {
    this._onWatchWorkerMessage(e);
  }
  onWatchWorkerMessageInternal(e) {
    this._onWatchWorkerMessage(e);
  }
  _onWatchWorkerMessage(e) {
    this.hvigorLifecycleHook.onWatchWorkerMessage(e);
  }
  getCommandEntryTask() {
    return this._getCommandEntryTask();
  }
  getCommandEntryTaskInternal() {
    return this._getCommandEntryTask();
  }
  _getCommandEntryTask() {
    return hvigor_core_js_1.hvigorCore.getCommandEntryTasks();
  }
  isCommandEntryTask(e) {
    return this._isCommandEntryTask(e);
  }
  isCommandEntryTaskInternal(e) {
    return this._isCommandEntryTask(e);
  }
  _isCommandEntryTask(e) {
    return hvigor_core_js_1.hvigorCore.isCommandEntryTask(e);
  }
  getExtraConfig() {
    return this._getExtraConfig();
  }
  getExtraConfigInternal() {
    return this._getExtraConfig();
  }
  _getExtraConfig() {
    return hvigor_core_js_1.hvigorCore.getExtraConfig();
  }
  getProject() {
    return this._getProject();
  }
  getProjectInternal() {
    return this._getProject();
  }
  _getProject() {
    return hvigor_core_js_1.hvigorCore.getProject();
  }
  getHvigorUserHome() {
    return this._getHvigorUserHome();
  }
  getHvigorUserHomeInternal() {
    return this._getHvigorUserHome();
  }
  _getHvigorUserHome() {
    return (0, hvigor_user_home_js_1.getHvigorUserHomeCacheDir)();
  }
  getHvigorVersion() {
    return this._getHvigorVersion();
  }
  getHvigorVersionInternal() {
    return this._getHvigorVersion();
  }
  _getHvigorVersion() {
    return hvigor_core_js_1.hvigorCore.getHvigorVersion();
  }
}
export const hvigor = new Hvigor();
__decorate([hvigor_trace_factory_js_1.TrackAPI], Hvigor.prototype, "getRootNode", null), __decorate([hvigor_trace_factory_js_1.TrackAPI], Hvigor.prototype, "getAllNodes", null), __decorate([hvigor_trace_factory_js_1.TrackAPI], Hvigor.prototype, "getNodeByName", null), __decorate([hvigor_trace_factory_js_1.TrackAPI], Hvigor.prototype, "getHvigorConfig", null), __decorate([hvigor_trace_factory_js_1.TrackAPI], Hvigor.prototype, "getParameter", null), __decorate([hvigor_trace_factory_js_1.TrackAPI], Hvigor.prototype, "beforeNodeEvaluate", null), __decorate([hvigor_trace_factory_js_1.TrackAPI], Hvigor.prototype, "afterNodeEvaluate", null), __decorate([hvigor_trace_factory_js_1.TrackAPI], Hvigor.prototype, "configEvaluated", null), __decorate([hvigor_trace_factory_js_1.TrackAPI], Hvigor.prototype, "nodesInitialized", null), __decorate([hvigor_trace_factory_js_1.TrackAPI], Hvigor.prototype, "nodesEvaluated", null), __decorate([hvigor_trace_factory_js_1.TrackAPI], Hvigor.prototype, "taskGraphResolved", null), __decorate([hvigor_trace_factory_js_1.TrackAPI], Hvigor.prototype, "buildFinished", null), __decorate([hvigor_trace_factory_js_1.TrackAPI], Hvigor.prototype, "onWatchWorkerMessage", null), __decorate([hvigor_trace_factory_js_1.TrackAPI], Hvigor.prototype, "getCommandEntryTask", null), __decorate([hvigor_trace_factory_js_1.TrackAPI], Hvigor.prototype, "isCommandEntryTask", null), __decorate([hvigor_trace_factory_js_1.TrackAPI], Hvigor.prototype, "getExtraConfig", null), __decorate([hvigor_trace_factory_js_1.TrackAPI], Hvigor.prototype, "getProject", null), __decorate([hvigor_trace_factory_js_1.TrackAPI], Hvigor.prototype, "getHvigorUserHome", null), __decorate([hvigor_trace_factory_js_1.TrackAPI], Hvigor.prototype, "getHvigorVersion", null);
