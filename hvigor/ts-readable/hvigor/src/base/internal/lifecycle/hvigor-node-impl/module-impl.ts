/*
 * Human-readable TypeScript reconstruction.
 * Generated from hvigor/ts by hvigor/tools/reconstruct-readable-ts.cjs.
 * Keep semantic edits in Rust ports or separate hand-refined notes.
 * Source baseline: hvigor/src/base/internal/lifecycle/hvigor-node-impl/module-impl.js
 */
import * as class_type_const_js_1 from "../../../common/options/class-type-const";
import * as default_node_impl_js_1 from "./default-node-impl";
export class ModuleImpl extends default_node_impl_js_1.DefaultNodeImpl {
  constructor(e, s, t) {
    super(s, t), this.classKind = class_type_const_js_1.ClassTypeConst.HVIGOR_MODULE, this._project = e;
  }
  findModuleByName(e) {
    return this._project.getSubModules().get(e);
  }
  getProject() {
    return this._project;
  }
}
