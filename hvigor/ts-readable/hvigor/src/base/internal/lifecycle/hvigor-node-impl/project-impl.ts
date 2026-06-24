/*
 * Human-readable TypeScript reconstruction.
 * Generated from hvigor/ts by hvigor/tools/reconstruct-readable-ts.cjs.
 * Keep semantic edits in Rust ports or separate hand-refined notes.
 * Source baseline: hvigor/src/base/internal/lifecycle/hvigor-node-impl/project-impl.js
 */
import * as class_type_const_js_1 from "../../../common/options/class-type-const";
import * as loader_profile_js_1 from "../../../vigor/plugin/interface/loader-profile";
import * as default_node_impl_js_1 from "./default-node-impl";
export class ProjectImpl extends default_node_impl_js_1.DefaultNodeImpl {
  constructor(e, s) {
    super(e, s), this.classKind = class_type_const_js_1.ClassTypeConst.HVIGOR_PROJECT, this._subProjects = new Map();
  }
  findModuleByName(e) {
    return this._subProjects.get(e);
  }
  getProject() {
    return this;
  }
  getSubModules() {
    return this._subProjects;
  }
  addSubModule(e) {
    this._subProjects.set(e.getName(), e);
  }
  getAllSubModules() {
    const e = [];
    for (const s of this._subProjects.values()) e.push(s);
    return e;
  }
  loadConfig(e) {
    this._config = e;
  }
  getConfigOpt() {
    return new loader_profile_js_1.CoreConfig(this._config);
  }
}
