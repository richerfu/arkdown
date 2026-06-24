/*
 * Human-readable TypeScript reconstruction.
 * Generated from hvigor/ts by hvigor/tools/reconstruct-readable-ts.cjs.
 * Keep semantic edits in Rust ports or separate hand-refined notes.
 * Source baseline: hvigor-ohos-plugin/src/project/dependency/core/default-module-dependency.js
 */
import * as default_npm_dependency_js_1 from "./default-npm-dependency";
export class DefaultModuleDependency extends default_npm_dependency_js_1.DefaultNpmDependency {
  constructor(e) {
    super(e), this._module = e.getModule();
  }
  getModuleName() {
    return this._module.getName();
  }
  getModule() {
    return this._module;
  }
}
