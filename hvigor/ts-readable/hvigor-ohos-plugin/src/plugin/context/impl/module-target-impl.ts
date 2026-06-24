/*
 * Human-readable TypeScript reconstruction.
 * Generated from hvigor/ts by hvigor/tools/reconstruct-readable-ts.cjs.
 * Keep semantic edits in Rust ports or separate hand-refined notes.
 * Source baseline: hvigor-ohos-plugin/src/plugin/context/impl/module-target-impl.js
 */
import * as hvigor_1 from "@ohos/hvigor";
import * as wdk_1 from "@baize/wdk";
import * as hvigor_common_1 from "@ohos/hvigor-common";
import * as core_product_js_1 from "../../../common/global/core-product";
import * as core_project_profile_js_1 from "../../../common/global/core-project-profile";
import * as build_mode_manager_js_1 from "../../../project/build-option/build-mode-manager";
var __decorate = this && this.__decorate || function (t, e, r, o) {
  var u,
    i = arguments.length,
    l = i < 3 ? e : null === o ? o = Object.getOwnPropertyDescriptor(e, r) : o;
  if ("object" == typeof Reflect && "function" == typeof Reflect.decorate) l = Reflect.decorate(t, e, r, o);else for (var a = t.length - 1; a >= 0; a--) (u = t[a]) && (l = (i < 3 ? u(l) : i > 3 ? u(e, r, l) : u(e, r)) || l);
  return i > 3 && l && Object.defineProperty(e, r, l), l;
};
export class ModuleTargetImpl {
  constructor(t, e, r, o) {
    this.targetInfo = t, this.modulePathInfo = e, this.projectModel = r, this.moduleModel = o, this.projectPath = this.projectModel.getProject().getNodeDir(), this.initCurrentProduct(), this.initBuildPath();
  }
  getBuildTargetOutputPath() {
    return this._getBuildTargetOutputPath();
  }
  getTargetName() {
    return this._getTargetName();
  }
  getCurrentProduct() {
    return this._getCurrentProduct();
  }
  getBuildOption() {
    return this._getBuildOption();
  }
  getModulePathDetails() {
    return this._getModulePathDetails();
  }
  getBuildTargetOutputPathInternal() {
    return this._getBuildTargetOutputPath();
  }
  getTargetNameInternal() {
    return this._getTargetName();
  }
  getCurrentProductInternal() {
    return this._getCurrentProduct();
  }
  getBuildOptionInternal() {
    return this._getBuildOption();
  }
  getModulePathDetailsInternal() {
    return this._getModulePathDetails();
  }
  _getBuildTargetOutputPath() {
    return this.buildTargetOutputPath;
  }
  _getTargetName() {
    return this.targetInfo.name;
  }
  _getCurrentProduct() {
    return this.currentProduct;
  }
  _getBuildOption() {
    const t = build_mode_manager_js_1.buildOptionManager.getTargetBuildOption(this.moduleModel.getName(), this.targetInfo.name);
    return (0, hvigor_common_1.freezeDeep)((0, wdk_1.cloneDeep)(null != t ? t : {}));
  }
  _getModulePathDetails() {
    return {
      getModuleBuildPath: this.modulePathInfo.getModuleBuildPath.bind(this.modulePathInfo),
      getIntermediatesRes: this.modulePathInfo.getIntermediatesRes.bind(this.modulePathInfo),
      getModuleBuildCachePath: this.modulePathInfo.getModuleBuildCachePath.bind(this.modulePathInfo)
    };
  }
  initCurrentProduct() {
    const t = core_project_profile_js_1.CoreProjectProfile.getInstance(this.projectPath).getCurrentProductOpt();
    this.currentProduct = new core_product_js_1.CoreProduct(t, this.projectModel);
  }
  initBuildPath() {
    this.buildTargetOutputPath = this.modulePathInfo.getModuleBuildOutputPath();
  }
}
__decorate([hvigor_1.TrackAPI], ModuleTargetImpl.prototype, "getBuildTargetOutputPath", null), __decorate([hvigor_1.TrackAPI], ModuleTargetImpl.prototype, "getTargetName", null), __decorate([hvigor_1.TrackAPI], ModuleTargetImpl.prototype, "getCurrentProduct", null), __decorate([hvigor_1.TrackAPI], ModuleTargetImpl.prototype, "getBuildOption", null), __decorate([hvigor_1.TrackAPI], ModuleTargetImpl.prototype, "getModulePathDetails", null);
