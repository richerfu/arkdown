/*
 * Human-readable TypeScript reconstruction.
 * Generated from hvigor/ts by hvigor/tools/reconstruct-readable-ts.cjs.
 * Keep semantic edits in Rust ports or separate hand-refined notes.
 * Source baseline: hvigor/src/base/vigor/plugin/impl/hvigor-node-impl.js
 */
import * as hvigor_file_util_1 from "../../../../common/util/hvigor-file-util";
import * as hvigor_trace_factory_js_1 from "../../../common/trace/hvigor-trace-factory";
import * as hvigor_core_js_1 from "../../../external/core/hvigor-core";
import * as hvigor_log_js_1 from "../../../log/hvigor-log";
import * as task_impl_js_1 from "../../task/impl/task-impl";
var __decorate = this && this.__decorate || function (e, t, r, o) {
  var a,
    i = arguments.length,
    _ = i < 3 ? t : null === o ? o = Object.getOwnPropertyDescriptor(t, r) : o;
  if ("object" == typeof Reflect && "function" == typeof Reflect.decorate) _ = Reflect.decorate(e, t, r, o);else for (var n = e.length - 1; n >= 0; n--) (a = e[n]) && (_ = (i < 3 ? a(_) : i > 3 ? a(t, r, _) : a(t, r)) || _);
  return i > 3 && _ && Object.defineProperty(t, r, _), _;
};
export class HvigorNodeImpl {
  constructor(e) {
    this.logger = hvigor_log_js_1.HvigorLogger.getLogger(HvigorNodeImpl.name), this.allSubNodes = [], this.node = e, this.nodeDir = new hvigor_file_util_1.NormalizedFile(e.getNodeDir()), this.classKind = e.classKind, Object.freeze(this.nodeDir);
  }
  getNodeName() {
    return this._getNodeName();
  }
  getNodeNameInternal() {
    return this._getNodeName();
  }
  _getNodeName() {
    return this.node.getName();
  }
  getNodePath() {
    return this._getNodePath();
  }
  getNodePathInternal() {
    return this._getNodePath();
  }
  _getNodePath() {
    return this.node.getNodeDir();
  }
  getParentNode() {
    return this._getParentNode();
  }
  getParentNodeInternal() {
    return this._getParentNode();
  }
  _getParentNode() {
    return this._loadParentNode(), this.parentNode;
  }
  getContext(e) {
    return this._getContext(e);
  }
  getContextInternal(e) {
    return this._getContext(e);
  }
  _getContext(e) {
    return this.node.getContext(e);
  }
  getAllPluginIds() {
    return this._getAllPluginIds();
  }
  getAllPluginIdsInternal() {
    return this._getAllPluginIds();
  }
  _getAllPluginIds() {
    return this.node.getAllPluginIds();
  }
  getSubNodeByName(e) {
    return this._getSubNodeByName(e);
  }
  getSubNodeByNameInternal(e) {
    return this._getSubNodeByName(e);
  }
  _getSubNodeByName(e) {
    return this._loadAllSubNodes(), this.allSubNodes.find(t => t.getNodeNameInternal() === e);
  }
  getTaskByName(e) {
    return this._getTaskByName(e);
  }
  getTaskByNameInternal(e) {
    return this._getTaskByName(e);
  }
  _getTaskByName(e) {
    const t = this.node.getTaskByName(e);
    if (t) return t.getHvigorTask();
    this.logger.warn(`The task '${e}' cannot be found on the current node.`);
  }
  registerTask(e) {
    return this._registerTask(e);
  }
  registerTaskInternal(e) {
    return this._registerTask(e);
  }
  _registerTask(e) {
    return this.node.registerTask(e);
  }
  subNodes(e) {
    this._subNodes(e);
  }
  subNodesInternal(e) {
    this._subNodes(e);
  }
  _subNodes(e) {
    this._loadAllSubNodes(), this.allSubNodes.forEach(t => e(t));
  }
  getNodeDir() {
    return this._getNodeDir();
  }
  getNodeDirInternal() {
    return this._getNodeDir();
  }
  _getNodeDir() {
    return this.nodeDir;
  }
  getConfigOpt() {
    return this._getConfigOpt();
  }
  getConfigOptInternal() {
    return this._getConfigOpt();
  }
  _getConfigOpt() {
    return this.node.getConfigOpt();
  }
  addExtraOption(e, t) {
    this._addExtraOption(e, t);
  }
  addExtraOptionInternal(e, t) {
    this._addExtraOption(e, t);
  }
  _addExtraOption(e, t) {
    this.node.addExtraOption(e, t);
  }
  getExtraOption(e) {
    return this._getExtraOption(e);
  }
  getExtraOptionInternal(e) {
    return this._getExtraOption(e);
  }
  _getExtraOption(e) {
    return this.node.getExtraOption(e);
  }
  beforeNodeEvaluate(e) {
    this._beforeNodeEvaluate(e);
  }
  beforeNodeEvaluateInternal(e) {
    this._beforeNodeEvaluate(e);
  }
  _beforeNodeEvaluate(e) {
    this.node.beforeNodeEvaluate(e);
  }
  afterNodeEvaluate(e) {
    this._afterNodeEvaluate(e);
  }
  afterNodeEvaluateInternal(e) {
    this._afterNodeEvaluate(e);
  }
  _afterNodeEvaluate(e) {
    this.node.afterNodeEvaluate(e);
  }
  getAllTasks() {
    return this._getAllTasks();
  }
  getAllTasksInternal() {
    return this._getAllTasks();
  }
  _getAllTasks() {
    return this.node.getAllTasks().map(e => new task_impl_js_1.TaskImpl(e));
  }
  _loadParentNode() {
    this.parentNode = hvigor_core_js_1.hvigorCore.getParentNodeByScriptPath(this.node.getBuildFilePath());
  }
  _loadAllSubNodes() {
    var e;
    this.allSubNodes = null !== (e = hvigor_core_js_1.hvigorCore.getSubNodeByScriptPath(this.node.getBuildFilePath())) && undefined !== e ? e : [];
  }
}
__decorate([hvigor_trace_factory_js_1.TrackAPI], HvigorNodeImpl.prototype, "getNodeName", null), __decorate([hvigor_trace_factory_js_1.TrackAPI], HvigorNodeImpl.prototype, "getNodePath", null), __decorate([hvigor_trace_factory_js_1.TrackAPI], HvigorNodeImpl.prototype, "getParentNode", null), __decorate([hvigor_trace_factory_js_1.TrackAPI], HvigorNodeImpl.prototype, "getContext", null), __decorate([hvigor_trace_factory_js_1.TrackAPI], HvigorNodeImpl.prototype, "getAllPluginIds", null), __decorate([hvigor_trace_factory_js_1.TrackAPI], HvigorNodeImpl.prototype, "getSubNodeByName", null), __decorate([hvigor_trace_factory_js_1.TrackAPI], HvigorNodeImpl.prototype, "getTaskByName", null), __decorate([hvigor_trace_factory_js_1.TrackAPI], HvigorNodeImpl.prototype, "registerTask", null), __decorate([hvigor_trace_factory_js_1.TrackAPI], HvigorNodeImpl.prototype, "subNodes", null), __decorate([hvigor_trace_factory_js_1.TrackAPI], HvigorNodeImpl.prototype, "getNodeDir", null), __decorate([hvigor_trace_factory_js_1.TrackAPI], HvigorNodeImpl.prototype, "getConfigOpt", null), __decorate([hvigor_trace_factory_js_1.TrackAPI], HvigorNodeImpl.prototype, "addExtraOption", null), __decorate([hvigor_trace_factory_js_1.TrackAPI], HvigorNodeImpl.prototype, "getExtraOption", null), __decorate([hvigor_trace_factory_js_1.TrackAPI], HvigorNodeImpl.prototype, "beforeNodeEvaluate", null), __decorate([hvigor_trace_factory_js_1.TrackAPI], HvigorNodeImpl.prototype, "afterNodeEvaluate", null), __decorate([hvigor_trace_factory_js_1.TrackAPI], HvigorNodeImpl.prototype, "getAllTasks", null);
