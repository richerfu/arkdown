/*
 * Human-readable TypeScript reconstruction.
 * Generated from hvigor/ts by hvigor/tools/reconstruct-readable-ts.cjs.
 * Keep semantic edits in Rust ports or separate hand-refined notes.
 * Source baseline: hvigor/src/common/node-config-path-info.js
 */
import * as json5_reader_js_1 from "../base/util/json5-reader";
import path_1 from "path";
class NodeConfigPathInfo {
  constructor() {
    this.nodeConfigPath = new Map(), this.excludeNodes = [], this.buildProfilePath = "", this.isModuleChanged = false;
  }
  addModuleConfigPath(e, t) {
    this.nodeConfigPath.set(e, path_1.resolve(t, "hvigorconfig.ts")), this.isModuleChanged = true;
  }
  deleteModuleConfigPath(e) {
    this.excludeNodes.push(e), this.nodeConfigPath.delete(e), this.isModuleChanged = true;
  }
  initNodeConfigPath(e) {
    this.nodeConfigPath.clear(), this.isModuleChanged = false, this.excludeNodes = [], this.buildProfilePath = e.getProject().getBuildProfilePath();
    const t = json5_reader_js_1.Json5Reader.getJson5Obj(this.buildProfilePath),
      o = null == t ? undefined : t.modules;
    o && o.forEach(e => {
      this.nodeConfigPath.set(e.name, this.buildProfilePath);
    });
  }
  getNodeConfigPathByName(e) {
    var t;
    return null !== (t = this.nodeConfigPath.get(e)) && undefined !== t ? t : this.buildProfilePath;
  }
  checkExcludeNode(e) {
    return this.excludeNodes.some(t => t === e);
  }
  getIsModuleChanged() {
    return this.isModuleChanged;
  }
}
export const nodeConfigPathInfo = new NodeConfigPathInfo();
