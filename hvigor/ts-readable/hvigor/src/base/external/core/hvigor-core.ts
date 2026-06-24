/*
 * Human-readable TypeScript reconstruction.
 * Generated from hvigor/ts by hvigor/tools/reconstruct-readable-ts.cjs.
 * Keep semantic edits in Rust ports or separate hand-refined notes.
 * Source baseline: hvigor/src/base/external/core/hvigor-core.js
 */
import fs_1 from "fs";
import path_1 from "path";
import * as const_js_1 from "../../../common/const/const";
import * as path_const_js_1 from "../../../common/const/path-const";
import * as path_const_js_2 from "../../../common/const/path-const";
import * as tree_node_js_1 from "../../../common/util/tree-node";
import * as extra_config_js_1 from "../../data/extra-config";
import * as parameters_js_1 from "../../data/parameters";
import * as file_util_js_1 from "../../util/file-util";
import * as json5_reader_js_1 from "../../util/json5-reader";
import * as meta_util_js_1 from "../../util/meta-util";
import * as hvigor_node_impl_js_1 from "../../vigor/plugin/impl/hvigor-node-impl";
import * as hvigor_log_js_1 from "../../log/hvigor-log";
const log = hvigor_log_js_1.HvigorLogger.getLogger("hvigor-core");
class HvigorCore {
  constructor() {
    this._hvigorAfterNodeInternalHook = new Map(), this._hvigorNodesEvaluatedInternalHook = [], this._hvigorNodesBeforeEvaluatedInternalHook = [], this.parameter = new parameters_js_1.ExternalParameter(), this.version = const_js_1.CUR_HVIGOR_VERSION, this._project = undefined, this._extraConfig = new Map(), this._scriptMap = new Map(), this.metaInfo = new Map(), this.cacheMap = new Map(), this.compileDependArr = new Set(), this.changedModuleNameList = new Set(), this.moduleDepRelationMap = new Map();
  }
  getHvigorVersion() {
    return this.version;
  }
  getModuleDepRelationMap() {
    return this.moduleDepRelationMap;
  }
  setChangedModuleNameList(e) {
    this.changedModuleNameList = e;
  }
  getChangedModuleNameList() {
    return this.changedModuleNameList;
  }
  setCompileDependArr(e) {
    this.compileDependArr = e;
  }
  getCompileDependArr() {
    return this.compileDependArr;
  }
  versionChanged() {
    const e = (0, meta_util_js_1.getLastHvigorVersion)() !== this.version;
    return e && this.updateMetaProperty("hvigorVersion", this.version), e;
  }
  updateMetaProperty(e, t) {
    this.metaInfo.set(e, t);
  }
  writeMetaProperties() {
    (0, meta_util_js_1.updateMetaProperties)(this.metaInfo), this.metaInfo.clear();
  }
  getMetaProperty(e) {
    return (0, meta_util_js_1.getMetaProperty)(e);
  }
  getProject() {
    return this._project;
  }
  getParameter() {
    return this.parameter;
  }
  getModuleByScriptPath(e) {
    return this._scriptMap.get(e);
  }
  getHvigorNodeByScriptPath(e) {
    var t;
    return null === (t = this._hvigorNodeTree.findNodeByKeyBFS(e)) || undefined === t ? undefined : t.value;
  }
  getParentNodeByScriptPath(e) {
    var t;
    return null === (t = this._hvigorNodeTree.findParentNodeByKey(e)) || undefined === t ? undefined : t.value;
  }
  getSubNodeByScriptPath(e) {
    var t;
    return null === (t = this._hvigorNodeTree.findNodeByKeyBFS(e)) || undefined === t ? undefined : t.children.map(e => e.value);
  }
  getExtraConfig() {
    return this._extraConfig;
  }
  setExtraConfig(e) {
    this._extraConfig = e, (0, extra_config_js_1.setExtraConfig)(this._extraConfig);
  }
  isCommandEntryTask(e) {
    return !!this._commandEntryTasks && this._commandEntryTasks.includes(e);
  }
  setCommandEntryTasks(e) {
    this._commandEntryTasks = e;
  }
  getCommandEntryTasks() {
    return this._commandEntryTasks;
  }
  initRootProject(e) {
    this._project = e, this._scriptMap.set(e.getBuildFilePath(), this._project);
    const t = new hvigor_node_impl_js_1.HvigorNodeImpl(this._project);
    this._hvigorNodeTree = new tree_node_js_1.Tree(e.getBuildFilePath(), t), this.initSubModules(e.getAllSubModules());
  }
  initSubModules(e) {
    e.forEach(e => {
      this._scriptMap.set(e.getBuildFilePath(), e), this._hvigorNodeTree.addNode(e.getBuildFilePath(), new hvigor_node_impl_js_1.HvigorNodeImpl(e), this._project.getBuildFilePath());
    });
  }
  reset() {
    var e, t, o, r;
    const i = new Set();
    i.add((0, file_util_js_1.findRealHvigorFilePath)(null === (e = this._project) || undefined === e ? undefined : e.getBuildFilePath())), null === (t = this._project) || undefined === t || t.getAllSubModules().forEach(e => {
      i.add((0, file_util_js_1.findRealHvigorFilePath)(e.getBuildFilePath()));
    }), i.forEach(e => {
      if (!e) return;
      const t = require(e);
      if (t) for (const e of Object.keys(t)) "object" == typeof t[e] && (t[e] = undefined);
    }), this._extraConfig.clear(), this._scriptMap.clear(), null === (o = this._project) || undefined === o || o.getAllSubModules().forEach(e => {
      e.getTaskContainer().clearTasks(), e.clearTaskGraph();
    }), null === (r = this._project) || undefined === r || r.getTaskContainer().clearTasks(), this._project = undefined, this._commandEntryTasks = undefined;
  }
  hvigorAfterNodeInternalHook(e, t) {
    const o = this._hvigorAfterNodeInternalHook.get(e);
    if (o) o.push(t);else {
      const o = [];
      o.push(t), this._hvigorAfterNodeInternalHook.set(e, o);
    }
  }
  getHvigorAfterNodeInternalHookList(e) {
    return this._hvigorAfterNodeInternalHook.get(e);
  }
  hvigorNodeEvaluatedInternalHook(e) {
    this._hvigorNodesEvaluatedInternalHook.push(e);
  }
  hvigorNodeBeforeEvaluatedInternalHook(e) {
    this._hvigorNodesBeforeEvaluatedInternalHook.push(e);
  }
  getCache(e) {
    return this.cacheMap.has(e) || this.cacheMap.set(e, new Map()), this.cacheMap.get(e);
  }
  cleanCache() {
    this.cacheMap.clear(), this.changedModuleNameList.clear(), this.getModuleDepRelationMap().clear();
  }
  getHvigorNodesEvaluatedInternalHook() {
    return this._hvigorNodesEvaluatedInternalHook;
  }
  getHvigorNodesBeforeEvaluatedInternalHook() {
    return this._hvigorNodesBeforeEvaluatedInternalHook;
  }
  clearHvigorInternalHookList() {
    this._hvigorAfterNodeInternalHook.clear(), this._hvigorNodesEvaluatedInternalHook = [], this._hvigorNodesBeforeEvaluatedInternalHook = [];
  }
}
export function getHvigorNode(e) {
  return e ? hvigorCore.getModuleByScriptPath(e.substring(0, e.lastIndexOf("."))) : hvigorCore.getProject();
}
export function getNode(e) {
  e || hvigorCore.getProject() || log.printErrorExit("ROOT_NODE_NOT_INITIALIZED");
  const t = e ? e.substring(0, e.lastIndexOf(".")) : hvigorCore.getProject().getBuildFilePath();
  return hvigorCore.getHvigorNodeByScriptPath(t);
}
export function getHvigorConfigValue(e) {
  const t = path_1.resolve(path_const_js_1.HVIGOR_PROJECT_WRAPPER_HOME, const_js_1.DEFAULT_HVIGOR_CONFIG_JSON_FILE_NAME);
  if (!fs_1.existsSync(t)) return false;
  const o = json5_reader_js_1.Json5Reader.getJson5Obj(t),
    r = json5_reader_js_1.Json5Reader.getJson5ObjProp(o, e);
  if (o && undefined !== r) return false === r;
  {
    const t = path_1.resolve(path_const_js_2.HVIGOR_USER_HOME, const_js_1.DEFAULT_HVIGOR_CONFIG_JSON_FILE_NAME);
    if (!fs_1.existsSync(t)) return false;
    const o = json5_reader_js_1.Json5Reader.getJson5Obj(t),
      r = json5_reader_js_1.Json5Reader.getJson5ObjProp(o, e);
    if (o && undefined !== r) return false === r;
  }
  return false;
}
export const hvigorCore = new HvigorCore();
