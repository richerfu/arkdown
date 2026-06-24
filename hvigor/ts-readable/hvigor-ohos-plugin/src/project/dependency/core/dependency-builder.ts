/*
 * Human-readable TypeScript reconstruction.
 * Generated from hvigor/ts by hvigor/tools/reconstruct-readable-ts.cjs.
 * Keep semantic edits in Rust ports or separate hand-refined notes.
 * Source baseline: hvigor-ohos-plugin/src/project/dependency/core/dependency-builder.js
 */
import * as default_module_dependency_js_1 from "./default-module-dependency";
import * as default_npm_dependency_js_1 from "./default-npm-dependency";
export class DependencyBuilder {
  constructor() {
    this._isLocal = false, this._isModuleDependency = true;
  }
  setIsModuleDependency(e) {
    return this._isModuleDependency = e, this;
  }
  isModuleDependency() {
    return this._isModuleDependency;
  }
  buildDefaultNpmDependency() {
    return new default_npm_dependency_js_1.DefaultNpmDependency(this);
  }
  buildDefaultModuleDependency() {
    return new default_module_dependency_js_1.DefaultModuleDependency(this);
  }
  setModule(e) {
    return this._module = e, this;
  }
  setDependencyName(e) {
    return this._dependencyName = e, this;
  }
  setDependedVersion(e) {
    return this._dependedVersion = e, this;
  }
  setDependencyRootPath(e) {
    return this._dependencyRootPath = e, this;
  }
  setPkgJsonPath(e) {
    return this._pkgJsonPath = e, this;
  }
  setPkgJsonObj(e) {
    return this._pkgJsonObj = e, this._dependencyVersion = this._pkgJsonObj.version, this._dependencies = this._pkgJsonObj.dependencies, this;
  }
  setDependencyType(e) {
    return this._dependencyType = e, this;
  }
  setDependencyEnum(e) {
    return this._dependencyEnum = e, this;
  }
  setIsLocal(e) {
    return this._isLocal = e, this;
  }
  setModuleJsonObj(e) {
    return this._moduleJsonObj = e, this;
  }
  setLastDependencyName(e) {
    return this._lastDependencyName = e, this;
  }
  getModule() {
    return this._module;
  }
  getDependencyName() {
    return this._dependencyName;
  }
  getDependedVersion() {
    return this._dependedVersion;
  }
  getDependencyVersion() {
    return this._dependencyVersion;
  }
  getDependencyRootPath() {
    return this._dependencyRootPath;
  }
  getPkgJsonPath() {
    return this._pkgJsonPath;
  }
  getPkgJsonObj() {
    return this._pkgJsonObj;
  }
  getDependencies() {
    return this._dependencies;
  }
  getDependencyType() {
    return this._dependencyType;
  }
  getDependencyEnum() {
    return this._dependencyEnum;
  }
  getLastDependencyName() {
    return this._lastDependencyName;
  }
  isLocal() {
    return this._isLocal;
  }
  getModuleJsonObj() {
    return this._moduleJsonObj;
  }
}
