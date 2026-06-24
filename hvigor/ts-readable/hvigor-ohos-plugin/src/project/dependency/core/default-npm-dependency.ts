/*
 * Human-readable TypeScript reconstruction.
 * Generated from hvigor/ts by hvigor/tools/reconstruct-readable-ts.cjs.
 * Keep semantic edits in Rust ports or separate hand-refined notes.
 * Source baseline: hvigor-ohos-plugin/src/project/dependency/core/default-npm-dependency.js
 */
import fs_extra_1 from "fs-extra";
import * as path from "path";
import * as build_directory_const_js_1 from "../../../const/build-directory-const";
import * as copy_resources_util_js_1 from "../../../utils/copy-resources-util";
import * as dependency_manager_js_1 from "../dependency-manager";
import * as dependency_interface_js_1 from "./dependency-interface";
import * as oh_exports_util_1 from "../../../utils/oh-exports-util";
import * as common_const_js_1 from "../../../const/common-const";
export class DefaultNpmDependency {
  constructor(e) {
    var t, n, i;
    this._isLocal = false, this._dependencyName = e.getDependencyName(), this._dependedVersion = e.getDependedVersion(), this._lastDependencyName = e.getLastDependencyName(), this._dependencyVersion = e.getDependencyVersion(), this._dependencyRootPath = e.getDependencyRootPath(), this._dependencies = e.getDependencies(), this._pkgJsonPath = e.getPkgJsonPath(), this._pkgJsonObj = e.getPkgJsonObj(), this._moduleJsonObj = e.getModuleJsonObj(), this._useNormalizedOHMUrl = null === (n = null === (t = this._pkgJsonObj) || undefined === t ? undefined : t.metadata) || undefined === n ? undefined : n.useNormalizedOHMUrl, this._pkgJsonName = this._pkgJsonObj.name, this._mainFilePath = path.resolve(this._dependencyRootPath, this._pkgJsonObj.main || "index.js"), this._typesFilePath = path.resolve(this._dependencyRootPath, null !== (i = this._pkgJsonObj.types) && undefined !== i ? i : "index.d.ts"), this._mainFileRelativePath = this.getMainFileRelativePath(this._dependencyRootPath, this._pkgJsonObj), this._dependencyType = e.getDependencyType(), this._dependencyEnum = e.getDependencyEnum(), this._isLocal = e.isLocal(), this._isModuleDependency = e.isModuleDependency();
  }
  getPackageJsonPath() {
    return this._pkgJsonPath;
  }
  getPackageFilePath() {
    return this._pkgJsonPath;
  }
  getPackageJsonObj() {
    return this._pkgJsonObj;
  }
  getPackageName() {
    return this._pkgJsonName;
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
  getDependencyMainFilePath() {
    return this._mainFilePath;
  }
  getDependencyTypesFilePath() {
    return this._typesFilePath;
  }
  getDependencyType() {
    return this._dependencyType;
  }
  getDependencyEnum() {
    return this._dependencyEnum;
  }
  getDependencies() {
    return this._dependencies;
  }
  getModuleJsonObj() {
    return this._moduleJsonObj;
  }
  getDependencyMainFileRelativePath() {
    return this._mainFileRelativePath;
  }
  getUseNormalizedOHMUrl() {
    return this._useNormalizedOHMUrl;
  }
  hasNative() {
    return fs_extra_1.existsSync(path.resolve(this._dependencyRootPath, "libs")) || fs_extra_1.existsSync(path.resolve(this._dependencyRootPath, "include"));
  }
  isLocal() {
    return this._isLocal;
  }
  isByteCodeHarDependency() {
    var e;
    const t = this.getPackageJsonObj();
    return !!(null === (e = null == t ? undefined : t.metadata) || undefined === e ? undefined : e.byteCodeHar);
  }
  isHarDependency() {
    return this._dependencyType === dependency_interface_js_1.DependencyType.DEPENDENCY_TYPE_HAR;
  }
  isHspDependency() {
    return this._dependencyType === dependency_interface_js_1.DependencyType.DEPENDENCY_TYPE_HSP;
  }
  isOtherDependency() {
    return this._dependencyType === dependency_interface_js_1.DependencyType.DEPENDENCY_TYPE_OTHER;
  }
  isSODependency() {
    return this._dependencyType === dependency_interface_js_1.DependencyType.DEPENDENCY_TYPE_SO;
  }
  isBundleDependency() {
    var e;
    const t = this.getPackageJsonObj();
    return this.isByteCodeHarDependency() && !!(null === (e = null == t ? undefined : t.metadata) || undefined === e ? undefined : e.bundleDependencies);
  }
  getLastDependencyName() {
    return this._lastDependencyName;
  }
  setLastDependencyName(e) {
    this._lastDependencyName = e;
  }
  getMainFileRelativePath(e, t) {
    const n = (0, dependency_manager_js_1.getEntryMainFilePath)(e, t);
    return n ? (0, copy_resources_util_js_1.toUnixPath)(path.relative(this._dependencyRootPath, n)) : "";
  }
  getOhExportsFilePaths() {
    const e = this._pkgJsonObj[common_const_js_1.CommonConst.OH_EXPORTS];
    if (e) return (0, oh_exports_util_1.collectOhExportsFilePaths)(this._dependencyRootPath, e);
  }
  getDefaultLibsDir() {
    return path.resolve(this._dependencyRootPath, build_directory_const_js_1.BuildDirConst.LIBS);
  }
  isModuleDependency() {
    return this._isModuleDependency;
  }
  getDependencySrcMainPath() {
    return path.resolve(this.getDependencyRootPath(), build_directory_const_js_1.BuildDirConst.SRC, build_directory_const_js_1.BuildDirConst.MAIN);
  }
  getInsightIntentPathByResourceDir(e) {
    return path.resolve(e, build_directory_const_js_1.BuildDirConst.BASE_PROFILE_DIR, build_directory_const_js_1.BuildArtifactConst.INSIGHT_INTENT_JSON);
  }
}
