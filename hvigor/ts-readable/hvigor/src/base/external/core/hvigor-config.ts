/*
 * Human-readable TypeScript reconstruction.
 * Generated from hvigor/ts by hvigor/tools/reconstruct-readable-ts.cjs.
 * Keep semantic edits in Rust ports or separate hand-refined notes.
 * Source baseline: hvigor/src/base/external/core/hvigor-config.js
 */
import fs_1 from "fs";
import path_1 from "path";
import * as node_config_path_info_js_1 from "../../../common/node-config-path-info";
import * as common_const_js_1 from "../../common/options/common-const";
import * as hvigor_trace_factory_js_1 from "../../common/trace/hvigor-trace-factory";
import * as hvigor_log_js_1 from "../../log/hvigor-log";
import * as json5_reader_js_1 from "../../util/json5-reader";
import * as validate_util_js_1 from "../../util/validate-util";
import * as hvigor_node_descriptor_impl_js_1 from "../../vigor/plugin/impl/hvigor-node-descriptor-impl";
var __decorate = this && this.__decorate || function (o, e, r, t) {
  var i,
    s = arguments.length,
    n = s < 3 ? e : null === t ? t = Object.getOwnPropertyDescriptor(e, r) : t;
  if ("object" == typeof Reflect && "function" == typeof Reflect.decorate) n = Reflect.decorate(o, e, r, t);else for (var _ = o.length - 1; _ >= 0; _--) (i = o[_]) && (n = (s < 3 ? i(n) : s > 3 ? i(e, r, n) : i(e, r)) || n);
  return s > 3 && n && Object.defineProperty(e, r, n), n;
};
const log = hvigor_log_js_1.HvigorLogger.getLogger("hvigor-config");
export class HvigorConfig {
  constructor(o) {
    const e = path_1.resolve(o.getNodeDir(), common_const_js_1.HvigorCommonConst.BUILD_PROFILE_FILE),
      r = json5_reader_js_1.Json5Reader.getJson5Obj(e).modules;
    checkProjectConfigFile(o, r), this.allChildrenNodeDescriptor = [], r.forEach(o => {
      validate_util_js_1.ValidateUtil.validateModule(o);
      const e = new hvigor_node_descriptor_impl_js_1.HvigorNodeDescriptorImpl(o.name, o.srcPath, undefined, this.rootNodeDescriptor);
      this.allChildrenNodeDescriptor.push(e);
    }), this.rootNodeDescriptor = new hvigor_node_descriptor_impl_js_1.HvigorNodeDescriptorImpl(o.getName(), o.getNodeDir(), this.allChildrenNodeDescriptor, undefined);
  }
  includeNode(o, e, r) {
    this._includeNode(o, e, r);
  }
  includeNodeInternal(o, e, r) {
    this._includeNode(o, e, r);
  }
  _includeNode(o, e, r) {
    fs_1.existsSync(e) || log.printErrorExit("SOME_PATH_NOT_FOUND", [e], [[e]]), path_1.isAbsolute(e) && log.printErrorExit("THE_SRCPATH_IS_NOT_A_RELATIVE_PATH", [e]);
    const t = new hvigor_node_descriptor_impl_js_1.HvigorNodeDescriptorImpl(o, e, undefined, this.rootNodeDescriptor);
    for (const o in r) Object.prototype.hasOwnProperty.call(r, o) && t.extraOptions.set(o, r[o]);
    node_config_path_info_js_1.nodeConfigPathInfo.addModuleConfigPath(o, this.rootNodeDescriptor.srcPath), this.allChildrenNodeDescriptor.push(t), this.rootNodeDescriptor.setChildNode(this.allChildrenNodeDescriptor);
  }
  excludeNodeByName(o) {
    this._excludeNodeByName(o);
  }
  excludeNodeByNameInternal(o) {
    this._excludeNodeByName(o);
  }
  _excludeNodeByName(o) {
    this.allChildrenNodeDescriptor.find(e => e.name === o) || log.warn(`The Node to exclude '${o}' was not found.`), this.allChildrenNodeDescriptor = this.allChildrenNodeDescriptor.filter(e => e.name !== o), this.rootNodeDescriptor.setChildNode(this.allChildrenNodeDescriptor), node_config_path_info_js_1.nodeConfigPathInfo.deleteModuleConfigPath(o);
  }
  getAllNodeDescriptor() {
    return this._getAllNodeDescriptor();
  }
  getAllNodeDescriptorInternal() {
    return this._getAllNodeDescriptor();
  }
  _getAllNodeDescriptor() {
    return this.allChildrenNodeDescriptor;
  }
  getRootNodeDescriptor() {
    return this._getRootNodeDescriptor();
  }
  getRootNodeDescriptorInternal() {
    return this._getRootNodeDescriptor();
  }
  _getRootNodeDescriptor() {
    return this.rootNodeDescriptor;
  }
  getNodeDescriptorByName(o) {
    return this._getNodeDescriptorByName(o);
  }
  getNodeDescriptorByNameInternal(o) {
    return this._getNodeDescriptorByName(o);
  }
  _getNodeDescriptorByName(o) {
    return this.allChildrenNodeDescriptor.find(e => e.name === o);
  }
}
function checkProjectConfigFile(o, e) {
  Array.isArray(e) || log.printErrorExit("CHECK_PROJECT_CONFIG_ERROR", [common_const_js_1.HvigorCommonConst.BUILD_PROFILE_FILE, path_1.resolve(o.getNodeDir(), common_const_js_1.HvigorCommonConst.BUILD_PROFILE_FILE)]), e && 0 !== e.length || log.printErrorExit("HVIGOR_CONFIG_NO_MODULES_FOUND", [path_1.resolve(o.getBuildProfilePath())], [[common_const_js_1.HvigorCommonConst.BUILD_PROFILE_FILE]]);
}
export function hvigorConfigInit(o) {
  hvigorConfig = new HvigorConfig(o);
}
__decorate([hvigor_trace_factory_js_1.TrackAPI], HvigorConfig.prototype, "includeNode", null), __decorate([hvigor_trace_factory_js_1.TrackAPI], HvigorConfig.prototype, "excludeNodeByName", null), __decorate([hvigor_trace_factory_js_1.TrackAPI], HvigorConfig.prototype, "getAllNodeDescriptor", null), __decorate([hvigor_trace_factory_js_1.TrackAPI], HvigorConfig.prototype, "getRootNodeDescriptor", null), __decorate([hvigor_trace_factory_js_1.TrackAPI], HvigorConfig.prototype, "getNodeDescriptorByName", null);
export { hvigorConfig };
